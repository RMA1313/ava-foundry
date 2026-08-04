import { FormEvent, useEffect, useMemo, useState } from "react";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import {
  fetchLiveness,
  fetchProjects,
  fetchProjectJobs,
  fetchProjectSources,
  importCsv,
  registerFolder,
  registerLocalFile,
  registerUrl,
  retryJob,
  type Job,
  type Project,
  type Source,
} from "./api";

function DashboardPage() {
  const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8080";
  const [liveness, setLiveness] = useState<"loading" | "ready" | "unavailable">("loading");
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [sources, setSources] = useState<Source[]>([]);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [message, setMessage] = useState("Ready");

  const selectedProject = useMemo(
    () => projects.find((project) => project.id === selectedProjectId) ?? null,
    [projects, selectedProjectId],
  );

  async function refreshProjectData(projectId: string) {
    const [nextSources, nextJobs] = await Promise.all([
      fetchProjectSources(baseUrl, projectId),
      fetchProjectJobs(baseUrl, projectId),
    ]);
    setSources(nextSources);
    setJobs(nextJobs);
  }

  useEffect(() => {
    fetchLiveness(baseUrl)
      .then((ok) => setLiveness(ok ? "ready" : "unavailable"))
      .catch(() => setLiveness("unavailable"));
  }, [baseUrl]);

  useEffect(() => {
    let cancelled = false;
    fetchProjects(baseUrl)
      .then((items) => {
        if (cancelled) return;
        setProjects(items);
        setSelectedProjectId((current) => current || items[0]?.id || "");
      })
      .catch(() => {
        if (!cancelled) {
          setProjects([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [baseUrl]);

  useEffect(() => {
    if (!selectedProjectId) {
      setSources([]);
      setJobs([]);
      return;
    }
    let cancelled = false;
    refreshProjectData(selectedProjectId).catch(() => {
      if (!cancelled) {
        setMessage("Failed to load project state");
      }
    });
    const interval = window.setInterval(() => {
      refreshProjectData(selectedProjectId).catch(() => {
        if (!cancelled) {
          setMessage("Polling failed");
        }
      });
    }, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [baseUrl, selectedProjectId]);

  async function handleSubmit(
    event: FormEvent<HTMLFormElement>,
    action: (projectId: string, value: string) => Promise<unknown>,
    fieldName: string,
  ) {
    event.preventDefault();
    const form = event.currentTarget;
    if (!selectedProjectId) return;
    const formData = new FormData(form);
    const value = String(formData.get(fieldName) ?? "").trim();
    if (!value) return;
    setMessage("Submitting...");
    try {
      await action(selectedProjectId, value);
      await refreshProjectData(selectedProjectId);
      setMessage("Submission accepted");
      form.reset();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Submission failed");
    }
  }

  async function handleRetry(jobId: string) {
    if (!selectedProjectId) return;
    setMessage("Retrying job...");
    try {
      await retryJob(baseUrl, selectedProjectId, jobId);
      await refreshProjectData(selectedProjectId);
      setMessage("Retry requested");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Retry failed");
    }
  }

  return (
    <main style={{ fontFamily: "system-ui, sans-serif", padding: 24, maxWidth: 1200, margin: "0 auto" }}>
      <header style={{ display: "flex", justifyContent: "space-between", gap: 16, alignItems: "baseline" }}>
        <div>
          <h1>Data Studio</h1>
          <p>{liveness === "ready" ? "Backend is reachable." : liveness === "unavailable" ? "Backend is unavailable." : "Checking backend..."}</p>
        </div>
        <div>{message}</div>
      </header>

      <section>
        <label>
          Project
          <select value={selectedProjectId} onChange={(event) => setSelectedProjectId(event.target.value)}>
            <option value="">Select a project</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </select>
        </label>
        {selectedProject ? <p>Working on {selectedProject.name}</p> : <p>Create or select a project to register inputs.</p>}
      </section>

      <section>
        <h2>Register Inputs</h2>
        <form onSubmit={(event) => handleSubmit(event, registerUrl, "url")}>
          <input name="url" placeholder="https://example.com/audio.mp3" />
          <button type="submit">Add URL</button>
        </form>
        <form onSubmit={(event) => handleSubmit(event, registerLocalFile, "path")}>
          <input name="path" placeholder="/data/audio.wav" />
          <button type="submit">Add Local File</button>
        </form>
        <form onSubmit={(event) => handleSubmit(event, registerFolder, "path")}>
          <input name="path" placeholder="/data/folder" />
          <button type="submit">Scan Folder</button>
        </form>
        <form onSubmit={(event) => handleSubmit(event, importCsv, "path")}>
          <input name="path" placeholder="/data/import.csv" />
          <button type="submit">Import CSV</button>
        </form>
      </section>

      <section>
        <h2>Sources</h2>
        <ul>
          {sources.map((source) => (
            <li key={source.id}>
              <strong>{source.source_type}</strong> {source.original_url ?? ""}
              <div>{source.progress}</div>
              {source.failure_summary ? <div>{source.failure_summary}</div> : null}
            </li>
          ))}
        </ul>
      </section>

      <section>
        <h2>Jobs</h2>
        <ul>
          {jobs.map((job) => (
            <li key={job.id}>
              <strong>{job.job_type}</strong> {job.status} attempts:{job.attempts}
              <div>{job.progress}</div>
              {job.failure_summary ? <div>{job.failure_summary}</div> : null}
              {job.status === "failed" ? (
                <button type="button" onClick={() => handleRetry(job.id)}>
                  Retry
                </button>
              ) : null}
            </li>
          ))}
        </ul>
      </section>
    </main>
  );
}

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="*" element={<DashboardPage />} />
      </Routes>
    </BrowserRouter>
  );
}
