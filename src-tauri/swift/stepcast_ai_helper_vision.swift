import Foundation
import Vision
import ImageIO

struct OcrCandidate {
  let text: String
  let confidence: Float
  /// Bounding box in ROI-normalized coordinates (origin bottom-left).
  let bbox: CGRect
}

func loadCGImage(atPath path: String) -> CGImage? {
  let url = URL(fileURLWithPath: path)
  guard let src = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
  return CGImageSourceCreateImageAtIndex(src, 0, nil)
}

func clamp01(_ v: CGFloat) -> CGFloat { max(0.0, min(1.0, v)) }

/// Convert click percent (CSS-style, origin top-left) to Vision normalized coordinates (origin bottom-left).
func clickNormPointVision(_ step: StepInput) -> CGPoint {
  let x = clamp01(CGFloat(step.clickXPercent / 100.0))
  let yTop = clamp01(CGFloat(step.clickYPercent / 100.0))
  return CGPoint(x: x, y: 1.0 - yTop)
}

func hasSidebarSemanticHint(_ step: StepInput) -> Bool {
  guard let ax = step.ax else { return false }
  let sub = (ax.containerSubrole ?? "").lowercased()
  let ident = (ax.containerIdentifier ?? "").lowercased()
  let selfIdent = (ax.identifier ?? "").lowercased()
  return sub.contains("sourcelist")
    || sub.contains("sidebar")
    || ident.contains("sidebar")
    || selfIdent.contains("sidebar")
}

func roiForClick(_ step: StepInput) -> CGRect {
  let click = clickNormPointVision(step)

  if let b = step.ax?.elementBounds {
    let ex = clamp01(CGFloat(b.xPercent / 100.0))
    let eyTop = clamp01(CGFloat(b.yPercent / 100.0))
    let ew = clamp01(CGFloat(b.widthPercent / 100.0))
    let eh = clamp01(CGFloat(b.heightPercent / 100.0))

    // Some AX elements (e.g. list containers) report bounds for the entire
    // scroll area. Using that as OCR ROI makes results worse. Fall back to a
    // click-centered ROI when bounds look like a container.
    let area = ew * eh
    let looksLikeContainer = (ew > 0.85 && eh > 0.40) || area > 0.65
    let role = (step.ax?.role ?? "").lowercased()
    let sub = (step.ax?.subrole ?? "").lowercased()
    let labelNorm = normalizeForMatch(step.ax?.label ?? "")
    let hasVolatileDayLikeLabel =
      labelNorm == "today" || labelNorm == "heute" || labelNorm == "yesterday" || labelNorm == "gestern"
    let menuLikeRole = role.contains("menu")
    let groupWithWeakLabel = role.contains("group") && (labelNorm.isEmpty || hasVolatileDayLikeLabel)
    let avoidElementBounds = menuLikeRole || groupWithWeakLabel

    if !looksLikeContainer && !avoidElementBounds {
    let isTextField = role.contains("textfield") || role.contains("text field") || sub.contains("searchfield")

    // Element bounds are in screenshot percent (origin top-left). Convert to Vision coords (origin bottom-left).
    let ey = clamp01(1.0 - eyTop - eh)

    let padX: CGFloat = 0.04
    let padY: CGFloat = 0.05
    let extraLeft: CGFloat = isTextField ? 0.06 : 0.0

    let x = clamp01(ex - padX - extraLeft)
    let y = clamp01(ey - padY)
    let w = clamp01(ew + padX * 2.0 + extraLeft)
    let h = clamp01(eh + padY * 2.0)

    if w > 0.02 && h > 0.02 {
      // Clamp to image bounds.
      let wClamped = min(w, 1.0 - x)
      let hClamped = min(h, 1.0 - y)
      if wClamped > 0.02 && hClamped > 0.02 {
        return CGRect(x: x, y: y, width: wClamped, height: hClamped)
      }
    }
    }
  }

  // Prefer semantic AX hints; fall back to geometry only if we have no AX metadata.
  let isLeftPane = hasSidebarSemanticHint(step) || (step.ax == nil && click.x < 0.30)
  let w: CGFloat = isLeftPane ? 0.42 : 0.72
  let h: CGFloat = isLeftPane ? 0.50 : 0.28

  let x = isLeftPane ? 0.0 : clamp01(click.x - w / 2.0)
  let y = clamp01(click.y - h / 2.0)

  return CGRect(
    x: min(x, 1.0 - w),
    y: min(y, 1.0 - h),
    width: w,
    height: h
  )
}

