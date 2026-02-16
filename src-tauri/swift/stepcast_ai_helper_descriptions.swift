import Foundation

func actionVerb(_ action: String) -> String {
  switch action {
  case "DoubleClick":
    return "Double-click"
  case "RightClick":
    return "Right-click"
  case "Shortcut":
    return "Press"
  case "Note":
    return "Add a note"
  default:
    return "Click"
  }
}

func localizedVerb(_ canonicalVerb: String) -> String {
  switch canonicalVerb {
  case "Double-click":
    return l("Double-click", "Doppelklicke")
  case "Right-click":
    return l("Right-click", "Klicke mit der rechten Maustaste")
  case "Press":
    return l("Press", "Drücke")
  case "Add a note":
    return l("Add a note", "Füge eine Notiz hinzu")
  case "Choose":
    return l("Choose", "Wähle")
  case "Select":
    return l("Select", "Wähle")
  case "Open":
    return l("Open", "Öffne")
  case "Close":
    return l("Close", "Schließe")
  case "Enable":
    return l("Enable", "Aktiviere")
  case "Disable":
    return l("Disable", "Deaktiviere")
  default:
    return l("Click", "Klicke")
  }
}

func localizedKindNoun(_ kind: String) -> String {
  switch kind {
  case "item":
    return l("item", "Element")
  case "button":
    return l("button", "Button")
  case "tab":
    return l("tab", "Tab")
  case "text field":
    return l("text field", "Textfeld")
  case "list item":
    return l("item", "Element")
  case "menu item":
    return l("menu item", "Menüeintrag")
  case "menu bar item":
    return l("menu bar icon", "Menüleisten-Symbol")
  case "checkbox":
    return l("checkbox", "Kontrollkästchen")
  default:
    return kind
  }
}

func normalizeForMatch(_ s: String) -> String {
  s
    .trimmingCharacters(in: .whitespacesAndNewlines)
    .lowercased()
    .replacingOccurrences(of: "…", with: "...")
    .replacingOccurrences(of: "\"", with: "")
    .replacingOccurrences(of: "“", with: "")
    .replacingOccurrences(of: "”", with: "")
}

func stripFormatMarks(_ s: String) -> String {
  s.filter { ch in
    switch ch.unicodeScalars.first?.value ?? 0 {
    case 0x200E, 0x200F, 0x202A, 0x202B, 0x202C, 0x2066, 0x2067, 0x2068, 0x2069:
      return false
    default:
      return true
    }
  }
}

func normalizedNameKey(_ s: String) -> String {
  stripFormatMarks(s)
    .lowercased()
    .unicodeScalars
    .filter { CharacterSet.alphanumerics.contains($0) }
    .map(String.init)
    .joined()
}

func safeWindowTitleContext(_ step: StepInput) -> String? {
  let w = stripFormatMarks(step.windowTitle).trimmingCharacters(in: .whitespacesAndNewlines)
  if w.isEmpty { return nil }

  let lower = w.lowercased()
  if lower == "menu" || lower == "dock" { return nil }
  if lower == "popup" { return nil }
  if lower == "ohne titel" || lower == "untitled" { return nil }
  if lower.hasPrefix("menu - ") || lower.hasPrefix("dialog") || lower.hasPrefix("button - ") { return nil }
  if lower.hasPrefix("click on ") { return nil }
  if lower.contains("authentication dialog") { return nil }

  // Avoid bundle-like titles / noisy long titles.
  if w.contains("::") { return nil }
  if w.count > 32 { return nil }

  return w
}

