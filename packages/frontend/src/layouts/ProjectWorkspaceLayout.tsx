import { ReactNode, useEffect, useState } from "react";
import { NavLink, useNavigate, useParams } from "react-router-dom";
import { fetchLiveness, fetchProjects, type Project } from "../lib/api-client";
import { ProjectRefreshProvider } from "../lib/refresh";
import { projectNavGroups, routes } from "../app/routes";

const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "/api";

export function ProjectWorkspaceLayout({ children }: { children: ReactNode }) {
  const navigate = useNavigate();
  const params = useParams();
  const [projects, setProjects] = useState<Project[]>([]);
  const [status, setStatus] = useState<"checking" | "ready" | "offline">("checking");

  useEffect(() => {
    void fetchLiveness(baseUrl).then((ok) => setStatus(ok ? "ready" : "offline"));
    void fetchProjects(baseUrl).then(setProjects);
  }, []);

  const selectedProjectId = params.projectId ?? projects[0]?.id ?? "";
  const project = projects.find((item) => item.id === selectedProjectId);
  const projectNavBase = selectedProjectId;
  useEffect(() => {
    if (!params.projectId && projects[0]?.id) navigate(routes.projectDashboard(projects[0].id), { replace: true });
  }, [navigate, params.projectId, projects]);

  return (
    <ProjectRefreshProvider>
      <div dir="rtl" lang="fa" style={shell}>
        <header style={header}>
          <div>
            <div style={brand}>سامانه محلی ساخت داده صوتی فارسی</div>
            <h1 style={title}>پروژه</h1>
            <div style={muted}>{status === "ready" ? "سرویس آماده است" : status === "offline" ? "سرویس در دسترس نیست" : "در حال بررسی"}</div>
          </div>
          <label style={projectSelect}>
            <span style={label}>پروژه</span>
            <select value={selectedProjectId} onChange={(e) => navigate(e.target.value ? routes.projectDashboard(e.target.value) : routes.projectNew)} style={input}>
              <option value="">پروژه جدید</option>
              {projects.map((projectItem) => <option key={projectItem.id} value={projectItem.id}>{projectItem.name}</option>)}
            </select>
          </label>
        </header>
        <div style={layout}>
          <aside style={sidebar}>
            <div style={{ display: "grid", gap: 8 }}>
              <div style={{ fontSize: 20, fontWeight: 700 }}>{project?.name ?? "بدون پروژه فعال"}</div>
              {project?.description ? <div style={muted}>{project.description}</div> : null}
            </div>
            <nav style={nav} aria-label="ناوبری پروژه">
              {projectNavGroups.map((group) => (
                <div key={group.title} style={navGroup}>
                  <div style={navGroupTitle}>{group.title}</div>
                  {group.items.map((item) => <NavLink key={item.label} to={item.to(selectedProjectId)} style={({ isActive }) => navLink(isActive)}>{item.label}</NavLink>)}
                </div>
              ))}
            </nav>
          </aside>
          <main style={main}>{children}</main>
        </div>
      </div>
    </ProjectRefreshProvider>
  );
}

const shell = { minHeight: "100vh", background: "#f4f7fb", color: "#0f172a", fontFamily: '"Vazirmatn", sans-serif' };
const layout = { display: "grid", gridTemplateColumns: "280px 1fr", minHeight: "calc(100vh - 88px)" };
const header = { display: "flex", justifyContent: "space-between", gap: 24, alignItems: "center", padding: "20px 24px", borderBottom: "1px solid #dbe4ee", background: "rgba(244,247,251,0.96)", backdropFilter: "blur(8px)", position: "sticky" as const, top: 0, zIndex: 10 };
const sidebar = { padding: 20, borderRight: "1px solid #dbe4ee", overflowY: "auto" as const, maxHeight: "calc(100vh - 88px)" };
const main = { padding: 24, overflow: "auto" as const };
const brand = { fontWeight: 700, fontSize: 18 };
const muted = { color: "#64748b" };
const title = { margin: 0, fontSize: 30, lineHeight: 1.2 };
const label = { fontSize: 13, color: "#64748b" };
const nav = { display: "grid", gap: 18, marginTop: 18 };
const navGroup = { display: "grid", gap: 8 };
const navGroupTitle = { fontSize: 12, color: "#64748b" };
const navLink = (active: boolean) => ({ textDecoration: "none", color: "#0f172a", padding: "10px 12px", borderRadius: 12, background: active ? "#dbeafe" : "transparent", display: "block" });
const projectSelect = { display: "grid", gap: 6, minWidth: 240 };
const input = { width: "100%", padding: "10px 12px", borderRadius: 12, border: "1px solid #cbd5e1", background: "#fff" };
