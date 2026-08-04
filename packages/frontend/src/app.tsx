import { FormEvent, ReactNode, useEffect, useMemo, useState } from "react";
import { BrowserRouter, Navigate, NavLink, Route, Routes, useNavigate, useParams, useSearchParams } from "react-router-dom";
import {
  fetchLiveness,
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
  type AudioAsset,
  type DuplicateGroup,
  type Job,
  type MediaAsset,
  type MetricsSummary,
  type Project,
  type Source,
} from "./api";

const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8080";
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
  useEffect(() => {
    if (!params.projectId && projects[0]?.id) navigate(`/projects/${projects[0].id}/sources`, { replace: true });
  }, [navigate, params.projectId, projects]);
  return (
    <div dir="rtl" style={pageShell}>
      <header style={headerShell}>
        <div>
          <div style={eyebrow}>وضعیت: {status === "ready" ? "آماده" : status === "offline" ? "آفلاین" : "در حال بررسی"}</div>
          <h1 style={title}>فضای کاری پروژه</h1>
        </div>
        <select value={selectedProjectId} onChange={(e) => navigate(`/projects/${e.target.value}/sources`)} style={projectSelect}>
          {projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}
        </select>
      </header>
      <div style={workspaceGrid}>
        <aside style={sidebar}>
          <nav style={{ display: "grid", gap: 8 }}>
            <SideLink to={`/projects/${selectedProjectId}/sources`}>منابع</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/url-import`}>درون‌ریزی URL</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/jobs`}>وظایف</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/media`}>رسانه</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/audio`}>صوت</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/duplicates`}>تکراری‌ها</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/statistics`}>آمار پروژه</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/pipeline`}>کنترل پایپلاین</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/import`}>درون‌ریزی دیتاست</SideLink>
          </nav>
        </aside>
        <main>{children}</main>
      </div>
    </div>
  );
}

const pageShell = { minHeight: "100vh", background: "radial-gradient(circle at top right, #f8fafc, #dbeafe 38%, #e2e8f0 100%)", color: "#0f172a", fontFamily: "Tahoma, sans-serif" };
const headerShell = { padding: "24px 28px", display: "flex", justifyContent: "space-between", alignItems: "center", borderBottom: "1px solid rgba(15,23,42,0.08)" };
const eyebrow = { fontSize: 14, color: "#475569" };
const title = { margin: "8px 0 0", fontSize: 28 };
const projectSelect = { padding: 12, borderRadius: 12 };
const workspaceGrid = { display: "grid", gridTemplateColumns: "260px 1fr", gap: 20, padding: 20 };
const sidebar = { background: "white", borderRadius: 20, padding: 16, boxShadow: "0 10px 30px rgba(15,23,42,0.08)" };
const card = { padding: 16, border: "1px solid #e2e8f0", borderRadius: 16 };
const metaGrid = { display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: 8, marginTop: 8, fontSize: 14, color: "#334155" };

function SideLink({ to, children }: { to: string; children: ReactNode }) {
  return <NavLink to={to} style={({ isActive }) => ({ padding: 12, borderRadius: 14, textDecoration: "none", background: isActive ? "#0f172a" : "#f8fafc", color: isActive ? "white" : "#0f172a" })}>{children}</NavLink>;
}

function FilterBar({ value, onChange, placeholder }: { value: string; onChange: (value: string) => void; placeholder: string }) {
  return <input value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} style={{ padding: 10, borderRadius: 12, flex: 1, minWidth: 180 }} />;
}

function Pager({ page, setPage, total }: { page: number; setPage: (page: number) => void; total: number }) {
  const canPrev = page > 0;
  const canNext = (page + 1) * pageSize < total;
  return <div style={{ display: "flex", gap: 8, alignItems: "center", justifyContent: "space-between" }}><span>صفحه {page + 1}</span><div style={{ display: "flex", gap: 8 }}><button onClick={() => setPage(page - 1)} disabled={!canPrev}>قبلی</button><button onClick={() => setPage(page + 1)} disabled={!canNext}>بعدی</button></div></div>;
}

function PageFrame({ title, children, error, loading, empty }: { title: string; children: ReactNode; error?: string; loading: boolean; empty: boolean }) {
  return <section style={{ background: "white", borderRadius: 20, padding: 20, boxShadow: "0 10px 30px rgba(15,23,42,0.08)" }}><h2 style={{ marginTop: 0 }}>{title}</h2>{error ? <p style={{ color: "#b91c1c" }}>{error}</p> : null}{loading ? <p>در حال بارگذاری...</p> : empty ? <p>موردی یافت نشد.</p> : children}</section>;
}

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

