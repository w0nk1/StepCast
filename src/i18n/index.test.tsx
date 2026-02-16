import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  I18nProvider,
  isSupportedAppLanguage,
  readAppLanguage,
  resolveLocale,
  useI18n,
  writeAppLanguage,
} from "./index";

const originalDisplayNames = Intl.DisplayNames;

function Probe(props: { keyName: string; count?: string | number; name?: string }) {
  const { appLanguage, availableLocales, getLanguageLabel, locale, setAppLanguage, t } = useI18n();
  return (
    <div>
      <button onClick={() => setAppLanguage("de")}>de</button>
      <button onClick={() => setAppLanguage("system")}>system</button>
      <span data-testid="value">
        {t(props.keyName, { count: props.count, name: props.name })}
      </span>
      <span data-testid="app-language">{appLanguage}</span>
      <span data-testid="locale">{locale}</span>
      <span data-testid="locales">{availableLocales.join(",")}</span>
      <span data-testid="label-system">{getLanguageLabel("system")}</span>
      <span data-testid="label-de">{getLanguageLabel("de")}</span>
      <span data-testid="missing-key">{t("i18n.unknown.key")}</span>
    </div>
  );
}

function FallbackProbe() {
  const { getLanguageLabel, t } = useI18n();
  return (
    <div>
      <span data-testid="fallback-label">{getLanguageLabel("system")}</span>
      <span data-testid="fallback-text">{t("test.hello")}</span>
    </div>
  );
}

beforeEach(() => {
  localStorage.clear();
  Object.defineProperty(navigator, "languages", {
    configurable: true,
    value: ["en-US"],
  });
  Object.defineProperty(navigator, "language", {
    configurable: true,
    value: "en-US",
  });
  Object.defineProperty(Intl, "DisplayNames", {
    configurable: true,
    value: originalDisplayNames,
  });
});

