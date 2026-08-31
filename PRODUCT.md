# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Runners who use Garmin watches and want to inspect the detailed activity data captured by their watch.

## Product Purpose

Garmin FIT Extractor accepts Garmin exports so runners can import the files, inspect the activity data in detail, and take the result to an AI assistant for further reading. It exposes both a normalized analysis view and the original raw JSON view.

## Positioning

The product makes detailed watch data portable outside the Garmin website, app, and watch interface. The key handoff is raw JSON that a user can give to ChatGPT or Claude, alongside a normalized view that is easier to scan first.

## Operating Context

A runner exports activity data from the Garmin website, uploads the resulting ZIP file, waits for FIT extraction, then reviews or downloads the normalized and raw JSON results. History provides a place to return to saved extractions.

## Capabilities and Constraints

- Google sign-in protects the authenticated workflow.
- Users can upload 1–10 ZIP files in one batch.
- Each uploaded file can be up to 20 megabytes.
- The system extracts FIT members from the uploaded ZIP files and discards extracted FIT files after processing.
- Each result can expose normalized analysis and raw JSON, and both views can be downloaded.
- Users can review, open, order, and delete saved extraction results in History.
- Existing terminology includes Garmin FIT files, normalized analysis, raw JSON, activity, laps, heart rate, pace, power, cadence, elevation, temperature, and calories.

## Brand Commitments

The product name is Garmin FIT Extractor. The authenticated app uses direct, technical, human-readable language and keeps the existing Catppuccin Latte interface system.

## Evidence on Hand

The repository contains the working upload, extraction, history, normalized-analysis, raw-JSON, and download flows. No customer testimonials, usage counts, performance benchmarks, pricing, or other commercial proof were provided; future surfaces must not invent them.

## Product Principles

- Show the detailed data the watch already captured.
- Keep the raw handoff inspectable and usable by AI assistants.
- Let normalized analysis make a large export easier to read before deeper inspection.
- Explain the workflow with concrete terms instead of vague promises.
- Never fill a proof slot with an invented metric or testimonial.
