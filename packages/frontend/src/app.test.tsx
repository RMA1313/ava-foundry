import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./app";

type ProjectState = {
  projects: Array<{ id: string; name: string; description: string | null; status: string; created_at: string; updated_at: string }>;
  sources: Array<{ id: string; project_id: string; source_type: string; original_url: string | null; ingestion_status: string; progress: string; failure_summary: string | null; created_at: string }>;
  jobs: Array<{ id: string; project_id: string; source_id: string | null; job_type: string; status: string; attempts: number; progress: string; failure_summary: string | null; created_at: string }>;
  media: Array<{ id: string; project_id: string; source_id: string; media_kind: string; staged_path: string; original_filename: string | null; byte_size: number; ingestion_status: string; inspection_status: string | null }>;
  audio: Array<{ id: string; project_id: string; media_asset_id: string; source_id: string | null; processing_status: string; normalization_profile: string | null; validation_status: string | null; validation_error: string | null; validated_at: string | null; duration_seconds: number; audio_format: string; codec: string; sample_rate_hz: number; channels: number; byte_size: number; storage_path: string }>;
  duplicates: Array<any>;
};

const state: ProjectState = {
  projects: [{ id: "project-1", name: "پژوهش", description: "collection", status: "active", created_at: "2026-08-04T00:00:00Z", updated_at: "2026-08-04T00:00:00Z" }],
  sources: [],
  jobs: [],
  media: [],
  audio: [],
  duplicates: [],
};

const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
  const url = String(input);
  const method = init?.method ?? "GET";

  if (url.endsWith("/health/live")) return json({ status: "ok" });
  if (url.includes("/projects?")) return json({ items: state.projects, limit: 8, offset: 0, total: state.projects.length });
  if (url.endsWith("/projects")) return json({ items: state.projects, limit: 8, offset: 0, total: state.projects.length });
  if (url.endsWith("/projects/project-1")) return json(state.projects[0]);
  if (url.endsWith("/projects/project-1/sources")) return json({ items: state.sources });
  if (url.endsWith("/projects/project-1/jobs")) return json({ items: state.jobs });
  if (url.endsWith("/projects/project-1/media-assets")) return json({ items: state.media });
  if (url.endsWith("/projects/project-1/audio-assets")) return json({ items: state.audio });
  if (url.endsWith("/projects/project-1/duplicates")) return json({ items: state.duplicates });
  if (url.endsWith("/projects/project-1/metrics/summary")) return json({ stage_runs: [], deduplication: { throughput: null, candidate_count: 0, match_rate: 0, duplicate_ratio: 0, unique_duration_seconds: 0, processing_failures: 0 } });
  if (url.endsWith("/projects/project-1") && method === "PUT") return json(state.projects[0]);
  if (url.endsWith("/projects/project-1") && method === "DELETE") return new Response(null, { status: 204 });

  if (url.endsWith("/projects/project-1/uploads/media") && method === "POST") {
    const sourceId = `source-${state.sources.length + 1}`;
    const jobId = `job-${state.jobs.length + 1}`;
    state.sources.unshift({ id: sourceId, project_id: "project-1", source_type: "upload_media", original_url: null, ingestion_status: "staged", progress: "staged", failure_summary: null, created_at: "2026-08-04T00:00:00Z" });
    state.jobs.unshift({ id: jobId, project_id: "project-1", source_id: sourceId, job_type: "media_ingestion", status: "queued", attempts: 0, progress: "queued", failure_summary: null, created_at: "2026-08-04T00:00:00Z" });
    return json({ path: "/tmp/uploaded.mp4", byte_size: 12 });
  }

  if (url.endsWith("/projects/project-1/uploads/archive") && method === "POST") return json({ path: "/tmp/uploaded.zip", byte_size: 12 });
  if (url.endsWith("/projects/project-1/sources/url") && method === "POST") {
    const sourceId = `source-${state.sources.length + 1}`;
    const jobId = `job-${state.jobs.length + 1}`;
    state.sources.unshift({ id: sourceId, project_id: "project-1", source_type: "public_url", original_url: "https://example.com", ingestion_status: "staged", progress: "staged", failure_summary: null, created_at: "2026-08-04T00:00:00Z" });
    state.jobs.unshift({ id: jobId, project_id: "project-1", source_id: sourceId, job_type: "media_ingestion", status: "queued", attempts: 0, progress: "queued", failure_summary: null, created_at: "2026-08-04T00:00:00Z" });
    return json({ source_id: sourceId, job_id: jobId });
  }

  if (url.endsWith("/projects/project-1/datasets/import") && method === "POST") return json({ imported: 2, duplicate: 1, invalid: 0, failed: 0, row_failures: [] });
  if (url.endsWith("/projects/project-1/jobs/recover") && method === "POST") return json({ recovered: 1 });
  if (url.endsWith("/projects/project-1/duplicates/recover") && method === "POST") return json({ recovered: 1 });
  if (url.endsWith("/projects/project-1/jobs/job-1/retry") && method === "POST") return json({ id: "job-1", project_id: "project-1", source_id: "source-1", job_type: "media_ingestion", status: "queued", attempts: 1, progress: "queued", failure_summary: null });
  if (url.endsWith("/projects/project-1/pipeline/pause") && method === "POST") return json({ paused: true });
  if (url.endsWith("/projects/project-1/pipeline/resume") && method === "POST") return json({ paused: false });

  return json({});
});

