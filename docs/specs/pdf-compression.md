# PDF Compression Research Spec

Date: 2026-02-08

## Context

StepCast exports HTML-to-PDF via `WKWebView.createPDF()`. Screenshots are embedded as base64 data URIs (WebP with PNG fallback). Resulting PDFs can be very large due to embedded screenshot images.

## Current Pipeline

```
PNG screenshot → WebP/PNG (Rust image crate) → base64 data URI in HTML → WKWebView.createPDF() → PDF bytes → disk
```

Key file: `src-tauri/src/export/pdf.rs` (WKWebView), `src-tauri/src/export/helpers.rs` (image optimization).

---

## Finding 1: WKWebView.createPDF() Has No Compression Controls

`WKWebView.createPDF(configuration:completionHandler:)` accepts only `WKPDFConfiguration` which exposes a single property: `rect` (the area to capture). There is **no way to control image compression quality, DPI, or encoding** within the `createPDF` API itself.

**Conclusion:** Pre-processing (before PDF generation) or post-processing (after PDF generation) is required.

## Finding 2: WebP in PDF = Disaster

The PDF specification (ISO 32000) does **not** support WebP as an image format. Supported formats are:
- JPEG (DCTDecode filter)
- JPEG2000 (JPXDecode, PDF 1.5+)
- JBIG2 (JBIG2Decode)
- Raw/Flate-compressed bitmap

When WKWebView encounters a WebP data URI in HTML and generates a PDF, it must **re-encode** the WebP image — likely to a lossless format (PNG-like flate stream). This means:
1. The WebP compression benefit is completely lost
2. The PDF ends up with a large flate-compressed bitmap instead of a compact lossy format
3. Using JPEG data URIs instead would let the PDF engine pass through the JPEG data directly (DCTDecode)

**This is likely the single biggest factor in PDF bloat.**

## Finding 3: PDFKit Write Options (macOS 13+)

Apple introduced `PDFDocumentWriteOption` in macOS Ventura (13) / iOS 16 via WWDC22:

```swift
let options: [PDFDocumentWriteOption: Any] = [
    .saveAllImagesAsJPEGOption: true,      // Re-encodes all images as JPEG
    .optimizeImagesForScreenOption: true,   // Downsamples to HiDPI screen resolution
    .createLinearizedPDFOption: true,       // Web-optimized page ordering
]
let data = pdfDocument.dataRepresentation(options: options)
```

Available via Rust: `objc2-pdf-kit` crate (add `PDFDocument` and `PDFDocumentWriteOption` features).

**This is the most promising post-processing approach.** Load the raw PDF bytes from `createPDF()`, open as `PDFDocument`, and re-save with these options.

## Finding 4: Quartz Filters (macOS 10.4+)

macOS has a `QuartzFilter` class (in the Quartz framework) that can compress PDFs. The built-in "Reduce File Size" filter at `/System/Library/Filters/Reduce File Size.qfilter` is too aggressive (downsamples to 512px).

Custom `.qfilter` files are XML plists with this structure:

```xml
<dict>
  <key>FilterData</key>
  <dict>
    <key>ColorSettings</key>
    <dict>
      <key>ImageSettings</key>
      <dict>
        <key>Compression Quality</key>
        <real>0.75</real>
        <key>ImageCompression</key>
        <string>ImageJPEGCompress</string>
        <key>ImageScaleSettings</key>
        <dict>
          <key>ImageResolution</key>
          <integer>150</integer>
          <key>ImageSizeMax</key>
          <integer>0</integer>
        </dict>
      </dict>
    </dict>
  </dict>
  <key>FilterType</key>
  <integer>1</integer>
  <key>Name</key>
  <string>Custom Reduce</string>
</dict>
```

Applied via:
```swift
let filter = QuartzFilter.quartzFilter(withURL: filterURL)
let doc = PDFDocument(url: pdfURL)
doc.write(toFile: path, withOptions: ["QuartzFilter": filter])
```

**Downside:** `QuartzFilter` is in the Quartz framework, not CoreGraphics or PDFKit. The `quartzFilterWithURL:` method is poorly documented. The `objc2` ecosystem does not have a `objc2-quartz` crate — would need raw FFI. More fragile than the PDFKit approach.

## Finding 5: CoreGraphics Manual PDF Rewrite

The "nuclear option": parse the PDF with `CGPDFDocument`, iterate pages, render each page to a bitmap via `CGBitmapContext`, compress as JPEG, and write into a new `CGPDFContext`.

```
PDF bytes → CGPDFDocument → for each page:
  CGBitmapContext (render page) → CGImage → JPEG data → draw into new CGPDFContext
→ new PDF
```

**Downsides:**
- Rasterizes vector content (text becomes pixels)
- Lossy for everything, not just images
- Complex implementation
- The `core-graphics` 0.24 crate already in deps doesn't expose PDF context creation APIs well

**Not recommended** unless all other approaches fail.

---

## Recommended Strategy (Ranked)

### Approach A: JPEG Data URIs (Pre-processing) — HIGH IMPACT, LOW EFFORT

Change `load_screenshot_optimized()` to produce JPEG instead of WebP when the export target is PDF. Use the `image` crate (already a dependency) to encode as JPEG with quality ~75-85.

Since PDF natively supports JPEG (DCTDecode), WKWebView can pass through the JPEG data directly without re-encoding, resulting in dramatically smaller PDFs.

**Implementation:**
1. Add a `for_pdf: bool` parameter or a separate function
2. When `for_pdf`, encode as JPEG with configurable quality (default 0.80)
3. Use `data:image/jpeg;base64,...` in the HTML

**Estimated size reduction:** 60-80% vs current WebP→PDF pipeline.

### Approach B: PDFKit Post-Processing — MEDIUM IMPACT, MEDIUM EFFORT

After `createPDF()` returns raw bytes, load into `PDFDocument` and re-save with `saveAllImagesAsJPEGOption` + `optimizeImagesForScreenOption`.

**Implementation:**
1. Add `objc2-pdf-kit` dependency with `PDFDocument` + `PDFDocumentWriteOption` features
2. After getting PDF data from WKWebView callback, create `PDFDocument(data:)`
3. Call `dataRepresentationWithOptions()` with compression options
4. Write the optimized data to disk

**Requires:** macOS 13+ (already the minimum for Tauri 2).

### Approach C: Combine A + B — MAXIMUM IMPACT

Use JPEG data URIs (Approach A) to avoid the WebP re-encoding penalty, then optionally apply PDFKit optimization (Approach B) to further compress and downsample.

### Approach D: Quartz Filter — BACKUP ONLY

Use only if PDFKit options prove insufficient. Requires raw FFI to the Quartz framework. More maintenance burden.

---

## Decision

**Implemented: Approach C (Combine A + B).**

1. **Approach A (JPEG data URIs)** — already shipped. `load_screenshot_optimized(ImageTarget::Pdf)` encodes as JPEG instead of WebP, so WKWebView can DCTDecode pass-through.

2. **Approach B (PDFKit post-processing)** — added as best-effort optimization after `createPDF()`. Uses `PDFDocument.dataRepresentationWithOptions` with:
   - `PDFDocumentSaveImagesAsJPEGOption` — re-encodes any remaining non-JPEG images
   - `PDFDocumentOptimizeImagesForScreenOption` — downsamples to HiDPI screen resolution
   - Best-effort fallback: if PDFKit optimization fails for any reason, the original WKWebView PDF bytes are written to disk unchanged.

**Note:** `createLinearizedPDFOption` is not exposed in `objc2-pdf-kit` bindings — skipped. Requires macOS 13+ (already Tauri 2 minimum).
