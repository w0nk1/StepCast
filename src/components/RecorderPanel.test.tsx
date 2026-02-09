import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save, ask } from "@tauri-apps/plugin-dialog";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import RecorderPanel from "./RecorderPanel";
import type { Step } from "../types/step";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);
const mockSave = vi.mocked(save);
const mockAsk = vi.mocked(ask);
const mockCheck = vi.mocked(check);
const mockRelaunch = vi.mocked(relaunch);

function fakeUpdate(overrides: Partial<Update> = {}): Update {
  return {
    available: true,
    version: "2.0.0",
    currentVersion: "0.1.0",
    downloadAndInstall: vi.fn(),
    download: vi.fn(),
    install: vi.fn(),
    close: vi.fn(),
    rid: 0,
    rawJson: "",
    ...overrides,
  } as unknown as Update;
}

function makeStep(overrides: Partial<Step> = {}): Step {
  return {
    id: `step-${Date.now()}-${Math.random()}`,
    ts: Date.now(),
    action: "Click",
    x: 100,
    y: 200,
    click_x_percent: 50,
    click_y_percent: 50,
    app: "Finder",
    window_title: "Documents",
    screenshot_path: null,
    note: null,
    ...overrides,
  };
}

// Capture listen callbacks so we can simulate events
let stepCapturedCallback: ((event: { payload: Step }) => void) | null = null;
let panelPositionedCallback: ((event: { payload: boolean }) => void) | null = null;

beforeEach(() => {
  stepCapturedCallback = null;
  panelPositionedCallback = null;
  mockInvoke.mockReset();
  mockListen.mockReset();
  mockSave.mockReset();
  mockAsk.mockReset();
  mockCheck.mockReset();
  mockRelaunch.mockReset();

  // Default: permissions granted
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === "check_permissions") {
      return { screen_recording: true, accessibility: true };
    }
    return undefined;
  });

  mockCheck.mockResolvedValue(null);

  // Capture listen callbacks
  mockListen.mockImplementation(async (event, handler) => {
    if (event === "step-captured") {
      stepCapturedCallback = handler as (event: { payload: Step }) => void;
    } else if (event === "panel-positioned") {
      panelPositionedCallback = handler as (event: { payload: boolean }) => void;
    }
    return vi.fn() as unknown as () => void; // unlisten
  });
});

function emitStep(step: Step) {
  act(() => {
    stepCapturedCallback?.({ payload: step });
  });
}

