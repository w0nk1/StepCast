import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import SettingsSheet, { initTheme } from "./SettingsSheet";

const mockCheck = vi.mocked(check);
const mockRelaunch = vi.mocked(relaunch);

function fakeUpdate(overrides: Partial<Update> = {}): Update {
  return {
    available: true,
    version: "1.2.0",
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

beforeEach(() => {
  mockCheck.mockReset();
  mockRelaunch.mockReset();
  mockCheck.mockResolvedValue(null);
});

describe("SettingsSheet", () => {
  it("renders settings header with back button", () => {
    render(<SettingsSheet onBack={vi.fn()} />);
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("defaults theme to system", () => {
    render(<SettingsSheet onBack={vi.fn()} />);
    expect(screen.getByText("System").className).toContain("active");
  });

  it("loads saved theme from localStorage", () => {
    localStorage.setItem("theme", "dark");
    render(<SettingsSheet onBack={vi.fn()} />);
    expect(screen.getByText("Dark").className).toContain("active");
  });

  it("sets data-theme and localStorage when selecting a theme", async () => {
    const user = userEvent.setup();
    render(<SettingsSheet onBack={vi.fn()} />);
    await user.click(screen.getByText("Dark"));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(localStorage.getItem("theme")).toBe("dark");
  });

  it("removes data-theme when selecting system", async () => {
    const user = userEvent.setup();
    document.documentElement.setAttribute("data-theme", "dark");
    localStorage.setItem("theme", "dark");
    render(<SettingsSheet onBack={vi.fn()} />);
    await user.click(screen.getByText("System"));
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
    expect(localStorage.getItem("theme")).toBe("system");
  });

  it("shows version number", () => {
    render(<SettingsSheet onBack={vi.fn()} />);
    expect(screen.getByText("Version 0.1.0")).toBeInTheDocument();
  });

  it("checks for updates and shows up-to-date", async () => {
    const user = userEvent.setup();
    mockCheck.mockResolvedValue(null);
    render(<SettingsSheet onBack={vi.fn()} />);
    await user.click(screen.getByText("Check for Updates"));
    expect(await screen.findByText("You're up to date")).toBeInTheDocument();
  });

  it("shows available update with install button", async () => {
    const user = userEvent.setup();
    mockCheck.mockResolvedValue(fakeUpdate());
    render(<SettingsSheet onBack={vi.fn()} />);
    await user.click(screen.getByText("Check for Updates"));
    expect(await screen.findByText("Install v1.2.0")).toBeInTheDocument();
  });

  it("installs update and relaunches", async () => {
    const user = userEvent.setup();
    const mockDownloadAndInstall = vi.fn().mockResolvedValue(undefined);
    mockCheck.mockResolvedValue(
      fakeUpdate({ downloadAndInstall: mockDownloadAndInstall }),
    );
    render(<SettingsSheet onBack={vi.fn()} />);

    // First check for updates
    await user.click(screen.getByText("Check for Updates"));
    const installBtn = await screen.findByText("Install v1.2.0");
    await user.click(installBtn);

    expect(mockDownloadAndInstall).toHaveBeenCalled();
    expect(mockRelaunch).toHaveBeenCalled();
  });

  it("shows error when update check fails", async () => {
    const user = userEvent.setup();
    mockCheck.mockRejectedValue(new Error("Network error"));
    render(<SettingsSheet onBack={vi.fn()} />);
    await user.click(screen.getByText("Check for Updates"));
    expect(
      await screen.findByText("Could not check for updates"),
    ).toBeInTheDocument();
  });

  it("calls onBack when Escape is pressed", () => {
    const onBack = vi.fn();
    render(<SettingsSheet onBack={onBack} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onBack).toHaveBeenCalled();
  });

  it("renders GitHub and bug report links", () => {
    render(<SettingsSheet onBack={vi.fn()} />);
    expect(screen.getByText("GitHub")).toBeInTheDocument();
    expect(screen.getByText("Report a Bug")).toBeInTheDocument();
  });
});

describe("initTheme", () => {
  it("sets data-theme from localStorage", () => {
    localStorage.setItem("theme", "light");
    initTheme();
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
  });

  it("does nothing for system theme", () => {
    localStorage.setItem("theme", "system");
    initTheme();
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });

  it("does nothing when no theme saved", () => {
    initTheme();
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
  });
});
