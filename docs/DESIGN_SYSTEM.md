# un-git — Surgical Precision, refined

Selected visual direction, September 27, 2026. This document defines presentation for the [technical build plan](BUILD_PLAN.md). The core's operation states, validation results, and publication rules remain authoritative.

## 1. Visual intent

A near-black graphite developer interface with warm off-white typography, restrained mineral-lime accents, subtle cyan tracing lines, compact Git diagrams, and carefully typeset metadata. Premium through spacing, contrast, and precision.

The intended balance is approximately 90% black/graphite/off-white, 6% muted gray, 3% signature lime, and less than 1% cyan/amber/coral. These are visual-weight guidelines, not pixel quotas: dense calendar data and necessary error messages may exceed them. Clarity takes precedence over accent restraint.

- Keep lime localized to a node, primary action, focused element, or small status detail.
- Keep cyan subordinate; use it for an active trace or mapping explanation.
- Use amber and coral when the actual state requires attention, never to make the whole app feel destructive.
- Use thin separators, flat surfaces, crisp type, and deliberate empty space. Avoid neon gradients, luminous edges, glass effects, scanlines, and dashboard ornament.
- Normal cards, buttons, graph nodes, and panels have no glow. An extremely subtle lime radial illumination is permitted in an optional brand/launch composition; it is not required in the working app and adds no marketing-site scope.

## 2. Color tokens

Implement these as semantic CSS custom properties mapped into Tailwind and shadcn/ui. Components consume the semantic names instead of scattering hex literals.

| Token | Value | Intended use |
| --- | --- | --- |
| `background` | `#09090B` | Window canvas |
| `elevated` | `#141416` | Sidebar, toolbar, raised regions |
| `surface` | `#1C1C1F` | Inputs, hover surfaces, dialogs |
| `border` | `#2A2A2F` | Decorative dividers and panel edges |
| `text-primary` | `#F4F4F5` | Main content and headings |
| `text-secondary` | `#A1A1AA` | Readable supporting copy and metadata |
| `muted` | `#64646D` | Nonessential decoration and truly disabled presentation |
| `accent` | `#B8E65C` | Signature mineral lime; primary actions and focused details |
| `accent-deep` | `#789A38` | Subordinate lime strokes or details |
| `trace` | `#55B8C7` | Sparse active paths and mapping hints |
| `warning` | `#D6A84B` | Pending checks, uncertainty, attention |
| `destructive` | `#E36D6D` | Destructive intent, selected attribution edits, errors with text/icon |
| `on-accent` | `#09090B` | Text/icons on filled lime; validate on other fills before reuse |

State uses:

| State/action | Treatment |
| --- | --- |
| Ordinary navigation and secondary actions | Neutral surfaces, off-white labels |
| Safe primary action: inspect, prepare, view report | Lime fill with near-black text; one dominant action per panel |
| Current navigation item | Elevated surface, off-white text, small lime marker |
| Local validation passed | Small lime check plus “Local validation passed” |
| Remote publication verified | Small lime check plus “Repository updated”; separate calendar status |
| Calendar pending or insufficient visibility | Neutral/amber icon plus explicit observation text |
| Publish or restore | Coral outline/text on dark surface, explicit verb and nearby consequence summary |
| Failed, blocked, or unknown remote result | Error/attention icon and text; color follows severity; recovery action remains readable |

Colors decorate explicit state. Lime is not a global success signal; coral alone does not distinguish a pending selection from a failure. No styling may imply that a blocked action is available.

### Contrast rules

Measured from the supplied opaque sRGB colors using relative luminance:

| Pair | Approximate contrast | Decision |
| --- | --- | --- |
| Primary text on surface | 15.47:1 | Main text |
| Secondary text on surface | 6.63:1 | Small metadata and supporting copy |
| Muted on background / surface | 3.40:1 / 2.90:1 | Not meaningful small text, help text, timestamps, or placeholders |
| Border on surface | 1.19:1 | Decorative separation, not the sole control or focus indicator |
| Coral on surface | 5.40:1 | Readable selection/destructive labels |
| Near-black on lime | 13.75:1 | Filled primary action labels |
| Off-white on lime | 1.32:1 | Do not use |