describe("RecorderPanel", () => {
  describe("initial render", () => {
    it("shows StepCast header and Ready status", async () => {
      render(<RecorderPanel />);
      expect(screen.getByText("StepCast")).toBeInTheDocument();
      expect(await screen.findByText("Ready")).toBeInTheDocument();
    });

    it("shows Start Recording button when permissions granted", async () => {
      render(<RecorderPanel />);
      expect(
        await screen.findByText("Start Recording"),
      ).toBeInTheDocument();
    });

    it("shows settings button", async () => {
      render(<RecorderPanel />);
      expect(screen.getByTitle("Settings")).toBeInTheDocument();
    });
  });

  describe("permissions", () => {
    it("shows permission UI when screen recording is missing", async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "check_permissions") {
          return { screen_recording: false, accessibility: true };
        }
        return undefined;
      });

      render(<RecorderPanel />);
      expect(
        await screen.findByText("Missing: Screen Recording"),
      ).toBeInTheDocument();
    });

    it("shows permission UI when accessibility is missing", async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "check_permissions") {
          return { screen_recording: true, accessibility: false };
        }
        return undefined;
      });

      render(<RecorderPanel />);
      expect(
        await screen.findByText("Missing: Accessibility"),
      ).toBeInTheDocument();
    });

    it("shows both missing permissions", async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "check_permissions") {
          return { screen_recording: false, accessibility: false };
        }
        return undefined;
      });

      render(<RecorderPanel />);
      expect(
        await screen.findByText("Missing: Screen Recording, Accessibility"),
      ).toBeInTheDocument();
    });

    it("disables Start Recording when permissions missing", async () => {
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "check_permissions") {
          return { screen_recording: false, accessibility: false };
        }
        return undefined;
      });

      render(<RecorderPanel />);
      const btn = await screen.findByText("Start Recording");
      expect(btn.closest("button")).toBeDisabled();
    });

    it("calls request_screen_recording when Open Settings clicked", async () => {
      const user = userEvent.setup();
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "check_permissions") {
          return { screen_recording: false, accessibility: true };
        }
        if (cmd === "request_screen_recording") {
          return { screen_recording: true, accessibility: true };
        }
        return undefined;
      });

      render(<RecorderPanel />);
      const buttons = await screen.findAllByText("Open Settings");
      await user.click(buttons[0]);
      expect(mockInvoke).toHaveBeenCalledWith("request_screen_recording");
    });

    it("calls request_accessibility when Open Settings clicked", async () => {
      const user = userEvent.setup();
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "check_permissions") {
          return { screen_recording: true, accessibility: false };
        }
        if (cmd === "request_accessibility") {
          return { screen_recording: true, accessibility: true };
        }
        return undefined;
      });

      render(<RecorderPanel />);
      const buttons = await screen.findAllByText("Open Settings");
      await user.click(buttons[0]);
      expect(mockInvoke).toHaveBeenCalledWith("request_accessibility");
    });
  });

  describe("recording state transitions", () => {
    it("start → recording state", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));
      expect(mockInvoke).toHaveBeenCalledWith("start_recording");
      expect(screen.getByText("Recording")).toBeInTheDocument();
    });

    it("recording → pause", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));
      await user.click(screen.getByText("Pause"));
      expect(mockInvoke).toHaveBeenCalledWith("pause_recording");
      expect(screen.getByText("Paused")).toBeInTheDocument();
    });

    it("paused → resume", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));
      await user.click(screen.getByText("Pause"));
      await user.click(screen.getByText("Resume"));
      expect(mockInvoke).toHaveBeenCalledWith("resume_recording");
      expect(screen.getByText("Recording")).toBeInTheDocument();
    });

    it("recording → stop shows stopped state with steps", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));
      emitStep(makeStep({ id: "s1" }));
      await user.click(screen.getByText("Stop"));
      expect(mockInvoke).toHaveBeenCalledWith("stop_recording");
      expect(screen.getByText("Stopped")).toBeInTheDocument();
      expect(screen.getByText("Export")).toBeInTheDocument();
    });

    it("shows error when recording command fails with missing permissions", async () => {
      const user = userEvent.setup();
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "check_permissions") {
          return { screen_recording: true, accessibility: true };
        }
        if (cmd === "start_recording") {
          throw new Error("missing screen recording permission");
        }
        return undefined;
      });

      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));
      expect(
        screen.getByText(
          "Grant Screen Recording and Accessibility permissions to record.",
        ),
      ).toBeInTheDocument();
    });

    it("shows raw error for other failures", async () => {
      const user = userEvent.setup();
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === "check_permissions") {
          return { screen_recording: true, accessibility: true };
        }
        if (cmd === "start_recording") {
          throw new Error("Something went wrong");
        }
        return undefined;
      });

      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));
      expect(
        screen.getByText("Error: Something went wrong"),
      ).toBeInTheDocument();
    });
  });

  describe("step-captured event", () => {
    it("adds steps from listen events", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));

      emitStep(makeStep({ id: "s1", app: "Finder" }));
      expect(screen.getByText("Clicked in Finder")).toBeInTheDocument();
      expect(screen.getByText("1 captured")).toBeInTheDocument();
    });

    it("ignores duplicate step IDs", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));

      const step = makeStep({ id: "s1", app: "Finder" });
      emitStep(step);
      emitStep(step); // duplicate
      expect(screen.getByText("1 captured")).toBeInTheDocument();
    });

    it("accumulates multiple steps", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));

      emitStep(makeStep({ id: "s1" }));
      emitStep(makeStep({ id: "s2" }));
      emitStep(makeStep({ id: "s3" }));
      expect(screen.getByText("3 captured")).toBeInTheDocument();
    });
  });

  describe("delete step", () => {
    it("deletes a step and resets to idle when last step removed", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);
      await user.click(await screen.findByText("Start Recording"));

      emitStep(makeStep({ id: "s1" }));
      await user.click(screen.getByText("Stop"));

      // Delete the step (two clicks: confirm flow)
      const deleteBtn = document.querySelector(".step-delete")!;
      await user.click(deleteBtn);
      await user.click(deleteBtn);

      // Should be back to idle
      expect(screen.getByText("Ready")).toBeInTheDocument();
      expect(screen.getByText("Start Recording")).toBeInTheDocument();
    });
  });

  describe("export flow", () => {
    it("opens export sheet and exports successfully", async () => {
      const user = userEvent.setup();
      mockSave.mockResolvedValue("/tmp/guide.pdf");
      render(<RecorderPanel />);

      // Record and stop
      await user.click(await screen.findByText("Start Recording"));
      emitStep(makeStep({ id: "s1" }));
      await user.click(screen.getByText("Stop"));

      // Open export sheet
      await user.click(screen.getByText("Export"));
      expect(screen.getByText("Export Guide")).toBeInTheDocument();

      // Click export button inside the sheet (not the action bar one)
      const sheet = document.querySelector(".export-sheet")!;
      const sheetExportBtn = sheet.querySelector(".button.primary") as HTMLElement;
      await user.click(sheetExportBtn);
      expect(mockSave).toHaveBeenCalled();
      expect(mockInvoke).toHaveBeenCalledWith("export_guide", {
        title: "New StepCast Guide",
        format: "pdf",
        outputPath: "/tmp/guide.pdf",
      });
    });

    it("cancels export when save dialog returns null", async () => {
      const user = userEvent.setup();
      mockSave.mockResolvedValue(null);
      render(<RecorderPanel />);

      await user.click(await screen.findByText("Start Recording"));
      emitStep(makeStep({ id: "s1" }));
      await user.click(screen.getByText("Stop"));
      await user.click(screen.getByText("Export"));
      const sheet = document.querySelector(".export-sheet")!;
      const sheetExportBtn = sheet.querySelector(".button.primary") as HTMLElement;
      await user.click(sheetExportBtn);

      // export_guide should NOT have been called
      expect(mockInvoke).not.toHaveBeenCalledWith(
        "export_guide",
        expect.anything(),
      );
    });

    it("closes export sheet via cancel button", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);

      await user.click(await screen.findByText("Start Recording"));
      emitStep(makeStep({ id: "s1" }));
      await user.click(screen.getByText("Stop"));
      await user.click(screen.getByText("Export"));
      await user.click(screen.getByText("Cancel"));

      expect(screen.queryByText("Export Guide")).not.toBeInTheDocument();
    });
  });

  describe("discard flow", () => {
    it("discards recording when confirmed", async () => {
      const user = userEvent.setup();
      mockAsk.mockResolvedValue(true);
      render(<RecorderPanel />);

      await user.click(await screen.findByText("Start Recording"));
      emitStep(makeStep({ id: "s1" }));

      await user.click(screen.getByTitle("Discard recording"));
      expect(mockAsk).toHaveBeenCalled();
      expect(mockInvoke).toHaveBeenCalledWith("discard_recording");
      expect(screen.getByText("Ready")).toBeInTheDocument();
    });

    it("does not discard when cancelled", async () => {
      const user = userEvent.setup();
      mockAsk.mockResolvedValue(false);
      render(<RecorderPanel />);

      await user.click(await screen.findByText("Start Recording"));
      emitStep(makeStep({ id: "s1" }));

      await user.click(screen.getByTitle("Discard recording"));
      expect(screen.getByText("Recording")).toBeInTheDocument();
      expect(screen.getByText("1 captured")).toBeInTheDocument();
    });
  });

  describe("new recording", () => {
    it("confirms before starting new recording with existing steps", async () => {
      const user = userEvent.setup();
      mockAsk.mockResolvedValue(true);
      render(<RecorderPanel />);

      await user.click(await screen.findByText("Start Recording"));
      emitStep(makeStep({ id: "s1" }));
      await user.click(screen.getByText("Stop"));

      await user.click(screen.getByText("New Recording"));
      expect(mockAsk).toHaveBeenCalled();
      expect(mockInvoke).toHaveBeenCalledWith("start_recording");
    });

    it("cancels new recording when user declines", async () => {
      const user = userEvent.setup();
      mockAsk.mockResolvedValue(false);
      render(<RecorderPanel />);

      await user.click(await screen.findByText("Start Recording"));
      emitStep(makeStep({ id: "s1" }));
      await user.click(screen.getByText("Stop"));

      await user.click(screen.getByText("New Recording"));
      expect(screen.getByText("Stopped")).toBeInTheDocument();
    });
  });

  describe("update banner", () => {
    it("shows update banner when update is available", async () => {
      mockCheck.mockResolvedValue(fakeUpdate());

      render(<RecorderPanel />);
      expect(await screen.findByText("v2.0.0 available")).toBeInTheDocument();
      expect(screen.getByText("Install")).toBeInTheDocument();
    });

    it("installs update when Install clicked", async () => {
      const user = userEvent.setup();
      const mockDownloadAndInstall = vi.fn().mockResolvedValue(undefined);
      mockCheck.mockResolvedValue(
        fakeUpdate({ downloadAndInstall: mockDownloadAndInstall }),
      );

      render(<RecorderPanel />);
      const installBtn = await screen.findByText("Install");
      await user.click(installBtn);

      expect(mockDownloadAndInstall).toHaveBeenCalled();
      expect(mockRelaunch).toHaveBeenCalled();
    });

    it("does not show update banner when no update", async () => {
      mockCheck.mockResolvedValue(null);
      render(<RecorderPanel />);
      // Wait for permissions to resolve
      await screen.findByText("Start Recording");
      expect(screen.queryByText(/available/)).not.toBeInTheDocument();
    });
  });

  describe("fallback positioning", () => {
    it("hides notch and shows hint when panel-positioned is false", async () => {
      render(<RecorderPanel />);
      await screen.findByText("Start Recording");

      act(() => {
        panelPositionedCallback?.({ payload: false });
      });

      const panel = document.querySelector(".panel")!;
      expect(panel.classList.contains("no-notch")).toBe(true);
      expect(screen.getByText(/Tray icon hidden/)).toBeInTheDocument();
    });

    it("dismisses fallback hint when close button clicked", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);
      await screen.findByText("Start Recording");

      act(() => {
        panelPositionedCallback?.({ payload: false });
      });

      await user.click(screen.getByTitle("Dismiss"));
      expect(screen.queryByText(/Tray icon hidden/)).not.toBeInTheDocument();

      const panel = document.querySelector(".panel")!;
      expect(panel.classList.contains("no-notch")).toBe(false);
    });

    it("restores notch when panel-positioned is true", async () => {
      render(<RecorderPanel />);
      await screen.findByText("Start Recording");

      act(() => {
        panelPositionedCallback?.({ payload: false });
      });
      expect(document.querySelector(".panel.no-notch")).toBeTruthy();

      act(() => {
        panelPositionedCallback?.({ payload: true });
      });
      expect(document.querySelector(".panel.no-notch")).toBeNull();
      expect(screen.queryByText(/Tray icon hidden/)).not.toBeInTheDocument();
    });
  });

  describe("settings view", () => {
    it("navigates to settings and back", async () => {
      const user = userEvent.setup();
      render(<RecorderPanel />);

      await user.click(screen.getByTitle("Settings"));
      expect(screen.getByText("Settings")).toBeInTheDocument();
      expect(screen.getByText("Appearance")).toBeInTheDocument();

      // Go back
      await user.click(screen.getByTitle("Back"));
      expect(await screen.findByText("StepCast")).toBeInTheDocument();
    });
  });
});
