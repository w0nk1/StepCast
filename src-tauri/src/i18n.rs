use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    De,
}

impl Locale {
    pub fn as_html_lang(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::De => "de",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLanguage {
    System,
    En,
    De,
}

pub fn parse_app_language(value: Option<&str>) -> AppLanguage {
    match value.map(|v| v.trim().to_lowercase()) {
        Some(v) if v == "en" => AppLanguage::En,
        Some(v) if v == "de" => AppLanguage::De,
        Some(v) if v == "system" => AppLanguage::System,
        _ => AppLanguage::System,
    }
}

pub fn resolve_locale(language: AppLanguage) -> Locale {
    match language {
        AppLanguage::System => system_locale(),
        AppLanguage::En => Locale::En,
        AppLanguage::De => Locale::De,
    }
}

pub fn system_locale() -> Locale {
    static DETECTED: OnceLock<Locale> = OnceLock::new();
    *DETECTED.get_or_init(detect_system_locale)
}

fn detect_system_locale() -> Locale {
    #[cfg(target_os = "macos")]
    if let Some(raw) = macos_apple_locale() {
        if let Some(locale) = parse_env_locale(&raw) {
            return locale;
        }
    }

    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(raw) = std::env::var(key) {
            if let Some(locale) = parse_env_locale(&raw) {
                return locale;
            }
        }
    }
    Locale::En
}

fn parse_env_locale(raw: &str) -> Option<Locale> {
    let normalized = raw.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let short = normalized
        .split(['.', '@'])
        .next()
        .unwrap_or(&normalized)
        .trim();
    if short.starts_with("de") {
        return Some(Locale::De);
    }
    if short.starts_with("en") {
        return Some(Locale::En);
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_apple_locale() -> Option<String> {
    let out = std::process::Command::new("defaults")
        .args(["read", "-g", "AppleLocale"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let value = String::from_utf8(out.stdout).ok()?;
    let value = value.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub fn export_step_count(locale: Locale, count: usize) -> String {
    let unit = match (locale, count == 1) {
        (Locale::En, true) => "step",
        (Locale::En, false) => "steps",
        (Locale::De, true) => "Schritt",
        (Locale::De, false) => "Schritte",
    };
    format!("{count} {unit}")
}

pub fn export_step_heading(locale: Locale, num: usize) -> String {
    match locale {
        Locale::En => format!("Step {num}"),
        Locale::De => format!("Schritt {num}"),
    }
}

pub fn export_step_image_alt(locale: Locale, num: usize) -> String {
    export_step_heading(locale, num)
}

pub fn step_action_note(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Note",
        Locale::De => "Notiz",
    }
}

pub fn step_action_clicked_in(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Clicked in",
        Locale::De => "Geklickt in",
    }
}

pub fn step_action_double_clicked_in(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Double-clicked in",
        Locale::De => "Doppelt geklickt in",
    }
}

pub fn step_action_right_clicked_in(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Right-clicked in",
        Locale::De => "Rechts geklickt in",
    }
}

pub fn step_action_shortcut_in(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Used keyboard shortcut in",
        Locale::De => "Tastenkürzel verwendet in",
    }
}

pub fn auth_placeholder_description(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Authenticate with Touch ID or enter your password to continue.",
        Locale::De => {
            "Authentifiziere dich mit Touch ID oder gib dein Passwort ein, um fortzufahren."
        }
    }
}

pub fn ai_eligibility_requires_apple_silicon(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Requires Apple Silicon (M1+).",
        Locale::De => "Erfordert Apple Silicon (M1+).",
    }
}

pub fn ai_eligibility_unknown_macos_version(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Could not detect macOS version.",
        Locale::De => "macOS-Version konnte nicht erkannt werden.",
    }
}

pub fn ai_eligibility_requires_macos_26(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Requires macOS 26+.",
        Locale::De => "Erfordert macOS 26+.",
    }
}

pub fn ai_eligibility_check_failed(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Could not check Apple Intelligence availability.",
        Locale::De => "Apple-Intelligence-Verfugbarkeit konnte nicht gepruft werden.",
    }
}

pub fn ai_eligibility_available(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Available.",
        Locale::De => "Verfugbar.",
    }
}

