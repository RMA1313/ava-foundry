import { FormEvent, ReactNode, useEffect, useMemo, useState } from "react";
import { BrowserRouter, Navigate, NavLink, Route, Routes, useNavigate, useParams, useSearchParams } from "react-router-dom";
import {
  fetchLiveness,
  fetchProject,
  fetchProjects,
  fetchProjectAudioAssets,
  fetchProjectDuplicateGroups,
  fetchProjectJobs,
  fetchProjectMediaAssets,
  fetchProjectMetricsSummary,
  fetchProjectSources,
  importDataset,
  pausePipeline,
  recoverDuplicates,
  recoverJobs,
  registerUrl,
  resumePipeline,
  retryJob,
  updateProject,
  type AudioAsset,
  type DuplicateGroup,
  type Job,
  type MediaAsset,
  type MetricsSummary,
  type Project,
  type Source,
} from "./api";

const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "/api";
const pageSize = 8;

function usePolling<T>(loader: () => Promise<T>, deps: unknown[], intervalMs = 5000) {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  useEffect(() => {
    let alive = true;
    let timer: number | undefined;
    const load = async () => {
      setLoading(true);
      try {
        const next = await loader();
        if (alive) {
          setData(next);
          setError("");
        }
      } catch (err) {
        if (alive) setError(err instanceof Error ? err.message : "خطای ناشناخته");
      } finally {
        if (alive) setLoading(false);
      }
    };
    void load();
    timer = window.setInterval(load, intervalMs);
    return () => {
      alive = false;
      if (timer) window.clearInterval(timer);
    };
  }, deps);
  return { data, loading, error, refresh: loader };
}

function WorkspaceShell({ children }: { children: ReactNode }) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [status, setStatus] = useState("checking");
  const navigate = useNavigate();
  const params = useParams();
  useEffect(() => {
    fetchLiveness(baseUrl).then((ok) => setStatus(ok ? "ready" : "offline"));
    fetchProjects(baseUrl).then(setProjects);
  }, []);
  const selectedProjectId = params.projectId ?? projects[0]?.id ?? "";
  const projectNavBase = selectedProjectId ? `/projects/${selectedProjectId}` : "/projects";
  useEffect(() => {
    if (!params.projectId && projects[0]?.id) navigate(`/projects/${projects[0].id}/sources`, { replace: true });
  }, [navigate, params.projectId, projects]);
  return (
    <div dir="rtl" lang="fa" style={pageShell}>
      <header style={headerShell}>
        <div style={{ display: "grid", gap: 10 }}>
          <div style={eyebrow}>وضعیت سرویس: {statusLabel(status)}</div>
          <div>
            <p style={kicker}>Phase 4</p>
            <h1 style={title}>فضای کاری پروژه</h1>
            <p style={lede}>مدیریت منابع، پردازش، صوت و تکراری‌ها در یک محیط محلی و پروژه‌محور.</p>
          </div>
        </div>
        <div style={projectPicker}>
          <label style={fieldLabel} htmlFor="project-switcher">پروژه فعال</label>
          <select id="project-switcher" value={selectedProjectId} onChange={(e) => navigate(`/projects/${e.target.value}/sources`)} style={projectSelect}>
            {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
          </select>
        </div>
      </header>
      <div style={workspaceGrid}>
        <aside style={sidebar}>
          <div style={sidebarSection}>
            <div style={sidebarHeading}>پروژه</div>
          <div style={sidebarProject}>{projects.find((project) => project.id === selectedProjectId)?.name ?? "بدون پروژه"}</div>
        </div>
        <nav style={navGrid} aria-label="ناوبری فضای کاری">
            <SideLink to={`${projectNavBase}/sources`}>منابع</SideLink>
            <SideLink to={`${projectNavBase}/url-import`}>درون‌ریزی URL</SideLink>
            <SideLink to={`${projectNavBase}/jobs`}>وظایف</SideLink>
            <SideLink to={`${projectNavBase}/media`}>رسانه</SideLink>
            <SideLink to={`${projectNavBase}/audio`}>صوت</SideLink>
            <SideLink to={`${projectNavBase}/duplicates`}>تکراری‌ها</SideLink>
            <SideLink to={`${projectNavBase}/statistics`}>آمار پروژه</SideLink>
            <SideLink to={`${projectNavBase}/pipeline`}>کنترل پایپلاین</SideLink>
            <SideLink to={`${projectNavBase}/settings`}>تنظیمات</SideLink>
            <SideLink to={`${projectNavBase}/import`}>درون‌ریزی دیتاست</SideLink>
          </nav>
        </aside>
        <main style={mainSurface}>{children}</main>
      </div>
    </div>
  );
}

