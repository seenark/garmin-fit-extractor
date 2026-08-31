---
version: 1
slug: "apps-web-src-routes-index-tsx"
primary_target: "apps/web/src/routes/index.tsx"
related_targets: ["apps/web/src/routes/upload.tsx","apps/web/src/routes/history.tsx"]
---

## Scope
Authenticated product homepage at `/`, shown after login before Upload or History. Visitor mode: persuade, then route.

## Audience and job
A runner who uses a Garmin watch and has exported activity data from Garmin’s website. They need to understand what this app does, why the detailed data is useful, and where to start.

## Action and proof
Primary action: go to Upload. Secondary action: go to History. The proof is the real product path: Garmin ZIP → FIT extraction → normalized analysis and raw JSON → copy/download for ChatGPT or Claude. Do not invent metrics, testimonials, customer logos, or integrations.

## Constraints
Keep the authenticated shell, Catppuccin Latte system, existing API behavior, selectors, and accessibility contracts. Keep copy direct and use the existing product terms. The page must work at 320, 375, 414, 768, and desktop widths without horizontal overflow.

## Chosen direction
Narrative Workflow with a split hero: a left-biased product statement and explicit actions beside a data-handoff diagram. Follow with numbered stages and a closing CTA. Memorable moment: one export becomes two useful reading paths — normalized analysis and raw JSON for an AI assistant.

## Unresolved decisions
No customer proof or numeric product claims are available. Keep the homepage product-led until real evidence is supplied.
