import { createContext, ReactNode, useContext, useEffect, useMemo, useState } from "react";
import { BrowserRouter, Navigate, NavLink, Route, Routes, useNavigate, useParams, useSearchParams } from "react-router-dom";
import {
  createProject,
  deleteProject,
  fetchAudioAsset,
  fetchLiveness,
  fetchProject,
  fetchProjectAudioAssets,
  fetchProjectDuplicateGroups,
  fetchProjectJobs,
  fetchProjectMediaAssets,
  fetchProjectSources,
  fetchProjects,
  importCsv,
  importDataset,
  pausePipeline,
  recoverDuplicates,
  recoverJobs,
  registerFolder,
  registerLocalFile,
  registerUrl,
  resumePipeline,
  retryJob,
  updateProject,
  uploadArchive,
  uploadMedia,
  type AudioAsset,
  type DuplicateGroup,
  type Job,
  type MediaAsset,
  type Project,
  type Source,
} from "./api";

const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "/api";
const pageSize = 8;

const appStyles = {
  shell: { minHeight: "100vh", background: "#f4f7fb", color: "#0f172a", fontFamily: '"Vazirmatn", sans-serif' },
  layout: { display: "grid", gridTemplateColumns: "280px 1fr", minHeight: "calc(100vh - 88px)" },
  header: { display: "flex", justifyContent: "space-between", gap: 24, alignItems: "center", padding: "20px 24px", borderBottom: "1px solid #dbe4ee", background: "rgba(244,247,251,0.96)", backdropFilter: "blur(8px)", position: "sticky" as const, top: 0, zIndex: 10 },
  sidebar: { padding: 20, borderRight: "1px solid #dbe4ee", overflowY: "auto" as const, maxHeight: "calc(100vh - 88px)" },
  main: { padding: 24, overflow: "auto" as const },
  brand: { fontWeight: 700, fontSize: 18 },
  muted: { color: "#64748b" },
  title: { margin: 0, fontSize: 30, lineHeight: 1.2 },
  pageTitle: { margin: 0, fontSize: 24, lineHeight: 1.3 },
  sectionTitle: { margin: 0, fontSize: 18 },
  sectionText: { margin: 0, color: "#64748b", fontSize: 14, lineHeight: 1.7 },
  nav: { display: "grid", gap: 18, marginTop: 18 },
  navGroup: { display: "grid", gap: 8 },
  navGroupTitle: { fontSize: 12, color: "#64748b" },
  navLink: (active: boolean) => ({
    textDecoration: "none",
    color: "#0f172a",
    padding: "10px 12px",
    borderRadius: 12,
    background: active ? "#dbeafe" : "transparent",
    display: "block",
  }),
  panel: { display: "grid", gap: 16, marginBottom: 20 },
  card: { display: "grid", gap: 12 },
  summaryGrid: { display: "grid", gap: 12, gridTemplateColumns: "repeat(auto-fit, minmax(170px, 1fr))" },
  summaryItem: { padding: 16, borderRadius: 16, background: "#fff" },
  summaryLabel: { color: "#64748b", fontSize: 13, marginBottom: 8 },
  summaryValue: { fontSize: 24, fontWeight: 700, marginBottom: 4 },
  toolbar: { display: "flex", flexWrap: "wrap", gap: 10, alignItems: "center" },
  input: { width: "100%", padding: "10px 12px", borderRadius: 12, border: "1px solid #cbd5e1", background: "#fff" },
  button: { border: 0, borderRadius: 12, padding: "10px 14px", background: "#1d4ed8", color: "#fff", cursor: "pointer" as const, textDecoration: "none", display: "inline-flex", alignItems: "center", justifyContent: "center" },
  buttonSecondary: { border: "1px solid #cbd5e1", borderRadius: 12, padding: "10px 14px", background: "#fff", color: "#0f172a", cursor: "pointer" as const, textDecoration: "none", display: "inline-flex", alignItems: "center", justifyContent: "center" },
  list: { display: "grid", gap: 12 },
  row: { padding: 14, borderRadius: 14, background: "#fff", display: "grid", gap: 10 },
  compactRow: { display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center" },
  labelValue: { display: "grid", gap: 4 },
  label: { fontSize: 13, color: "#64748b" },
  value: { fontSize: 15, fontWeight: 500 },
  empty: { padding: 20, borderRadius: 16, background: "#fff", display: "grid", gap: 10 },
  error: { padding: 14, borderRadius: 14, background: "#fee2e2", color: "#991b1b" },
  ok: { padding: 14, borderRadius: 14, background: "#dcfce7", color: "#166534" },
  form: { display: "grid", gap: 12, maxWidth: 560 },
};

const ProjectRefreshContext = createContext(0);
const ProjectRefreshActionContext = createContext<(() => void) | null>(null);

function useProjectRefresh() {
  return useContext(ProjectRefreshActionContext);
}

function usePolling<T>(loader: () => Promise<T>, deps: unknown[], intervalMs = 5000) {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let alive = true;
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
    const timer = window.setInterval(load, intervalMs);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [...deps, tick]);

  return { data, loading, error, refresh: () => setTick((value) => value + 1) };
}

