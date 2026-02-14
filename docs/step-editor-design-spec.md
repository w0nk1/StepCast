# Step Editor Design Spec

## Research Date: 2026-02-09

## Current State

The editor (`editor.css` + `EditorStepCard.tsx` + `EditorWindow.tsx`) already has a solid foundation:
- CSS custom properties for light/dark theming
- 12px border-radius cards with `var(--border)` borders
- SF Pro system font stack
- 900px max-width centered layout
- Correct accent color (`#7C5CFC` / `#A78BFA`)

What follows are concrete, high-impact upgrades derived from analyzing CleanShot X, Scribe, Tango, Linear, Notion, and Apple HIG patterns.

---

## 1. Layout Pattern: Timeline + Cards (Hybrid)

**What the best tools do:**
- Scribe/Tango: Vertical card stack, each card = one step (screenshot + number + description + note)
- Linear: Single-column list of dense, aligned cards with 8px spacing scale
- CleanShot X: Floating annotation overlay with numbered step counters

**Recommendation — "Timeline Cards":**

Add a thin vertical timeline connector on the left side of the step list. Each step card gets a small numbered badge that sits on the timeline line. This creates visual flow and progression without the complexity of a full alternating timeline.

```
  [1]---[ Step Card: header / note / screenshot ]
   |
  [2]---[ Step Card: header / note / screenshot ]
   |
  [3]---[ Step Card: header / note / screenshot ]
```

### CSS Implementation

```css
/* Timeline container */
.editor-steps {
  display: flex;
  flex-direction: column;
  gap: 0;                          /* remove gap, use padding instead */
  position: relative;
  padding-left: 48px;              /* space for timeline + badge */
}

/* Vertical connector line */
.editor-steps::before {
  content: '';
  position: absolute;
  left: 19px;                      /* center of 40px badge area */
  top: 0;
  bottom: 0;
  width: 2px;
  background: var(--border);
  border-radius: 1px;
}

/* Each step card wrapper */
.editor-step {
  position: relative;
  margin-bottom: 20px;
}

/* Step number badge on the timeline */
.editor-step-badge {
  position: absolute;
  left: -48px;
  top: 14px;                       /* align with header center */
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--accent);
  color: #fff;
  font-size: 12px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1;
  box-shadow: 0 0 0 4px var(--bg-primary);  /* "cut through" the line */
}
```

**Why this works:** Creates visual progression (users can scan step numbers instantly), feels like a professional documentation tool (Scribe/Tango pattern), and the connecting line gives a sense of flow. The `box-shadow` trick on the badge creates a clean "punch through" effect on the timeline line without extra markup.

---

## 2. Screenshot Display

**What the best tools do:**
- CleanShot X: `border-radius: 10px`, `box-shadow: 0 10px 30px rgba(0,0,0,0.1)`, screenshots sit on a subtle background
- Tango: Screenshots have a thin border + slight shadow, displayed at full card width
- Scribe: Screenshots are full-bleed within the card, annotation highlights drawn on top
- Linear: Not screenshot-heavy, but media always has consistent rounding

**Recommendation — "Elevated Screenshot":**

```css
.editor-step-image {
  padding: 16px;
  background: var(--bg-secondary);
  border-radius: 0 0 12px 12px;       /* match card bottom corners */
}

.editor-image-wrapper img {
  display: block;
  max-width: 100%;
  height: auto;
  border-radius: 8px;                  /* rounded screenshot corners */
  border: 1px solid var(--border);     /* thin frame */
  box-shadow:
    0 2px 8px rgba(0, 0, 0, 0.06),
    0 8px 24px rgba(0, 0, 0, 0.04);   /* layered soft shadow */
}

/* Dark mode: deeper shadow */
@media (prefers-color-scheme: dark) {
  .editor-image-wrapper img {
    box-shadow:
      0 2px 8px rgba(0, 0, 0, 0.2),
      0 8px 24px rgba(0, 0, 0, 0.15);
    border-color: rgba(255, 255, 255, 0.06);
  }
}
```

**Key details:**
- 8px border-radius on screenshots (not too rounded, not sharp)
- Layered box-shadow (two layers = more depth than a single shadow)
- 1px border gives definition, especially in light mode where shadows are subtle
- The `bg-secondary` padding area creates a "stage" for the screenshot (CleanShot X pattern)

---

## 3. Step Number / Header Design

**What the best tools do:**
- Scribe: Numbered circular badges (purple/blue accent) with bold step text beside them
- Tango: Step numbers in colored pills, description in regular weight below
- CleanShot X Counter tool: Numbered circles with white text on colored background
- Linear: No step numbers, but uses pill-shaped status badges with subtle backgrounds

**Recommendation — Two options (pick one):**