func ocrCandidatesNearClick(_ step: StepInput) -> (roi: CGRect, clickImage: CGPoint, clickInRoi: CGPoint, bboxesInImage: Bool, candidates: [OcrCandidate]) {
  guard let path = step.screenshotPath, !path.isEmpty else {
    return (roi: .zero, clickImage: .zero, clickInRoi: .zero, bboxesInImage: false, candidates: [])
  }
  guard FileManager.default.fileExists(atPath: path) else {
    return (roi: .zero, clickImage: .zero, clickInRoi: .zero, bboxesInImage: false, candidates: [])
  }
  guard let cgImage = loadCGImage(atPath: path) else {
    return (roi: .zero, clickImage: .zero, clickInRoi: .zero, bboxesInImage: false, candidates: [])
  }

  let click = clickNormPointVision(step) // image-normalized (origin bottom-left)
  let roi = roiForClick(step)
  let clickInRoi = CGPoint(
    x: roi.width > 0 ? clamp01((click.x - roi.minX) / roi.width) : 0,
    y: roi.height > 0 ? clamp01((click.y - roi.minY) / roi.height) : 0
  )

  let request = VNRecognizeTextRequest()
  request.recognitionLevel = .accurate
  request.usesLanguageCorrection = false
  request.regionOfInterest = roi
  request.minimumTextHeight = 0.008

  let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
  do {
    try handler.perform([request])
  } catch {
    return (roi: roi, clickImage: click, clickInRoi: clickInRoi, bboxesInImage: false, candidates: [])
  }

  let observations = request.results ?? []
  if observations.isEmpty {
    return (roi: roi, clickImage: click, clickInRoi: clickInRoi, bboxesInImage: false, candidates: [])
  }

  var out: [OcrCandidate] = []
  out.reserveCapacity(min(observations.count, 30))

  for obs in observations {
    guard let cand = obs.topCandidates(1).first else { continue }
    let text = cand.string.trimmingCharacters(in: .whitespacesAndNewlines)
    if text.isEmpty { continue }
    out.append(OcrCandidate(text: text, confidence: cand.confidence, bbox: obs.boundingBox))
  }

  // Vision docs say boundingBox is image-normalized, but we have observed ROI-normalized boxes
  // in some configurations. Detect by checking whether many boxes "escape" the ROI bounds.
  let eps: CGFloat = 0.02
  let needsDetection = roi.width > 0.0 && roi.height > 0.0
    && (abs(roi.minX - 0.0) > 0.001 || abs(roi.minY - 0.0) > 0.001 || abs(roi.maxX - 1.0) > 0.001 || abs(roi.maxY - 1.0) > 0.001)
  var bboxesInImage = true
  if needsDetection {
    let escapes = out.filter { c in
      let b = c.bbox
      return b.minX < roi.minX - eps
        || b.minY < roi.minY - eps
        || b.maxX > roi.maxX + eps
        || b.maxY > roi.maxY + eps
    }.count
    // If many escape, treat as ROI-normalized.
    let escapeThreshold = max(2, Int(Double(out.count) * 0.25))
    bboxesInImage = escapes < escapeThreshold
  }

  return (roi: roi, clickImage: click, clickInRoi: clickInRoi, bboxesInImage: bboxesInImage, candidates: out)
}

