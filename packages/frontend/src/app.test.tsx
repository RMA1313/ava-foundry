import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { App } from "./app";

vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify({ status: "ok" }), { status: 200 })));

describe("App", () => {
  it("renders the home page", async () => {
    render(<App />);
    expect(screen.getByText("Data Studio")).toBeInTheDocument();
  });
});