function WorkspaceShell({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const params = useParams();
  const [projects, setProjects] = useState<Project[]>([]);
  const [status, setStatus] = useState<"checking" | "ready" | "offline">("checking");
  const [refreshTick, setRefreshTick] = useState(0);

  useEffect(() => {
    void fetchLiveness(baseUrl).then((ok) => setStatus(ok ? "ready" : "offline"));
    void fetchProjects(baseUrl, { limit: 100 }).then((page) => setProjects(page.items ?? []));
  }, [refreshTick]);

  const selectedProjectId = params.projectId ?? projects[0]?.id ?? "";
  const project = projects.find((item) => item.id === selectedProjectId);
  const projectNavBase = selectedProjectId ? `/projects/${selectedProjectId}` : "/projects";

  useEffect(() => {
    if (!params.projectId && projects[0]?.id) navigate(`/projects/${projects[0].id}/dashboard`, { replace: true });
  }, [navigate, params.projectId, projects]);

  return (
    <ProjectRefreshContext.Provider value={refreshTick}>
    <ProjectRefreshActionContext.Provider value={() => setRefreshTick((value) => value + 1)}>
      <div dir="rtl" lang="fa" style={appStyles.shell}>
        <header style={appStyles.header}>
          <div>
            <div style={appStyles.brand}>سامانه محلی ساخت داده صوتی فارسی</div>
            <h1 style={appStyles.title}>پروژه</h1>
            <div style={appStyles.muted}>{status === "ready" ? "سرویس آماده است" : status === "offline" ? "سرویس در دسترس نیست" : "در حال بررسی"}</div>
          </div>
          <label style={{ display: "grid", gap: 6, minWidth: 240 }}>
            <span style={appStyles.label}>پروژه</span>
            <select value={selectedProjectId} onChange={(e) => navigate(e.target.value ? `/projects/${e.target.value}/dashboard` : "/projects/new")} style={appStyles.input}>
              <option value="">پروژه جدید</option>
              {projects.map((projectItem) => (
                <option key={projectItem.id} value={projectItem.id}>
                  {projectItem.name}
                </option>
              ))}
            </select>
          </label>
        </header>
        <div style={appStyles.layout}>
          <aside style={appStyles.sidebar}>
            <div style={{ display: "grid", gap: 8 }}>
              <div style={{ fontSize: 20, fontWeight: 700 }}>{project?.name ?? "بدون پروژه فعال"}</div>
              {project?.description ? <div style={appStyles.muted}>{project.description}</div> : null}
            </div>
            <nav style={appStyles.nav} aria-label="ناوبری پروژه">
              <NavGroup title="پروژه" items={[["داشبورد", `${projectNavBase}/dashboard`], ["منابع", `${projectNavBase}/sources`], ["jobها", `${projectNavBase}/jobs`], ["رسانه", `${projectNavBase}/media`], ["صوت", `${projectNavBase}/audio`], ["تنظیمات", `${projectNavBase}/settings`]]} />
              <NavGroup title="ورود داده" items={[["فایل محلی", `${projectNavBase}/local-file`], ["اسکن پوشه", `${projectNavBase}/folder-scan`], ["لینک", `${projectNavBase}/url-import`], ["CSV", `${projectNavBase}/csv-import`], ["آپلود رسانه", `${projectNavBase}/upload-media`], ["آپلود آرشیو", `${projectNavBase}/upload-archive`], ["درون‌ریزی مجموعه", `${projectNavBase}/import`]]} />
            </nav>
          </aside>
          <main style={appStyles.main}>{children}</main>
        </div>
      </div>
    </ProjectRefreshActionContext.Provider>
    </ProjectRefreshContext.Provider>
  );
}

function NavGroup({ title, items }: { title: string; items: Array<[string, string]> }) {
  return (
    <div style={appStyles.navGroup}>
      <div style={appStyles.navGroupTitle}>{title}</div>
      {items.map(([label, to]) => (
        <NavLink key={to} to={to} style={({ isActive }) => appStyles.navLink(isActive)}>
          {label}
        </NavLink>
      ))}
    </div>
  );
}

function WorkspacePage({ title, subtitle, children, actions, loading, error, empty }: { title: string; subtitle: string; children?: ReactNode; actions?: ReactNode; loading?: boolean; error?: string; empty?: boolean }) {
  return (
    <section style={appStyles.panel}>
      <div style={appStyles.toolbar}>
        <div style={{ flex: 1 }}>
          <h2 style={appStyles.pageTitle}>{title}</h2>
          <p style={appStyles.sectionText}>{subtitle}</p>
        </div>
        {actions}
      </div>
      {error ? <div style={appStyles.error}>{error}</div> : null}
      {loading ? <div style={appStyles.empty}>در حال بارگذاری...</div> : null}
      {!loading && !error && empty ? <div style={appStyles.empty}>داده‌ای برای نمایش وجود ندارد.</div> : null}
      {!loading && !error && !empty ? children : null}
    </section>
  );
}

function ProjectsLanding() {
  const [searchParams, setSearchParams] = useSearchParams();
  const page = Number(searchParams.get("page") ?? "1");
  const sort = searchParams.get("sort") ?? "created_at";
  const direction = searchParams.get("direction") ?? "asc";
  const status = searchParams.get("status") ?? "";
  const q = searchParams.get("q") ?? "";
  const { data, loading, error, refresh } = usePolling(
    () => fetchProjects(baseUrl, { limit: pageSize, offset: (page - 1) * pageSize, status: status || undefined, search: q || undefined, sort, direction }),
    [page, sort, direction, status, q],
    10000,
  );
  const pageData = {
    items: data?.items ?? [],
    limit: data?.limit ?? pageSize,
    offset: data?.offset ?? 0,
    total: data?.total ?? 0,
  };
  const update = (patch: Record<string, string>) => {
    const next = new URLSearchParams(searchParams);
    for (const [key, value] of Object.entries(patch)) {
      if (value) next.set(key, value);
      else next.delete(key);
    }
    next.set("page", "1");
    setSearchParams(next, { replace: true });
  };
  return (
    <WorkspacePage
      title="پروژه‌ها"
      subtitle="فهرست، جستجو، فیلتر، sort و pagination روی backend واقعی."
      loading={loading}
      error={error}
      empty={pageData.items.length === 0}
      actions={
        <div style={appStyles.toolbar}>
          <button type="button" style={appStyles.buttonSecondary} onClick={() => void refresh()}>
            بازنشانی
          </button>
          <NavLink to="/projects/new" style={appStyles.button}>
            پروژه تازه
          </NavLink>
        </div>
      }
    >
      <div style={appStyles.toolbar}>
        <input aria-label="جستجوی پروژه" value={q} onChange={(e) => update({ q: e.target.value })} placeholder="جستجو بر اساس نام..." style={{ ...appStyles.input, maxWidth: 280 }} />
        <select aria-label="فیلتر وضعیت" value={status} onChange={(e) => update({ status: e.target.value })} style={appStyles.input}>
          <option value="">همه وضعیت‌ها</option>
          <option value="active">active</option>
          <option value="archived">archived</option>
        </select>
        <select aria-label="مرتب‌سازی" value={sort} onChange={(e) => update({ sort: e.target.value })} style={appStyles.input}>
          <option value="created_at">created_at</option>
          <option value="updated_at">updated_at</option>
          <option value="name">name</option>
        </select>
        <select aria-label="جهت مرتب‌سازی" value={direction} onChange={(e) => update({ direction: e.target.value })} style={appStyles.input}>
          <option value="asc">asc</option>
          <option value="desc">desc</option>
        </select>
      </div>
      <div style={appStyles.list}>
        {pageData.items.map((project) => (
          <div key={project.id} style={appStyles.row}>
            <div style={appStyles.compactRow}>
              <div>
                <div style={appStyles.value}>{project.name}</div>
                <div style={appStyles.sectionText}>{project.status}</div>
                {project.description ? <div style={appStyles.muted}>{project.description}</div> : null}
              </div>
              <NavLink to={`/projects/${project.id}`} style={appStyles.buttonSecondary}>
                جزئیات
              </NavLink>
            </div>
          </div>
        ))}
      </div>
      <Pager page={page - 1} setPage={(nextPage) => {
        const next = new URLSearchParams(searchParams);
        next.set("page", String(nextPage + 1));
        setSearchParams(next, { replace: true });
      }} total={pageData.total} />
    </WorkspacePage>
  );
}

function ProjectCreatePage() {
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [status, setStatus] = useState("active");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  return (
    <WorkspacePage title="پروژه جدید" subtitle="یک پروژه محلی تازه بسازید." error={error}>
      <form
        style={appStyles.form}
        onSubmit={async (event) => {
          event.preventDefault();
          const trimmedName = name.trim();
          if (!trimmedName) {
            setError("نام پروژه الزامی است.");
            return;
          }
          setSaving(true);
          setError("");
          try {
            const project = await createProject(baseUrl, trimmedName, description.trim() || null, status);
            navigate(`/projects/${project.id}`);
          } catch (err) {
            setError(defaultError(err));
          } finally {
            setSaving(false);
          }
        }}
      >
        <input aria-label="نام پروژه" value={name} onChange={(e) => setName(e.target.value)} style={appStyles.input} />
        <textarea aria-label="توضیح" value={description} onChange={(e) => setDescription(e.target.value)} style={{ ...appStyles.input, minHeight: 120, resize: "vertical" }} />
        <select aria-label="وضعیت پروژه" value={status} onChange={(e) => setStatus(e.target.value)} style={appStyles.input}>
          <option value="active">active</option>
          <option value="archived">archived</option>
        </select>
        <button type="submit" style={appStyles.button} disabled={saving}>
          {saving ? "در حال ساخت..." : "ساخت پروژه"}
        </button>
      </form>
    </WorkspacePage>
  );
}

function ProjectDetailPage() {
  const navigate = useNavigate();
  const { projectId = "" } = useParams();
  const { data: project, loading, error, refresh } = usePolling(() => fetchProject(baseUrl, projectId), [projectId], 5000);
  const { data: sources } = usePolling(() => fetchProjectSources(baseUrl, projectId), [projectId], 5000);
  const { data: jobs } = usePolling(() => fetchProjectJobs(baseUrl, projectId), [projectId], 5000);
  const { data: media } = usePolling(() => fetchProjectMediaAssets(baseUrl, projectId), [projectId], 5000);
  const { data: audio } = usePolling(() => fetchProjectAudioAssets(baseUrl, projectId), [projectId], 5000);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [status, setStatus] = useState("active");
  const [saving, setSaving] = useState(false);
  const [actionError, setActionError] = useState("");

  useEffect(() => {
    if (project) {
      setName(project.name);
      setDescription(project.description ?? "");
      setStatus(project.status);
    }
  }, [project]);

  const stats = [
    ["منابع", sources?.length ?? 0],
    ["jobها", jobs?.length ?? 0],
    ["رسانه", media?.length ?? 0],
    ["صوت", audio?.length ?? 0],
  ] as const;

  return (
    <WorkspacePage
      title="جزئیات پروژه"
      subtitle="نمایش، ویرایش و حذف پروژه."
      loading={loading}
      error={error || actionError}
      actions={
        <div style={appStyles.toolbar}>
          <NavLink to={`/projects/${projectId}/dashboard`} style={appStyles.buttonSecondary}>
            داشبورد
          </NavLink>
          <button
            type="button"
            style={appStyles.buttonSecondary}
            onClick={async () => {
              if (!window.confirm("این پروژه حذف شود؟")) return;
              try {
                await deleteProject(baseUrl, projectId);
                navigate("/projects");
              } catch (err) {
                setActionError(defaultError(err));
              }
            }}
          >
            حذف پروژه
          </button>
        </div>
      }
    >
      {project ? (
        <>
          <div style={appStyles.row}>
            <div style={appStyles.value}>{project.name}</div>
            <div style={appStyles.sectionText}>{project.status}</div>
            {project.description ? <div style={appStyles.muted}>{project.description}</div> : null}
          </div>
          <div style={appStyles.summaryGrid}>
            {stats.map(([label, value]) => <SummaryItem key={label} label={label} value={value} />)}
          </div>
          <form
            style={appStyles.form}
            onSubmit={async (event) => {
              event.preventDefault();
              if (!name.trim()) {
                setActionError("نام پروژه الزامی است.");
                return;
              }
              setSaving(true);
              setActionError("");
              try {
                await updateProject(baseUrl, projectId, name.trim(), description.trim() || null, status);
                await refresh();
              } catch (err) {
                setActionError(defaultError(err));
              } finally {
                setSaving(false);
              }
            }}
          >
            <input aria-label="نام پروژه" value={name} onChange={(e) => setName(e.target.value)} style={appStyles.input} />
            <textarea aria-label="توضیح پروژه" value={description} onChange={(e) => setDescription(e.target.value)} style={{ ...appStyles.input, minHeight: 120, resize: "vertical" }} />
            <select aria-label="وضعیت پروژه" value={status} onChange={(e) => setStatus(e.target.value)} style={appStyles.input}>
              <option value="active">active</option>
              <option value="archived">archived</option>
            </select>
            <button type="submit" style={appStyles.button} disabled={saving}>
              {saving ? "در حال ذخیره..." : "ذخیره تغییرات"}
            </button>
          </form>
        </>
      ) : null}
    </WorkspacePage>
  );
}

function DashboardPage() {
  const projectId = useProjectId();
  const { data: project } = usePolling(() => fetchProject(baseUrl, projectId), [projectId], 10000);
  const refreshToken = useContext(ProjectRefreshContext);
  const sources = usePolling(() => fetchProjectSources(baseUrl, projectId), [projectId, refreshToken], 5000);
  const jobs = usePolling(() => fetchProjectJobs(baseUrl, projectId), [projectId, refreshToken], 5000);
  const media = usePolling(() => fetchProjectMediaAssets(baseUrl, projectId), [projectId, refreshToken], 5000);
  const audio = usePolling(() => fetchProjectAudioAssets(baseUrl, projectId), [projectId, refreshToken], 5000);

  const latestSources = (sources.data ?? []).slice(0, 3);
  const latestJobs = (jobs.data ?? []).slice(0, 3);

  return (
    <WorkspacePage
      title="داشبورد"
      subtitle="فقط داده‌های واقعی backend در این نما دیده می‌شود."
      loading={sources.loading || jobs.loading || media.loading || audio.loading}
      error={sources.error || jobs.error || media.error || audio.error}
    >
      {project ? (
        <div style={appStyles.row}>
          <div style={appStyles.value}>{project.name}</div>
          {project.description ? <div style={appStyles.muted}>{project.description}</div> : null}
        </div>
      ) : null}
      <div style={appStyles.summaryGrid}>
        <SummaryItem label="منابع" value={sources.data?.length ?? 0} detail="از /sources" />
        <SummaryItem label="رسانه" value={media.data?.length ?? 0} detail="از /media-assets" />
        <SummaryItem label="صوت" value={audio.data?.length ?? 0} detail="از /audio-assets" />
        <SummaryItem label="jobها" value={jobs.data?.length ?? 0} detail="از /jobs" />
      </div>
      <Section title="آخرین منابع" subtitle="فهرست مستقیم منابع backend">
        {latestSources.length === 0 ? <EmptyState title="منبعی وجود ندارد" message="یک فایل یا لینک وارد کنید تا این بخش پر شود." /> : latestSources.map((source) => <SourceRow key={source.id} source={source} compact />)}
      </Section>
      <Section title="آخرین jobها" subtitle="فقط jobهای واقعی">
        {latestJobs.length === 0 ? <EmptyState title="jobی وجود ندارد" message="پس از import یا upload، jobها اینجا دیده می‌شوند." /> : latestJobs.map((job) => <JobRow key={job.id} job={job} compact />)}
      </Section>
    </WorkspacePage>
  );
}

function SourcesPage() {
  const projectId = useProjectId();
  const refreshToken = useContext(ProjectRefreshContext);
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const sourceType = readQuery(searchParams, "source_type");
  const { data, loading, error } = usePolling(() => fetchProjectSources(baseUrl, projectId), [projectId, refreshToken], 5000);
  const filtered = useMemo(() => (data ?? []).filter((item) => matches([item.source_type, item.original_url, item.ingestion_status, item.failure_summary], q) && (!sourceType || item.source_type === sourceType)), [data, q, sourceType]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return (
    <WorkspacePage title="منابع" subtitle="فهرست منابع واقعی پروژه." loading={loading} error={error} empty={filtered.length === 0}>
      <div style={appStyles.toolbar}>
        <input value={q} onChange={(e) => update({ q: e.target.value })} placeholder="جستجو در منابع..." style={{ ...appStyles.input, maxWidth: 280 }} />
        <select value={sourceType} onChange={(e) => update({ source_type: e.target.value })} style={appStyles.input}>
          <option value="">همه نوع‌ها</option>
          <option value="public_url">public_url</option>
          <option value="local_file">local_file</option>
          <option value="local_folder">local_folder</option>
          <option value="csv_import">csv_import</option>
          <option value="existing_dataset">existing_dataset</option>
          <option value="archive">archive</option>
        </select>
      </div>
      {paged.length === 0 ? <EmptyState title="منبعی وجود ندارد" message="یک فایل یا لینک وارد کنید تا این بخش پر شود." /> : paged.map((source) => <SourceRow key={source.id} source={source} />)}
      <Pager page={page} setPage={setPage} total={filtered.length} />
    </WorkspacePage>
  );
}

function UrlImportPage() {
  const projectId = useProjectId();
  const refresh = useProjectRefresh();
  const [url, setUrl] = useState("");
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);

  return (
    <WorkspacePage title="ثبت لینک" subtitle="ثبت لینک عمومی برای ingest." error={error}>
      <form
        style={appStyles.form}
        onSubmit={async (event) => {
          event.preventDefault();
          try {
            const parsed = new URL(url);
            if (!["http:", "https:"].includes(parsed.protocol)) {
              setError("نشانی باید http یا https باشد.");
              return;
            }
            setSubmitting(true);
            setError("");
            const result = await registerUrl(baseUrl, projectId, parsed.toString());
            setMessage(`منبع ثبت شد: ${result.source_id}`);
            refresh?.();
          } catch (err) {
            setError(defaultError(err));
          } finally {
            setSubmitting(false);
          }
        }}
      >
        <input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://..." style={appStyles.input} dir="ltr" />
        <button type="submit" style={appStyles.button} disabled={submitting}>
          {submitting ? "در حال ارسال..." : "ثبت URL"}
        </button>
      </form>
      {message ? <div style={appStyles.ok}>{message}</div> : null}
    </WorkspacePage>
  );
}

function SourcePathForm({ title, subtitle, submitLabel, fieldLabel, placeholder, onSubmit }: { title: string; subtitle: string; submitLabel: string; fieldLabel: string; placeholder: string; onSubmit: (path: string) => Promise<unknown> }) {
  const refresh = useProjectRefresh();
  const [path, setPath] = useState("");
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);

  return (
    <WorkspacePage title={title} subtitle={subtitle} error={error}>
      <form
        style={appStyles.form}
        onSubmit={async (event) => {
          event.preventDefault();
          if (!path.trim()) {
            setError("مسیر الزامی است.");
            return;
          }
          setSubmitting(true);
          setError("");
          try {
            const result = await onSubmit(path.trim());
            setMessage(JSON.stringify(result, null, 2));
            refresh?.();
          } catch (err) {
            setError(defaultError(err));
          } finally {
            setSubmitting(false);
          }
        }}
      >
        <label style={appStyles.label}>{fieldLabel}</label>
        <input value={path} onChange={(e) => setPath(e.target.value)} placeholder={placeholder} style={appStyles.input} dir="ltr" />
        <button type="submit" style={appStyles.button} disabled={submitting}>
          {submitting ? "در حال ارسال..." : submitLabel}
        </button>
      </form>
      {message ? <pre style={{ whiteSpace: "pre-wrap" }}>{message}</pre> : null}
    </WorkspacePage>
  );
}

