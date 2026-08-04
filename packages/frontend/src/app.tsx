import { FormEvent, useEffect, useMemo, useState } from "react";
import { BrowserRouter, Navigate, NavLink, Route, Routes, useNavigate, useParams } from "react-router-dom";
import {
  fetchLiveness,
  fetchProjects,
  fetchProjectSources,
  importDataset,
  uploadArchive,
  uploadMedia,
  type Project,
  type Source,
} from "./api";

const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8080";

function WorkspaceShell({ children }: { children: React.ReactNode }) {
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
    if (!params.projectId && projects[0]?.id) {
      navigate(`/projects/${projects[0].id}/sources`, { replace: true });
    }
  }, [navigate, params.projectId, projects]);

  return (
    <div dir="rtl" style={{ minHeight: "100vh", background: "linear-gradient(180deg,#f8fafc,#e2e8f0)", color: "#0f172a", fontFamily: "Tahoma, sans-serif" }}>
      <header style={{ padding: "24px 28px", display: "flex", justifyContent: "space-between", alignItems: "center", borderBottom: "1px solid rgba(15,23,42,0.08)" }}>
        <div>
          <div style={{ fontSize: 14, color: "#475569" }}>وضعیت: {status === "ready" ? "آماده" : status === "offline" ? "آفلاین" : "در حال بررسی"}</div>
          <h1 style={{ margin: "8px 0 0", fontSize: 28 }}>فضای کاری پروژه</h1>
        </div>
        <select value={selectedProjectId} onChange={(e) => navigate(`/projects/${e.target.value}/sources`)} style={{ padding: 12, borderRadius: 12 }}>
          {projects.map((project) => (
            <option key={project.id} value={project.id}>{project.name}</option>
          ))}
        </select>
      </header>
      <div style={{ display: "grid", gridTemplateColumns: "260px 1fr", gap: 20, padding: 20 }}>
        <aside style={{ background: "white", borderRadius: 20, padding: 16, boxShadow: "0 10px 30px rgba(15,23,42,0.08)" }}>
          <nav style={{ display: "grid", gap: 8 }}>
            <SideLink to={`/projects/${selectedProjectId}/sources`}>منابع</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/upload`}>بارگذاری فایل</SideLink>
            <SideLink to={`/projects/${selectedProjectId}/import`}>درون‌ریزی دیتاست</SideLink>
          </nav>
        </aside>
        <main>{children}</main>
      </div>
    </div>
  );
}

function SideLink({ to, children }: { to: string; children: React.ReactNode }) {
  return <NavLink to={to} style={({ isActive }) => ({ padding: 12, borderRadius: 14, textDecoration: "none", background: isActive ? "#0f172a" : "#f8fafc", color: isActive ? "white" : "#0f172a" })}>{children}</NavLink>;
}

function SourcesPage() {
  const { projectId = "" } = useParams();
  const [sources, setSources] = useState<Source[] | null>(null);
  const [filter, setFilter] = useState("");
  const [error, setError] = useState("");
  const load = () => fetchProjectSources(baseUrl, projectId).then(setSources).catch((err) => setError(err.message));
  useEffect(() => { load(); }, [projectId]);
  const filtered = useMemo(() => (sources ?? []).filter((s) => !filter || s.source_type.includes(filter) || s.ingestion_status.includes(filter) || (s.failure_summary ?? "").includes(filter)), [sources, filter]);
  return <section style={{ background: "white", borderRadius: 20, padding: 20 }}><div style={{ display: "flex", gap: 12, alignItems: "center" }}><h2 style={{ margin: 0 }}>منابع</h2><input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="فیلتر..." style={{ padding: 10, borderRadius: 12, flex: 1 }} /><button onClick={load}>بازنشانی</button></div>{error ? <p>{error}</p> : null}{sources === null ? <p>در حال بارگذاری...</p> : filtered.length === 0 ? <p>منبعی یافت نشد.</p> : <div style={{ display: "grid", gap: 12, marginTop: 16 }}>{filtered.map((s) => <article key={s.id} style={{ padding: 16, border: "1px solid #e2e8f0", borderRadius: 16 }}><div>{s.source_type}</div><div>{s.original_url ?? "بدون آدرس"}</div><div>{s.ingestion_status}</div>{s.failure_summary ? <div style={{ color: "#b91c1c" }}>{s.failure_summary}</div> : null}</article>)}</div>}</section>;
}

function UploadPage() {
  const { projectId = "" } = useParams();
  const [progress, setProgress] = useState(0);
  const [message, setMessage] = useState("");
  const submit = (endpoint: string) => (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const file = (event.currentTarget.elements.namedItem("file") as HTMLInputElement).files?.[0];
    if (!file) { setMessage("یک فایل انتخاب کنید."); return; }
    const xhr = new XMLHttpRequest();
    xhr.upload.onprogress = (e) => setProgress(e.lengthComputable ? Math.round((e.loaded / e.total) * 100) : 0);
    xhr.onload = () => setMessage(xhr.status < 400 ? "بارگذاری با موفقیت انجام شد." : "خطا در بارگذاری");
    xhr.open("POST", `${baseUrl}/projects/${projectId}/${endpoint}`);
    const data = new FormData();
    data.append("file", file);
    xhr.send(data);
  };
  return <section style={{ background: "white", borderRadius: 20, padding: 20, display: "grid", gap: 20 }}><h2 style={{ margin: 0 }}>بارگذاری فایل</h2><form onSubmit={submit("uploads/media")}><input name="file" type="file" accept=".wav,.mp3,.m4a,.mp4,.mkv,.webm,.flac,.ogg,.aac" /><button type="submit">بارگذاری رسانه</button></form><form onSubmit={submit("uploads/archive")}><input name="file" type="file" accept=".zip,.tar,.gz,.tgz" /><button type="submit">بارگذاری آرشیو</button></form><div>پیشرفت: {progress}%</div><div>{message}</div></section>;
}

function ImportPage() {
  const { projectId = "" } = useParams();
  const [summary, setSummary] = useState<string>("");
  const handleSubmit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const file = (e.currentTarget.elements.namedItem("file") as HTMLInputElement).files?.[0];
    const audioPathField = (e.currentTarget.elements.namedItem("audio_path_field") as HTMLInputElement).value || "audio_path";
    if (!file) return;
    const data = new FormData();
    data.append("file", file);
    data.append("audio_path_field", audioPathField);
    fetch(`${baseUrl}/projects/${projectId}/datasets/import`, { method: "POST", body: data }).then((r) => r.json()).then((json) => setSummary(JSON.stringify(json, null, 2)));
  };
  return <section style={{ background: "white", borderRadius: 20, padding: 20 }}><h2>درون‌ریزی دیتاست</h2><form onSubmit={handleSubmit} style={{ display: "grid", gap: 12 }}><input name="file" type="file" accept=".csv,.jsonl,.json,.parquet,.zip,.tar,.gz,.tgz" /><input name="audio_path_field" placeholder="audio_path" /><button type="submit">درون‌ریزی</button></form><pre style={{ whiteSpace: "pre-wrap" }}>{summary}</pre></section>;
}

export function App() {
  return <BrowserRouter><Routes><Route path="/" element={<Navigate to="/projects" replace />} /><Route path="/projects" element={<WorkspaceShell><div /></WorkspaceShell>} /><Route path="/projects/:projectId" element={<WorkspaceShell><Navigate to="sources" replace /></WorkspaceShell>} /><Route path="/projects/:projectId/sources" element={<WorkspaceShell><SourcesPage /></WorkspaceShell>} /><Route path="/projects/:projectId/upload" element={<WorkspaceShell><UploadPage /></WorkspaceShell>} /><Route path="/projects/:projectId/import" element={<WorkspaceShell><ImportPage /></WorkspaceShell>} /><Route path="*" element={<Navigate to="/projects" replace />} /></Routes></BrowserRouter>;
}