const pageShell = {
  minHeight: "100vh",
  background: "linear-gradient(180deg, #f8fafc 0%, #eef2ff 100%)",
  color: "#0f172a",
  fontFamily: '"Vazirmatn", "IRANSans", "Noto Sans Arabic", "Segoe UI", Tahoma, sans-serif',
};
const headerShell = { padding: "28px 32px 20px", display: "flex", justifyContent: "space-between", alignItems: "end", gap: 24, borderBottom: "1px solid rgba(15,23,42,0.08)" };
const kicker = { margin: 0, fontSize: 12, letterSpacing: "0.14em", textTransform: "uppercase", color: "#475569" };
const title = { margin: 0, fontSize: 32, lineHeight: 1.2 };
const lede = { margin: "8px 0 0", maxWidth: 720, color: "#334155", lineHeight: 1.8 };
const eyebrow = { fontSize: 13, color: "#475569" };
const projectPicker = { display: "grid", gap: 8, minWidth: 240 };
const fieldLabel = { fontSize: 13, color: "#334155" };
const projectSelect = { padding: "12px 14px", borderRadius: 14, border: "1px solid #cbd5e1", background: "white" };
const workspaceGrid = { display: "grid", gridTemplateColumns: "minmax(220px, 280px) minmax(0, 1fr)", gap: 20, padding: 20, alignItems: "start" };
const sidebar = { background: "rgba(255,255,255,0.82)", borderRadius: 24, padding: 16, boxShadow: "0 12px 36px rgba(15,23,42,0.08)", border: "1px solid rgba(148,163,184,0.24)", position: "sticky", top: 20 };
const sidebarSection = { padding: "8px 8px 14px", borderBottom: "1px solid rgba(148,163,184,0.2)", marginBottom: 14 };
const sidebarHeading = { fontSize: 12, color: "#64748b", marginBottom: 6 };
const sidebarProject = { fontSize: 16, fontWeight: 700 };
const navGrid = { display: "grid", gap: 8 };
const mainSurface = { minWidth: 0 };
const card = { padding: 18, border: "1px solid #dbe4f0", borderRadius: 18, background: "rgba(255,255,255,0.9)", boxShadow: "0 10px 28px rgba(15,23,42,0.06)" };
const panel = { ...card, display: "grid", gap: 16 };
const metaGrid = { display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: 10, marginTop: 10, fontSize: 13, color: "#334155" };
const formGrid = { display: "grid", gap: 12 };
const inputStyle = { padding: "12px 14px", borderRadius: 12, border: "1px solid #cbd5e1", background: "white", width: "100%" };
const buttonStyle = { padding: "12px 16px", borderRadius: 12, border: "1px solid #0f172a", background: "#0f172a", color: "white", cursor: "pointer" };
const secondaryButtonStyle = { ...buttonStyle, background: "white", color: "#0f172a", borderColor: "#cbd5e1" };
const ghostButtonStyle = { ...buttonStyle, background: "#f8fafc", color: "#0f172a", borderColor: "#cbd5e1" };
const subtleText = { color: "#475569", lineHeight: 1.8 };

function SideLink({ to, children }: { to: string; children: ReactNode }) {
  return <NavLink to={to} style={({ isActive }) => ({ padding: "12px 14px", borderRadius: 14, textDecoration: "none", background: isActive ? "#0f172a" : "#f8fafc", color: isActive ? "white" : "#0f172a", border: "1px solid " + (isActive ? "#0f172a" : "#e2e8f0"), fontWeight: isActive ? 700 : 500 })}>{children}</NavLink>;
}

function FilterBar({ value, onChange, placeholder }: { value: string; onChange: (value: string) => void; placeholder: string }) {
  return <input value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} style={{ ...inputStyle, flex: 1, minWidth: 180 }} />;
}

function Pager({ page, setPage, total }: { page: number; setPage: (page: number) => void; total: number }) {
  const canPrev = page > 0;
  const canNext = (page + 1) * pageSize < total;
  return <div style={{ display: "flex", gap: 12, alignItems: "center", justifyContent: "space-between", flexWrap: "wrap" }}><span style={subtleText}>صفحه {page + 1} از {Math.max(1, Math.ceil(total / pageSize))}</span><div style={{ display: "flex", gap: 8 }}><button style={secondaryButtonStyle} onClick={() => setPage(page - 1)} disabled={!canPrev}>قبلی</button><button style={secondaryButtonStyle} onClick={() => setPage(page + 1)} disabled={!canNext}>بعدی</button></div></div>;
}

