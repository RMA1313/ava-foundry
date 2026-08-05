import { useNavigate } from "react-router-dom";
import { useState } from "react";
import { createProject, fetchProjects } from "../../lib/api-client";
import { defaultError, EmptyState, pageStyles, WorkspacePage } from "../shared/workspace-ui";
import { usePolling } from "../shared/use-polling";

const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "/api";

export function ProjectsLanding() {
  const { data, loading, error } = usePolling(() => fetchProjects(baseUrl), [], 10000);
  const projects = data ?? [];
  return <WorkspacePage title="پروژه‌ها" subtitle="یک پروژه را انتخاب کنید یا پروژه جدید بسازید." loading={loading} error={error} empty={projects.length === 0}><div style={{ display: "flex", gap: 10 }}><a href="/projects/new" style={pageStyles.button}>پروژه تازه</a></div>{projects.length === 0 ? <EmptyState title="پروژه‌ای وجود ندارد" message="پروژه تازه بسازید." /> : null}</WorkspacePage>;
}

export function ProjectCreatePage() {
  const navigate = useNavigate();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  return <WorkspacePage title="پروژه جدید" subtitle="یک پروژه محلی تازه بسازید." error={error}><form style={{ display: "grid", gap: 12, maxWidth: 560 }} onSubmit={async (event) => { event.preventDefault(); const trimmedName = name.trim(); if (!trimmedName) return setError("نام پروژه الزامی است."); setSaving(true); setError(""); try { const project = await createProject(baseUrl, trimmedName, description.trim() || null); navigate(`/projects/${project.id}/dashboard`); } catch (err) { setError(defaultError(err)); } finally { setSaving(false); } }}><input aria-label="نام پروژه" value={name} onChange={(e) => setName(e.target.value)} style={pageStyles.input} /><textarea aria-label="توضیح" value={description} onChange={(e) => setDescription(e.target.value)} style={{ ...pageStyles.input, minHeight: 120, resize: "vertical" }} /><button type="submit" style={pageStyles.button} disabled={saving}>{saving ? "در حال ساخت..." : "ساخت پروژه"}</button></form></WorkspacePage>;
}