Use at least 4.5:1 for normal text and 3:1 for required non-text control/state indicators against adjacent colors. Preserve the quiet border token, but strengthen interactive boundaries with secondary text, lime, or off-white where needed. Hover and focus details must be tested on their actual composited backgrounds. These are implementation targets, not a claim that an unbuilt UI passes accessibility checks. [Text contrast](https://www.w3.org/WAI/WCAG21/Understanding/contrast-minimum), [non-text contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast)

## 3. Typography

**Geist Sans + Berkeley Mono** is the intended pairing.

| Role | Typeface | Starting specification |
| --- | --- | --- |
| Page title | Geist | 24/30 px, medium weight |
| Section heading | Geist | 16/22 px, medium weight |
| Body, controls, table descriptions | Geist | 14/20 px, regular or medium |
| Small supporting labels | Geist | 12/16 px; secondary text, not muted |
| SHAs, branches, dates, counts, logs, verification details | Berkeley Mono | 12–13/18–20 px; tabular numerals |
| Main numerical summary | Geist or Berkeley Mono by context | 24–28 px; stable alignment and explicit unit |

Use sentence case, restrained weight changes, and clear column alignment. Avoid excessive uppercase labels and tracking. Disable discretionary coding ligatures for hashes, refs, and logs. Truncated SHAs must expose the full value through keyboard-accessible details and copy actions; never copy the abbreviated display when the full ID is expected. Long branch names need inspectable full text.

Bundle fonts locally for offline use; no runtime font CDN requests. Preserve font license notices. Geist Sans and Geist Mono are available from the [official Geist project](https://github.com/vercel/geist-font).

**Berkeley Mono distribution is an explicit packaging dependency.** The foundry currently states that commercial licenses are not compatible with open-source apps and limits commercial usage to UI elements. Resolve the actual distribution model and applicable rights before embedding it in the app or repository. Retain Berkeley Mono as the design target; use bundled Geist Mono as the development/distribution fallback until rights are established, and recheck table metrics with both. Do not purchase a license or download restricted font files as part of plan integration. [Foundry licensing information](https://usgraphics.com/products/berkeley-mono)

## 4. Layout and component vocabulary

Starting dimensions, to refine against the real macOS build:

- 4 px spacing grid; common gaps 8, 12, 16, 24, and 32 px.
- Sidebar around 208–224 px; main content padding 24 px; toolbar around 48 px.
- Main content plus an optional 320–360 px inspector. At narrower widths, collapse the inspector into a sheet and preserve table readability. Verify layouts at 1024×700 and larger, with zoom and long labels.
- Controls around 32–36 px high; table rows around 36–40 px. Compact visuals must still provide usable pointer and keyboard targets.
- Small controls around 6 px radius; panels/dialogs 10–12 px; one-pixel decorative borders. Avoid pill-shaped containers around every item.
- Use a shared line-icon family, normally 16 px with consistent stroke weight. Build Git nodes and the logo as vectors rather than raster screenshots.

Give a page one main working surface. Use section spacing and dividers instead of nesting every metric in another card. Keep critical risks and validation failures in the normal reading flow, not hidden in hover-only affordances. Menus and sheets may use a restrained shadow for separation; no colored glow.

## 5. Signature contribution heatmap

| Data level | Color |
| --- | --- |
| Empty, known zero | `#18181B` |
| Level 1 | `#252B20` |
| Level 2 | `#374629` |
| Level 3 | `#617A3C` |
| Level 4 | `#B8E65C` |

Map GitHub's observed contribution-level categories to these colors. If locally computed bins are ever used, label their scale explicitly. Every legend must state whether it represents total observed contributions or a clearly labeled filtered view. Do not silently switch to commit-only counts.

Use compact square cells with small corner radii and regular gutters. Starting visible cell size: 10–12 px with 3–4 px gutters. Provide month/day orientation, a clear legend, keyboard traversal, an accessible day name/count, and a synchronized list/detail view with larger targets. Tooltips alone are insufficient. The low-intensity colors are intentionally subtle; provide a higher-contrast/patterned data view when distinctions are needed.

### Selection and data truth

- A cell selected for a proposed attribution change gets a **coral outline and diagonal slash over a dark interior**, with a distinct off-white focus ring outside it when focused. Do not fill it solid red.
- Place that treatment in the explicitly labeled **Plan selection** view/layer. Keep the observed snapshot available, keep its count unchanged in storage and detail text, and provide a simple switch back to it.
- User-facing label: “Selected for attribution change.” A tooltip/detail says how many candidate commits are selected and whether the selection is partial. Avoid “scheduled for removal”: selection has not approved publication or guaranteed a calendar change.
- A selected day can contain untouched commits, PRs, reviews, or unresolved contributions. Indicate partial selection with a marker/count; never imply the entire square will vanish.
- Unknown, restricted, and unloaded days use a distinct pattern/label. They do not use the known-zero state.
- After publication, keep the last observed values until a new API snapshot arrives. Any forecast appears separately as an estimate, never as an already-cleaned calendar.
- Preserve identical color meaning across years and screens; selection, focus, hover, loading, and data intensity are separate layers.

## 6. The Disconnect logo and recurring motif

The logo uses a continuing monochrome line and nodes with one small detached lime node. Keep the wordmark and most of the mark monochrome. The lime node supplies recognition; the entire logo is not green.

```text
Connected identity       Detached identity

●──────●──────●           ●─────────────●
                                ◌
```

This is an abstract **attribution** motif. In an actual rewrite visualization, every preserved commit and parent edge remains visible. Represent a changed association with a small detached identity marker beside its commit, and show old/new SHAs for rewritten objects. Never animate a commit disappearing or a DAG bypassing it: metadata rewriting preserves topology.

Use the motif in the app icon/wordmark, empty states, a restrained loading indicator, the attribution explanation, and a brief completion detail. Produce a crisp SVG master, monochrome variant, and macOS icon exports during implementation; inspect them at small sizes. Informational nodes/edges need sufficient contrast, even when decorative logo strokes are quieter.

| Context | Motif behavior |
| --- | --- |
| Empty state | Static line and detached node beside useful next-step text |
| Loading | Small trace movement beside the actual operation label; no invented percent completion |
| Rewrite preview | Attribution marker separates; commit nodes, parent edges, and old/new identity labels stay readable |
| Local validation | Small check beside “Local validation passed”; no remote-success animation |
| Verified publication | One restrained completion transition plus “Repository updated”; calendar pending/uncertain status remains visible |

## 7. Motion and interactions

Use roughly 120–180 ms for hover/focus/surface transitions and 180–240 ms for sheets/details. Keep movement small and easing calm. Avoid perpetual pulsing, bouncing nodes, particle bursts, scanning beams, and success confetti.

Honor reduced-motion preferences: static status icons and immediate state changes preserve the same information. Motion follows core events; animations do not advance operation state, dismiss warnings, trigger publication, or unlock retries. A loading motif never substitutes for a recoverable operation status. After interrupted publication, show the unknown-result state until remote reconciliation completes.

## 8. Screen application

| Existing screen | Visual application |
| --- | --- |
| Connect | Quiet identity panel, small Disconnect mark, lime authorization action, readable permission/access detail |
| Calendar | Graphite-to-lime heatmap as the focal point; modest year title, mono counts, visible freshness/coverage |
| Day details | Structured repository rows and counts with thin dividers; non-commit and unresolved categories remain explicit |
| Repository inspector | Dense, aligned commit table; mono SHA/ref/date columns; selection markers and readable attribution badges |
| Plan and advisor | Large selected-versus-affected counts with full labels, compact truthful DAG, coral selection markers, visible blockers |
| Review and publish | Neutral summary with concrete ref changes, lime validation checks, conspicuous consequence text, coral publish control |
| Operation details | Simple checkpoint timeline; separate Repository and Calendar status rows; clear recovery/report controls |
| Settings | Plain grouped rows for access, backups, storage, and tool versions; no ornamental statistics |

Keep names such as “Selected commits,” “Commit objects rewritten,” “Repository updated,” and “Calendar observation pending” distinct. Full details remain available when hashes or technical information are collapsed for scanning.

## 9. Implementation and acceptance

Build the design system within the existing desktop stack and milestones. Planned ownership:

```text
apps/desktop/src/styles/tokens.css       Palette, semantic states, type, spacing, motion
apps/desktop/src/components/ui/          Shared shadcn/ui theme and interaction states
apps/desktop/src/components/brand/       Disconnect vector and motion variants
apps/desktop/src/features/calendar/      Observed grid and distinct selection layer
apps/desktop/src/features/operations/    Core-state-to-presentation mapping
apps/desktop/src/assets/fonts/           Fonts permitted for redistribution + notices
```

This is a proposed layout, not generated application code. Raw GitHub observations and core operation states keep their existing schemas. Persist any UI preferences separately; style tokens never determine permissions, validation, or eligibility.

Design acceptance criteria accompany the technical release gates:

- Exact palette tokens and five heatmap levels; no ad hoc fluorescent lime or competing cyan primary action.
- Readable real metadata, required control boundaries, visible keyboard focus, and a usable day-list alternative.
- Clear distinctions between known zero, unavailable data, partial selection, planned changes, local validation, verified publication, and unresolved observation.
- Correct commit topology and truthful old/new SHA display during preview and motion.
- No card/button/node glow; restrained accent density in normal use, with adequate prominence for errors.
- Reduced motion, zoom, narrow windows, long refs, font fallback, and missing-font behavior verified in the native app.
- Font redistribution requirements resolved before bundling restricted assets; local/offline typography works.
- Repeated visual review of Connect, Calendar, Inspector, Plan, Publish, and Recovery against one shared component set.

The visual direction is selected. Future mockups should refine this direction and its screen details without reopening competing themes unless requested.