function UploadForm({ title, subtitle, submitLabel, accept, onSubmit }: { title: string; subtitle: string; submitLabel: string; accept: string; onSubmit: (file: File) => Promise<Response> }) {
  const refresh = useProjectRefresh();
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);

  return (
    <WorkspacePage title={title} subtitle={subtitle} error={error}>
      <form
        style={appStyles.form}
        onSubmit={async (event) => {
          event.preventDefault();
          const file = (event.currentTarget.elements.namedItem("file") as HTMLInputElement).files?.[0];
          if (!file) {
            setError("یک فایل انتخاب کنید.");
            return;
          }
          setSubmitting(true);
          setError("");
          try {
            const response = await onSubmit(file);
            setMessage(JSON.stringify(await response.json(), null, 2));
            refresh?.();
          } catch (err) {
            setError(defaultError(err));
          } finally {
            setSubmitting(false);
          }
        }}
      >
        <input aria-label="فایل" name="file" type="file" accept={accept} style={appStyles.input} />
        <button type="submit" style={appStyles.button} disabled={submitting}>
          {submitting ? "در حال ارسال..." : submitLabel}
        </button>
      </form>
      {message ? <pre style={{ whiteSpace: "pre-wrap" }}>{message}</pre> : null}
    </WorkspacePage>
  );
}

