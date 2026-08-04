import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { App } from "./app";

const responses = [
  new Response(JSON.stringify({ status: "ok" }), { status: 200 }),
  new Response(JSON.stringify({ items: [{ id: "project-1", name: "Research", description: null }] }), { status: 200 }),
  new Response(JSON.stringify({ items: [{ id: "source-1", project_id: "project-1", source_type: "public_url", original_url: "https://example.com", ingestion_status: "staged", progress: "staged", failure_summary: null }] }), { status: 200 }),
  new Response(JSON.stringify({ items: [{ id: "job-1", project_id: "project-1", source_id: "source-1", job_type: "media_ingestion", status: "failed", attempts: 1, progress: "failed", failure_summary: "temporary failure" }] }), { status: 200 }),
  new Response(JSON.stringify({ error: { code: "database_error", message: "Retry failed" } }), { status: 500 }),
  new Response(JSON.stringify({ source_id: "source-2", job_id: "job-2" }), { status: 200 }),
];

vi.stubGlobal("fetch", vi.fn(async () => responses.shift() ?? new Response(JSON.stringify({ items: [] }), { status: 200 })));

describe("App", () => {
  it("renders the ingestion dashboard and retry action", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("Research")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText("public_url")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText("temporary failure")).toBeInTheDocument());

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(screen.getByText("Retry failed")).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText("https://example.com/audio.mp3"), {
      target: { value: "https://example.com/audio.mp3" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add URL" }));

    await waitFor(() => expect(screen.getByText("Submission accepted")).toBeInTheDocument());
  });
});
