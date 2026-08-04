import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App } from "./app";

const responses = [
  new Response(JSON.stringify({ status: "ok" }), { status: 200 }),
  new Response(JSON.stringify({ items: [{ id: "project-1", name: "Research", description: null }] }), { status: 200 }),
  new Response(JSON.stringify({ items: [{ id: "audio-1", project_id: "project-1", media_asset_id: "media-1", source_id: "source-1", processing_status: "validated", normalization_profile: "flac:flac:16000:1", validation_status: "validated", validation_error: null, validated_at: null, duration_seconds: 12.5, audio_format: "flac", codec: "flac", sample_rate_hz: 16000, channels: 1, byte_size: 1024, storage_path: "/audio/audio-1.flac" }] }), { status: 200 }),
  new Response(JSON.stringify({ items: [{ id: "group-1", project_id: "project-1", canonical_audio_asset_id: "audio-1", created_at: "2026-08-04T00:00:00Z", updated_at: "2026-08-04T00:00:00Z", members: [{ duplicate_group_id: "group-1", audio_asset_id: "audio-1", created_at: "2026-08-04T00:00:00Z", updated_at: "2026-08-04T00:00:00Z" }], decisions: [{ id: "decision-1", audio_asset_low_id: "audio-1", audio_asset_high_id: "audio-2", candidate_fingerprint_low_id: "fp-1", candidate_fingerprint_high_id: "fp-2", algorithm: "chromaprint", algorithm_version: "1", duration_delta_seconds: 0, similarity_score: 1, decision: "duplicate", confidence: "high", duplicate_group_id: "group-1", created_at: "2026-08-04T00:00:00Z", updated_at: "2026-08-04T00:00:00Z" }] }] }), { status: 200 }),
  new Response(JSON.stringify({ stage_runs: [{ id: "run-1", project_id: "project-1", media_asset_id: "media-1", audio_asset_id: "audio-1", job_id: "job-1", stage_name: "audio_validation", status: "completed", queue_wait_ms: 10, wall_time_ms: 20, input_bytes: 100, output_bytes: 1024, input_duration_seconds: 12.5, throughput: 0.625, failure_reason: null, cpu_percent: null, memory_bytes: null }], deduplication: { throughput: 0.625, candidate_count: 1, match_rate: 1, duplicate_ratio: 0.5, unique_duration_seconds: 12.5, processing_failures: 0 } }), { status: 200 }),
];

vi.stubGlobal("fetch", vi.fn(async () => responses.shift() ?? new Response(JSON.stringify({ items: [] }), { status: 200 })));

describe("App", () => {
  it("renders the audio processing dashboard", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("Audio Processing Dashboard")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText("Research")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText("audio-1")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText("Duplicate Groups")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText(/Canonical Audio Asset: audio-1/)).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText(/Media Asset: media-1/)).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText(/Dedup metrics:/)).toBeInTheDocument());
    await waitFor(() => {
      const stageRun = screen.getAllByRole("listitem")[0];
      expect(stageRun.textContent).toContain("audio_validation");
      expect(stageRun.textContent).toContain("completed");
      expect(stageRun.textContent).toContain("wall 20ms throughput 0.625");
    });
    expect(screen.queryByText("Loading audio assets...")).not.toBeInTheDocument();
  });
});
