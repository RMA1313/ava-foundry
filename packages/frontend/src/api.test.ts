import { describe, expect, it, vi } from "vitest";
import { fetchLiveness } from "./api";

describe("fetchLiveness", () => {
  it("returns true when backend is healthy", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ status: "ok" }), { status: 200 })));
    await expect(fetchLiveness("http://localhost:8080")).resolves.toBe(true);
  });
});

