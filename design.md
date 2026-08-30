# Design — Garmin FIT Extractor

A shared Hallmark design system for the authenticated app surfaces. Upload and
history are one product, so they share type, color, spacing, navigation, and
interaction rules rather than rotating through separate page themes.

## Genre

**Modern-minimal technical.** The interface is an instrument panel for a
specific file workflow: direct, quiet, and precise. The visual language uses Catppuccin Latte — a soft lavender-tinted near-white
field, blue-gray ink, and a mauve signal color used sparingly for action, with
blue reserved for focus.

## Macrostructure family

- Marketing pages: not applicable to this authenticated app.
- App pages: **Workbench**. Upload is action-led with a primary intake surface
  and a factual constraints panel. History is data-led with a live record
  count, order control, destructive clear action, and a responsive data surface.
- Content pages: not applicable.

App pages may vary their composition within Workbench, but they do not add
invented stats, decorative imagery, or promotional sections. Function carries
the page.

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

Primary actions use short, concrete verbs: `Upload ZIP files`, `Open`,
`Delete`, and `Clear history`. Secondary actions use equally direct labels:
`Choose files`, `Previous`, `Next`, and `Cancel`. Navigation stays short:
`Upload`, `History`, and `Sign out`. Avoid hype, invented metrics, and generic
success language.

## Navigation and app chrome

The authenticated shell uses an N5-inspired detached floating pill: wordmark,
short navigation links, signed-in identity, and sign-out action stay in one
compact shared bar. At touch widths the bar becomes a two-row grid so labels
remain single-line while all existing functions remain available. No fake links
or decorative footer are added.

## Responsive allowances

The system is mobile-first and verified at 320, 375, 414, and 768px. Root
`html` and `body` use `overflow-x: clip`. Upload work surfaces collapse from
primary surface + constraints panel to a single column. History keeps a real
`table` and real `td` order in the DOM, then presents each row as a readable
stacked card below the content breakpoint. Activity date and Activity type cells remain
visible on mobile. Table cells use `min-width: 0` and long filenames can break
inside their content without making the document scroll.

App pages may use cards, bordered data surfaces, semantic status badges, and
technical metadata. They may not use decorative imagery, fake browser chrome,
marketing proof bars, invented stats, or unrelated page-level sections.
