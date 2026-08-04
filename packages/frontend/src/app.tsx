import { FormEvent, useEffect, useMemo, useState } from "react";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import {
  fetchLiveness,
  fetchProjects,
  fetchProjectAudioAssets,
  fetchProjectMetricsSummary,
  type AudioAsset,
  type Project,
  type StageRun,
} from "./api";

function badge(value: string) {
  return <span style={{ padding: "2px 8px", borderRadius: 999, background: "#1f2937", color: "white" }}>{value}</span>;
}

function DashboardPage() {
  const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8080";
  const [liveness, setLiveness] = useState<"loading" | "ready" | "unavailable">("loading");
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [audioAssets, setAudioAssets] = useState<AudioAsset[] | null>(null);
  const [stageRuns, setStageRuns] = useState<StageRun[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const selectedProject = useMemo(
    () => projects.find((project) => project.id === selectedProjectId) ?? null,
    [projects, selectedProjectId],
  );

  async function refreshProjectData(projectId: string) {
    const [nextAssets, nextRuns] = await Promise.all([
      fetchProjectAudioAssets(baseUrl, projectId),
      fetchProjectMetricsSummary(baseUrl, projectId),
    ]);
    setAudioAssets(nextAssets);
    setStageRuns(nextRuns);
    setError(null);
  }

  useEffect(() => {
    fetchLiveness(baseUrl)
      .then((ok) => setLiveness(ok ? "ready" : "unavailable"))
      .catch(() => setLiveness("unavailable"));
  }, [baseUrl]);

  useEffect(() => {
    fetchProjects(baseUrl)
      .then((items) => {
        setProjects(items);
        setSelectedProjectId((current) => current || items[0]?.id || "");
      })
      .catch(() => {
        setProjects([]);
      });
  }, [baseUrl]);

  useEffect(() => {
    if (!selectedProjectId) {
      setAudioAssets(null);
      setStageRuns(null);
      return;
    }
    let cancelled = false;
    setAudioAssets(null);
    setStageRuns(null);
    refreshProjectData(selectedProjectId).catch(() => {
      if (!cancelled) setError("API error while loading audio assets.");
    });
    const interval = window.setInterval(() => {
      refreshProjectData(selectedProjectId).catch(() => {
        if (!cancelled) setError("API error while refreshing audio assets.");
      });
    }, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [baseUrl, selectedProjectId]);

  async function handleSelect(event: FormEvent<HTMLSelectElement>) {
    setSelectedProjectId((event.currentTarget as HTMLSelectElement).value);
  }

  const projectStatus =
    liveness === "ready" ? "Backend is reachable." : liveness === "unavailable" ? "Backend is unavailable." : "Checking backend...";

  return (
    <main style={{ fontFamily: "system-ui, sans-serif", maxWidth: 1280, margin: "0 auto", padding: 24, background: "linear-gradient(180deg, #f8fafc, #eef2ff)" }}>
      <header style={{ display: "flex", justifyContent: "space-between", gap: 16, alignItems: "baseline", marginBottom: 24 }}>
        <div>
          <h1 style={{ margin: 0 }}>Audio Processing Dashboard</h1>
          <p style={{ margin: "8px 0 0" }}>{projectStatus}</p>
        </div>
      </header>

      <section style={{ marginBottom: 24 }}>
        <label>
          Project{" "}
          <select value={selectedProjectId} onChange={handleSelect}>
            <option value="">Select a project</option>
            {projects.map((project) => (
              <option key={project.id} value={project.id}>
                {project.name}
              </option>
            ))}
          </select>
        </label>
        {selectedProject ? <p>Viewing {selectedProject.name}</p> : <p>Select a project to inspect audio assets.</p>}
      </section>

      {error ? <section><strong>{error}</strong></section> : null}

      <section style={{ marginBottom: 24 }}>
        <h2>Audio Assets</h2>
        {audioAssets === null ? (
          <p>Loading audio assets...</p>
        ) : audioAssets.length === 0 ? (
          <p>No audio assets yet.</p>
        ) : (
          <div style={{ display: "grid", gap: 16 }}>
            {audioAssets.map((asset) => (
              <article key={asset.id} style={{ padding: 16, border: "1px solid #cbd5e1", borderRadius: 12, background: "white" }}>
                <div style={{ display: "flex", justifyContent: "space-between", gap: 12 }}>
                  <strong>{asset.id}</strong>
                  {badge(asset.processing_status)}
                </div>
                <div>Media Asset: {asset.media_asset_id}</div>
                <div>Source: {asset.source_id ?? "unknown"}</div>
                <div>Duration: {asset.duration_seconds}s</div>
                <div>
                  Format: {asset.audio_format} | Codec: {asset.codec} | {asset.sample_rate_hz} Hz | {asset.channels} ch | {asset.byte_size} bytes
                </div>
                <div>Storage: {asset.storage_path}</div>
                <div>Validation: {asset.validation_status ?? "unknown"}</div>
                {asset.validation_error ? <div style={{ color: "#b91c1c" }}>Failure: {asset.validation_error}</div> : null}
                {asset.normalization_profile ? <div>Profile: {asset.normalization_profile}</div> : null}
              </article>
            ))}
          </div>
        )}
      </section>

      <section>
        <h2>Stage Runs</h2>
        {stageRuns === null ? (
          <p>Loading stage runs...</p>
        ) : stageRuns.length === 0 ? (
          <p>No stage runs recorded yet.</p>
        ) : (
          <ul>
            {stageRuns.map((run) => (
              <li key={run.id}>
                {run.stage_name} {badge(run.status)} wait {run.queue_wait_ms}ms wall {run.wall_time_ms}ms throughput {run.throughput ?? 0}
                {run.failure_reason ? <div>Failure: {run.failure_reason}</div> : null}
                <div>
                  input {run.input_bytes}B output {run.output_bytes}B duration {run.input_duration_seconds}s cpu {run.cpu_percent ?? "n/a"} mem {run.memory_bytes ?? "n/a"}
                </div>
              </li>
            ))}
          </ul>
        )}
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
