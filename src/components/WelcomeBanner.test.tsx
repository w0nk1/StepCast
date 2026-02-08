import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import WelcomeBanner from "./WelcomeBanner";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

describe("WelcomeBanner", () => {
  let onDismiss: () => void;

  beforeEach(() => {
    onDismiss = vi.fn();
  });

  it("renders welcome title and tips", () => {
    render(<WelcomeBanner onDismiss={onDismiss} />);
    expect(screen.getByText("Welcome to StepCast")).toBeTruthy();
    expect(screen.getByText(/menu bar icon/)).toBeTruthy();
    expect(screen.getByText(/Right-click/)).toBeTruthy();
  });

  it("calls onDismiss and mark_startup_seen when got it is clicked", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    render(<WelcomeBanner onDismiss={onDismiss} />);
    fireEvent.click(screen.getByText("Got it"));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("mark_startup_seen");
      expect(onDismiss).toHaveBeenCalled();
    });
  });

  it("still dismisses if invoke fails", async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    vi.mocked(invoke).mockRejectedValueOnce(new Error("save failed"));
    render(<WelcomeBanner onDismiss={onDismiss} />);
    fireEvent.click(screen.getByText("Got it"));
    await waitFor(() => {
      expect(onDismiss).toHaveBeenCalled();
    });
  });
});
