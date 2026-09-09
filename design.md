# Design — Runner’s Garage

A shared Hallmark design system for the Runner’s Garage surfaces. Runs and Shoes
are separate product areas inside one practical garage, so they share type,
color, spacing, navigation, and interaction rules without pretending their jobs
are the same.

## Genre

**Modern-minimal technical.** The interface is an instrument panel for a
specific file workflow: direct, quiet, and precise. The visual language uses Catppuccin Latte — a soft lavender-tinted near-white
field, blue-gray ink, and a mauve signal color used sparingly for action, with
blue reserved for focus.

## Macrostructure family

- Homepage: **Garage Entrance**. The homepage names Runner’s Garage in one
  memorable frame, then lets the visitor choose independently between the Runs
  workspace and the Shoes library. It proves the actual product paths without
  inventing metrics, testimonials, or a story that links unrelated jobs.
- Runs pages: **Workbench**. Upload is action-led with a primary intake surface
  and a factual constraints panel. History is data-led with a live record
  count, order control, destructive clear action, and a responsive data surface.
- Shoes pages: **Evidence Workbench**. Catalog pages help a runner find a shoe;
  detail pages lead with the reference comparison task, then expose the evidence
  and chart used to interpret it.

The shared system keeps the shell calm and recognizable. Each product area can
vary its composition within its own task, but neither is allowed to borrow
claims from the other.

## Theme

All production colors live in the root `tokens.css` file and use OKLCH.

- `--color-paper` — Catppuccin Latte base page field
- `--color-paper-2` / `--color-paper-3` — Catppuccin mantle and crust working surfaces
- `--color-surface` / `--color-surface-raised` — quiet raised panels
- `--color-ink` / `--color-ink-2` — primary and secondary text
- `--color-muted` / `--color-muted-2` — explanatory and metadata text
- `--color-rule` / `--color-rule-strong` — visible technical boundaries
- `--color-accent` / `--color-accent-strong` — Catppuccin mauve action signal
- `--color-accent-soft` — restrained mauve tint
- `--color-accent-ink` — text on an accent-filled control
- `--color-focus` — Catppuccin blue keyboard focus ring
- `--color-success` / `--color-success-soft` — extraction success
- `--color-danger` / `--color-danger-soft` / `--color-danger-ink` — errors and permanent actions
- `--color-code` / `--color-code-ink` — raw JSON surface

Accent coverage stays small: active navigation, primary actions, focus, and
status emphasis. Surfaces remain tinted and never use pure black or pure white.

## Typography

- **Display:** Bricolage Grotesque, weights 600–700, roman, tight tracking. Used for
  page headings and major section headings.
- **Body:** IBM Plex Sans, weights 400–600. Used for prose, controls, tables,
  and labels.
- **Technical register:** JetBrains Mono, weights 400–600. Used only for
  metadata, constraints, status labels, timestamps, and raw JSON.

The scale follows a major-third rhythm from `--text-base`; headings use
`--text-display` or the named heading steps. Body copy stays at or above 16px,
with a 45–75 character measure where prose allows. Heading style is always
roman and safe to wrap (`min-width: 0`, `overflow-wrap: anywhere`).

## Spacing

The app uses the named 4-point scale in `tokens.css`, from `--space-3xs` to
`--space-4xl`. Use `gap` for sibling relationships. Page framing uses
`--space-md` through `--space-3xl`; controls use `--space-xs` through
`--space-md`; major page sections use `--space-xl` and above. Do not introduce
one-off spacing values in page CSS.

## Motion

Motion is functional and quiet.

- `--ease-out` enters or settles an element; `--ease-in` exits; `--ease-in-out`
  toggles a state.
- `--dur-micro` handles press feedback; `--dur-short` handles hover and color
  shifts; `--dur-medium` handles surface changes; `--dur-long` is reserved for
  the confirmation overlay.
- Only `transform` and opacity may move. Focus rings appear instantly and are
  never transitioned. There are no bounce, parallax, scroll-linked, or
  celebratory animations.
- Reduced motion removes spatial movement and limits transitions to a short
  opacity/color response.

## Microinteraction rules

- Buttons expose default, hover (fine pointer only), focus-visible, active,
  disabled, loading, error, and success styling hooks via semantic classes or
  `data-state` attributes.
- Touch targets are at least 44px high. Labels, links, tabs, and controls use
  `white-space: nowrap`; parents reflow instead of clickable text wrapping.
- Drag-and-drop keeps a visible idle/dragging status and always has the
  Choose files fallback.
- Upload and extraction success is shown in the result surface; there are no
  success toasts. Errors remain inline, factual, and instructive.
- Deletion is permanent, so the confirmation surface is retained. The dialog
  states exactly what will be removed and offers an explicit cancel action.
- Keyboard focus is visible with a tokenized ring. Hover is never the only
  route to an affordance.

## CTA voice

Primary actions use short, concrete verbs: `เพิ่มข้อมูลวิ่ง`, `Open`, `Compare sizes`,
`Delete`, and `Clear history`. Secondary actions use equally direct labels:
`Choose files`, `Previous`, `Next`, and `Cancel`. Navigation is intentionally
short: `Runs`, `Shoes`, and `เพิ่มข้อมูลวิ่ง`. Avoid hype, invented metrics, and
generic success language.

## Navigation and app chrome

The shared shell uses an N5-inspired detached floating pill: the Runner’s
Garage wordmark and byline, short navigation links, signed-in identity, and
sign-out action stay in one compact shared bar. The primary navigation is
`Runs`, `Shoes`, and `เพิ่มข้อมูลวิ่ง`; there is no separate Home or Garmin/FIT brand
link. At touch widths the bar becomes a two-row grid so labels remain
single-line while all existing functions remain available.

## Responsive allowances

The system is mobile-first and verified at 320, 375, 414, and 768px. Root
`html` and `body` use `overflow-x: clip`. The homepage collapses its garage
workbench and independent product areas into one readable column while keeping
both routes visible. Upload work surfaces collapse from primary surface +
constraints panel to a single column. History keeps a real `table` and real
`td` order in the DOM, then presents each row as a readable stacked card below
the content breakpoint. Shoe detail keeps the size task above evidence and
charts; table cells use `min-width: 0` and long names can break inside their
content without making the document scroll.

App pages may use cards, bordered data surfaces, semantic status badges, and
technical metadata. They may not use decorative imagery, fake browser chrome,
marketing proof bars, invented stats, or unrelated page-level sections.
