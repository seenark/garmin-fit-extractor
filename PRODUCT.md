# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Runners who want a practical place for the data and decisions around running: people who use Garmin watches and want to inspect their activity data, and people who want to compare running-shoe fit from reported reviewer evidence.

## Product Purpose

Runner’s Garage is a small collection of useful tools for runners. Its Runs workspace accepts Garmin exports so runners can inspect detailed activity data, while its Shoes library helps them compare size relationships from reviewer evidence and official size charts.

## Positioning

Runner’s Garage keeps two jobs clear rather than pretending they are one workflow: Garmin FIT data becomes portable outside the Garmin website, app, and watch interface; the Shoes library makes cross-shoe sizing evidence easier to inspect. The app does not claim that either tool measures universal fit or replaces a runner’s own judgment.

## Operating Context

For Runs, a runner exports activity data from the Garmin website, uploads the resulting ZIP file, waits for FIT extraction, then reviews or downloads normalized and raw JSON results. History provides a place to return to saved extractions. For Shoes, a runner opens a shoe, chooses a reference model and size, records the fit feeling that matters to them, then reads any available reviewer bridge, consensus, source links, and size-chart context.

## Capabilities and Constraints

- Google sign-in protects the authenticated Runs workflow.
- Users can upload 1–10 ZIP files in one batch.
- Each uploaded file can be up to 20 megabytes.
- The system extracts FIT members from the uploaded ZIP files and discards extracted FIT files after processing.
- Each result can expose normalized analysis and raw JSON, and both views can be downloaded.
- Users can review, open, order, and delete saved extraction results in History.
- The Shoes catalog, reviews, size charts, and comparison logic are static frontend data.
- Shoe comparison reports only the reviewer bridges available in the catalog; it does not invent a bridge when evidence is missing.
- Existing Runs terminology includes Garmin FIT files, normalized analysis, raw JSON, activity, laps, heart rate, pace, power, cadence, elevation, temperature, and calories.

## Brand Commitments

The product name is Runner’s Garage, with the byline `by น้ำเน่ารันคลับ`. The site has two clear product areas: `Runs` for Garmin FIT data and `Shoes` for running-shoe sizing evidence. The interface uses direct, human-readable language and does not use Garmin as the name of the whole product.

## Evidence on Hand

The repository contains the working upload, extraction, history, normalized-analysis, raw-JSON, and download flows. No customer testimonials, usage counts, performance benchmarks, pricing, or other commercial proof were provided; future surfaces must not invent them.

## Product Principles

- Show the detailed data the watch already captured.
- Keep the raw handoff inspectable and usable by AI assistants.
- Let normalized analysis make a large export easier to read before deeper inspection.
- Explain the workflow with concrete terms instead of vague promises.
- Never fill a proof slot with an invented metric or testimonial.