describe("i18n", () => {
  it("resolves english text by default", () => {
    render(
      <I18nProvider>
        <Probe keyName="test.hello" />
      </I18nProvider>,
    );
    expect(screen.getByTestId("value").textContent).toBe("Hello");
    expect(screen.getByTestId("locales").textContent).toBe("en,de");
  });

  it("switches locale and falls back to english when key is missing in target locale", async () => {
    const user = userEvent.setup();
    render(
      <I18nProvider>
        <Probe keyName="test.only_en" />
      </I18nProvider>,
    );
    await user.click(screen.getByRole("button", { name: "de" }));
    expect(screen.getByTestId("value").textContent).toBe("Only English");
    expect(screen.getByTestId("app-language").textContent).toBe("de");
  });

  it("supports interpolation, pluralization, and missing key fallback", () => {
    render(
      <I18nProvider>
        <Probe keyName="test.count" count={2} name="Sam" />
      </I18nProvider>,
    );
    expect(screen.getByTestId("value").textContent).toBe("Sam has 2 steps");
    expect(screen.getByTestId("missing-key").textContent).toBe("i18n.unknown.key");
  });

  it("handles string and missing interpolation values", () => {
    const { rerender } = render(
      <I18nProvider>
        <Probe keyName="test.count" count={"2"} name="Sam" />
      </I18nProvider>,
    );
    expect(screen.getByTestId("value").textContent).toBe("Sam has 2 steps");

    rerender(
      <I18nProvider>
        <Probe keyName="test.count" />
      </I18nProvider>,
    );
    expect(screen.getByTestId("value").textContent).toContain("0 steps");
  });

  it("synchronizes language changes from storage events", () => {
    render(
      <I18nProvider>
        <Probe keyName="test.hello" />
      </I18nProvider>,
    );
    expect(screen.getByTestId("app-language").textContent).toBe("system");

    act(() => {
      localStorage.setItem("appLanguage", "de");
      window.dispatchEvent(new StorageEvent("storage", { key: "not_app_language" }));
    });
    expect(screen.getByTestId("app-language").textContent).toBe("system");

    act(() => {
      localStorage.setItem("appLanguage", "de");
      window.dispatchEvent(new StorageEvent("storage", { key: "appLanguage" }));
    });
    expect(screen.getByTestId("app-language").textContent).toBe("de");
  });

  it("falls back to code labels when Intl.DisplayNames is unavailable", () => {
    Object.defineProperty(Intl, "DisplayNames", {
      configurable: true,
      value: undefined,
    });
    render(
      <I18nProvider>
        <Probe keyName="test.hello" />
      </I18nProvider>,
    );
    expect(screen.getByTestId("label-de").textContent).toBe("de");
  });

  it("falls back to UI-locale language names when self-locale name is unavailable", () => {
    class DisplayNamesWithFallback {
      private locale: string;
      constructor(locales: string[]) {
        this.locale = locales[0] ?? "en";
      }
      of(code: string): string | undefined {
        if (this.locale === code) return undefined;
        return `lang-${code}`;
      }
    }
    Object.defineProperty(Intl, "DisplayNames", {
      configurable: true,
      value: DisplayNamesWithFallback,
    });

    render(
      <I18nProvider>
        <Probe keyName="test.hello" />
      </I18nProvider>,
    );
    expect(screen.getByTestId("label-de").textContent).toBe("lang-de");
  });

  it("uses fallback context when provider is missing", () => {
    render(<FallbackProbe />);
    expect(screen.getByTestId("fallback-label").textContent).toBe("System");
    expect(screen.getByTestId("fallback-text").textContent).toBe("Hello");
  });

  it("normalizes and validates app language persistence", () => {
    writeAppLanguage("DE" as never);
    expect(localStorage.getItem("appLanguage")).toBe("de");
    expect(readAppLanguage()).toBe("de");

    localStorage.setItem("appLanguage", "system");
    expect(readAppLanguage()).toBe("system");

    localStorage.setItem("appLanguage", "fr");
    expect(readAppLanguage()).toBe("system");

    writeAppLanguage("system");
    expect(localStorage.getItem("appLanguage")).toBe("system");
  });

  it("resolves locales from direct, base, and fallback system values", () => {
    Object.defineProperty(navigator, "languages", {
      configurable: true,
      value: ["de", "fr-FR"],
    });
    expect(resolveLocale("system")).toBe("de");

    Object.defineProperty(navigator, "languages", {
      configurable: true,
      value: ["fr-FR", "de-DE"],
    });
    expect(resolveLocale("system")).toBe("de");

    Object.defineProperty(navigator, "languages", {
      configurable: true,
      value: ["fr-FR"],
    });
    expect(resolveLocale("system")).toBe("en");

    Object.defineProperty(navigator, "languages", {
      configurable: true,
      value: [],
    });
    Object.defineProperty(navigator, "language", {
      configurable: true,
      value: "de-DE",
    });
    expect(resolveLocale("system")).toBe("de");

    expect(resolveLocale(" DE " as never)).toBe("de");
    expect(resolveLocale("fr" as never)).toBe("en");
  });

  it("validates supported app language values", () => {
    expect(isSupportedAppLanguage("system")).toBe(true);
    expect(isSupportedAppLanguage("de")).toBe(true);
    expect(isSupportedAppLanguage("en")).toBe(true);
    expect(isSupportedAppLanguage("fr")).toBe(false);
  });

  it("uses safe fallbacks when browser globals are unavailable", () => {
    const originalNavigator = globalThis.navigator;
    const originalStorage = globalThis.localStorage;

    vi.stubGlobal("navigator", undefined);
    vi.stubGlobal("localStorage", undefined);

    expect(resolveLocale("system")).toBe("en");
    expect(readAppLanguage()).toBe("system");
    expect(() => writeAppLanguage("de")).not.toThrow();

    vi.stubGlobal("navigator", originalNavigator);
    vi.stubGlobal("localStorage", originalStorage);
  });
});