pub fn ai_eligibility_not_enabled(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Apple Intelligence is disabled in System Settings.",
        Locale::De => "Apple Intelligence ist in den Systemeinstellungen deaktiviert.",
    }
}

pub fn ai_eligibility_device_not_eligible(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "This device is not eligible for Apple Intelligence.",
        Locale::De => "Dieses Gerat ist nicht fur Apple Intelligence geeignet.",
    }
}

pub fn ai_eligibility_model_not_ready(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Apple Intelligence model is not ready yet (downloading/initializing).",
        Locale::De => "Apple-Intelligence-Modell ist noch nicht bereit (Download/Initialisierung).",
    }
}

pub fn ai_eligibility_unavailable(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Apple Intelligence unavailable.",
        Locale::De => "Apple Intelligence ist nicht verfugbar.",
    }
}

pub fn tray_tooltip(_locale: Locale) -> &'static str {
    "StepCast"
}

pub fn tray_recording_tooltip(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "StepCast - Recording...",
        Locale::De => "StepCast - Aufnahme läuft...",
    }
}

pub fn tray_menu_open(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Open StepCast",
        Locale::De => "StepCast öffnen",
    }
}

pub fn tray_menu_quick_start(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Quick Start",
        Locale::De => "Schnellstart",
    }
}

pub fn tray_menu_quit(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Quit StepCast",
        Locale::De => "StepCast beenden",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_app_language_handles_known_values() {
        assert_eq!(parse_app_language(Some("system")), AppLanguage::System);
        assert_eq!(parse_app_language(Some("en")), AppLanguage::En);
        assert_eq!(parse_app_language(Some("de")), AppLanguage::De);
        assert_eq!(parse_app_language(Some(" DE ")), AppLanguage::De);
    }

    #[test]
    fn parse_app_language_falls_back_to_system() {
        assert_eq!(parse_app_language(None), AppLanguage::System);
        assert_eq!(parse_app_language(Some("fr")), AppLanguage::System);
        assert_eq!(parse_app_language(Some("")), AppLanguage::System);
    }

    #[test]
    fn parse_env_locale_parses_de_and_en() {
        assert_eq!(parse_env_locale("de_DE.UTF-8"), Some(Locale::De));
        assert_eq!(parse_env_locale("en_US.UTF-8"), Some(Locale::En));
        assert_eq!(parse_env_locale("de@euro"), Some(Locale::De));
        assert_eq!(parse_env_locale("fr_FR.UTF-8"), None);
    }

    #[test]
    fn resolve_locale_resolves_explicit_languages() {
        assert_eq!(resolve_locale(AppLanguage::En), Locale::En);
        assert_eq!(resolve_locale(AppLanguage::De), Locale::De);
    }

    #[test]
    fn export_text_helpers_render_translated_strings() {
        assert_eq!(export_step_count(Locale::En, 2), "2 steps");
        assert_eq!(export_step_count(Locale::De, 1), "1 Schritt");
        assert_eq!(export_step_heading(Locale::De, 3), "Schritt 3");
        assert_eq!(
            auth_placeholder_description(Locale::De),
            "Authentifiziere dich mit Touch ID oder gib dein Passwort ein, um fortzufahren."
        );
    }

    #[test]
    fn tray_helpers_render_translated_strings() {
        assert_eq!(tray_menu_open(Locale::En), "Open StepCast");
        assert_eq!(tray_menu_open(Locale::De), "StepCast öffnen");
        assert_eq!(tray_menu_quick_start(Locale::De), "Schnellstart");
        assert_eq!(tray_menu_quit(Locale::De), "StepCast beenden");
        assert_eq!(
            tray_recording_tooltip(Locale::De),
            "StepCast - Aufnahme läuft..."
        );
    }

    #[test]
    fn ai_eligibility_helpers_render_translated_strings() {
        assert_eq!(
            ai_eligibility_requires_apple_silicon(Locale::En),
            "Requires Apple Silicon (M1+)."
        );
        assert_eq!(
            ai_eligibility_requires_apple_silicon(Locale::De),
            "Erfordert Apple Silicon (M1+)."
        );
        assert_eq!(
            ai_eligibility_not_enabled(Locale::De),
            "Apple Intelligence ist in den Systemeinstellungen deaktiviert."
        );
    }
}