function PageFrame({ title, subtitle, children, error, loading, empty }: { title: string; subtitle?: string; children: ReactNode; error?: string; loading: boolean; empty: boolean }) {
  return <section style={panel}><header style={{ display: "grid", gap: 8 }}><h2 style={{ margin: 0, fontSize: 24 }}>{title}</h2>{subtitle ? <p style={{ margin: 0, ...subtleText }}>{subtitle}</p> : null}</header>{error ? <StateMessage tone="error" title="خطا" message={error} /> : null}{loading ? <StateMessage tone="info" title="در حال بارگذاری" message="لطفاً کمی صبر کنید." /> : empty ? <StateMessage tone="empty" title="موردی یافت نشد" message="داده‌ای برای نمایش در این بخش وجود ندارد." /> : children}</section>;
}

function StateMessage({ tone, title, message }: { tone: "error" | "info" | "empty"; title: string; message: string }) {
  const palette = tone === "error" ? { background: "#fef2f2", border: "#fecaca", color: "#991b1b" } : tone === "info" ? { background: "#eff6ff", border: "#bfdbfe", color: "#1d4ed8" } : { background: "#f8fafc", border: "#e2e8f0", color: "#475569" };
  return <div style={{ padding: 16, borderRadius: 14, border: `1px solid ${palette.border}`, background: palette.background, color: palette.color, display: "grid", gap: 6 }}><strong>{title}</strong><span>{message}</span></div>;
}

function StatusBadge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "success" | "warning" | "danger" }) {
  const styles = tone === "success" ? badgeSuccess : tone === "warning" ? badgeWarning : tone === "danger" ? badgeDanger : badgeNeutral;
  return <span style={styles}>{children}</span>;
}
const badgeNeutral = { padding: "4px 10px", borderRadius: 999, background: "#e2e8f0", color: "#0f172a", fontSize: 12, fontWeight: 700, width: "fit-content" };
const badgeSuccess = { ...badgeNeutral, background: "#dcfce7", color: "#166534" };
const badgeWarning = { ...badgeNeutral, background: "#fef3c7", color: "#92400e" };
const badgeDanger = { ...badgeNeutral, background: "#fee2e2", color: "#991b1b" };

function readQuery(searchParams: URLSearchParams, key: string) {
  return searchParams.get(key) ?? "";
}

function useWorkspaceQuery() {
  const [searchParams, setSearchParams] = useSearchParams();
  const update = (patch: Record<string, string>) => {
    const next = new URLSearchParams(searchParams);
    for (const [key, value] of Object.entries(patch)) {
      if (value) next.set(key, value);
      else next.delete(key);
    }
    setSearchParams(next, { replace: true });
  };
  return { searchParams, update };
}

function useProjectId() {
  return useParams().projectId ?? "";
}

function defaultError(err: unknown) {
  return err instanceof Error ? err.message : "خطای ناشناخته";
}

function ProjectsLanding() {
  const { data: projects, loading, error } = usePolling(() => fetchProjects(baseUrl), [], 10000);
  return <PageFrame title="پروژه‌ها" subtitle="پروژه‌ای را برای ورود به فضای کاری انتخاب کنید." loading={loading} error={error} empty={(projects ?? []).length === 0}><div style={{ display: "grid", gap: 12 }}>{(projects ?? []).map((project) => <article key={project.id} style={card}><div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "start", flexWrap: "wrap" }}><div><h3 style={{ margin: 0 }}>{project.name}</h3><p style={{ margin: "6px 0 0", ...subtleText }}>{project.description ?? "بدون توضیح"}</p></div><NavLink to={`/projects/${project.id}/sources`} style={buttonStyle}>باز کردن</NavLink></div><div style={metaGrid}><span className="technical-inline">ID: {project.id}</span></div></article>)}</div></PageFrame>;
}