func cleanupOcrLabel(_ s: String) -> String {
  var t = s.trimmingCharacters(in: .whitespacesAndNewlines)
  if t.isEmpty { return "" }

  // Normalize slash-like glyphs and spacing artifacts first.
  t = t.replacingOccurrences(of: "／", with: "/")
  t = t.replacingOccurrences(of: "∕", with: "/")
  t = t.replacingOccurrences(of: #"\s*/\s*"#, with: "/", options: .regularExpression)
  t = t.replacingOccurrences(of: #"\s*\.\s*"#, with: ".", options: .regularExpression)

  // Drop common OCR noise prefixes like "cz " before a filename token.
  let parts = t.split(separator: " ", omittingEmptySubsequences: true)
  if parts.count == 2 {
    let p0 = String(parts[0])
    let p1 = String(parts[1])
    if p0.count <= 2 && p0.allSatisfy({ $0.isLetter }) && looksLikeFileName(p1) {
      t = p1
    }
  }

  // OCR sometimes reads "l" as "/" in file names (e.g. "s-/1600.webp").
  if t.contains("-/") {
    let alt = t.replacingOccurrences(of: "-/", with: "-l")
    if looksLikeFileName(alt) {
      t = alt
    }
  }
  if t.contains("/") {
    let alt = t.replacingOccurrences(of: "/", with: "")
    if looksLikeFileName(alt) {
      t = alt
    }
  }

  // OCR can append context-menu artifacts to file names (e.g. "foo @ öffnen").
  t = t.replacingOccurrences(
    of: #"(?i)\s*@\s*(open|offnen|oeffnen|öffnen)\s*$"#,
    with: "",
    options: .regularExpression
  )

  return t.trimmingCharacters(in: .whitespacesAndNewlines)
}

func isDockStep(_ step: StepInput) -> Bool {
  step.app.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "dock"
}

func bestLabel(_ step: StepInput) -> String {
  if let ax = step.ax {
    let l = ax.label.trimmingCharacters(in: .whitespacesAndNewlines)
    if !l.isEmpty { return l }
  }
  let title = step.windowTitle.trimmingCharacters(in: .whitespacesAndNewlines)
  if title.hasPrefix("Menu - ") { return String(title.dropFirst("Menu - ".count)) }
  if title.hasPrefix("Button - ") { return String(title.dropFirst("Button - ".count)) }
  return ""
}

func isGenericUiLabel(_ step: StepInput, kind: String, label: String) -> Bool {
  let l = normalizeForMatch(label)
  if l.isEmpty { return true }

  if let ax = step.ax {
    let role = ax.role.lowercased()
    if role.contains("group")
      && (l == "create"
        || l.hasPrefix("create ")
        || l.hasPrefix("new ")
        || l.hasPrefix("add ")
        || l.contains(" erstellen")
        || l.hasPrefix("erstelle")
        || l.hasPrefix("hinzuf"))
    {
      return true
    }
  }

  // Common structural labels that are rarely meaningful UI targets.
  // Keep this small + cross-app (avoid per-app rules).
  if l == "editor area"
    || l == "content area"
    || l == "scroll area"
    || l == "split group"
    || l == "split view"
    || l == "workspace"
    || l == "popover dismiss region"
  {
    return true
  }
  if l.contains("dismiss region") || (l.contains("popover") && l.contains("dismiss")) {
    return true
  }
  if (l.hasSuffix(" area") || l.hasSuffix(" group")) && l.split(separator: " ").count <= 2 {
    return true
  }
  if l == "sidebar"
    || l == "seitenleiste"
    || l == "source list"
    || l == "list"
    || l == "list view"
    || l == "outline view"
    || l == "column view"
  {
    return true
  }

  let app = normalizeForMatch(step.app)
  if !app.isEmpty && l == app {
    // When the clicked thing is literally the app menu in the menu bar,
    // the label equals the app name and is meaningful.
    if kind == "menu bar item" || kind == "menu item" {
      return false
    }
    return true
  }

  let w = normalizeForMatch(step.windowTitle)
  if !w.isEmpty && l == w {
    // In many chat/file sidebars the selected row equals the window title.
    // Treating this as generic causes OCR drift ("vat" instead of "Pepper").
    if kind == "list item" {
      let containerRole = (step.ax?.containerRole ?? "").lowercased()
      let containerIdent = (step.ax?.containerIdentifier ?? "").lowercased()
      if containerRole.contains("list")
        || containerRole.contains("outline")
        || containerRole.contains("table")
        || containerRole.contains("collection")
        || containerIdent.contains("sidebar")
        || containerIdent.contains("list")
        || containerIdent.contains("outline")
        || containerIdent.contains("table")
        || containerIdent.contains("collection")
      {
        return false
      }
    }
    return true
  }

  if l == "window" || l == "menu" || l == "dialog" || l == "application" {
    return true
  }

  return false
}

func isLikelyViewModeLabel(_ step: StepInput, kind: String, label: String) -> Bool {
  if kind != "list item" { return false }
  guard let ax = step.ax else { return false }

  let role = ax.role.lowercased()
  if !(role.contains("outline") || role.contains("table")) { return false }

  let l = normalizeForMatch(label)
  if l.isEmpty { return false }

  // Common view-mode labels (Finder and many AppKit tables). Keep small; avoid per-app rules.
  if l == "list view" || l == "outline view" || l == "column view" || l == "icon view" { return true }
  if l.contains("listendarstellung") || l.contains("spaltendarstellung") || l.contains("symbolansicht") { return true }

  // Finder list container often exposes identifier like "ListView".
  if let ident = ax.identifier?.lowercased(), ident.contains("listview") {
    // Single-word labels here are very likely view-mode labels, not item names.
    if !l.contains(" ") { return true }
  }

  return false
}

func safeQuoted(_ label: String) -> String {
  let cleaned = label
    .trimmingCharacters(in: .whitespacesAndNewlines)
    .replacingOccurrences(of: "\"", with: "'")
  if cleaned.isEmpty { return "" }
  return "\"\(cleaned)\""
}

func sanitizeUiLabel(_ label: String) -> String {
  var s = label.trimmingCharacters(in: .whitespacesAndNewlines)
  if s.isEmpty { return "" }

  // Remove common invisible Unicode "format" marks that often appear in app AX labels.
  // (e.g. LRM/RLM and bidi isolate marks). Keep it small and explicit.
  s = s.filter { ch in
    let v = ch.unicodeScalars.first?.value ?? 0
    switch v {
    case 0x200E, 0x200F, 0x202A, 0x202B, 0x202C, 0x2066, 0x2067, 0x2068, 0x2069:
      return false
    case 0xE000...0xF8FF:
      // Private-use icon glyphs (common in web UIs) are not user-facing labels.
      return false
    default:
      return true
    }
  }

  // Drop common OCR bullet / list prefix noise: "* ", "• ", "- ".
  if s.hasPrefix("* ") || s.hasPrefix("• ") || s.hasPrefix("- ") {
    s = String(s.dropFirst(2)).trimmingCharacters(in: .whitespacesAndNewlines)
  }

  // Collapse repeated whitespace.
  s = s.replacingOccurrences(of: #"\s+"#, with: " ", options: .regularExpression)

  // Remove spaces around dots in filenames ("a .jpeg" -> "a.jpeg").
  s = s.replacingOccurrences(of: " .", with: ".")

  return s.trimmingCharacters(in: .whitespacesAndNewlines)
}

func looksLikeGuidOrHexLabel(_ label: String) -> Bool {
  let t = label
    .trimmingCharacters(in: .whitespacesAndNewlines)
    .replacingOccurrences(of: " ", with: "")
    .replacingOccurrences(of: "-", with: "")
    .uppercased()
  if t.count < 12 { return false }
  var hex = 0
  var other = 0
  for ch in t.unicodeScalars {
    if ("0"..."9").contains(ch) || ("A"..."F").contains(ch) {
      hex += 1
    } else {
      other += 1
    }
  }
  // Treat labels that are almost entirely hex as non-human UI labels (GUID/hash style).
  return other == 0 || (Double(hex) / Double(max(1, hex + other))) > 0.92
}

func looksLikeImplementationIdentifier(_ label: String) -> Bool {
  let t = label.trimmingCharacters(in: .whitespacesAndNewlines)
  if t.isEmpty { return false }
  if t.range(of: #"^\d+$"#, options: .regularExpression) != nil { return true }
  if t.range(of: #"^[A-Za-z]{1,4}[:._-]?\d+$"#, options: .regularExpression) != nil { return true }
  if t.range(of: #"^(?:ns|ax|ui)[:._-]?\d+$"#, options: [.regularExpression, .caseInsensitive]) != nil {
    return true
  }
  return false
}

func isNoisyLabel(_ label: String) -> Bool {
  let t = label.trimmingCharacters(in: .whitespacesAndNewlines)
  if t.isEmpty { return true }
  if looksLikeFileName(t) { return false }
  if isLikelyTimestampOrDateLabel(t) { return true }
  if isLikelySizeOrCountLabel(t) { return true }
  if isLikelyStatusOnlyLabel(t) { return true }
  if t.count > 56 { return true }

  var digits = 0
  var commas = 0
  for ch in t.unicodeScalars {
    if CharacterSet.decimalDigits.contains(ch) { digits += 1 }
    if ch == "," { commas += 1 }
  }
  if digits >= 10 && t.count >= 28 { return true }
  if commas >= 2 && t.count >= 34 { return true }
  return false
}

func isLikelyTimestampOrDateLabel(_ label: String) -> Bool {
  let t = label.trimmingCharacters(in: .whitespacesAndNewlines)
  if t.isEmpty { return false }
  if t.range(of: #"^\d{1,2}:\d{2}$"#, options: .regularExpression) != nil { return true }
  if t.range(of: #"^\d{1,2}[./-]\d{1,2}[./-]\d{2,4}$"#, options: .regularExpression) != nil { return true }
  if t.range(of: #"^\d{1,2}\s*[:.]\s*\d{2}$"#, options: .regularExpression) != nil { return true }
  // Relative day separators in chat/list UIs.
  let rel = normalizeForMatch(t)
  if rel == "today" || rel == "heute" || rel == "yesterday" || rel == "gestern" {
    return true
  }
  return false
}

func containsLikelyTimestampToken(_ label: String) -> Bool {
  label.range(of: #"\d{1,2}[:.]\d{2}"#, options: .regularExpression) != nil
}

func isLikelySizeOrCountLabel(_ label: String) -> Bool {
  let t = label.trimmingCharacters(in: .whitespacesAndNewlines)
  if t.isEmpty { return false }

  // Common size/value patterns in list columns (cross-app, language-agnostic enough).
  if t.range(of: #"^\d+(?:[.,]\d+)?\s*(?:b|kb|mb|gb|tb|bytes?)$"#, options: [.regularExpression, .caseInsensitive]) != nil {
    return true
  }
  if t.range(of: #"^\d+[.,]?\d*\s*(?:items?|elemente?|einträge?)$"#, options: [.regularExpression, .caseInsensitive]) != nil {
    return true
  }
  return false
}

func isLikelyStatusOnlyLabel(_ label: String) -> Bool {
  let l = normalizeForMatch(label)
  if l.isEmpty { return false }
  let statuses: Set<String> = [
    "sent", "gesendet",
    "read", "gelesen",
    "delivered", "zugestellt",
    "status", "inaktiv", "inactive",
  ]
  return statuses.contains(l)
}

func isLikelyFileKindLabel(_ label: String) -> Bool {
  let l = normalizeForMatch(label)
  if l.isEmpty { return false }

  // Common Finder/file-list "Kind" column values (cross-language minimal set).
  if l == "folder" || l == "folders"
    || l == "ordner"
    || l == "image" || l == "images"
    || l == "bild" || l == "bilder"
    || l == "video" || l == "videos"
    || l == "film" || l == "filme"
  {
    return true
  }
  if l.hasSuffix("-bild") || l.hasSuffix("-film") || l.hasSuffix("-document") || l.hasSuffix("-dokument") {
    return true
  }
  if l.hasSuffix(" dokument") || l.hasSuffix(" image") || l.hasSuffix(" video") {
    return true
  }
  if l.contains("jpeg-bild") || l.contains("png-bild") || l.contains("webp-bild") || l.contains("mpeg-4-film") {
    return true
  }
  return false
}

func isLikelySidebarLocationLabel(_ label: String) -> Bool {
  let l = normalizeForMatch(label)
  if l.isEmpty { return false }

  let exact: Set<String> = [
    "desktop", "schreibtisch",
    "downloads", "download",
    "documents", "dokumente",
    "movies", "filme",
    "icloud drive", "onedrive", "dropbox", "airdrop",
    "applications", "programme",
    "macintosh hd",
    "network", "netzwerk",
    "home",
  ]
  if exact.contains(l) { return true }

  return l.hasPrefix("macintosh hd")
    || l.hasSuffix("reibtisch")
    || l.hasPrefix("leben-mac-")
    || l.hasPrefix("eben-mac-")
    || l.hasPrefix("mac-")
}

func isLikelyPickerCategoryLabel(_ label: String) -> Bool {
  let l = normalizeForMatch(label)
  if l.isEmpty { return false }
  return l == "emoji"
    || l == "emojis"
    || l == "gif"
    || l == "gifs"
    || l == "sticker"
    || l == "stickers"
    || l == "today"
    || l == "heute"
}

func isLikelyPickerInteraction(_ step: StepInput) -> Bool {
  let role = (step.ax?.role ?? "").lowercased()
  let ident = (step.ax?.identifier ?? "").lowercased()
  let containerIdent = (step.ax?.containerIdentifier ?? "").lowercased()
  let roleDesc = (step.ax?.roleDescription ?? "").lowercased()

  let haystack = [role, ident, containerIdent, roleDesc].joined(separator: " ")
  return haystack.contains("sticker")
    || haystack.contains("emoji")
    || haystack.contains("gif")
    || haystack.contains("picker")
}

func isLikelyContextMenuPhrase(_ label: String) -> Bool {
  let l = normalizeForMatch(label)
  if l.isEmpty { return false }

  if l == "open" || l == "öffnen" || l == "offnen" || l == "oeffnen"
    || l == "open with" || l == "öffnen mit" || l == "offnen mit" || l == "oeffnen mit"
    || l == "information" || l == "informationen"
    || l == "rename" || l == "umbenennen"
    || l == "copy" || l == "kopieren"
    || l == "share" || l == "teilen"
    || l == "delete" || l == "in den papierkorb legen"
  {
    return true
  }

  if l.hasSuffix(" @ öffnen") || l.hasSuffix(" @ offnen") || l.hasSuffix(" @ oeffnen") || l.hasSuffix(" @ open") { return true }
  if l.contains(" öffnen mit") || l.contains(" offnen mit") || l.contains(" oeffnen mit") || l.contains(" open with") { return true }
  if l.contains("@") && (l.contains("öffnen") || l.contains("offnen") || l.contains("oeffnen") || l.contains("open")) {
    return true
  }
  return false
}

func extractRecipientTargetSegment(_ segment: String) -> (value: String, boosted: Bool) {
  let trimmed = segment.trimmingCharacters(in: .whitespacesAndNewlines)
  if trimmed.isEmpty { return ("", false) }

  let lowered = normalizeForMatch(trimmed)
  let prefixMap: [(String, String)] = [
    ("gesendet an ", "gesendet an "),
    ("sent to ", "sent to "),
  ]

  for (key, rawPrefix) in prefixMap {
    if lowered.hasPrefix(key) {
      let idx = trimmed.index(trimmed.startIndex, offsetBy: rawPrefix.count, limitedBy: trimmed.endIndex) ?? trimmed.endIndex
      let tail = String(trimmed[idx...]).trimmingCharacters(in: .whitespacesAndNewlines)
      if !tail.isEmpty { return (tail, true) }
    }
  }
  return (trimmed, false)
}

func simplifyMetadataLabel(_ label: String) -> String {
  let t = label.trimmingCharacters(in: .whitespacesAndNewlines)
  if t.isEmpty { return "" }
  if looksLikeFileName(t) { return t }

  // Many apps encode list-row metadata in a single comma-separated AX label.
  // Pick the most "entity-like" segment instead of always taking the first one.
  if t.contains(",") {
    let rawSegments = t
      .split(separator: ",", omittingEmptySubsequences: false)
      .map { String($0).trimmingCharacters(in: .whitespacesAndNewlines) }
      .filter { !$0.isEmpty }

    var best = ""
    var bestScore = Int.min

    for raw in rawSegments {
      let extracted = extractRecipientTargetSegment(raw)
      let candidate = sanitizeUiLabel(extracted.value)
      if candidate.isEmpty { continue }
      if isLikelyTimestampOrDateLabel(candidate) { continue }
      if isLikelySizeOrCountLabel(candidate) { continue }
      if isLikelyStatusOnlyLabel(candidate) { continue }
      if isNoisyLabel(candidate) && !looksLikeFileName(candidate) { continue }

      var score = 0
      if extracted.boosted { score += 10 }
      if looksLikeFileName(candidate) { score += 8 }
      if candidate.count >= 4 && candidate.count <= 36 { score += 4 }
      if candidate.split(separator: " ").count >= 2 { score += 2 }

      if score > bestScore {
        bestScore = score
        best = candidate
      }
    }

    if !best.isEmpty { return best }
  }

  return t
}

func humanizeIdentifier(_ identifier: String) -> String {
  var s = identifier.trimmingCharacters(in: .whitespacesAndNewlines)
  if s.isEmpty { return "" }

  // Normalize separators to spaces.
  s = s.replacingOccurrences(of: "_", with: " ")
  s = s.replacingOccurrences(of: "-", with: " ")

  // Insert spaces for simple camelCase / PascalCase transitions.
  var out = ""
  out.reserveCapacity(s.count + 8)
  var prevWasLower = false
  for ch in s {
    let isUpper = ch.isUppercase
    if isUpper && prevWasLower {
      out.append(" ")
    }
    out.append(ch)
    prevWasLower = ch.isLowercase
  }

  let rawTokens = out
    .split(whereSeparator: { $0 == " " || $0 == "\t" || $0 == "\n" })
    .map { String($0) }

  // Drop common implementation-detail tokens. Keep list small + generic (not app-specific).
  let drop: Set<String> = [
    "button", "btn",
    "view", "collectionview", "collection",
    "cell", "row", "item",
    "group", "container",
    "icon", "image",
  ]

  var tokens: [String] = []
  tokens.reserveCapacity(rawTokens.count)
  for tok in rawTokens {
    let lower = tok.lowercased()
    if drop.contains(lower) { continue }
    if lower.count <= 1 { continue }
    tokens.append(tok)
  }

  if tokens.isEmpty { return "" }

  // Prefer "semantic" tokens (works across apps without per-app rules).
  let semantic: Set<String> = [
    "emoji", "emojis",
    "sticker", "stickers",
    "gif", "gifs",
    "search",
    "send",
    "connect", "connection",
    "download", "downloads",
    "import", "export",
    "delete", "remove",
    "settings",
    "close", "minimize", "zoom",
  ]
  let semanticTokens = tokens.filter { semantic.contains($0.lowercased()) }
  if !semanticTokens.isEmpty {
    let s = semanticTokens.prefix(2).joined(separator: " ")
    return sanitizeUiLabel(s)
  }

  if tokens.count > 4 { tokens = Array(tokens.prefix(4)) }

  var label = tokens.joined(separator: " ").trimmingCharacters(in: .whitespacesAndNewlines)
  if label.count > 32 {
    label = String(label.prefix(32)).trimmingCharacters(in: .whitespacesAndNewlines)
  }
  return label
}

func requiresQuotedLabel(_ label: String) -> Bool {
  // Enforce quotes for filenames / non-ASCII / punctuation labels. This improves readability.
  if label.isEmpty { return false }
  for u in label.unicodeScalars {
    if u.value > 0x7F { return true }
    if CharacterSet.letters.contains(u) || CharacterSet.decimalDigits.contains(u) || u == " " { continue }
    return true
  }
  return false
}

func locationHint(_ step: StepInput, kind: String) -> String? {
  // Prefer semantic AX signals; avoid geometry guesses when possible.
  if let ax = step.ax {
    let sub = (ax.containerSubrole ?? "").lowercased()
    let ident = (ax.containerIdentifier ?? "").lowercased()
    let selfIdent = (ax.identifier ?? "").lowercased()

    if sub.contains("sourcelist") || sub.contains("sidebar") || ident.contains("sidebar") || selfIdent.contains("sidebar") {
      return l("sidebar", "Seitenleiste")
    }
  }

  if kind == "menu bar item" {
    return l("menu bar", "Menüleiste")
  }

  let app = step.app.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
  if app == "finder" {
    // If we can’t prove “sidebar”, still add a non-guessy hint for list interactions.
    if kind == "list item" { return l("file list", "Dateiliste") }
  }
  return nil
}

func appContextSuffix(_ step: StepInput, location: String?) -> String {
  let app = step.app.trimmingCharacters(in: .whitespacesAndNewlines)
  if app.isEmpty { return "" }
  let lower = app.lowercased()
  if lower == "application" || lower == "dock" { return "" }

  let winRaw = safeWindowTitleContext(step)
  let appKey = normalizedNameKey(app)
  let win = winRaw.flatMap { w -> String? in
    let winKey = normalizedNameKey(w)
    if !appKey.isEmpty && !winKey.isEmpty && winKey == appKey {
      return nil
    }
    return w
  }
  if let location = location, !location.isEmpty {
    if lower == "finder" {
      if let win = win { return l(" in the Finder \(location) (\(win))", " in der Finder-\(location) (\(win))") }
      return l(" in the Finder \(location)", " in der Finder-\(location)")
    }
    if let win = win { return l(" in \(app) \(location) (\(win))", " in \(app) \(location) (\(win))") }
    return l(" in \(app) \(location)", " in \(app) \(location)")
  }
  if let win = win {
    if lower == "finder" {
      return l(" in Finder (\(win))", " in Finder (\(win))")
    }
    return l(" in \(app) (\(win))", " in \(app) (\(win))")
  }
  return l(" in \(app)", " in \(app)")
}

func classifyKind(_ step: StepInput) -> String {
  if isDockStep(step) { return "dock item" }
  let windowTitleNorm = normalizeForMatch(step.windowTitle)
  let hasMenuWindowHint = windowTitleNorm == "menu" && step.clickYPercent <= 70.0
  guard let ax = step.ax else {
    // Only treat as menu when the recorder explicitly tagged a menu selection.
    let w = step.windowTitle.trimmingCharacters(in: .whitespacesAndNewlines)
    if w.lowercased().hasPrefix("menu - ") { return "menu item" }
    if hasMenuWindowHint { return "menu item" }
    return "item"
  }

  let role = ax.role.lowercased()
  let subrole = (ax.subrole ?? "").lowercased()
  let container = (ax.containerRole ?? "").lowercased()
  let containerIdent = (ax.containerIdentifier ?? "").lowercased()
  let selfIdent = (ax.identifier ?? "").lowercased()
  let roleDesc = (ax.roleDescription ?? "").lowercased()

  let roleLooksMenuLike =
    role.contains("menuitem")
    || role == "axmenu"
    || role.contains(" menu")
    || role.hasSuffix("menu")
    || role.contains("menubar")
    || role.contains("menubutton")
  if hasMenuWindowHint && roleLooksMenuLike {
    return "menu item"
  }

  if subrole.contains("close") && role.contains("button") { return "close button" }
  if (subrole.contains("minimize") || roleDesc.contains("minim")) && role.contains("button") {
    return "minimize button"
  }
  if (subrole.contains("zoom") || roleDesc.contains("zoom")) && role.contains("button") {
    return "zoom button"
  }
  if role.contains("textfield") || role.contains("text field") || subrole.contains("searchfield") {
    return "text field"
  }
  if role.contains("menubar") || role.contains("menubutton") { return "menu bar item" }
  if role.contains("menuitem") || role == "axmenu" || role.contains(" menu") || role.hasSuffix("menu") { return "menu item" }
  if role.contains("application") {
    // Some apps report AXApplication when clicking traffic-light window controls.
    // Use a conservative geometry fallback near the top-left corner.
    if step.clickXPercent <= 8.0 && step.clickYPercent <= 8.0 {
      return "close button"
    }
  }
  if role.contains("checkbox") { return "checkbox" }
  if role.contains("tab") || subrole.contains("tab") { return "tab" }

  // Many apps model list rows as pressable groups/buttons, so the clicked role may be "button".
  // Use container hints (role/identifier) to classify those as list interactions.
  if container.contains("row")
    || container.contains("outline")
    || container.contains("table")
    || container.contains("list")
    || containerIdent.contains("table")
    || containerIdent.contains("outline")
    || containerIdent.contains("list")
    || containerIdent.contains("collection")
    || selfIdent.contains("table")
    || selfIdent.contains("outline")
    || selfIdent.contains("list")
    || selfIdent.contains("collection")
  {
    return "list item"
  }
  if role.contains("row") || role.contains("cell") || role.contains("outline") || role.contains("table") || role.contains("list") { return "list item" }

  if role.contains("popupbutton") || role.contains("button") { return "button" }
  if role.contains("radiobutton") { return "option" }
  return "item"
}

func checkboxVerb(_ step: StepInput) -> String {
  if let checked = step.ax?.isChecked {
    // AXValue reflects current state at click-time; action semantics are state toggle.
    return checked ? "Disable" : "Enable"
  }
  return "Enable"
}

func preferredVerb(_ step: StepInput, kind: String) -> String {
  let action = actionVerb(step.action)
  if isDockStep(step) { return "Open" }
  if action == "Right-click" || action == "Double-click" || action == "Press" {
    return action
  }
  switch kind {
  case "menu item":
    return "Choose"
  case "menu bar item":
    return "Click"
  case "close button":
    return "Close"
  case "text field":
    return "Click"
  case "checkbox":
    return checkboxVerb(step)
  case "list item":
    return "Select"
  default:
    return action
  }
}

func chooseGroundingLabel(_ step: StepInput, kind: String) -> (label: String, ocr: String?) {
  var axLabel = simplifyMetadataLabel(sanitizeUiLabel(bestLabel(step)))
  if looksLikeGuidOrHexLabel(axLabel) {
    axLabel = ""
  }
  if !axLabel.isEmpty && isNoisyLabel(axLabel) {
    axLabel = ""
  }
  if kind == "list item" && isLikelyFileKindLabel(axLabel) {
    axLabel = ""
  }
  if isLikelyTimestampOrDateLabel(axLabel) || isLikelySizeOrCountLabel(axLabel) || isLikelyStatusOnlyLabel(axLabel) {
    axLabel = ""
  }
  if kind == "list item" && step.clickXPercent > 30.0 && isLikelySidebarLocationLabel(axLabel) {
    axLabel = ""
  }
  if kind == "list item" && isLikelyPickerInteraction(step) {
    axLabel = ""
  }
  if (kind == "item" || kind == "button") && isLikelyPickerCategoryLabel(axLabel) {
    axLabel = ""
  }

  let identLabel: String = {
    guard let ident = step.ax?.identifier else { return "" }
    let identLower = ident.lowercased()
    let role = (step.ax?.role ?? "").lowercased()
    if identLower.contains("collectionview")
      || identLower.contains("listview")
      || identLower.contains("tableview")
    {
      if role.contains("group") || role.contains("collection") || role.contains("list") {
        return ""
      }
    }
    let h = sanitizeUiLabel(humanizeIdentifier(ident))
    if h.isEmpty { return "" }
    if looksLikeGuidOrHexLabel(h) { return "" }
    if looksLikeImplementationIdentifier(h) { return "" }
    if isNoisyLabel(h) { return "" }
    if isLikelyTimestampOrDateLabel(h) || isLikelySizeOrCountLabel(h) || isLikelyStatusOnlyLabel(h) {
      return ""
    }
    if isGenericUiLabel(step, kind: kind, label: h) || isLikelyViewModeLabel(step, kind: kind, label: h) {
      return ""
    }
    return h
  }()

  if kind == "menu bar item" {
    // Menu bar icons often have no AX label; OCR may pick unrelated menu text like "Status".
    // Use the app name as the grounding label and skip OCR for this kind.
    if axLabel.isEmpty {
      let app = step.app.trimmingCharacters(in: .whitespacesAndNewlines)
      if !app.isEmpty {
        return (label: app, ocr: nil)
      }
    }
    return (label: axLabel, ocr: nil)
  }

  // For window controls: avoid OCR and any unrelated labels; baseline handles it.
  if kind == "close button" || kind == "minimize button" || kind == "zoom button" {
    return (label: "", ocr: nil)
  }

  let axGeneric =
    isGenericUiLabel(step, kind: kind, label: axLabel)
    || isLikelyViewModeLabel(step, kind: kind, label: axLabel)

  // OCR is expensive; run it only when it can materially improve results.
  // Prefer AXIdentifier-derived labels over OCR when AXTitle is missing/cryptic.
  if (axLabel.isEmpty || axGeneric), !identLabel.isEmpty {
    return (label: identLabel, ocr: nil)
  }

  let shouldOcr = axLabel.isEmpty || step.action == "RightClick" || axGeneric
  let rawOcr = shouldOcr ? bestOcrLabelNearClick(step) : nil
  let cleanedOcr = rawOcr
    .map(cleanupOcrLabel)
    .map(sanitizeUiLabel)
  let ocr: String? = {
    guard let candidate = cleanedOcr else { return nil }
    if isGenericUiLabel(step, kind: kind, label: candidate) { return nil }
    if isLikelyPickerCategoryLabel(candidate) && (kind == "item" || kind == "button") { return nil }
    if kind == "list item" && isLikelyContextMenuPhrase(candidate) { return nil }
    if isLikelyTimestampOrDateLabel(candidate) { return nil }
    if containsLikelyTimestampToken(candidate) { return nil }
    if isLikelySizeOrCountLabel(candidate) { return nil }
    if isLikelyStatusOnlyLabel(candidate) { return nil }
    if kind == "list item" && isLikelyFileKindLabel(candidate) { return nil }
    if kind == "list item" && step.clickXPercent > 30.0 && isLikelySidebarLocationLabel(candidate) {
      return nil
    }
    if kind == "list item" && isLikelyPickerInteraction(step) { return nil }
    if kind == "list item" && step.clickXPercent < 30.0 && !looksLikeFileName(candidate) {
      // Sidebar OCR without a strong token is error-prone across apps/locales.
      return nil
    }
    if isNoisyLabel(candidate) && !looksLikeFileName(candidate) { return nil }
    return candidate
  }()

  if step.action == "RightClick" {
    // Right-click: prefer OCR near click for row targets, including folder names without file extensions.
    if let ocr = ocr, !ocr.isEmpty {
      if axLabel.isEmpty || axGeneric {
        return (label: ocr, ocr: ocr)
      }
      // List rows frequently expose generic AX labels; OCR is often closer to user intent.
      if kind == "list item"
        && normalizeForMatch(ocr) != normalizeForMatch(axLabel)
        && !isLikelyContextMenuPhrase(ocr)
      {
        return (label: ocr, ocr: ocr)
      }
    }
    if axLabel.isEmpty || axGeneric {
      return (label: "", ocr: nil)
    }
    return (label: axLabel, ocr: ocr)
  }

  if axLabel.isEmpty || axGeneric {
    if let ocr = ocr { return (label: ocr, ocr: ocr) }
    // Prefer a blank label over a generic "RustDesk in RustDesk" label.
    if axGeneric { return (label: "", ocr: nil) }
  }

  if axLabel.isEmpty {
    return (label: ocr ?? "", ocr: ocr)
  }

  return (label: axLabel, ocr: ocr)
}

func sanitizeDescription(_ raw: String, maxChars: Int) -> String {
  var s = raw.trimmingCharacters(in: .whitespacesAndNewlines)
  if s.hasPrefix("\"") && s.hasSuffix("\"") && s.count >= 2 {
    s = String(s.dropFirst().dropLast()).trimmingCharacters(in: .whitespacesAndNewlines)
  }

  // Drop common prefixes ("Step 1:", "1.", etc.)
  s = s.replacingOccurrences(
    of: #"^\s*(Step\s*\d+[:.)-]\s*)"#,
    with: "",
    options: .regularExpression
  )
  s = s.replacingOccurrences(
    of: #"^\s*\d+[:.)-]\s*"#,
    with: "",
    options: .regularExpression
  )

  // Collapse whitespace / newlines.
  s = s.replacingOccurrences(of: #"\s+"#, with: " ", options: .regularExpression)

  // Remove bullet noise, including cases like "Double-click * Foo".
  s = s.replacingOccurrences(
    of: #"^(click|double-click|right-click|choose|select)\s+[*•-]\s+"#,
    with: "$1 ",
    options: [.regularExpression, .caseInsensitive]
  )
  s = s.replacingOccurrences(
    of: #"^\s*[*•-]\s+"#,
    with: "",
    options: .regularExpression
  )

  // Keep only the first sentence if multiple are present.
  if let idx = s.indices.first(where: { i in
    let ch = s[i]
    guard ch == "." || ch == "!" || ch == "?" else { return false }
    let next = s.index(after: i)
    guard next < s.endIndex, s[next] == " " else { return false }
    let after = s.index(after: next)
    return after < s.endIndex
  }) {
    s = String(s[...idx])
  }

  s = s.trimmingCharacters(in: .whitespacesAndNewlines)
  if s.isEmpty { return "" }

  if s.count > maxChars {
    let prefix = String(s.prefix(maxChars))
    if let lastSpace = prefix.lastIndex(of: " ") {
      s = String(prefix[..<lastSpace]).trimmingCharacters(in: .whitespacesAndNewlines)
    } else {
      s = prefix.trimmingCharacters(in: .whitespacesAndNewlines)
    }
  }

  return s
}

func baselineDescription(_ step: StepInput, kind: String, label: String, location: String?, maxChars: Int) -> String {
  let cleanLabel = sanitizeUiLabel(label)
  let q = safeQuoted(cleanLabel)
  let suffix = appContextSuffix(step, location: location)
  let verb = actionVerb(step.action)
  let rightClickVerb = localizedVerb("Right-click")
  let doubleClickVerb = localizedVerb("Double-click")
  let pressVerb = localizedVerb("Press")

  var s: String
  if kind == "close button" {
    let app = step.app.trimmingCharacters(in: .whitespacesAndNewlines)
    if !app.isEmpty && app.lowercased() != "application" && app.lowercased() != "dock" {
      s = l("Close the \(app) window.", "Schließe das \(app)-Fenster.")
    } else {
      s = l("Close the window.", "Schließe das Fenster.")
    }
  } else if kind == "minimize button" {
    let app = step.app.trimmingCharacters(in: .whitespacesAndNewlines)
    if !app.isEmpty && app.lowercased() != "application" && app.lowercased() != "dock" {
      s = l("Minimize the \(app) window.", "Minimiere das \(app)-Fenster.")
    } else {
      s = l("Minimize the window.", "Minimiere das Fenster.")
    }
  } else if kind == "zoom button" {
    let app = step.app.trimmingCharacters(in: .whitespacesAndNewlines)
    if !app.isEmpty && app.lowercased() != "application" && app.lowercased() != "dock" {
      s = l("Zoom the \(app) window.", "Vergrößere das \(app)-Fenster.")
    } else {
      s = l("Zoom the window.", "Vergrößere das Fenster.")
    }
  } else if kind == "menu bar item" {
    let app = step.app.trimmingCharacters(in: .whitespacesAndNewlines)
    if !app.isEmpty && app.lowercased() != "application" && app.lowercased() != "dock" {
      s = l(
        "Click the \(app) icon in the menu bar to open its menu.",
        "Klicke auf das \(app)-Symbol in der Menüleiste, um das Menü zu öffnen."
      )
    } else if !label.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
      s = l(
        "Click \(q) in the menu bar to open its menu.",
        "Klicke in der Menüleiste auf \(q), um das Menü zu öffnen."
      )
    } else {
      s = l(
        "Click the menu bar icon to open its menu.",
        "Klicke auf das Menüleisten-Symbol, um das Menü zu öffnen."
      )
    }
  } else if kind == "text field" {
    // We only record clicks (no global key capture); avoid pretending we saw typing.
    if label.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
      s = l("Click the text field\(suffix).", "Klicke auf das Textfeld\(suffix).")
    } else if label.allSatisfy({ $0.isLetter || $0.isNumber }) && !label.contains(" ") {
      s = l("Click the \(label) field\(suffix).", "Klicke auf das Feld \(label)\(suffix).")
    } else {
      s = l("Click \(q) field\(suffix).", "Klicke auf das Feld \(q)\(suffix).")
    }
  } else if isDockStep(step) {
    s = label.isEmpty
      ? l("Open the app from the Dock.", "Öffne die App im Dock.")
      : l("Open \(label) from the Dock.", "Öffne \(label) im Dock.")
  } else if kind == "menu item" {
    s = q.isEmpty
      ? l("Choose the menu item\(suffix).", "Wähle den Menüeintrag\(suffix).")
      : l("Choose \(q) from the menu\(suffix).", "Wähle \(q) aus dem Menü\(suffix).")
  } else if kind == "checkbox" {
    let checkboxAction = localizedVerb(checkboxVerb(step))
    s = q.isEmpty
      ? l("\(checkboxAction) the checkbox\(suffix).", "\(checkboxAction) das Kontrollkästchen\(suffix).")
      : "\(checkboxAction) \(q)\(suffix)."
  } else if verb == "Right-click" {
    s = q.isEmpty
      ? "\(rightClickVerb) auf \(localizedKindNoun(kind))\(suffix)."
      : "\(rightClickVerb) auf \(q)\(suffix)."
  } else if verb == "Double-click" {
    s = q.isEmpty
      ? "\(doubleClickVerb) auf \(localizedKindNoun(kind))\(suffix)."
      : "\(doubleClickVerb) auf \(q)\(suffix)."
  } else if verb == "Press" {
    let note = step.note?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    if !note.isEmpty {
      s = "\(pressVerb) \(note)\(suffix)."
    } else {
      s = l("Press the keyboard shortcut\(suffix).", "Drücke das Tastenkürzel\(suffix).")
    }
  } else {
    switch kind {
    case "list item":
      s = q.isEmpty
        ? l("Select the item\(suffix).", "Wähle das Element\(suffix).")
        : l("Select \(q)\(suffix).", "Wähle \(q)\(suffix).")
    case "tab":
      s = q.isEmpty
        ? l("Click the tab\(suffix).", "Klicke auf den Tab\(suffix).")
        : l("Click the \(q) tab\(suffix).", "Klicke auf den Tab \(q)\(suffix).")
    case "button":
      if q.isEmpty {
        s = l("Click the button\(suffix).", "Klicke auf den Button\(suffix).")
      } else if cleanLabel.allSatisfy({ $0.isLetter || $0.isNumber || $0 == " " }) && !cleanLabel.contains(" ") {
        s = l(
          "Click the \(cleanLabel) button\(suffix).",
          "Klicke auf den Button \(cleanLabel)\(suffix)."
        )
      } else {
        s = l("Click \(q)\(suffix).", "Klicke auf \(q)\(suffix).")
      }
    default:
      s = q.isEmpty
        ? l("Click the item\(suffix).", "Klicke auf das Element\(suffix).")
        : l("Click \(q)\(suffix).", "Klicke auf \(q)\(suffix).")
    }
  }

  return sanitizeDescription(s, maxChars: maxChars)
}

func startsWithVerb(_ s: String) -> Bool {
  let t = s.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
  return t.hasPrefix("click")
    || t.hasPrefix("double-click")
    || t.hasPrefix("right-click")
    || t.hasPrefix("close")
    || t.hasPrefix("open")
    || t.hasPrefix("choose")
    || t.hasPrefix("select")
    || t.hasPrefix("enable")
    || t.hasPrefix("disable")
    || t.hasPrefix("press")
    || t.hasPrefix("type")
    || t.hasPrefix("drag")
    || t.hasPrefix("klicke")
    || t.hasPrefix("doppelklicke")
    || t.hasPrefix("wähle")
    || t.hasPrefix("waehle")
    || t.hasPrefix("öffne")
    || t.hasPrefix("oeffne")
    || t.hasPrefix("schließe")
    || t.hasPrefix("schliesse")
    || t.hasPrefix("aktiviere")
    || t.hasPrefix("deaktiviere")
    || t.hasPrefix("drücke")
    || t.hasPrefix("druecke")
    || t.hasPrefix("ziehe")
}

struct QualityGateDecision {
  let text: String
  let reason: String
}

func firstQuotedSegment(_ s: String) -> String? {
  let pattern = #""([^"]+)"|'([^']+)'"#
  guard let regex = try? NSRegularExpression(pattern: pattern) else { return nil }
  let range = NSRange(s.startIndex..<s.endIndex, in: s)
  guard let m = regex.firstMatch(in: s, options: [], range: range) else { return nil }
  if m.numberOfRanges >= 2, let r = Range(m.range(at: 1), in: s) {
    return String(s[r]).trimmingCharacters(in: .whitespacesAndNewlines)
  }
  if m.numberOfRanges >= 3, let r = Range(m.range(at: 2), in: s) {
    return String(s[r]).trimmingCharacters(in: .whitespacesAndNewlines)
  }
  return nil
}

func applyQualityGate(step: StepInput, kind: String, baseline: String, candidate: String, label: String) -> QualityGateDecision {
  let cand = candidate.trimmingCharacters(in: .whitespacesAndNewlines)
  if cand.isEmpty { return QualityGateDecision(text: baseline, reason: "candidate_empty") }
  if !startsWithVerb(cand) { return QualityGateDecision(text: baseline, reason: "candidate_missing_verb") }

  let candNorm = normalizeForMatch(cand)
  let baseNorm = normalizeForMatch(baseline)

  // Conservative-by-default: for fragile UI targets, keep deterministic baseline wording.
  // This avoids cross-app/cross-language drift in generated rewrites.
  let baselinePreferredKinds: Set<String> = ["list item", "item", "button", "text field", "tab"]
  if baselinePreferredKinds.contains(kind), candNorm != baseNorm {
    return QualityGateDecision(text: baseline, reason: "prefer_baseline_for_kind")
  }

  let pref = localizedVerb(preferredVerb(step, kind: kind)).lowercased()
  if !cand.lowercased().hasPrefix(pref) {
    // Keep the style consistent and avoid generic verbs ("Click") when we have a stronger signal.
    return QualityGateDecision(text: baseline, reason: "candidate_wrong_verb")
  }

  if isDockStep(step) && !candNorm.contains("dock") {
    return QualityGateDecision(text: baseline, reason: "dock_context_missing")
  }
  if kind == "menu item"
    && !candNorm.contains("menu")
    && !candNorm.contains("menü")
  {
    return QualityGateDecision(text: baseline, reason: "menu_context_missing")
  }
  if kind == "menu bar item"
    && !candNorm.contains("menu bar")
    && !candNorm.contains("menüleiste")
  {
    return QualityGateDecision(text: baseline, reason: "menu_bar_context_missing")
  }
  if kind != "tab"
    && (candNorm.contains(" tab")
      || candNorm.contains(" registerkarte"))
  {
    // Avoid "tab" hallucinations when we don't have a tab signal.
    return QualityGateDecision(text: baseline, reason: "tab_hallucination")
  }

  if !label.isEmpty {
    let cleanLabel = sanitizeUiLabel(label)
    let labelNorm = normalizeForMatch(cleanLabel)
    if !candNorm.contains(labelNorm) {
      return QualityGateDecision(text: baseline, reason: "label_not_present")
    }

    let quoted = "\"\(cleanLabel)\""
    let quotedAlt = "'\(cleanLabel)'"
    let baselineHasQuotedTarget = baseline.contains(quoted) || baseline.contains(quotedAlt)
    if baselineHasQuotedTarget {
      let candHasQuotedTarget = cand.contains(quoted) || cand.contains(quotedAlt)
      if !candHasQuotedTarget {
        return QualityGateDecision(text: baseline, reason: "quoted_label_lost")
      }

      // For target-bearing kinds, a mismatched first quoted object is almost always wrong.
      if kind == "list item" || kind == "button" || kind == "tab" || kind == "item" || kind == "text field" {
        if let firstQuoted = firstQuotedSegment(cand) {
          if normalizeForMatch(firstQuoted) != labelNorm {
            return QualityGateDecision(text: baseline, reason: "quoted_target_mismatch")
          }
        }
      }
    }

    if requiresQuotedLabel(cleanLabel) {
      // Keep labels readable and unambiguous for novices.
      let quoted = "\"\(cleanLabel)\""
      let quotedAlt = "'\(cleanLabel)'"
      if !cand.contains(quoted) && !cand.contains(quotedAlt) {
        return QualityGateDecision(text: baseline, reason: "quoted_label_required")
      }
    }

    if let win = safeWindowTitleContext(step) {
      let winNorm = normalizeForMatch(win)
      if !winNorm.isEmpty && baseNorm.contains(winNorm) && !candNorm.contains(winNorm) {
        // If baseline includes useful window context (e.g. "Downloads"), keep it.
        return QualityGateDecision(text: baseline, reason: "window_context_lost")
      }
    }

    let app = step.app.trimmingCharacters(in: .whitespacesAndNewlines)
    if !appContextSuffix(step, location: nil).isEmpty {
      let appNorm = normalizeForMatch(app)
      if !appNorm.isEmpty && !candNorm.contains(appNorm) {
        // If we know the target app, keep that grounding (prevents "Select front.").
        return QualityGateDecision(text: baseline, reason: "app_context_lost")
      }
    }
  } else {
    // No trustworthy grounding label: keep deterministic baseline, avoid hallucinations.
    return QualityGateDecision(text: baseline, reason: "no_grounding_label")
  }

  return QualityGateDecision(text: cand, reason: "accepted")
}

func promptForStep(
  _ step: StepInput,
  kind: String,
  baseline: String,
  label: String,
  ocr: String?,
  location: String?,
  maxChars: Int
) -> String {
  var lines: [String] = []
  lines.append(l(
    "Write ONE short UI tutorial step description.",
    "Schreibe EINE kurze UI-Tutorial-Schrittbeschreibung."
  ))
  lines.append(l("Rules:", "Regeln:"))
  lines.append(l(
    "- ONE sentence, max \(maxChars) characters.",
    "- EINE Satzzeile, maximal \(maxChars) Zeichen."
  ))
  lines.append(l(
    "- Start with a verb (e.g. Click, Double-click, Right-click, Close, Open, Choose, Select).",
    "- Starte mit einem Verb (z. B. Klicke, Doppelklicke, Wähle, Öffne, Schließe)."
  ))
  lines.append(l(
    "- No numbering, no markdown, no quotes unless quoting a UI label.",
    "- Keine Nummerierung, kein Markdown, keine Anführungszeichen außer bei UI-Labels."
  ))
  lines.append(l(
    "- Do NOT invent UI labels. Use the provided label; if missing, stay generic.",
    "- Erfinde KEINE UI-Labels. Nutze nur bereitgestellte Labels; sonst bleibe generisch."
  ))
  lines.append(l(
    "- Avoid vague output like \"Click Finder.\"; include location when known (Dock/menu/dialog).",
    "- Vermeide vage Ausgaben wie \"Klicke Finder.\"; nutze Ortshinweise (Dock/Menü/Dialog), wenn bekannt."
  ))
  lines.append(l(
    "- If unsure, return the Baseline exactly.",
    "- Wenn unsicher, gib die Baseline exakt zurück."
  ))
  lines.append(l(
    "- Return ONLY the description text.",
    "- Gib NUR den Beschreibungstext zurück."
  ))
  lines.append("")
  lines.append(l("Detected UI element kind: \(kind)", "Erkannter UI-Elementtyp: \(kind)"))
  lines.append(l(
    "Click position: x=\(Int(step.clickXPercent))% y=\(Int(step.clickYPercent))% (from top-left)",
    "Klickposition: x=\(Int(step.clickXPercent))% y=\(Int(step.clickYPercent))% (von oben links)"
  ))
  lines.append(l(
    "Preferred verb: \(localizedVerb(preferredVerb(step, kind: kind)))",
    "Bevorzugtes Verb: \(localizedVerb(preferredVerb(step, kind: kind)))"
  ))
  if let location = location { lines.append(l("Location hint: \(location)", "Ortshinweis: \(location)")) }
  if !label.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
    lines.append(l("Grounding label: \(label)", "Grounding-Label: \(label)"))
  }
  if let ocr = ocr, !ocr.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
    lines.append(l("OCR near click: \(ocr)", "OCR nahe Klick: \(ocr)"))
  }
  lines.append(l("Baseline (safe): \(baseline)", "Baseline (sicher): \(baseline)"))
  lines.append("")
  lines.append(l("Action: \(localizedVerb(actionVerb(step.action)))", "Aktion: \(localizedVerb(actionVerb(step.action)))"))
  lines.append(l("App: \(step.app)", "App: \(step.app)"))
  if !step.windowTitle.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
    lines.append(l("Window title: \(step.windowTitle)", "Fenstertitel: \(step.windowTitle)"))
  }
  if let ax = step.ax {
    lines.append(l("AX role: \(ax.role)", "AX-Rolle: \(ax.role)"))
    if let sub = ax.subrole { lines.append(l("AX subrole: \(sub)", "AX-Unterrolle: \(sub)")) }
    if let desc = ax.roleDescription { lines.append(l("AX role description: \(desc)", "AX-Rollenbeschreibung: \(desc)")) }
    if let ident = ax.identifier { lines.append(l("AX identifier: \(ident)", "AX-Identifier: \(ident)")) }
    lines.append(l("AX label: \(ax.label)", "AX-Label: \(ax.label)"))
    if let containerRole = ax.containerRole { lines.append(l("AX container role: \(containerRole)", "AX-Containerrolle: \(containerRole)")) }
    if let containerSub = ax.containerSubrole { lines.append(l("AX container subrole: \(containerSub)", "AX-Containerunterrolle: \(containerSub)")) }
    if let containerIdent = ax.containerIdentifier { lines.append(l("AX container identifier: \(containerIdent)", "AX-Container-Identifier: \(containerIdent)")) }
    if let windowRole = ax.windowRole { lines.append(l("AX window role: \(windowRole)", "AX-Fensterrolle: \(windowRole)")) }
    if let windowSubrole = ax.windowSubrole { lines.append(l("AX window subrole: \(windowSubrole)", "AX-Fensterunterrolle: \(windowSubrole)")) }
    if let topRole = ax.topLevelRole { lines.append(l("AX top role: \(topRole)", "AX-Toprolle: \(topRole)")) }
    if let topSubrole = ax.topLevelSubrole { lines.append(l("AX top subrole: \(topSubrole)", "AX-Topunterrolle: \(topSubrole)")) }
    if let dialogRole = ax.parentDialogRole { lines.append(l("AX dialog role: \(dialogRole)", "AX-Dialogrolle: \(dialogRole)")) }
    if let dialogSub = ax.parentDialogSubrole { lines.append(l("AX dialog subrole: \(dialogSub)", "AX-Dialogunterrolle: \(dialogSub)")) }
    if let checked = ax.isChecked { lines.append(l("AX checked: \(checked ? "true" : "false")", "AX aktiviert: \(checked ? "true" : "false")")) }
    if ax.isDefaultButton { lines.append(l("AX hint: default button", "AX-Hinweis: Standardbutton")) }
    if ax.isCancelButton { lines.append(l("AX hint: cancel button", "AX-Hinweis: Abbrechen-Button")) }
  }
  if let note = step.note?.trimmingCharacters(in: .whitespacesAndNewlines), !note.isEmpty {
    lines.append(l("User note: \(note)", "Benutzernotiz: \(note)"))
  }
  return lines.joined(separator: "\n")
}