func looksLikeFileName(_ s: String) -> Bool {
  let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
  if t.count < 3 || t.count > 80 { return false }
  let parts = t.split(separator: ".", omittingEmptySubsequences: false)
  if parts.count < 2 { return false }
  let ext = parts.last ?? ""
  if ext.count < 1 || ext.count > 8 { return false }
  if !ext.allSatisfy({ $0.isLetter || $0.isNumber }) { return false }
  // Reject common context-menu ellipsis items like "Open With…".
  // Keep truncated filenames like "foo...bar.jpeg" valid.
  let lower = t.lowercased()
  if lower.hasSuffix("…") || lower.hasSuffix("...") { return false }
  return true
}

func isMostlyNonLetters(_ s: String) -> Bool {
  let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
  if t.isEmpty { return true }
  var letters = 0
  var digits = 0
  var other = 0
  for ch in t.unicodeScalars {
    if CharacterSet.letters.contains(ch) { letters += 1 }
    else if CharacterSet.decimalDigits.contains(ch) { digits += 1 }
    else if CharacterSet.whitespacesAndNewlines.contains(ch) { continue }
    else { other += 1 }
  }
  if letters == 0 { return true }
  // "Mostly non-letters": more digits+symbols than letters.
  return (digits + other) > letters
}

func isLikelyMetadataValue(_ s: String) -> Bool {
  let t = s.trimmingCharacters(in: .whitespacesAndNewlines)
  if t.isEmpty { return false }

  if t.range(of: #"^\d{1,2}:\d{2}$"#, options: .regularExpression) != nil { return true }
  if t.range(of: #"^\d{1,2}[./-]\d{1,2}[./-]\d{2,4}$"#, options: .regularExpression) != nil {
    return true
  }
  if t.range(of: #"^\d+(?:[.,]\d+)?\s*(?:b|kb|mb|gb|tb|bytes?)$"#, options: [.regularExpression, .caseInsensitive]) != nil {
    return true
  }
  return false
}

func isLikelyFileKindText(_ s: String) -> Bool {
  let t = normalizeForMatch(s)
  if t.isEmpty { return false }

  if t == "folder" || t == "folders"
    || t == "ordner"
    || t == "image" || t == "images"
    || t == "bild" || t == "bilder"
    || t == "video" || t == "videos"
    || t == "film" || t == "filme"
  {
    return true
  }
  if t.hasSuffix("-bild") || t.hasSuffix("-film") || t.hasSuffix("-document") || t.hasSuffix("-dokument") {
    return true
  }
  if t.contains("jpeg-bild") || t.contains("png-bild") || t.contains("webp-bild") || t.contains("mpeg-4-film") {
    return true
  }
  return false
}

func bestOcrLabelNearClick(_ step: StepInput) -> String? {
  let res = ocrCandidatesNearClick(step)
  let candidates = res.candidates
  if candidates.isEmpty { return nil }

  let click = res.bboxesInImage ? res.clickImage : res.clickInRoi

  let axRole = (step.ax?.role ?? "").lowercased()
  let containerRole = (step.ax?.containerRole ?? "").lowercased()
  let isListInteraction = axRole.contains("outline")
    || axRole.contains("table")
    || axRole.contains("list")
    || containerRole.contains("outline")
    || containerRole.contains("table")
    || containerRole.contains("list")
  let avoidSidebarLeak = isListInteraction && step.clickXPercent > 30.0 && !hasSidebarSemanticHint(step)
  let preferRowAligned = isListInteraction
  let pool = candidates

  if preferRowAligned {
    // For list/table rows, the target is usually the row's primary label (often left-most).
    // Prefer row-aligned semantic text over metadata columns (kind/date/size).
    let rowSlack: CGFloat = 0.015
    let rowCandidates = pool.filter { c in
      let b = c.bbox
      return click.y >= b.minY - rowSlack && click.y <= b.maxY + rowSlack
    }

    let semanticRowCandidates = rowCandidates.filter { c in
      let trimmed = c.text.trimmingCharacters(in: .whitespacesAndNewlines)
      if trimmed.isEmpty { return false }
      if isLikelyMetadataValue(trimmed) { return false }
      if isLikelyFileKindText(trimmed) { return false }
      if isLikelyTimestampOrDateLabel(trimmed) { return false }
      if containsLikelyTimestampToken(trimmed) { return false }
      if isLikelyStatusOnlyLabel(trimmed) { return false }
      if trimmed.count <= 2 { return false }
      return true
    }

      if !semanticRowCandidates.isEmpty {
      let noSidebarLeak = semanticRowCandidates.filter { c in
        if !avoidSidebarLeak { return true }
        let trimmed = c.text.trimmingCharacters(in: .whitespacesAndNewlines)
        if !isLikelySidebarLocationLabel(trimmed) { return true }
        return c.bbox.midX >= 0.18
      }

      let effectiveCandidates = noSidebarLeak.isEmpty ? semanticRowCandidates : noSidebarLeak

      // If OCR produced both a clipped and a full variant of the same row label,
      // prefer the longer variant.
      let strengthened = effectiveCandidates.enumerated().map { idx, cand -> (cand: OcrCandidate, strength: Int) in
        let trimmed = cand.text.trimmingCharacters(in: .whitespacesAndNewlines)
        let norm = normalizeForMatch(trimmed)
        var strength = 0
        for (otherIdx, other) in effectiveCandidates.enumerated() {
          if otherIdx == idx { continue }
          let otherNorm = normalizeForMatch(other.text)
          if otherNorm.isEmpty || norm.isEmpty { continue }
          if norm.hasSuffix(otherNorm) && norm.count > otherNorm.count {
            strength += 1
          }
        }
        return (cand: cand, strength: strength)
      }

      let chosen = strengthened.sorted { a, b in
        let ax = abs(a.cand.bbox.midX - click.x)
        let bx = abs(b.cand.bbox.midX - click.x)
        if abs(ax - bx) > 0.08 { return ax < bx }
        if a.strength != b.strength { return a.strength > b.strength }
        let xDelta = abs(a.cand.bbox.minX - b.cand.bbox.minX)
        if xDelta > 0.03 { return a.cand.bbox.minX < b.cand.bbox.minX }
        let wDelta = abs(a.cand.bbox.width - b.cand.bbox.width)
        if wDelta > 0.01 { return a.cand.bbox.width > b.cand.bbox.width }
        if a.cand.text.count != b.cand.text.count { return a.cand.text.count > b.cand.text.count }
        return a.cand.confidence > b.cand.confidence
      }.first

      if let chosen {
        return chosen.cand.text.trimmingCharacters(in: .whitespacesAndNewlines)
      }
    }
  }

  var best: (score: Double, text: String)? = nil

  for c in pool {
    let bbox = c.bbox
    let contains = bbox.contains(click)
    let center = CGPoint(x: bbox.midX, y: bbox.midY)
    let dx = Double(center.x - click.x)
    let dy = Double(center.y - click.y)
    let rowWeight = preferRowAligned ? 6.0 : 1.0
    var dist = dx * dx + (dy * dy * rowWeight)
    if preferRowAligned {
      // Prefer text on the same row band even if the click is on whitespace.
      if click.y >= Double(bbox.minY) && click.y <= Double(bbox.maxY) {
        dist *= 0.35
      }
    }
    if contains { dist *= 0.02 } // strongly prefer the text actually under the click
    let confPenalty = Double(1.0 - c.confidence) * 0.03
    var score = dist + confPenalty

    // Avoid column headers / dates / sizes when we want an item label.
    let trimmed = c.text.trimmingCharacters(in: .whitespacesAndNewlines)
    if isMostlyNonLetters(trimmed) {
      score += 0.06
    }
    if isLikelyMetadataValue(trimmed) {
      score += 0.20
    }
    if isLikelyFileKindText(trimmed) {
      score += 0.30
    }
    if avoidSidebarLeak && isLikelySidebarLocationLabel(trimmed) {
      score += 0.40
    }
    if trimmed.count <= 2 {
      score += 0.05
    }

    if best == nil || score < best!.score {
      best = (score: score, text: c.text)
    }
  }

  return best?.text.trimmingCharacters(in: .whitespacesAndNewlines)
}