function JobsPage() {
  const projectId = useProjectId();
  const refreshToken = useContext(ProjectRefreshContext);
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const statusFilter = readQuery(searchParams, "job_status");
  const { data, loading, error, refresh } = usePolling(() => fetchProjectJobs(baseUrl, projectId), [projectId, refreshToken], 5000);
  const filtered = useMemo(() => (data ?? []).filter((job) => matches([job.job_type, job.status, job.failure_summary], q) && (!statusFilter || job.status === statusFilter)), [data, q, statusFilter]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return (
    <WorkspacePage
      title="jobها"
      subtitle="فهرست jobهای پروژه."
      loading={loading}
      error={error}
      empty={filtered.length === 0}
      actions={
        <div style={appStyles.toolbar}>
          <button style={appStyles.buttonSecondary} onClick={() => void refresh()}>
            بازنشانی
          </button>
          <button style={appStyles.buttonSecondary} onClick={async () => { await recoverJobs(baseUrl, projectId); refresh(); }}>
            بازیابی صف
          </button>
        </div>
      }
    >
      <div style={appStyles.toolbar}>
        <input value={q} onChange={(e) => update({ q: e.target.value })} placeholder="جستجو..." style={{ ...appStyles.input, maxWidth: 280 }} />
        <select value={statusFilter} onChange={(e) => update({ job_status: e.target.value })} style={appStyles.input}>
          <option value="">همه وضعیت‌ها</option>
          <option value="queued">queued</option>
          <option value="running">running</option>
          <option value="completed">completed</option>
          <option value="failed">failed</option>
          <option value="cancelled">cancelled</option>
        </select>
      </div>
      {paged.length === 0 ? <EmptyState title="jobی وجود ندارد" message="پس از import یا upload، jobها اینجا دیده می‌شوند." /> : paged.map((job) => <JobRow key={job.id} job={job} onRetry={async () => { await retryJob(baseUrl, projectId, job.id); refresh(); }} />)}
      <Pager page={page} setPage={setPage} total={filtered.length} />
    </WorkspacePage>
  );
}

function MediaPage() {
  const projectId = useProjectId();
  const refreshToken = useContext(ProjectRefreshContext);
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const mediaKind = readQuery(searchParams, "media_kind");
  const { data, loading, error } = usePolling(() => fetchProjectMediaAssets(baseUrl, projectId), [projectId, refreshToken], 5000);
  const filtered = useMemo(() => (data ?? []).filter((item) => matches([item.media_kind, item.ingestion_status, item.original_filename, item.staged_path], q) && (!mediaKind || item.media_kind === mediaKind)), [data, q, mediaKind]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return (
    <WorkspacePage title="رسانه" subtitle="فهرست media assetهای پروژه." loading={loading} error={error} empty={filtered.length === 0}>
      <div style={appStyles.toolbar}>
        <input value={q} onChange={(e) => update({ q: e.target.value })} placeholder="جستجو..." style={{ ...appStyles.input, maxWidth: 280 }} />
        <select value={mediaKind} onChange={(e) => update({ media_kind: e.target.value })} style={appStyles.input}>
          <option value="">همه نوع‌ها</option>
          <option value="video">video</option>
          <option value="audio">audio</option>
        </select>
      </div>
      {paged.length === 0 ? <EmptyState title="رسانه‌ای وجود ندارد" message="پس از upload یا import این بخش پر می‌شود." /> : paged.map((asset) => <MediaRow key={asset.id} asset={asset} />)}
      <Pager page={page} setPage={setPage} total={filtered.length} />
    </WorkspacePage>
  );
}

function AudioPage() {
  const projectId = useProjectId();
  const refreshToken = useContext(ProjectRefreshContext);
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const processingStatus = readQuery(searchParams, "processing_status");
  const { data, loading, error } = usePolling(() => fetchProjectAudioAssets(baseUrl, projectId), [projectId, refreshToken], 5000);
  const filtered = useMemo(() => (data ?? []).filter((item) => matches([item.processing_status, item.validation_status, item.audio_format, item.codec, item.storage_path], q) && (!processingStatus || item.processing_status === processingStatus)), [data, q, processingStatus]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return (
    <WorkspacePage title="صوت" subtitle="فهرست audio assetهای پروژه." loading={loading} error={error} empty={filtered.length === 0}>
      <div style={appStyles.toolbar}>
        <input value={q} onChange={(e) => update({ q: e.target.value })} placeholder="جستجو..." style={{ ...appStyles.input, maxWidth: 280 }} />
        <select value={processingStatus} onChange={(e) => update({ processing_status: e.target.value })} style={appStyles.input}>
          <option value="">همه وضعیت‌ها</option>
          <option value="validated">validated</option>
          <option value="queued">queued</option>
          <option value="running">running</option>
          <option value="failed">failed</option>
        </select>
      </div>
      {paged.length === 0 ? <EmptyState title="فایلی وجود ندارد" message="پس از پردازش، فایل‌های صوتی اینجا دیده می‌شوند." /> : paged.map((asset) => <AudioRow key={asset.id} asset={asset} />)}
      <Pager page={page} setPage={setPage} total={filtered.length} />
    </WorkspacePage>
  );
}

function DuplicatePage() {
  const projectId = useProjectId();
  const refreshToken = useContext(ProjectRefreshContext);
  const { searchParams, update } = useWorkspaceQuery();
  const [page, setPage] = useState(0);
  const q = readQuery(searchParams, "q");
  const stateFilter = readQuery(searchParams, "duplicate_state");
  const { data, loading, error, refresh } = usePolling(() => fetchProjectDuplicateGroups(baseUrl, projectId), [projectId, refreshToken], 5000);
  const filtered = useMemo(() => (data ?? []).filter((group) => matches([group.id, group.canonical_audio_asset_id, group.members.map((m) => m.audio_asset_id).join(" "), group.decisions.map((d) => `${d.decision} ${d.similarity_score}`).join(" ")], q) && (!stateFilter || duplicateState(group) === stateFilter)), [data, q, stateFilter]);
  const paged = filtered.slice(page * pageSize, (page + 1) * pageSize);
  return (
    <WorkspacePage
      title="تکراری‌ها"
      subtitle="گروه‌های duplicate پروژه."
      loading={loading}
      error={error}
      empty={filtered.length === 0}
      actions={
        <button style={appStyles.buttonSecondary} onClick={async () => { await recoverDuplicates(baseUrl, projectId); refresh(); }}>
          بازیابی canonical
        </button>
      }
    >
      <div style={appStyles.toolbar}>
        <input value={q} onChange={(e) => update({ q: e.target.value })} placeholder="جستجو..." style={{ ...appStyles.input, maxWidth: 280 }} />
        <select value={stateFilter} onChange={(e) => update({ duplicate_state: e.target.value })} style={appStyles.input}>
          <option value="">همه وضعیت‌ها</option>
          <option value="canonical">canonical</option>
          <option value="missing_canonical">missing_canonical</option>
        </select>
      </div>
      {paged.length === 0 ? <EmptyState title="گروه تکراری وجود ندارد" message="در صورت وجود، اینجا نمایش داده می‌شود." /> : paged.map((group) => <DuplicateRow key={group.id} group={group} />)}
      <Pager page={page} setPage={setPage} total={filtered.length} />
    </WorkspacePage>
  );
}

function StatisticsPage() {
  const projectId = useProjectId();
  const refreshToken = useContext(ProjectRefreshContext);
  const { data: sources } = usePolling(() => fetchProjectSources(baseUrl, projectId), [projectId, refreshToken], 5000);
  const { data: jobs } = usePolling(() => fetchProjectJobs(baseUrl, projectId), [projectId, refreshToken], 5000);
  const { data: media } = usePolling(() => fetchProjectMediaAssets(baseUrl, projectId), [projectId, refreshToken], 5000);
  const { data: audio } = usePolling(() => fetchProjectAudioAssets(baseUrl, projectId), [projectId, refreshToken], 5000);
  const { data: duplicates } = usePolling(() => fetchProjectDuplicateGroups(baseUrl, projectId), [projectId, refreshToken], 5000);
  return (
    <WorkspacePage title="آمار" subtitle="خلاصه‌ای از شمارش‌های واقعی پروژه." loading={false}>
      <div style={appStyles.summaryGrid}>
        <SummaryItem label="منابع" value={sources?.length ?? 0} />
        <SummaryItem label="رسانه" value={media?.length ?? 0} />
        <SummaryItem label="صوت" value={audio?.length ?? 0} />
        <SummaryItem label="گروه‌های تکراری" value={duplicates?.length ?? 0} />
        <SummaryItem label="jobها" value={jobs?.length ?? 0} />
      </div>
    </WorkspacePage>
  );
}

function SettingsPage() {
  const projectId = useProjectId();
  const refreshToken = useContext(ProjectRefreshContext);
  const { data: project, loading, error, refresh } = usePolling(() => fetchProject(baseUrl, projectId), [projectId, refreshToken], 5000);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [saveState, setSaveState] = useState<{ kind: "idle" | "saving" | "success" | "error"; message: string }>({ kind: "idle", message: "" });

  useEffect(() => {
    if (project) {
      setName(project.name);
      setDescription(project.description ?? "");
    }
  }, [project]);

  return (
    <WorkspacePage title="تنظیمات" subtitle="نام و توضیح پروژه." loading={loading} error={error}>
      <form
        style={appStyles.form}
        onSubmit={async (event) => {
          event.preventDefault();
          const trimmedName = name.trim();
          if (!trimmedName) {
            setSaveState({ kind: "error", message: "نام پروژه الزامی است." });
            return;
          }
          setSaveState({ kind: "saving", message: "" });
          try {
            await updateProject(baseUrl, projectId, trimmedName, description.trim() || null);
            await refresh();
            setSaveState({ kind: "success", message: "تنظیمات پروژه ذخیره شد." });
          } catch (err) {
            setSaveState({ kind: "error", message: defaultError(err) });
          }
        }}
      >
        <input aria-label="نام پروژه" value={name} onChange={(e) => setName(e.target.value)} style={appStyles.input} />
        <textarea aria-label="توضیح" value={description} onChange={(e) => setDescription(e.target.value)} style={{ ...appStyles.input, minHeight: 120, resize: "vertical" }} />
        <div style={appStyles.toolbar}>
          <button type="submit" style={appStyles.button} disabled={saveState.kind === "saving"}>
            {saveState.kind === "saving" ? "در حال ذخیره..." : "ذخیره تنظیمات"}
          </button>
          <button type="button" style={appStyles.buttonSecondary} onClick={async () => refresh()}>
            بازگشت به آخرین مقدار
          </button>
        </div>
      </form>
      {saveState.kind === "success" ? <div style={appStyles.ok}>{saveState.message}</div> : null}
      {saveState.kind === "error" ? <div style={appStyles.error}>{saveState.message}</div> : null}
    </WorkspacePage>
  );
}

function ImportPage() {
  const projectId = useProjectId();
  const refresh = useProjectRefresh();
  const [summary, setSummary] = useState("");
  const [error, setError] = useState("");

  return (
    <WorkspacePage title="درون‌ریزی مجموعه" subtitle="CSV، JSONL، JSON، Parquet و آرشیو." error={error}>
      <form
        style={appStyles.form}
        onSubmit={async (event) => {
          event.preventDefault();
          const file = (event.currentTarget.elements.namedItem("file") as HTMLInputElement).files?.[0];
          const audioPathField = (event.currentTarget.elements.namedItem("audio_path_field") as HTMLInputElement).value || "audio_path";
          if (!file) {
            setError("یک فایل انتخاب کنید.");
            return;
          }
          setError("");
          try {
            const result = await importDataset(baseUrl, projectId, file, audioPathField);
            setSummary(JSON.stringify(result, null, 2));
            refresh?.();
          } catch (err) {
            setError(defaultError(err));
          }
        }}
      >
        <input aria-label="فایل" name="file" type="file" accept=".csv,.jsonl,.json,.parquet,.zip,.tar,.gz,.tgz" style={appStyles.input} />
        <input name="audio_path_field" placeholder="audio_path" style={appStyles.input} />
        <button type="submit" style={appStyles.button}>درون‌ریزی</button>
      </form>
      {summary ? <pre style={{ whiteSpace: "pre-wrap" }}>{summary}</pre> : null}
    </WorkspacePage>
  );
}

function LocalFileRoute() {
  const projectId = useProjectId();
  return <SourcePathForm title="فایل محلی" subtitle="مسیر یک فایل محلی را ثبت کنید." submitLabel="ثبت فایل" fieldLabel="مسیر فایل" placeholder="/path/to/media.mp4" onSubmit={async (path) => registerLocalFile(baseUrl, projectId, path)} />;
}

function FolderScanRoute() {
  const projectId = useProjectId();
  return <SourcePathForm title="اسکن پوشه" subtitle="پوشه‌ای را برای کشف رسانه ثبت کنید." submitLabel="اسکن پوشه" fieldLabel="مسیر پوشه" placeholder="/path/to/folder" onSubmit={async (path) => registerFolder(baseUrl, projectId, path)} />;
}

function CsvImportRoute() {
  const projectId = useProjectId();
  return <SourcePathForm title="درون‌ریزی CSV" subtitle="فایل CSV حاوی URLها را ثبت کنید." submitLabel="درون‌ریزی CSV" fieldLabel="مسیر CSV" placeholder="/path/to/urls.csv" onSubmit={async (path) => importCsv(baseUrl, projectId, path)} />;
}

function EmptyState({ title, message }: { title: string; message: string }) {
  return (
    <div style={appStyles.empty}>
      <strong>{title}</strong>
      <div style={appStyles.muted}>{message}</div>
    </div>
  );
}

function Section({ title, subtitle, children }: { title: string; subtitle: string; children: ReactNode }) {
  return (
    <section style={appStyles.card}>
      <div>
        <h3 style={appStyles.sectionTitle}>{title}</h3>
        <p style={appStyles.sectionText}>{subtitle}</p>
      </div>
      <div style={appStyles.list}>{children}</div>
    </section>
  );
}

function SummaryItem({ label, value, detail }: { label: string; value: string | number; detail?: string }) {
  return (
    <div style={appStyles.summaryItem}>
      <div style={appStyles.summaryLabel}>{label}</div>
      <div style={appStyles.summaryValue}>{value}</div>
      {detail ? <div style={appStyles.sectionText}>{detail}</div> : null}
    </div>
  );
}

function SourceRow({ source, compact = false }: { source: Source; compact?: boolean }) {
  return (
    <div style={appStyles.row}>
      <div style={appStyles.compactRow}>
        <div className={compact ? "" : ""}>
          <div style={appStyles.value}>{source.source_type}</div>
          <div style={appStyles.sectionText}>{source.ingestion_status}</div>
        </div>
        <div style={appStyles.muted}>{source.created_at ?? "—"}</div>
      </div>
      <div style={appStyles.sectionText}>{source.original_url ?? source.failure_summary ?? "—"}</div>
    </div>
  );
}

function JobRow({ job, compact = false, onRetry }: { job: Job; compact?: boolean; onRetry?: () => Promise<void> }) {
  return (
    <div style={appStyles.row}>
      <div style={appStyles.compactRow}>
        <div>
          <div style={appStyles.value}>{job.job_type}</div>
          <div style={appStyles.sectionText}>{job.status}</div>
        </div>
        <div style={appStyles.muted}>{job.created_at ?? "—"}</div>
      </div>
      <div style={appStyles.sectionText}>{job.failure_summary ?? job.progress ?? "—"}</div>
      {job.status === "failed" && onRetry ? (
        <button
          type="button"
          style={appStyles.buttonSecondary}
          onClick={() => {
            void onRetry();
          }}
        >
          تلاش دوباره
        </button>
      ) : null}
    </div>
  );
}

function MediaRow({ asset }: { asset: MediaAsset }) {
  return (
    <div style={appStyles.row}>
      <div style={appStyles.compactRow}>
        <div>
          <div style={appStyles.value}>{asset.media_kind}</div>
          <div style={appStyles.sectionText}>{asset.ingestion_status}</div>
        </div>
        <div style={appStyles.muted}>{asset.original_filename ?? asset.staged_path}</div>
      </div>
      <div style={appStyles.sectionText}>{asset.inspection_status ?? "—"}</div>
    </div>
  );
}

function AudioRow({ asset }: { asset: AudioAsset }) {
  return (
    <div style={appStyles.row}>
      <div style={appStyles.compactRow}>
        <div>
          <div style={appStyles.value}>{asset.audio_format}</div>
          <div style={appStyles.sectionText}>{asset.processing_status}</div>
        </div>
        <NavLink to={`/projects/${asset.project_id}/audio/${asset.id}`} style={appStyles.buttonSecondary}>
          جزئیات
        </NavLink>
      </div>
      <div style={appStyles.sectionText}>{asset.storage_path}</div>
    </div>
  );
}

function DuplicateRow({ group }: { group: DuplicateGroup }) {
  return (
    <div style={appStyles.row}>
      <div style={appStyles.compactRow}>
        <div>
          <div style={appStyles.value}>{group.id}</div>
          <div style={appStyles.sectionText}>{duplicateState(group)}</div>
        </div>
        <div style={appStyles.muted}>{group.members.length} عضو</div>
      </div>
      <div style={appStyles.sectionText}>{group.canonical_audio_asset_id ?? "—"}</div>
    </div>
  );
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

function readQuery(searchParams: URLSearchParams, key: string) {
  return searchParams.get(key) ?? "";
}

function useProjectId() {
  return useParams().projectId ?? "";
}

function Pager({ page, setPage, total }: { page: number; setPage: (page: number) => void; total: number }) {
  const canPrev = page > 0;
  const canNext = (page + 1) * pageSize < total;
  return (
    <div style={appStyles.toolbar}>
      <span style={appStyles.muted}>
        صفحه {page + 1} از {Math.max(1, Math.ceil(total / pageSize))}
      </span>
      <button style={appStyles.buttonSecondary} disabled={!canPrev} onClick={() => setPage(page - 1)}>
        قبلی
      </button>
      <button style={appStyles.buttonSecondary} disabled={!canNext} onClick={() => setPage(page + 1)}>
        بعدی
      </button>
    </div>
  );
}

function duplicateState(group: DuplicateGroup) {
  return group.canonical_audio_asset_id ? "canonical" : "missing_canonical";
}

function matches(values: Array<string | null | undefined>, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return values.some((value) => (value ?? "").toLowerCase().includes(needle));
}

function defaultError(err: unknown) {
  return err instanceof Error ? err.message : "خطای ناشناخته";
}

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Navigate to="/projects" replace />} />
        <Route path="/projects" element={<WorkspaceShell><ProjectsLanding /></WorkspaceShell>} />
        <Route path="/projects/new" element={<WorkspaceShell><ProjectCreatePage /></WorkspaceShell>} />
        <Route path="/projects/:projectId" element={<WorkspaceShell><ProjectDetailPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/dashboard" element={<WorkspaceShell><DashboardPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/sources" element={<WorkspaceShell><SourcesPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/url-import" element={<WorkspaceShell><UrlImportPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/local-file" element={<WorkspaceShell><LocalFileRoute /></WorkspaceShell>} />
        <Route path="/projects/:projectId/folder-scan" element={<WorkspaceShell><FolderScanRoute /></WorkspaceShell>} />
        <Route path="/projects/:projectId/csv-import" element={<WorkspaceShell><CsvImportRoute /></WorkspaceShell>} />
        <Route path="/projects/:projectId/upload-media" element={<WorkspaceShell><UploadMediaRoute /></WorkspaceShell>} />
        <Route path="/projects/:projectId/upload-archive" element={<WorkspaceShell><UploadArchiveRoute /></WorkspaceShell>} />
        <Route path="/projects/:projectId/jobs" element={<WorkspaceShell><JobsPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/media" element={<WorkspaceShell><MediaPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/audio" element={<WorkspaceShell><AudioPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/audio/:audioAssetId" element={<WorkspaceShell><AudioDetailPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/duplicates" element={<WorkspaceShell><DuplicatePage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/statistics" element={<WorkspaceShell><StatisticsPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/settings" element={<WorkspaceShell><SettingsPage /></WorkspaceShell>} />
        <Route path="/projects/:projectId/import" element={<WorkspaceShell><ImportPage /></WorkspaceShell>} />
        <Route path="*" element={<Navigate to="/projects" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

function UploadMediaRoute() {
  const projectId = useProjectId();
  return <UploadForm title="آپلود رسانه" subtitle="فایل رسانه‌ای را در staging ثبت کنید." submitLabel="آپلود رسانه" accept=".mp4,.mkv,.mov,.webm,.mp3,.wav,.flac,.aac,.m4a" onSubmit={async (file) => uploadMedia(baseUrl, projectId, file)} />;
}

function UploadArchiveRoute() {
  const projectId = useProjectId();
  return <UploadForm title="آپلود آرشیو" subtitle="zip/tar/tgz را برای پردازش ثبت کنید." submitLabel="آپلود آرشیو" accept=".zip,.tar,.gz,.tgz" onSubmit={async (file) => uploadArchive(baseUrl, projectId, file)} />;
}

function AudioDetailPage() {
  const projectId = useProjectId();
  const { audioAssetId = "" } = useParams();
  const { data, loading, error } = usePolling(() => fetchAudioAsset(baseUrl, projectId, audioAssetId), [projectId, audioAssetId], 5000);
  return (
    <WorkspacePage title="جزئیات صوت" subtitle="جزئیات یک audio asset." loading={loading} error={error} empty={!data}>
      {data ? (
        <div style={appStyles.row}>
          <div style={appStyles.value}>{data.audio_format}</div>
          <div style={appStyles.sectionText}>{data.storage_path}</div>
        </div>
      ) : null}
    </WorkspacePage>
  );
}