function json(body: unknown) {
  return new Response(JSON.stringify(body), { status: 200 });
}

function renderAt(pathname: string) {
  cleanup();
  window.history.pushState({}, "", pathname);
  return render(<App />);
}

vi.stubGlobal("fetch", fetchMock);
vi.stubGlobal("confirm", vi.fn(() => true));

describe("App", () => {
  it("shows a concise empty dashboard without fake operational metrics", async () => {
    renderAt("/projects/project-1/dashboard");
    await waitFor(() => expect(screen.getAllByRole("heading", { name: "داشبورد" })[0]).toBeInTheDocument());
    expect(screen.getByText("سامانه محلی ساخت داده صوتی فارسی")).toBeInTheDocument();
    expect(screen.getAllByText("منابع")[0]).toBeInTheDocument();
    expect(screen.queryByText("Healthy")).not.toBeInTheDocument();
    expect(screen.queryByText("Current Phase")).not.toBeInTheDocument();
    expect(screen.queryByText("Storage Used")).not.toBeInTheDocument();
  });

  it("renders source counts from backend data and updates after upload", async () => {
    renderAt("/projects/project-1/dashboard");
    await waitFor(() => expect(screen.getAllByRole("heading", { name: "داشبورد" })[0]).toBeInTheDocument());

    fireEvent.click(screen.getAllByRole("link", { name: "آپلود رسانه" })[0]);
    await waitFor(() => expect(screen.getAllByRole("heading", { name: "آپلود رسانه" })[0]).toBeInTheDocument());
    const file = new File(["abc"], "clip.mp4", { type: "video/mp4" });
    fireEvent.change(screen.getByLabelText("فایل"), { target: { files: [file] } });
    fireEvent.click(screen.getByRole("button", { name: "آپلود رسانه" }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(expect.stringContaining("/projects/project-1/uploads/media"), expect.objectContaining({ method: "POST" })));

    fireEvent.click(screen.getAllByRole("link", { name: "منابع" })[0]);
    await waitFor(() => expect(screen.getAllByRole("heading", { name: "منابع" })[0]).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText("upload_media")).toBeInTheDocument());
  });

  it("shows upload errors visibly", async () => {
    fetchMock.mockImplementation(async (input: RequestInfo | URL) => {
      const url = String(input);
      if (url.endsWith("/health/live")) return json({ status: "ok" });
      if (url.includes("/projects?")) return json({ items: state.projects, limit: 8, offset: 0, total: state.projects.length });
      if (url.endsWith("/projects")) return json({ items: state.projects, limit: 8, offset: 0, total: state.projects.length });
      if (url.endsWith("/projects/project-1")) return json(state.projects[0]);
      if (url.endsWith("/projects/project-1/uploads/archive")) throw new Error("خطای آپلود");
      return json({});
    });
    renderAt("/projects/project-1/dashboard");
    fireEvent.click(screen.getAllByRole("link", { name: "آپلود آرشیو" })[0]);
    await waitFor(() => expect(screen.getAllByRole("heading", { name: "آپلود آرشیو" })[0]).toBeInTheDocument());
    const file = new File(["abc"], "bundle.zip", { type: "application/zip" });
    fireEvent.change(screen.getByLabelText("فایل"), { target: { files: [file] } });
    fireEvent.click(screen.getByRole("button", { name: "آپلود آرشیو" }));
    await waitFor(() => expect(screen.getByText("خطای آپلود")).toBeInTheDocument());
  });

  it("renders project list and detail workflows", async () => {
    renderAt("/projects");
    await waitFor(() => expect(screen.getByRole("heading", { name: "پروژه‌ها" })).toBeInTheDocument());
    const detailsLink = await screen.findByRole("link", { name: /جزئیات/i });
    fireEvent.click(detailsLink);
    await waitFor(() => expect(screen.getByRole("heading", { name: /جزئیات/i })).toBeInTheDocument());
    expect(screen.getByLabelText("نام پروژه")).toHaveValue("پژوهش");
  });

  it("keeps navigation active on the current page", async () => {
    renderAt("/projects/project-1/dashboard");
    await waitFor(() => expect(screen.getAllByRole("heading", { name: "داشبورد" })[0]).toBeInTheDocument());
    fireEvent.click(screen.getAllByRole("link", { name: "منابع" })[0]);
    await waitFor(() => expect(screen.getAllByRole("heading", { name: "منابع" })[0]).toBeInTheDocument());
    expect(screen.getAllByRole("link", { name: "منابع" })[0]).toHaveStyle({ background: "rgb(219, 234, 254)" });
  });

  it("renders the local-file route without throwing", async () => {
    renderAt("/projects/project-1/local-file");
    await waitFor(() => expect(screen.getByRole("heading", { name: "فایل محلی" })).toBeInTheDocument());
    expect(screen.getByText("مسیر یک فایل محلی را ثبت کنید.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "ثبت فایل" })).toBeInTheDocument();
  });

  it("smoke-tests the primary route branches", async () => {
    const cases = [
      ["/projects/project-1/dashboard", "داشبورد"],
      ["/projects/project-1/sources", "منابع"],
      ["/projects/project-1/local-file", "فایل محلی"],
      ["/projects/project-1/folder-scan", "اسکن پوشه"],
      ["/projects/project-1/url-import", "ثبت لینک"],
      ["/projects/project-1/csv-import", "درون‌ریزی CSV"],
      ["/projects/project-1/upload-media", "آپلود رسانه"],
      ["/projects/project-1/upload-archive", "آپلود آرشیو"],
      ["/projects/project-1/jobs", "jobها"],
      ["/projects/project-1/media", "رسانه"],
      ["/projects/project-1/audio", "صوت"],
      ["/projects/project-1/audio/audio-1", "جزئیات صوت"],
      ["/projects/project-1/settings", "تنظیمات"],
    ] as const;

    state.sources = [];
    state.jobs = [];
    state.media = [];
    state.audio = [
      {
        id: "audio-1",
        project_id: "project-1",
        media_asset_id: "media-1",
        source_id: null,
        processing_status: "queued",
        normalization_profile: null,
        validation_status: null,
        validation_error: null,
        validated_at: null,
        duration_seconds: 0,
        audio_format: "wav",
        codec: "pcm_s16le",
        sample_rate_hz: 16000,
        channels: 1,
        byte_size: 1,
        storage_path: "/tmp/audio-1.wav",
      },
    ];

    for (const [path, heading] of cases) {
      const { unmount } = renderAt(path);
      await waitFor(() => expect(screen.getByRole("heading", { name: heading })).toBeInTheDocument());
      unmount();
    }
  });
  });
