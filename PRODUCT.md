# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Runners who want a practical place for the data and decisions around running: people who use Garmin watches and want to inspect their activity data, and people who want to compare running-shoe fit from reported reviewer evidence.

## Product Purpose

Runner’s Garage is a small collection of useful tools for runners. Its Runs workspace makes verified Garmin-running FIT data inspectable and portable through selected privacy-filtered JSON and a manual AI handoff. Its Shoes library helps runners compare size relationships from reviewer evidence and official size charts.

## Positioning

Runner’s Garage keeps two jobs clear rather than pretending they are one workflow: Garmin FIT data becomes portable outside the Garmin website, app, and watch interface; the Shoes library makes cross-shoe sizing evidence easier to inspect. The app does not claim that either tool measures universal fit or replaces a runner’s own judgment.

## Operating Context

For Runs, a runner signs in with Google, uploads direct FIT files or ZIP exports, reviews per-file outcomes and processing state, then inspects activity history, linked Pace/HR/Power charts, recorded laps, and detected segments. The runner selects one or many ready activities and copies or downloads Coach JSON or Full JSON. Editable ChatGPT and Claude prompts are copied separately; the runner decides what to share externally. There is no runtime LLM request or AI API key.

For Shoes, a runner opens a shoe, chooses a reference model and size, records the fit feeling that matters to them, then reads any available reviewer bridge, consensus, source links, and size-chart context.

## Capabilities and Constraints

- Google sign-in protects the authenticated Runs workflow.
- Users can upload 1–10 direct FIT or ZIP files per batch. Source support requires valid CRC, a verified Garmin manufacturer, and exactly one running session. Unsupported manufacturers, sports, and session layouts receive explicit reasons; valid siblings can succeed independently.
- Original FIT bytes are immutable private sources. Original ZIP archives are not retained. Decoded archives, normalized data, analysis, and historical evidence have separate revisions with one coherent published manifest.
- Stable activity IDs survive reprocessing. Durable jobs expose queued, processing, ready, or failed states separately from stale last-good results and experimental analysis availability.
- History uses activity event time rather than upload time. Late imports, changed evidence, and deletion invalidate affected historical results; checkbox selection never determines automatic history evidence.
- Permanent activity deletion erases source and derived storage, cancels work, and revokes affected exports. Already downloaded or externally shared data cannot be recalled.
- Coach JSON discloses boundary-preserving aggregation. Full JSON retains decoder-supported fields and full-resolution samples subject to the same privacy policy. Both modes have separate Copy and browser-native Download actions using identical pinned pretty JSON bytes, not a truncated preview.
- Export snapshots require the exact complete owner-scoped ready selection, pin revisions and privacy flags, and expire after 15 minutes. Deletion revokes snapshots; reprocessing does not rewrite their pinned bytes. Clipboard or transfer failure never means success.
- Both location and device identifiers are omitted by default, with independent opt-ins. Unknown or unclassified native/developer fields remain omitted even with both opt-ins. Safe omission metadata explains filtering without revealing hidden values.
- Exports never include original FIT/base64, filenames, private paths, account data, unselected activity payloads or raw evidence contributors, or the account-wide trend. “Full” does not mean unfiltered or anonymized.
- Source-less legacy rows retain stable IDs and safe summaries with fidelity warnings. They do not have a fabricated original, strict-v2 normalized detail, reprocess support, or Full/Coach export support. Obsolete browser extraction APIs are retired, not privacy bypasses.
- LT1/LT2 labels represent separate experimental DFA-a1 VT1/VT2 proxies, not validated blood-lactate thresholds. Native RR alignment and observed evidence are required; HR-derived RR, fixed target ratios, invented maximal intent, and extrapolation are not substitutes.
- Each target can estimate, report low confidence, or abstain with actual reasons and lineage. Engine availability, processing failure, and freshness are distinct. Device-reported thresholds remain separate from independent estimates.
- Historical estimates use event-time cutoffs, seven-day recency, and distinct observation groups. Latest LT1 and LT2 values retain their original dates independently when a newer attempt estimates only one target.
- Linked charts preserve missing-data and pause gaps; display sampling does not change stored samples or numerical input. Recorded laps remain separate from detected segments.
- Numeric PNG cards offer three layouts, square/portrait sizes, and Light/Dark/Transparent themes without requiring a photo. Optional raster photos stay in the browser; exported PNG pixels do not carry original EXIF. Prompt text is not embedded in JSON or PNG.
- The Shoes catalog, reviews, size charts, and comparison logic are static frontend data.
- Shoe comparison reports only the reviewer bridges available in the catalog; it does not invent a bridge when evidence is missing.
- The CLI `garmin-coach analyze` schema 1.0.0, FIT Coach OAuth activity contract, Google sessions, Shoes, and transcript administration remain protected boundaries outside the Runs v2 export contract.

## Brand Commitments

The product name is Runner’s Garage, with the byline `by น้ำเน่ารันคลับ`. The site has two clear product areas: `Runs` for Garmin FIT data and `Shoes` for running-shoe sizing evidence. The interface uses direct, human-readable language and does not use Garmin as the name of the whole product.

## Evidence on Hand

Decoder, analysis, and web implementations are merged into the integration branch. Core persistence/API implementation is still on its unmerged branch. This document describes the frozen cutover contract, not a released or fully verified deployment. Final combined runtime, browser, recovery, and full-suite evidence is still outstanding; worker-scoped checks do not prove these workflows.

Numerical synthetic fixtures and independent mathematical comparators can establish software correctness, not physiological validity. Licensed paired human running RR with independent gas-exchange references, participant-level validation, and empirical uncertainty remain required for validated VT claims; validated lactate claims require paired running lactate references. Quantified segment-detection accuracy also requires independent human annotations.

No production deployment, reset, or legacy migration apply is claimed. No customer testimonials, usage counts, production performance guarantees, pricing, or other commercial proof were provided; product surfaces must not invent them.

## Product Principles

- Show the detailed data the watch already captured.
- Keep the selected, privacy-filtered handoff inspectable and usable by AI assistants.
- Keep recorded data, normalization, independent analysis, and experimental estimates distinct.
- Preserve originals and explain missing evidence rather than manufacturing values.
- Explain the workflow with concrete terms instead of vague promises.
- Never fill a proof slot with an invented metric or testimonial.