### Option A: Accent Badge (Scribe-style) — Recommended

Already described in the timeline section above. The numbered circle sits on the timeline. Inside the header, remove the redundant "Step N" text and instead show just the action description.

```css
.editor-step-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 16px;
  background: var(--bg-primary);         /* same as card, not secondary */
  border-bottom: 1px solid var(--border);
}

/* Remove old step-number styling; number is now in the badge */
.editor-step-desc {
  font-size: 14px;                       /* bump up slightly */
  font-weight: 600;
  color: var(--text-primary);
  line-height: 1.3;
}
```

### Option B: Inline Pill Badge (Linear-style)

If you prefer no timeline, keep the step number inline but style it as a colored pill:

```css
.editor-step-number {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 24px;
  height: 22px;
  padding: 0 8px;
  border-radius: 6px;
  background: rgba(124, 92, 252, 0.12);  /* accent at 12% opacity */
  color: var(--accent);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

/* Dark mode */
@media (prefers-color-scheme: dark) {
  .editor-step-number {
    background: rgba(167, 139, 250, 0.15);
  }
}
```

**Option A is recommended** because it matches the timeline pattern and creates a more distinctive, premium feel. Option B is simpler to implement if you skip the timeline.

---

## 4. Notes / Annotations Area

**What the best tools do:**
- Scribe: Inline text below screenshot, editable on click, with a subtle dashed border placeholder
- Tango: Description text sits above the screenshot, editable inline
- Notion: Inline editing with a subtle placeholder that says "Empty. Click to edit."
- Linear: Descriptions use a muted secondary color, become editable with a single click

**Recommendation — "Refined Inline Note":**

Your current implementation is already close. Key refinements:

```css
.editor-step-note-row {
  padding: 10px 16px 12px;
  border-bottom: 1px solid var(--border);
}

.editor-step-note-btn {
  display: block;
  width: 100%;
  text-align: left;
  padding: 8px 12px;
  border: 1px dashed transparent;        /* invisible border by default */
  border-radius: 8px;
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  font-family: inherit;
  line-height: 1.5;
  cursor: pointer;
  transition: all 0.15s ease;
  -webkit-app-region: no-drag;
}

/* Placeholder state */
.editor-step-note-btn:not(.has-note) {
  font-style: italic;
  opacity: 0.6;
}

/* Has a note — show it as normal text */
.editor-step-note-btn.has-note {
  color: var(--text-primary);
  opacity: 1;
}

/* Hover reveals the edit affordance */
.editor-step-note-btn:hover {
  border-color: var(--border);
  background: var(--bg-secondary);
}

.editor-step-note-input {
  display: block;
  width: 100%;
  padding: 8px 12px;
  border: 1px solid var(--accent);
  border-radius: 8px;
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  line-height: 1.5;
  resize: vertical;
  outline: none;
  box-shadow: 0 0 0 3px rgba(124, 92, 252, 0.1);  /* focus ring */
}
```

**What makes this feel polished:**
- No visible border until hover (Notion pattern — clean by default)
- Subtle background shift on hover signals "this is editable"
- Focus ring using box-shadow (not outline) for consistent cross-browser appearance
- Italic + reduced opacity for empty placeholder state (visual hierarchy)

---

## 5. Card Container Refinements

**What the best tools do:**
- CleanShot X: `border-radius: 10px`, light shadows
- Linear: 8px spacing scale, minimal borders, increased contrast via subtle elevation
- Modern cards (2025): 12-16px border-radius, layered shadows, 1px borders at 10% opacity

**Recommendation:**

```css
.editor-step {
  border: 1px solid var(--border);
  border-radius: 14px;                   /* slightly larger for premium feel */
  overflow: hidden;
  background: var(--bg-primary);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);  /* minimal resting elevation */
  transition: box-shadow 0.2s ease, border-color 0.2s ease;
}

.editor-step:hover {
  box-shadow:
    0 2px 8px rgba(0, 0, 0, 0.06),
    0 8px 24px rgba(0, 0, 0, 0.03);
  border-color: rgba(0, 0, 0, 0.15);     /* slightly stronger border */
}

/* Dark mode hover */
@media (prefers-color-scheme: dark) {
  .editor-step:hover {
    box-shadow:
      0 2px 8px rgba(0, 0, 0, 0.15),
      0 8px 24px rgba(0, 0, 0, 0.1);
    border-color: rgba(255, 255, 255, 0.12);
  }
}
```

**Why:** Hover elevation change gives cards a feeling of interactivity and depth. The transition duration (0.2s) is fast enough to feel responsive but smooth enough to look intentional.

---

## 6. What Makes It Feel "Modern and Polished"

Synthesized from all sources:

