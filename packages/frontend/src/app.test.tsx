import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./app";

const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
  const url = String(input);
  if (url.endsWith("/health/live")) return new Response(JSON.stringify({ status: "ok" }), { status: 200 });
  if (url.endsWith("/projects")) return new Response(JSON.stringify({ items: [{ id: "project-1", name: "پژوهش", description: null }] }), { status: 200 });
  if (url.includes("/sources")) return new Response(JSON.stringify({ items: [{ id: "source-1", project_id: "project-1", source_type: "public_url", original_url: "https://example.com", ingestion_status: "staged", progress: "staged", failure_summary: null }] }), { status: 200 });
  if (url.includes("/jobs/job-1/retry")) return new Response(JSON.stringify({ id: "job-1", project_id: "project-1", source_id: "source-1", job_type: "media_ingestion", status: "queued", attempts: 2, progress: "queued", failure_summary: null }), { status: 200 });
  if (url.includes("/jobs/recover")) return new Response(JSON.stringify({ recovered: 1 }), { status: 200 });
  if (url.includes("/jobs")) return new Response(JSON.stringify({ items: [{ id: "job-1", project_id: "project-1", source_id: "source-1", job_type: "media_ingestion", status: "failed", attempts: 1, progress: "failed", failure_summary: "duplicate" }] }), { status: 200 });
  if (url.includes("/media-assets")) return new Response(JSON.stringify({ items: [{ id: "media-1", project_id: "project-1", source_id: "source-1", media_kind: "video", staged_path: "/tmp/source.bin", original_filename: "source.mp4", byte_size: 12, ingestion_status: "ready", inspection_status: "inspectable", extraction_status: "extracted", extraction_progress: "running", extraction_tmp_path: null, extracted_at: null, container_format: "mp4", duration_seconds: 12.5, codecs: "aac", sample_rate_hz: 16000, channels: 1, inspected_at: null }] }), { status: 200 });
  if (url.includes("/audio-assets")) return new Response(JSON.stringify({ items: [{ id: "audio-1", project_id: "project-1", media_asset_id: "media-1", source_id: "source-1", processing_status: "validated", normalization_profile: "flac", validation_status: "validated", validation_error: null, validated_at: null, duration_seconds: 12.5, audio_format: "flac", codec: "flac", sample_rate_hz: 16000, channels: 1, byte_size: 1024, storage_path: "/data/audio.flac" }] }), { status: 200 });
  if (url.includes("/duplicates/recover")) return new Response(JSON.stringify({ recovered: 1 }), { status: 200 });
  if (url.includes("/duplicates")) return new Response(JSON.stringify({ items: [{ id: "group-1", project_id: "project-1", canonical_audio_asset_id: "audio-1", created_at: "2026-08-04T00:00:00Z", updated_at: "2026-08-04T00:00:00Z", members: [{ duplicate_group_id: "group-1", audio_asset_id: "audio-1", created_at: "2026-08-04T00:00:00Z", updated_at: "2026-08-04T00:00:00Z" }, { duplicate_group_id: "group-1", audio_asset_id: "audio-2", created_at: "2026-08-04T00:00:00Z", updated_at: "2026-08-04T00:00:00Z" }], decisions: [{ id: "dec-1", audio_asset_low_id: "audio-1", audio_asset_high_id: "audio-2", candidate_fingerprint_low_id: "fp-1", candidate_fingerprint_high_id: "fp-2", algorithm: "perceptual", algorithm_version: "1", duration_delta_seconds: 0.1, similarity_score: 0.98, decision: "duplicate", confidence: "high", duplicate_group_id: "group-1", created_at: "2026-08-04T00:00:00Z", updated_at: "2026-08-04T00:00:00Z" }] }] }), { status: 200 });
  if (url.includes("/metrics/summary")) return new Response(JSON.stringify({ stage_runs: [{ id: "stage-1", project_id: "project-1", stage_name: "duplicate_detection", status: "completed", queue_wait_ms: 10, wall_time_ms: 20, input_bytes: 100, output_bytes: 200, input_duration_seconds: 12.5, throughput: 0.625, failure_reason: null, cpu_percent: null, memory_bytes: null }], deduplication: { throughput: 0.625, candidate_count: 1, match_rate: 1, duplicate_ratio: 0.25, unique_duration_seconds: 12.5, processing_failures: 0 } }), { status: 200 });
  if (url.includes("/pipeline/pause")) return new Response(JSON.stringify({ paused: true }), { status: 200 });
  if (url.includes("/pipeline/resume")) return new Response(JSON.stringify({ paused: false }), { status: 200 });
  if (url.includes("/sources/url") && init?.method === "POST") {
    const body = JSON.parse(String(init.body ?? "{}")) as { url?: string };
    if (body.url === "https://example.com") return new Response(JSON.stringify({ source_id: "source-1", job_id: "job-1" }), { status: 200 });
  }
  return new Response(JSON.stringify({}), { status: 200 });
});

vi.stubGlobal("fetch", fetchMock);
vi.stubGlobal("confirm", vi.fn(() => true));

describe("App", () => {
  it("renders the workspace and the new browsers", async () => {
    render(<App />);
    await waitFor(() => expect(screen.getByRole("heading", { name: "فضای کاری پروژه" })).toBeInTheDocument());
    expect(screen.getByText("تکراری‌ها")).toBeInTheDocument();
    expect(screen.getByText("آمار پروژه")).toBeInTheDocument();
    expect(screen.getByText("کنترل پایپلاین")).toBeInTheDocument();
  });

  it("persists search and filters in the url", async () => {
    render(<App />);
    await waitFor(() => expect(screen.getAllByRole("heading", { name: "منابع" }).length).toBeGreaterThan(0));
    fireEvent.change(screen.getAllByPlaceholderText("جستجو...")[0], { target: { value: "example" } });
    expect(window.location.search).toContain("q=example");
  });

  it("shows duplicate groups and pipeline actions", async () => {
    render(<App />);
    fireEvent.click(screen.getAllByRole("link", { name: "تکراری‌ها" })[0]);
    await waitFor(() => expect(screen.getByRole("heading", { name: "تکراری‌ها" })).toBeInTheDocument());
    expect(screen.getByText(/canonical: audio-1/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "بازیابی canonical" }));
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(expect.stringContaining("/duplicates/recover"), expect.any(Object)));
  });

  it("shows project statistics and pipeline controls", async () => {
    render(<App />);
    fireEvent.click(screen.getAllByRole("link", { name: "آمار پروژه" })[0]);
    await waitFor(() => expect(screen.getByRole("heading", { name: "آمار پروژه" })).toBeInTheDocument());
    expect(screen.getAllByText("1", { selector: "div" }).length).toBeGreaterThan(0);
    fireEvent.click(screen.getAllByRole("link", { name: "کنترل پایپلاین" })[0]);
    await waitFor(() => expect(screen.getByRole("heading", { name: "کنترل پایپلاین" })).toBeInTheDocument());
    expect(screen.getByRole("button", { name: "توقف" })).toBeInTheDocument();
  });
});
