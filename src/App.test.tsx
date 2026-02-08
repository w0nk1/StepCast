import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

describe("App", () => {
  it("renders the RecorderPanel with StepCast header", async () => {
    vi.mocked(invoke).mockResolvedValue({
      screen_recording: true,
      accessibility: true,
    });

    render(<App />);
    expect(screen.getByText("StepCast")).toBeInTheDocument();
  });
});
