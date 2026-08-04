import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { App } from "./app";

vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
  const url = String(input);
  if (url.endsWith("/health/live")) return new Response(JSON.stringify({ status: "ok" }), { status: 200 });
  if (url.endsWith("/projects")) return new Response(JSON.stringify({ items: [{ id: "project-1", name: "پژوهش", description: null }] }), { status: 200 });
  if (url.includes("/sources")) return new Response(JSON.stringify({ items: [{ id: "source-1", project_id: "project-1", source_type: "local_file", original_url: "/tmp/a.wav", ingestion_status: "pending", progress: "0%", failure_summary: null }] }), { status: 200 });
  if (url.includes("/datasets/import")) return new Response(JSON.stringify({ imported: 1, duplicate: 0, invalid: 0, failed: 0, row_failures: [] }), { status: 200 });
  return new Response(JSON.stringify({}), { status: 200 });
}));

describe("App", () => {
  it("renders workspace navigation and source list", async () => {
    render(<App />);
    await waitFor(() => expect(screen.getByRole("heading", { name: "فضای کاری پروژه" })).toBeInTheDocument());
    expect(screen.getAllByText("منابع").length).toBeGreaterThan(0);
  });

  it("shows dataset import form", async () => {
    window.history.pushState({}, "", "/projects/project-1/import");
    render(<App />);
    await waitFor(() => expect(screen.getByRole("heading", { name: "درون‌ریزی دیتاست" })).toBeInTheDocument());
  });
});