### Typography
- **Use Inter Display or SF Pro Display for the page title** (22px, weight 700). You already use SF Pro via `-apple-system` — good.
- **Body text at 13-14px** with 1.4-1.5 line-height. Your current 13px is correct.
- **Step descriptions at 14px weight 600** — slightly larger than body to create hierarchy.
- **Secondary text at reduced opacity** rather than a completely different color (Linear pattern: use 60-75% opacity of primary text).

### Spacing
- **8px spacing scale** (Linear's approach): 8, 12, 16, 24, 32, 48. You're already mostly aligned.
- **24px gap between cards** (upgrade from current 20px for slightly more breathing room).
- **16px internal padding** on all card sections — consistent horizontal rhythm.

### Color
- **Neutral-first palette** — you already have this with `--text-secondary` and `--bg-secondary`.
- **Accent color used sparingly** — only for badges, focus states, and primary actions.
- **Avoid pure black text on white** — your `#1d1d1f` is perfect (Apple's approach).
- **Dark mode: increase shadow intensity** 2-3x, reduce border opacity. You're doing this.

### Micro-interactions
- All interactive elements should have `transition: all 0.15s ease` (you already do this).
- Consider adding a very subtle `transform: translateY(-1px)` on card hover for "lift" effect.
- Focus states should use `box-shadow` focus rings, not browser outlines (already in place).

### Overall "Premium" Signals
1. **Consistent border-radius**: Pick 8, 12, or 14px and use it EVERYWHERE. Mixed radii feel cheap.
2. **Layered shadows** (two `box-shadow` values): Creates more realistic depth than a single shadow.
3. **Subtle background differentiation**: `--bg-secondary` for screenshot areas, `--bg-primary` for text areas.
4. **Generous whitespace**: Better to have too much padding than too little.
5. **No hard borders between sections** where possible — use background color changes instead.

---

## 7. Dark Mode Considerations

Based on Apple HIG and Linear's approach:

| Element | Light | Dark |
|---------|-------|------|
| Card background | `#ffffff` | `#1c1c1e` (current) |
| Screenshot stage | `#f5f5f7` | `#2c2c2e` (current) |
| Border | `rgba(0,0,0,0.1)` | `rgba(255,255,255,0.1)` (current) |
| Shadow intensity | 4-6% opacity | 15-20% opacity |
| Badge text on accent | `#fff` | `#fff` (stays) |
| Accent color | `#7C5CFC` | `#A78BFA` (lighter — current) |

Your current dark mode is well-considered. The one upgrade:

```css
/* Dark mode: add a very subtle inner glow to cards for definition */
@media (prefers-color-scheme: dark) {
  .editor-step {
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.04),  /* top edge highlight */
      0 1px 3px rgba(0, 0, 0, 0.2);
  }
}
```

This `inset` top-edge highlight is a technique Apple uses in macOS system controls and Linear uses in its cards. It gives a subtle "glass edge" effect in dark mode that prevents cards from looking flat.

---

## 8. Priority Implementation Order (Biggest Impact First)

1. **Screenshot shadow + border** (5 min) — Instant premium feel
2. **Step number pill/badge styling** (10 min) — Visual identity for steps
3. **Card hover elevation** (5 min) — Interactivity signal
4. **Note area refinement** (10 min) — Clean placeholder + focus ring
5. **Timeline connector** (20 min) — Requires small React changes + CSS
6. **Dark mode inset highlights** (5 min) — Polish
7. **Typography fine-tuning** (5 min) — Step description weight/size bump

---

## Sources

- [CleanShot X Features](https://cleanshot.com/features)
- [How We Redesigned Linear UI](https://linear.app/now/how-we-redesigned-the-linear-ui)
- [10 Card UI Design Examples 2025](https://bricxlabs.com/blogs/card-ui-design-examples)
- [Linear Design: The SaaS Design Trend](https://blog.logrocket.com/ux-design/linear-design/)
- [Accessible Linear Design Dark/Light Modes](https://blog.logrocket.com/how-do-you-implement-accessible-linear-design-across-light-and-dark-modes)
- [Scribe Step-by-Step Guides](https://scribe.com)
- [Tango Guide Editor](https://www.tango.ai/)
- [Apple Dark Mode HIG](https://developer.apple.com/design/human-interface-guidelines/dark-mode)
- [Flowbite Timeline Component](https://flowbite.com/docs/components/timeline/)
- [Vertical Timelines with Tailwind CSS](https://cruip.com/3-examples-of-brilliant-vertical-timelines-with-tailwind-css/)
- [20 Modern UI Design Trends 2025](https://medium.com/@baheer224/20-modern-ui-design-trends-for-developers-in-2025-efdefa5d69e0)
- [Notion Gallery View Guide](https://super.so/blog/notion-gallery-view-a-comprehensive-guide)