function SourcesPage() {
  const projectId = useProjectId();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const sourceType = readQuery(searchParams, "source_type");
  const { data, loading, error } = usePolling(() => fetchProjectSources(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((item) => matches([item.source_type, item.original_url, item.ingestion_status, item.failure_summary], q) && (!sourceType || item.source_type === sourceType)), [data, q, sourceType]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="منابع" subtitle="ورودی‌های ثبت‌شده هر پروژه اینجا دیده می‌شوند." error={error} loading={loading} empty={filtered.length === 0}><WorkspaceToolbar><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={sourceType} onChange={(e) => update({ source_type: e.target.value })} style={inputStyle}><option value="">همه نوع‌ها</option><option value="public_url">public_url</option><option value="local_file">local_file</option><option value="local_folder">local_folder</option><option value="csv_import">csv_import</option><option value="existing_dataset">existing_dataset</option><option value="archive">archive</option></select></WorkspaceToolbar>{paged.map((s) => <AssetCard key={s.id} title={s.source_type} meta={[s.original_url ?? "بدون آدرس", s.ingestion_status, s.failure_summary ?? ""]} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></PageFrame>;
}

function UrlImportPage() {
  const projectId = useProjectId();
  const [url, setUrl] = useState("");
  const [state, setState] = useState<{ kind: "idle" | "success" | "error"; message: string }>({ kind: "idle", message: "" });
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    let parsed: URL;
    try {
      parsed = new URL(url);
    } catch {
      setState({ kind: "error", message: "نشانی معتبر وارد کنید." });
      return;
    }
    if (!["http:", "https:"].includes(parsed.protocol)) {
      setState({ kind: "error", message: "نشانی باید http یا https باشد." });
      return;
    }
    try {
      const result = await registerUrl(baseUrl, projectId, parsed.toString());
      setState({ kind: "success", message: `منبع ثبت شد. source ${result.source_id}` });
    } catch (err) {
      setState({ kind: "error", message: defaultError(err) });
    }
  };
  return <PageFrame title="درون‌ریزی URL" subtitle="فقط URLهای پشتیبانی‌شده ثبت می‌شوند." error={state.kind === "error" ? state.message : ""} loading={false} empty={false}><form onSubmit={submit} style={formGrid}><input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://..." style={inputStyle} dir="ltr" /><button type="submit" style={buttonStyle}>ثبت URL</button></form>{state.kind === "success" ? <StateMessage tone="info" title="موفق" message={state.message} /> : null}</PageFrame>;
}

function JobsPage() {
  const projectId = useProjectId();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const statusFilter = readQuery(searchParams, "job_status");
  const { data, loading, error, refresh } = usePolling(() => fetchProjectJobs(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((job) => matches([job.job_type, job.status, job.failure_summary], q) && (!statusFilter || job.status === statusFilter)), [data, q, statusFilter]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="وظایف" subtitle="صف، اجرا، خطا و تلاش دوباره." error={error} loading={loading} empty={filtered.length === 0}><WorkspaceToolbar><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={statusFilter} onChange={(e) => update({ job_status: e.target.value })} style={inputStyle}><option value="">همه وضعیت‌ها</option><option value="queued">queued</option><option value="running">running</option><option value="completed">completed</option><option value="failed">failed</option><option value="cancelled">cancelled</option></select><button style={secondaryButtonStyle} onClick={() => void refresh()}>بازنشانی</button><button style={ghostButtonStyle} onClick={async () => { if (confirm("همه leaseهای منقضی را بازیابی کنیم؟")) await recoverJobs(baseUrl, projectId); await refresh(); }}>بازیابی صف</button></WorkspaceToolbar>{paged.map((job) => <JobCard key={job.id} job={job} onRetry={async () => { if (confirm("این job دوباره صف شود؟")) { await retryJob(baseUrl, projectId, job.id); await refresh(); } }} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></PageFrame>;
}

function MediaPage() {
  const projectId = useProjectId();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const mediaKind = readQuery(searchParams, "media_kind");
  const { data, loading, error } = usePolling(() => fetchProjectMediaAssets(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((asset) => matches([asset.media_kind, asset.ingestion_status, asset.inspection_status, asset.extraction_status, asset.original_filename, asset.staged_path], q) && (!mediaKind || asset.media_kind === mediaKind)), [data, q, mediaKind]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="رسانه" subtitle="فایل‌های ورودی و حالت پردازش آنها." error={error} loading={loading} empty={filtered.length === 0}><WorkspaceToolbar><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={mediaKind} onChange={(e) => update({ media_kind: e.target.value })} style={inputStyle}><option value="">همه نوع‌ها</option><option value="video">video</option><option value="audio">audio</option></select></WorkspaceToolbar>{paged.map((asset) => <MediaCard key={asset.id} asset={asset} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></PageFrame>;
}

function AudioPage() {
  const projectId = useProjectId();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const processingStatus = readQuery(searchParams, "processing_status");
  const { data, loading, error } = usePolling(() => fetchProjectAudioAssets(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((asset) => matches([asset.processing_status, asset.validation_status, asset.audio_format, asset.codec, asset.storage_path], q) && (!processingStatus || asset.processing_status === processingStatus)), [data, q, processingStatus]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="صوت" subtitle="دارایی‌های نهایی و مسیرهای ذخیره‌سازی." error={error} loading={loading} empty={filtered.length === 0}><WorkspaceToolbar><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={processingStatus} onChange={(e) => update({ processing_status: e.target.value })} style={inputStyle}><option value="">همه وضعیت‌ها</option><option value="validated">validated</option><option value="queued">queued</option><option value="running">running</option><option value="failed">failed</option></select></WorkspaceToolbar>{paged.map((asset) => <AudioCard key={asset.id} asset={asset} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></PageFrame>;
}

function DuplicatePage() {
  const projectId = useProjectId();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const stateFilter = readQuery(searchParams, "duplicate_state");
  const { data, loading, error, refresh } = usePolling(() => fetchProjectDuplicateGroups(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((group) => matches([group.id, group.canonical_audio_asset_id, group.members.map((m) => m.audio_asset_id).join(" "), group.decisions.map((d) => `${d.decision} ${d.similarity_score}`).join(" ")], q) && (!stateFilter || duplicateState(group) === stateFilter)), [data, q, stateFilter]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="تکراری‌ها" subtitle="گروه‌ها، اعضا و canonical انتخاب‌شده." error={error} loading={loading} empty={filtered.length === 0}><WorkspaceToolbar><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={stateFilter} onChange={(e) => update({ duplicate_state: e.target.value })} style={inputStyle}><option value="">همه حالت‌ها</option><option value="canonical">canonical</option><option value="missing_canonical">missing_canonical</option></select><button style={ghostButtonStyle} onClick={async () => { if (confirm("canonicalهای گم‌شده را بازیابی کنیم؟")) await recoverDuplicates(baseUrl, projectId); await refresh(); }}>بازیابی canonical</button></WorkspaceToolbar>{paged.map((group) => <DuplicateCard key={group.id} group={group} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></PageFrame>;
}

function StatisticsPage() {
  const projectId = useProjectId();
  const { data: sources } = usePolling(() => fetchProjectSources(baseUrl, projectId), [projectId], 5000);
  const { data: jobs } = usePolling(() => fetchProjectJobs(baseUrl, projectId), [projectId], 5000);
  const { data: media } = usePolling(() => fetchProjectMediaAssets(baseUrl, projectId), [projectId], 5000);
  const { data: audio } = usePolling(() => fetchProjectAudioAssets(baseUrl, projectId), [projectId], 5000);
  const { data: duplicates } = usePolling(() => fetchProjectDuplicateGroups(baseUrl, projectId), [projectId], 5000);
  const { data: summary } = usePolling(() => fetchProjectMetricsSummary(baseUrl, projectId), [projectId], 5000);
  const duplicateCount = duplicates?.length ?? 0;
  const audioCount = audio?.length ?? 0;
  const mediaCount = media?.length ?? 0;
  const sourceCount = sources?.length ?? 0;
  const storageUsage = (audio ?? []).reduce((sum, item) => sum + item.byte_size, 0) + (media ?? []).reduce((sum, item) => sum + item.byte_size, 0);
  const uniqueDuration = summary?.deduplication.unique_duration_seconds ?? 0;
  const duplicateRatio = summary?.deduplication.duplicate_ratio ?? 0;
  const throughput = summary?.deduplication.throughput ?? 0;
  const jobStates = countBy((jobs ?? []).map((job) => job.status));
  return <PageFrame title="آمار پروژه" subtitle="نمایی خلاصه از داده‌ها و throughput." loading={false} error="" empty={false}><div style={{ display: "grid", gap: 12 }}><StatsRow label="منابع" value={sourceCount} /><StatsRow label="رسانه" value={mediaCount} /><StatsRow label="صوت" value={audioCount} /><StatsRow label="گروه‌های تکراری" value={duplicateCount} /><StatsRow label="نسبت تکراری" value={formatPercent(duplicateRatio)} /><StatsRow label="دقایق یکتا" value={formatNumber(uniqueDuration)} /><StatsRow label="مصرف ذخیره" value={formatBytes(storageUsage)} /><StatsRow label="توان پایپلاین" value={formatNumber(throughput)} /><div style={card}><strong>حالت‌های job</strong><div style={metaGrid}>{Object.entries(jobStates).map(([key, value]) => <StatusBadge key={key}>{key}: {value}</StatusBadge>)}</div></div></div></PageFrame>;
}

function PipelinePage() {
  const projectId = useProjectId();
  const { data: summary } = usePolling(() => fetchProjectMetricsSummary(baseUrl, projectId), [projectId], 5000);
  const [message, setMessage] = useState("");
  return <PageFrame title="کنترل پایپلاین" subtitle="بازرسی و بازیابی عملیات جاری." loading={false} error="" empty={false}><div style={{ display: "grid", gap: 12 }}><p style={subtleText}>وضعیت throughput: {formatNumber(summary?.deduplication.throughput ?? 0)}</p><div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}><button style={ghostButtonStyle} onClick={async () => { if (confirm("بازیابی jobهای منقضی را اجرا کنیم؟")) { await recoverJobs(baseUrl, projectId); setMessage("بازیابی job انجام شد."); } }}>بازیابی شکست‌خورده‌ها</button><button style={ghostButtonStyle} onClick={async () => { if (confirm("canonicalهای گم‌شده را بازیابی کنیم؟")) { await recoverDuplicates(baseUrl, projectId); setMessage("بازیابی duplicate انجام شد."); } }}>بازیابی تکراری‌ها</button><button style={secondaryButtonStyle} onClick={async () => { if (confirm("پایپلاین را متوقف کنیم؟")) { await pausePipeline(baseUrl, projectId); setMessage("پایپلاین متوقف شد."); } }}>توقف</button><button style={buttonStyle} onClick={async () => { if (confirm("پایپلاین را ادامه دهیم؟")) { await resumePipeline(baseUrl, projectId); setMessage("پایپلاین ادامه یافت."); } }}>ادامه</button></div><StatusBadge tone="success">{message || "حالت اجرا: فعال"}</StatusBadge></div></PageFrame>;
}

function SettingsPage() {
  const projectId = useProjectId();
  const { data: project, loading, error, refresh } = usePolling(() => fetchProject(baseUrl, projectId), [projectId], 5000);
  const { data: summary } = usePolling(() => fetchProjectMetricsSummary(baseUrl, projectId), [projectId], 5000);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [saveState, setSaveState] = useState<{ kind: "idle" | "saving" | "success" | "error"; message: string }>({ kind: "idle", message: "" });
  useEffect(() => {
    if (project) {
      setName(project.name);
      setDescription(project.description ?? "");
    }
  }, [project]);
  const submitted = project?.name === name.trim() && (project?.description ?? "") === description.trim();
  const onSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const trimmedName = name.trim();
    const trimmedDescription = description.trim();
    if (!trimmedName) {
      setSaveState({ kind: "error", message: "نام پروژه الزامی است." });
      return;
    }
    if (trimmedName.length > 120) {
      setSaveState({ kind: "error", message: "نام پروژه باید 120 کاراکتر یا کمتر باشد." });
      return;
    }
    setSaveState({ kind: "saving", message: "در حال ذخیره..." });
    try {
      await updateProject(baseUrl, projectId, trimmedName, trimmedDescription || null);
      await refresh();
      setSaveState({ kind: "success", message: "تنظیمات پروژه ذخیره شد." });
    } catch (err) {
      setSaveState({ kind: "error", message: defaultError(err) });
    }
  };
  const activeProfile = "FLAC 16 kHz mono";
  return <PageFrame title="تنظیمات" subtitle="فقط فیلدهای پشتیبانی‌شده توسط backend نمایش داده می‌شوند." loading={loading} error={error} empty={false}><div style={{ display: "grid", gap: 16 }}><form onSubmit={onSubmit} style={{ display: "grid", gap: 16 }}><section style={card}><div style={{ display: "grid", gap: 10 }}><strong>تنظیمات پروژه</strong><p style={subtleText}>شما فقط می‌توانید نام و توضیح پروژه را تغییر دهید. این فرم از backend contract فعلی فراتر نمی‌رود.</p><label style={fieldLabel} htmlFor="project-name">نام پروژه</label><input id="project-name" value={name} onChange={(e) => setName(e.target.value)} style={inputStyle} /><label style={fieldLabel} htmlFor="project-description">توضیح</label><textarea id="project-description" value={description} onChange={(e) => setDescription(e.target.value)} style={{ ...inputStyle, minHeight: 110, resize: "vertical" }} /></div></section><section style={card}><strong>تنظیمات پردازش و پایپلاین</strong><div style={metaGrid}><StatusBadge tone="neutral">پروفایل فعال: {activeProfile}</StatusBadge><StatusBadge tone={summary?.deduplication.processing_failures ? "warning" : "success"}>شکست پردازش: {summary?.deduplication.processing_failures ?? 0}</StatusBadge><StatusBadge tone="neutral">Throughput: {formatNumber(summary?.deduplication.throughput ?? 0)}</StatusBadge></div><p style={subtleText}>این صفحه وضعیت pipeline را نشان می‌دهد و از کنترل‌های موجود برای توقف، ادامه و بازیابی استفاده می‌کند.</p></section><div style={{ display: "flex", gap: 10, flexWrap: "wrap", alignItems: "center" }}><button type="submit" style={buttonStyle} disabled={submitted || saveState.kind === "saving"}>{saveState.kind === "saving" ? "در حال ذخیره..." : "ذخیره تنظیمات"}</button><button type="button" style={secondaryButtonStyle} onClick={async () => { await refresh(); }}>بازگشت به آخرین مقدار</button>{saveState.kind === "success" ? <StatusBadge tone="success">{saveState.message}</StatusBadge> : null}{saveState.kind === "error" ? <StatusBadge tone="danger">{saveState.message}</StatusBadge> : null}</div></form></div></PageFrame>;
}

function ImportPage() {
  const projectId = useProjectId();
  const [summary, setSummary] = useState("");
  const [error, setError] = useState("");
  const handleSubmit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const file = (e.currentTarget.elements.namedItem("file") as HTMLInputElement).files?.[0];
    const audioPathField = (e.currentTarget.elements.namedItem("audio_path_field") as HTMLInputElement).value || "audio_path";
    if (!file) {
      setError("یک فایل انتخاب کنید.");
      return;
    }
    setError("");
    const result = await importDataset(baseUrl, projectId, file, audioPathField);
    setSummary(JSON.stringify(result, null, 2));
  };
  return <PageFrame title="درون‌ریزی دیتاست" subtitle="CSV، JSONL، JSON، Parquet و آرشیوهای پشتیبانی‌شده." error={error} loading={false} empty={false}><form onSubmit={handleSubmit} style={formGrid}><label style={fieldLabel} htmlFor="dataset-file">فایل ورودی</label><input id="dataset-file" name="file" type="file" accept=".csv,.jsonl,.json,.parquet,.zip,.tar,.gz,.tgz" style={inputStyle} /><label style={fieldLabel} htmlFor="audio-path-field">فیلد مسیر صوت</label><input id="audio-path-field" name="audio_path_field" placeholder="audio_path" style={inputStyle} /><button type="submit" style={buttonStyle}>درون‌ریزی</button></form><pre style={{ whiteSpace: "pre-wrap", direction: "ltr", textAlign: "left", marginTop: 12 }}>{summary}</pre></PageFrame>;
}

function WorkspaceToolbar({ children }: { children: ReactNode }) {
  return <div style={{ display: "flex", gap: 12, flexWrap: "wrap", alignItems: "center" }}>{children}</div>;
}

function matches(values: Array<string | null | undefined>, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return values.some((value) => (value ?? "").toLowerCase().includes(needle));
}

function duplicateState(group: DuplicateGroup) {
  return group.canonical_audio_asset_id ? "canonical" : "missing_canonical";
}

function StatsRow({ label, value }: { label: string; value: string | number }) {
  return <article style={card}><strong>{label}</strong><div style={{ marginTop: 8, fontSize: 18 }}>{value}</div></article>;
}

function countBy(items: string[]) {
  return items.reduce<Record<string, number>>((acc, item) => ({ ...acc, [item]: (acc[item] ?? 0) + 1 }), {});
}

function formatPercent(value: number) {
  return `${(value * 100).toFixed(1)}%`;
}

function formatNumber(value: number) {
  return new Intl.NumberFormat("fa-IR").format(value);
}

function formatBytes(value: number) {
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let size = value / 1024;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  return `${size.toFixed(1)} ${units[unit]}`;
}

function technicalLine(value: string) {
  return <span className="technical-inline">{value}</span>;
}

function AssetCard({ title, meta }: { title: string; meta: string[] }) {
  return <article style={card}><strong>{title}</strong><div style={metaGrid}>{meta.filter(Boolean).map((item) => <span key={item} style={{ direction: isTechnicalValue(item) ? "ltr" : "rtl", unicodeBidi: "plaintext" }}>{item}</span>)}</div></article>;
}

function DuplicateCard({ group }: { group: DuplicateGroup }) {
  return <article style={card}><div style={{ display: "flex", justifyContent: "space-between", gap: 12, flexWrap: "wrap", alignItems: "center" }}><strong>گروه {group.id}</strong><StatusBadge tone={group.canonical_audio_asset_id ? "success" : "warning"}>{duplicateState(group)}</StatusBadge></div><div style={metaGrid}><span>{technicalLine(`canonical: ${group.canonical_audio_asset_id ?? "ندارد"}`)}</span><span>اعضا: {group.members.length}</span><span>وضعیت: {duplicateState(group)}</span><span>شباهت: {group.decisions[0]?.similarity_score?.toFixed(3) ?? "نامشخص"}</span></div><div style={{ marginTop: 12, display: "grid", gap: 8 }}>{group.members.map((member) => <div key={member.audio_asset_id} style={technicalValueBox}>{technicalLine(`member: ${member.audio_asset_id}`)}</div>)}{group.decisions.map((decision) => <div key={decision.id} style={technicalValueBox}>{technicalLine(`decision: ${decision.decision} confidence: ${decision.confidence}`)}</div>)}</div></article>;
}

function JobCard({ job, onRetry }: { job: Job; onRetry: () => Promise<void> }) {
  return <article style={card}><div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}><strong>{job.job_type}</strong><StatusBadge tone={job.status === "failed" ? "danger" : job.status === "completed" ? "success" : "neutral"}>{job.status}</StatusBadge></div><div style={metaGrid}><span>پیشرفت: {job.progress}</span><span>تلاش‌ها: {job.attempts}</span><span>{job.priority ?? 0}</span>{job.failure_summary ? <span style={{ color: "#b91c1c" }}>{job.failure_summary}</span> : null}</div>{job.status === "failed" ? <button style={ghostButtonStyle} onClick={() => void onRetry()}>تلاش دوباره</button> : null}</article>;
}

function MediaCard({ asset }: { asset: MediaAsset }) {
  return <article style={card}><div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}><strong>{asset.media_kind}</strong><StatusBadge tone={asset.ingestion_status === "ready" ? "success" : "neutral"}>{asset.ingestion_status}</StatusBadge></div><div style={metaGrid}><span>منبع: {asset.source_id}</span><span>بازرسی: {asset.inspection_status ?? "نامشخص"}</span><span>استخراج: {asset.extraction_status ?? "نامشخص"}</span><span>حجم: {asset.byte_size}</span><span>پروفایل: {asset.container_format ?? "نامشخص"}</span><span style={ltrValueStyle}>منشأ: {asset.staged_path}</span></div></article>;
}

function AudioCard({ asset }: { asset: AudioAsset }) {
  return <article style={card}><div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center", flexWrap: "wrap" }}><strong>{asset.audio_format}</strong><StatusBadge tone={asset.processing_status === "validated" ? "success" : asset.processing_status === "failed" ? "danger" : "neutral"}>{asset.processing_status}</StatusBadge></div><div style={metaGrid}><span style={ltrValueStyle}>شناسه رسانه: {asset.media_asset_id}</span><span>اعتبارسنجی: {asset.validation_status ?? "نامشخص"}</span><span>کدک: {asset.codec}</span><span>نرخ نمونه‌برداری: {asset.sample_rate_hz}</span><span>کانال: {asset.channels}</span><span>حجم: {asset.byte_size}</span><span style={ltrValueStyle}>مسیر: {asset.storage_path}</span></div></article>;
}

function isTechnicalValue(value: string) {
  return /[A-Za-z0-9_./:-]/.test(value);
}

const technicalValueBox = { padding: "10px 12px", borderRadius: 12, background: "#f8fafc", border: "1px solid #e2e8f0" };
const ltrValueStyle = { direction: "ltr" as const, unicodeBidi: "plaintext" as const };

function statusLabel(status: string) {
  if (status === "ready") return "آماده";
  if (status === "offline") return "آفلاین";
  return "در حال بررسی";
}

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Navigate to="/projects" replace />} />
        <Route path="/projects" element={<WorkspaceShell><ProjectsLanding /></WorkspaceShell>} />
        <Route path="/projects/:projectId" element={<WorkspaceShell><Navigate to="sources" replace /></WorkspaceShell>} />
        <Route path="/projects/:projectId/sources" element={<WorkspaceShell><SourcesPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/url-import" element={<WorkspaceShell><UrlImportPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/jobs" element={<WorkspaceShell><JobsPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/media" element={<WorkspaceShell><MediaPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/audio" element={<WorkspaceShell><AudioPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/duplicates" element={<WorkspaceShell><DuplicatePage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/statistics" element={<WorkspaceShell><StatisticsPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/pipeline" element={<WorkspaceShell><PipelinePage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/settings" element={<WorkspaceShell><SettingsPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/import" element={<WorkspaceShell><ImportPage /></WorkspaceShell>} />
        <Route path="*" element={<Navigate to="/projects" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