function SourcesPage() {
  const { projectId = "" } = useParams();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const sourceType = readQuery(searchParams, "source_type");
  const { data, loading, error } = usePolling(() => fetchProjectSources(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((item) => matches([item.source_type, item.original_url, item.ingestion_status, item.failure_summary], q) && (!sourceType || item.source_type === sourceType)), [data, q, sourceType]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="منابع" error={error} loading={loading} empty={filtered.length === 0}><div style={{ display: "grid", gap: 12 }}><div style={{ display: "flex", gap: 12 }}><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={sourceType} onChange={(e) => update({ source_type: e.target.value })}><option value="">همه نوع‌ها</option><option value="public_url">public_url</option><option value="local_file">local_file</option><option value="local_folder">local_folder</option><option value="csv_import">csv_import</option><option value="existing_dataset">existing_dataset</option><option value="archive">archive</option></select></div>{paged.map((s) => <AssetCard key={s.id} title={s.source_type} meta={[s.original_url ?? "بدون آدرس", s.ingestion_status, s.failure_summary ?? ""]} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></div></PageFrame>;
}

function UrlImportPage() {
  const { projectId = "" } = useParams();
  const [url, setUrl] = useState("");
  const [message, setMessage] = useState("");
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    let parsed: URL;
    try { parsed = new URL(url); } catch { setMessage("نشانی معتبر وارد کنید."); return; }
    if (!["http:", "https:"].includes(parsed.protocol)) { setMessage("نشانی باید http یا https باشد."); return; }
    try {
      const result = await registerUrl(baseUrl, projectId, parsed.toString());
      setMessage(`منبع ثبت شد: ${JSON.stringify(result)}`);
    } catch (err) {
      setMessage(err instanceof Error ? err.message : "ثبت نشد.");
    }
  };
  return <PageFrame title="درون‌ریزی URL" error={""} loading={false} empty={false}><form onSubmit={submit} style={{ display: "grid", gap: 12 }}><input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://..." style={{ padding: 12, borderRadius: 12 }} /><button type="submit">ثبت URL</button></form><p>{message}</p></PageFrame>;
}

function JobsPage() {
  const { projectId = "" } = useParams();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const statusFilter = readQuery(searchParams, "job_status");
  const { data, loading, error, refresh } = usePolling(() => fetchProjectJobs(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((job) => matches([job.job_type, job.status, job.failure_summary], q) && (!statusFilter || job.status === statusFilter)), [data, q, statusFilter]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="وظایف" error={error} loading={loading} empty={filtered.length === 0}><div style={{ display: "grid", gap: 12 }}><div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={statusFilter} onChange={(e) => update({ job_status: e.target.value })}><option value="">همه وضعیت‌ها</option><option value="queued">queued</option><option value="running">running</option><option value="completed">completed</option><option value="failed">failed</option><option value="cancelled">cancelled</option></select><button onClick={() => void refresh()}>بازنشانی</button><button onClick={async () => { if (confirm("همه leaseهای منقضی را بازیابی کنیم؟")) await recoverJobs(baseUrl, projectId); await refresh(); }}>بازیابی صف</button></div>{paged.map((job) => <JobCard key={job.id} job={job} onRetry={async () => { if (confirm("این job دوباره صف شود؟")) { await retryJob(baseUrl, projectId, job.id); await refresh(); } }} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></div></PageFrame>;
}

function MediaPage() {
  const { projectId = "" } = useParams();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const mediaKind = readQuery(searchParams, "media_kind");
  const { data, loading, error } = usePolling(() => fetchProjectMediaAssets(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((asset) => matches([asset.media_kind, asset.ingestion_status, asset.inspection_status, asset.extraction_status, asset.original_filename, asset.staged_path], q) && (!mediaKind || asset.media_kind === mediaKind)), [data, q, mediaKind]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="رسانه" error={error} loading={loading} empty={filtered.length === 0}><div style={{ display: "grid", gap: 12 }}><div style={{ display: "flex", gap: 12 }}><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={mediaKind} onChange={(e) => update({ media_kind: e.target.value })}><option value="">همه نوع‌ها</option><option value="video">video</option><option value="audio">audio</option></select></div>{paged.map((asset) => <MediaCard key={asset.id} asset={asset} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></div></PageFrame>;
}

function AudioPage() {
  const { projectId = "" } = useParams();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const processingStatus = readQuery(searchParams, "processing_status");
  const { data, loading, error } = usePolling(() => fetchProjectAudioAssets(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((asset) => matches([asset.processing_status, asset.validation_status, asset.audio_format, asset.codec, asset.storage_path], q) && (!processingStatus || asset.processing_status === processingStatus)), [data, q, processingStatus]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="صوت" error={error} loading={loading} empty={filtered.length === 0}><div style={{ display: "grid", gap: 12 }}><div style={{ display: "flex", gap: 12 }}><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={processingStatus} onChange={(e) => update({ processing_status: e.target.value })}><option value="">همه وضعیت‌ها</option><option value="validated">validated</option><option value="queued">queued</option><option value="running">running</option><option value="failed">failed</option></select></div>{paged.map((asset) => <AudioCard key={asset.id} asset={asset} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></div></PageFrame>;
}

function DuplicatePage() {
  const { projectId = "" } = useParams();
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const stateFilter = readQuery(searchParams, "duplicate_state");
  const { data, loading, error, refresh } = usePolling(() => fetchProjectDuplicateGroups(baseUrl, projectId), [projectId], 5000);
  const filtered = useMemo(() => (data ?? []).filter((group) => matches([group.id, group.canonical_audio_asset_id, group.members.map((m) => m.audio_asset_id).join(" "), group.decisions.map((d) => `${d.decision} ${d.similarity_score}`).join(" ")], q) && (!stateFilter || duplicateState(group) === stateFilter)), [data, q, stateFilter]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return <PageFrame title="تکراری‌ها" error={error} loading={loading} empty={filtered.length === 0}><div style={{ display: "grid", gap: 12 }}><div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}><FilterBar value={q} onChange={(value) => update({ q: value })} placeholder="جستجو..." /><select value={stateFilter} onChange={(e) => update({ duplicate_state: e.target.value })}><option value="">همه حالت‌ها</option><option value="canonical">canonical</option><option value="missing_canonical">missing_canonical</option></select><button onClick={async () => { if (confirm("canonicalهای گم‌شده را بازیابی کنیم؟")) await recoverDuplicates(baseUrl, projectId); await refresh(); }}>بازیابی canonical</button></div>{paged.map((group) => <DuplicateCard key={group.id} group={group} />)}<Pager page={page} setPage={setPage} total={filtered.length} /></div></PageFrame>;
}

function StatisticsPage() {
  const { projectId = "" } = useParams();
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
  return <PageFrame title="آمار پروژه" error={""} loading={false} empty={false}><div style={{ display: "grid", gap: 12 }}><StatsRow label="منابع" value={sourceCount} /><StatsRow label="رسانه" value={mediaCount} /><StatsRow label="صوت" value={audioCount} /><StatsRow label="گروه‌های تکراری" value={duplicateCount} /><StatsRow label="نسبت تکراری" value={formatPercent(duplicateRatio)} /><StatsRow label="دقایق یکتا" value={formatNumber(uniqueDuration)} /><StatsRow label="مصرف ذخیره" value={formatBytes(storageUsage)} /><StatsRow label="توان پایپلاین" value={formatNumber(throughput)} /><div style={card}><strong>حالت‌های job</strong><div style={metaGrid}>{Object.entries(jobStates).map(([key, value]) => <span key={key}>{key}: {value}</span>)}</div></div></div></PageFrame>;
}

function PipelinePage() {
  const { projectId = "" } = useParams();
  const { data: summary } = usePolling(() => fetchProjectMetricsSummary(baseUrl, projectId), [projectId], 5000);
  const [message, setMessage] = useState("");
  const paused = false;
  return <PageFrame title="کنترل پایپلاین" error={""} loading={false} empty={false}><div style={{ display: "grid", gap: 12 }}><p>وضعیت throughput: {formatNumber(summary?.deduplication.throughput ?? 0)}</p><div style={{ display: "flex", gap: 12, flexWrap: "wrap" }}><button onClick={async () => { if (confirm("بازیابی jobهای منقضی را اجرا کنیم؟")) { await recoverJobs(baseUrl, projectId); setMessage("بازیابی job انجام شد."); } }}>بازیابی شکست‌خورده‌ها</button><button onClick={async () => { if (confirm("canonicalهای گم‌شده را بازیابی کنیم؟")) { await recoverDuplicates(baseUrl, projectId); setMessage("بازیابی duplicate انجام شد."); } }}>بازیابی تکراری‌ها</button><button onClick={async () => { if (confirm("پایپلاین را متوقف کنیم؟")) { await pausePipeline(baseUrl, projectId); setMessage("پایپلاین متوقف شد."); } }}>توقف</button><button onClick={async () => { if (confirm("پایپلاین را ادامه دهیم؟")) { await resumePipeline(baseUrl, projectId); setMessage("پایپلاین ادامه یافت."); } }}>ادامه</button></div><p>{paused ? "متوقف" : "فعال"}</p><p>{message}</p></div></PageFrame>;
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
  return <article style={card}><strong>{label}</strong><div style={{ marginTop: 8 }}>{value}</div></article>;
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
  while (size >= 1024 && unit < units.length - 1) { size /= 1024; unit += 1; }
  return `${size.toFixed(1)} ${units[unit]}`;
}

function AssetCard({ title, meta }: { title: string; meta: string[] }) {
  return <article style={card}><strong>{title}</strong><div style={metaGrid}>{meta.filter(Boolean).map((item) => <span key={item}>{item}</span>)}</div></article>;
}

function DuplicateCard({ group }: { group: DuplicateGroup }) {
  return <article style={card}><strong>گروه {group.id}</strong><div style={metaGrid}><span>canonical: {group.canonical_audio_asset_id ?? "ندارد"}</span><span>اعضا: {group.members.length}</span><span>وضعیت: {duplicateState(group)}</span><span>شباهت: {group.decisions[0]?.similarity_score?.toFixed(3) ?? "نامشخص"}</span></div><div style={{ marginTop: 12, display: "grid", gap: 8 }}>{group.members.map((member) => <div key={member.audio_asset_id}>عضو: {member.audio_asset_id}</div>)}{group.decisions.map((decision) => <div key={decision.id}>تصمیم: {decision.decision} - confidence: {decision.confidence}</div>)}</div></article>;
}

function JobCard({ job, onRetry }: { job: Job; onRetry: () => Promise<void> }) {
  return <article style={card}><div style={{ display: "flex", justifyContent: "space-between", gap: 12 }}><strong>{job.job_type}</strong><span>{job.status}</span></div><div style={metaGrid}><span>پیشرفت: {job.progress}</span><span>تلاش‌ها: {job.attempts}</span><span>{job.priority ?? 0}</span>{job.failure_summary ? <span style={{ color: "#b91c1c" }}>{job.failure_summary}</span> : null}</div>{job.status === "failed" ? <button onClick={() => void onRetry()}>تلاش دوباره</button> : null}</article>;
}

function MediaCard({ asset }: { asset: MediaAsset }) {
  return <article style={card}><strong>{asset.media_kind}</strong><div style={metaGrid}><span>منبع: {asset.source_id}</span><span>وضعیت: {asset.ingestion_status}</span><span>بازرسی: {asset.inspection_status ?? "نامشخص"}</span><span>استخراج: {asset.extraction_status ?? "نامشخص"}</span><span>حجم: {asset.byte_size}</span><span>پروفایل: {asset.container_format ?? "نامشخص"}</span><span>منشأ: {asset.staged_path}</span></div></article>;
}

function AudioCard({ asset }: { asset: AudioAsset }) {
  return <article style={card}><strong>{asset.audio_format}</strong><div style={metaGrid}><span>شناسه رسانه: {asset.media_asset_id}</span><span>وضعیت: {asset.processing_status}</span><span>اعتبارسنجی: {asset.validation_status ?? "نامشخص"}</span><span>کدک: {asset.codec}</span><span>نرخ نمونه‌برداری: {asset.sample_rate_hz}</span><span>کانال: {asset.channels}</span><span>حجم: {asset.byte_size}</span><span>مسیر: {asset.storage_path}</span></div></article>;
}

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Navigate to="/projects" replace />} />
        <Route path="/projects" element={<WorkspaceShell><div /></WorkspaceShell>} />
        <Route path="/projects/:projectId" element={<WorkspaceShell><Navigate to="sources" replace /></WorkspaceShell>} />
        <Route path="/projects/:projectId/sources" element={<WorkspaceShell><SourcesPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/url-import" element={<WorkspaceShell><UrlImportPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/jobs" element={<WorkspaceShell><JobsPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/media" element={<WorkspaceShell><MediaPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/audio" element={<WorkspaceShell><AudioPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/duplicates" element={<WorkspaceShell><DuplicatePage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/statistics" element={<WorkspaceShell><StatisticsPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/pipeline" element={<WorkspaceShell><PipelinePage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/import" element={<WorkspaceShell><ImportPage /></WorkspaceShell>} />
        <Route path="*" element={<Navigate to="/projects" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

function ImportPage() {
  const { projectId = "" } = useParams();
  const [summary, setSummary] = useState("");
  const handleSubmit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const file = (e.currentTarget.elements.namedItem("file") as HTMLInputElement).files?.[0];
    const audioPathField = (e.currentTarget.elements.namedItem("audio_path_field") as HTMLInputElement).value || "audio_path";
    if (!file) return setSummary("یک فایل انتخاب کنید.");
    const result = await importDataset(baseUrl, projectId, file, audioPathField);
    setSummary(JSON.stringify(result, null, 2));
  };
  return <PageFrame title="درون‌ریزی دیتاست" error={""} loading={false} empty={false}><form onSubmit={handleSubmit} style={{ display: "grid", gap: 12 }}><input name="file" type="file" accept=".csv,.jsonl,.json,.parquet,.zip,.tar,.gz,.tgz" /><input name="audio_path_field" placeholder="audio_path" /><button type="submit">درون‌ریزی</button></form><pre style={{ whiteSpace: "pre-wrap" }}>{summary}</pre></PageFrame>;
}
