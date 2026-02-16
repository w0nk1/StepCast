import {
  createContext,
  type PropsWithChildren,
  useEffect,
  useContext,
  useMemo,
  useState,
} from "react";
type Catalog = Record<string, string>;
type CatalogModule = { default: Catalog };

export type Locale = string;
export type AppLanguage = "system" | Locale;
type Vars = Record<string, string | number | undefined>;

const APP_LANGUAGE_KEY = "appLanguage";
const localeModules = import.meta.glob<CatalogModule>("./locales/*.json", {
  eager: true,
});

function localeCodeFromPath(path: string): string | null {
  const match = path.match(/\/([^/]+)\.json$/);
  if (!match) return null;
  return match[1].toLowerCase();
}

const catalogs = Object.fromEntries(
  Object.entries(localeModules)
    .map(([path, module]) => {
      const code = localeCodeFromPath(path);
      if (!code) return null;
      return [code, module.default] as const;
    })
    .filter((entry): entry is readonly [string, Catalog] => entry !== null),
);

const localeCodes = Object.keys(catalogs).sort((a, b) => {
  if (a === "en") return -1;
  if (b === "en") return 1;
  return a.localeCompare(b);
});
const localeCodeSet = new Set(localeCodes);
const DEFAULT_LOCALE: Locale = localeCodeSet.has("en")
  ? "en"
  : (localeCodes[0] ?? "en");

type I18nContextValue = {
  appLanguage: AppLanguage;
  locale: Locale;
  availableLocales: Locale[];
  setAppLanguage: (language: AppLanguage) => void;
  getLanguageLabel: (language: AppLanguage) => string;
  t: (key: string, vars?: Vars) => string;
};

const FALLBACK_CONTEXT: I18nContextValue = {
  appLanguage: "system",
  locale: DEFAULT_LOCALE,
  availableLocales: [DEFAULT_LOCALE],
  setAppLanguage: () => {},
  getLanguageLabel: (language: AppLanguage) =>
    language === "system" ? "System" : language,
  t: (key: string, vars?: Vars) => translate(DEFAULT_LOCALE, key, vars),
};

const I18nContext = createContext<I18nContextValue>(FALLBACK_CONTEXT);

function isLocale(value: string): value is Locale {
  return localeCodeSet.has(value.toLowerCase());
}

function normalizeLocaleCode(value: string): string {
  return value.trim().toLowerCase();
}

export function isSupportedAppLanguage(value: string): value is AppLanguage {
  return value === "system" || isLocale(value);
}

function resolveBestLocale(candidates: string[]): Locale {
  for (const candidate of candidates) {
    const normalized = normalizeLocaleCode(candidate);
    if (isLocale(normalized)) return normalized;
  }
  for (const candidate of candidates) {
    const normalized = normalizeLocaleCode(candidate);
    const base = normalized.split("-")[0];
    if (isLocale(base)) return base;
  }
  return DEFAULT_LOCALE;
}

function detectSystemLocale(): Locale {
  if (typeof navigator === "undefined") return DEFAULT_LOCALE;
  const langs = Array.isArray(navigator.languages) && navigator.languages.length > 0
    ? navigator.languages
    : [navigator.language];
  return resolveBestLocale(langs.filter(Boolean));
}

export function resolveLocale(language: AppLanguage): Locale {
  if (language === "system") return detectSystemLocale();
  const normalized = normalizeLocaleCode(language);
  return isLocale(normalized) ? normalized : DEFAULT_LOCALE;
}

export function readAppLanguage(): AppLanguage {
  const raw =
    typeof localStorage !== "undefined" ? localStorage.getItem(APP_LANGUAGE_KEY) : null;
  if (raw === "system") return "system";
  if (raw) {
    const normalized = normalizeLocaleCode(raw);
    if (isLocale(normalized)) return normalized;
  }
  return "system";
}

export function writeAppLanguage(language: AppLanguage) {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(
    APP_LANGUAGE_KEY,
    language === "system" ? language : normalizeLocaleCode(language),
  );
}

function displayLanguageName(code: string, uiLocale: Locale): string {
  if (typeof Intl !== "undefined" && typeof Intl.DisplayNames === "function") {
    try {
      const selfDisplay = new Intl.DisplayNames([code], { type: "language" });
      const self = selfDisplay.of(code);
      if (self) return self;
    } catch {
      // Ignore and try ui-locale fallback.
    }
    try {
      const uiDisplay = new Intl.DisplayNames([uiLocale], { type: "language" });
      const uiName = uiDisplay.of(code);
      if (uiName) return uiName;
    } catch {
      // Ignore and use fallback.
    }
  }
  return code;
}

function formatMessage(template: string, vars: Vars = {}): string {
  const pluralized = template.replace(
    /\{(\w+),\s*plural,\s*one\s*\{([^{}]*)\}\s*other\s*\{([^{}]*)\}\s*\}/g,
    (_match, varName: string, one: string, other: string) => {
      const countRaw = vars[varName];
      const count = typeof countRaw === "number" ? countRaw : Number(countRaw ?? 0);
      const selected = count === 1 ? one : other;
      return selected.replace(/#/g, String(count));
    },
  );
  return pluralized.replace(/\{(\w+)\}/g, (_match, varName: string) => {
    const value = vars[varName];
    return value == null ? "" : String(value);
  });
}

function translate(locale: Locale, key: string, vars?: Vars): string {
  const inLocale = catalogs[locale]?.[key];
  const inEnglish = catalogs.en?.[key];
  const inDefault = catalogs[DEFAULT_LOCALE]?.[key];
  const template = inLocale ?? inEnglish ?? inDefault ?? key;
  return formatMessage(template, vars);
}

export function I18nProvider({ children }: PropsWithChildren) {
  const [appLanguage, setAppLanguageState] = useState<AppLanguage>(() => readAppLanguage());
  const locale = resolveLocale(appLanguage);

  useEffect(() => {
    const onStorage = (event: StorageEvent) => {
      if (event.key !== APP_LANGUAGE_KEY) return;
      setAppLanguageState(readAppLanguage());
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);

  const value = useMemo<I18nContextValue>(
    () => ({
      appLanguage,
      locale,
      availableLocales: localeCodes,
      setAppLanguage: (language: AppLanguage) => {
        writeAppLanguage(language);
        setAppLanguageState(language);
      },
      getLanguageLabel: (language: AppLanguage) => {
        if (language === "system") {
          const systemLocale = detectSystemLocale();
          return `${translate(locale, "settings.language.system")} (${displayLanguageName(systemLocale, locale)})`;
        }
        return displayLanguageName(language, locale);
      },
      t: (key: string, vars?: Vars) => translate(locale, key, vars),
    }),
    [appLanguage, locale],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  return useContext(I18nContext);
}
