import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./app";

const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
  const url = String(input);
  if (url.endsWith("/health/live")) return new Response(JSON.stringify({ status: "ok" }), { status: 200 });
  if (url.endsWith("/projects")) return new Response(JSON.stringify({ items: [{ id: "project-1", name: "پژوهش", description: null }] }), { status: 200 });
  if (url.includes("/sources")) return new Response(JSON.stringify({ items: [{ id: "source-1", project_id: "project-1", source_type: "public_url", original_url: "https://example.com", ingestion_status: "staged", progress: "staged", failure_summary: null }] }), { status: 200 });
  if (url.includes("/jobs")) return new Response(JSON.stringify({ items: [{ id: "job-1", project_id: "project-1", source_id: "source-1", job_type: "media_ingestion", status: "failed", attempts: 1, progress: "failed", failure_summary: "duplicate" }] }), { status: 200 });
  if (url.includes("/media-assets")) return new Response(JSON.stringify({ items: [{ id: "media-1", project_id: "project-1", source_id: "source-1", media_kind: "video", staged_path: "/tmp/source.bin", original_filename: "source.mp4", byte_size: 12, ingestion_status: "ready", inspection_status: "inspectable", extraction_status: "extracted", extraction_progress: "running", extraction_tmp_path: null, extracted_at: null, container_format: "mp4", duration_seconds: 12.5, codecs: "aac", sample_rate_hz: 16000, channels: 1, inspected_at: null }] }), { status: 200 });
  if (url.includes("/audio-assets")) return new Response(JSON.stringify({ items: [{ id: "audio-1", project_id: "project-1", media_asset_id: "media-1", source_id: "source-1", processing_status: "validated", normalization_profile: "flac", validation_status: "validated", validation_error: null, validated_at: null, duration_seconds: 12.5, audio_format: "flac", codec: "flac", sample_rate_hz: 16000, channels: 1, byte_size: 1024, storage_path: "/data/audio.flac" }] }), { status: 200 });
  if (url.includes("/sources/url") && init?.method === "POST") {
    const body = JSON.parse(String(init.body ?? "{}")) as { url?: string };
    if (body.url === "https://example.com") return new Response(JSON.stringify({ source_id: "source-1", job_id: "job-1" }), { status: 200 });
  }
  if (url.includes("/jobs/job-1/retry")) return new Response(JSON.stringify({ id: "job-1", project_id: "project-1", source_id: "source-1", job_type: "media_ingestion", status: "queued", attempts: 2, progress: "queued", failure_summary: null }), { status: 200 });
  return new Response(JSON.stringify({}), { status: 200 });
});

vi.stubGlobal("fetch", fetchMock);

describe("App", () => {
  it("renders the workspace and the new browsers", async () => {
    render(<App />);
    await waitFor(() => expect(screen.getByRole("heading", { name: "فضای کاری پروژه" })).toBeInTheDocument());
    expect(screen.getByText("درون‌ریزی URL")).toBeInTheDocument();
    expect(screen.getByText("رسانه")).toBeInTheDocument();
    expect(screen.getByText("صوت")).toBeInTheDocument();
  });

  it("validates URL import input before submitting", async () => {
    render(<App />);
    fireEvent.click(screen.getAllByRole("link", { name: "درون‌ریزی URL" })[0]);
    await waitFor(() => expect(screen.getByRole("heading", { name: "درون‌ریزی URL" })).toBeInTheDocument());
    fireEvent.change(screen.getByPlaceholderText("https://..."), { target: { value: "not-a-url" } });
    fireEvent.click(screen.getByRole("button", { name: "ثبت URL" }));
    expect(await screen.findByText("نشانی معتبر وارد کنید.")).toBeInTheDocument();
  });

  it("shows the job monitor and retry action", async () => {
    render(<App />);
    fireEvent.click(screen.getAllByRole("link", { name: "وظایف" })[0]);
    await waitFor(() => expect(screen.getByRole("heading", { name: "وظایف" })).toBeInTheDocument());
    expect(screen.getByText(/duplicate/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "تلاش دوباره" })).toBeInTheDocument();
  });
});
