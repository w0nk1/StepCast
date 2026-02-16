import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import SettingsSheet, { initTheme } from "./SettingsSheet";
import { I18nProvider } from "../i18n";

const mockInvoke = vi.mocked(invoke);
const mockEmit = vi.mocked(emit);
const mockOpenUrl = vi.mocked(openUrl);
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

function renderSettings(onBack = vi.fn()) {
  return render(
    <I18nProvider>
      <SettingsSheet onBack={onBack} />
    </I18nProvider>,
  );
}

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  mockInvoke.mockReset();
  mockEmit.mockReset();
  mockOpenUrl.mockReset();
  mockCheck.mockReset();
  mockRelaunch.mockReset();
  mockInvoke.mockResolvedValue({ eligible: true, reason: "ok" });
  mockEmit.mockResolvedValue();
  mockOpenUrl.mockResolvedValue();
  mockCheck.mockResolvedValue(null);
});

describe("SettingsSheet", () => {
  it("renders settings header with back button", () => {
    renderSettings();
    expect(screen.getByText("Settings")).toBeInTheDocument();
  });

  it("defaults theme to system", () => {
    renderSettings();
    expect(screen.getByRole("button", { name: "System" }).className).toContain("active");
  });

  it("loads saved theme from localStorage", () => {
    localStorage.setItem("theme", "dark");
    renderSettings();
    expect(screen.getByText("Dark").className).toContain("active");
  });

  it("changes app language and emits sync event", async () => {
    const user = userEvent.setup();
    renderSettings();
    await user.selectOptions(screen.getByRole("combobox", { name: "Language" }), "de");
    expect(localStorage.getItem("appLanguage")).toBe("de");
    expect(mockEmit).toHaveBeenCalledWith("language-changed", { language: "de" });
  });

  it("sets data-theme and localStorage when selecting a theme", async () => {
    const user = userEvent.setup();
    renderSettings();
    await user.click(screen.getByText("Dark"));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
    expect(localStorage.getItem("theme")).toBe("dark");
  });

  it("removes data-theme when selecting system", async () => {
    const user = userEvent.setup();
    document.documentElement.setAttribute("data-theme", "dark");
    localStorage.setItem("theme", "dark");
    renderSettings();
    await user.click(screen.getByRole("button", { name: "System" }));
    expect(document.documentElement.hasAttribute("data-theme")).toBe(false);
    expect(localStorage.getItem("theme")).toBe("system");
  });

  it("shows version number from Tauri API", async () => {
    renderSettings();
    expect(await screen.findByText("Version 0.2.0")).toBeInTheDocument();
  });

  it("checks for updates and shows up-to-date", async () => {
    const user = userEvent.setup();
    mockCheck.mockResolvedValue(null);
    renderSettings();
    await user.click(screen.getByText("Check for Updates"));
    expect(await screen.findByText("You're up to date")).toBeInTheDocument();
  });

  it("shows available update with install button", async () => {
    const user = userEvent.setup();
    mockCheck.mockResolvedValue(fakeUpdate());
    renderSettings();
    await user.click(screen.getByText("Check for Updates"));
    expect(await screen.findByText("Install v1.2.0")).toBeInTheDocument();
  });

  it("installs update and relaunches", async () => {
    const user = userEvent.setup();
    const mockDownloadAndInstall = vi.fn().mockResolvedValue(undefined);
    mockCheck.mockResolvedValue(
      fakeUpdate({ downloadAndInstall: mockDownloadAndInstall }),
    );
    renderSettings();

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
    renderSettings();
    await user.click(screen.getByText("Check for Updates"));
    expect(
      await screen.findByText("Could not check for updates"),
    ).toBeInTheDocument();
  });

  it("calls onBack when Escape is pressed", () => {
    const onBack = vi.fn();
    renderSettings(onBack);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onBack).toHaveBeenCalled();
  });

  it("renders GitHub and bug report links", () => {
    renderSettings();
    expect(screen.getByText("GitHub")).toBeInTheDocument();
    expect(screen.getByText("Report a Bug")).toBeInTheDocument();
  });

  it("renders Apple-style switch control for Apple Intelligence toggle", () => {
    renderSettings();
    const toggle = screen.getByLabelText("Apple Intelligence step descriptions");
    expect(toggle).toHaveAttribute("role", "switch");
  });

  it("toggles Apple Intelligence via row click and emits sync event", async () => {
    const user = userEvent.setup();
    renderSettings();
    const row = screen.getByLabelText("Toggle Apple Intelligence step descriptions");
    await user.click(row);
    expect(localStorage.getItem("appleIntelligenceDescriptions")).toBe("true");
    expect(mockEmit).toHaveBeenCalledWith("ai-toggle-changed", { enabled: true });
  });

  it("toggles Apple Intelligence via keyboard (Enter and Space)", () => {
    renderSettings();
    const row = screen.getByLabelText("Toggle Apple Intelligence step descriptions");
    fireEvent.keyDown(row, { key: "Enter" });
    expect(localStorage.getItem("appleIntelligenceDescriptions")).toBe("true");
    fireEvent.keyDown(row, { key: " " });
    expect(localStorage.getItem("appleIntelligenceDescriptions")).toBe("false");
  });

  it("shows eligibility fallback when response shape is invalid", async () => {
    mockInvoke.mockResolvedValue({ foo: "bar" });
    renderSettings();
    expect(await screen.findByText(/Could not check system eligibility\./)).toBeInTheDocument();
  });

  it("shows eligibility fallback when eligibility call fails", async () => {
    mockInvoke.mockRejectedValue(new Error("boom"));
    renderSettings();
    expect(await screen.findByText(/Could not check system eligibility\./)).toBeInTheDocument();
  });

  it("opens Apple Intelligence settings with fallback URL on failure", async () => {
    const user = userEvent.setup();
    mockOpenUrl
      .mockRejectedValueOnce(new Error("primary fail"))
      .mockResolvedValueOnce();
    renderSettings();
    await user.click(screen.getByText("Open Apple Intelligence & Siri Settings"));
    expect(mockOpenUrl).toHaveBeenNthCalledWith(
      1,
      "x-apple.systempreferences:com.apple.Siri-Settings.extension",
    );
    expect(mockOpenUrl).toHaveBeenNthCalledWith(
      2,
      "x-apple.systempreferences:com.apple.preference.siri",
    );
  });

  it("swallows errors when both settings URLs fail", async () => {
    const user = userEvent.setup();
    mockOpenUrl.mockRejectedValue(new Error("all fail"));
    renderSettings();
    await user.click(screen.getByText("Open Apple Intelligence & Siri Settings"));
    expect(mockOpenUrl).toHaveBeenCalledTimes(2);
  });

  it("opens About links", async () => {
    const user = userEvent.setup();
    renderSettings();
    await user.click(screen.getByText("GitHub"));
    await user.click(screen.getByText("Report a Bug"));
    expect(mockOpenUrl).toHaveBeenCalledWith("https://github.com/w0nk1/StepCast");
    expect(mockOpenUrl).toHaveBeenCalledWith("https://github.com/w0nk1/StepCast/issues");
  });

  it("shows update error when install flow fails", async () => {
    const user = userEvent.setup();
    mockCheck
      .mockResolvedValueOnce(fakeUpdate())
      .mockRejectedValueOnce(new Error("install check fail"));
    renderSettings();
    await user.click(screen.getByText("Check for Updates"));
    await user.click(await screen.findByText("Install v1.2.0"));
    expect(await screen.findByText("Could not check for updates")).toBeInTheDocument();
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
