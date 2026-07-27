# garmin-coach

A small CLI that converts a Garmin FIT activity file into deterministic JSON that is easy for an AI coach to consume.

It does **not** call an LLM and contains **no coaching logic**.

## Requirements

- Node.js 22+
- pnpm 10+

## Install

```bash
pnpm install
pnpm build
pnpm link --global
```

## Usage

```bash
garmin-coach analyze activity.fit
```

This writes `analysis.json` in the current directory.

Choose another output path:

```bash
garmin-coach analyze activity.fit --output runs/2026-07-19.json
```

During development:

```bash
pnpm dev -- analyze activity.fit
```

## Output design

- The root object has a `schemaVersion`.
- Every metric has an explicit unit.
- Missing FIT data is represented as `null`.
- Arrays are always present, even when empty.
- Numbers are rounded consistently.
- JSON keys are emitted in a fixed order.
- Pace uses numeric `seconds_per_kilometer`, which is easier for software and LLMs than formatted strings such as `5:54/km`.

Example excerpt:

```json
{
  "schemaVersion": "1.0.0",
  "source": {
    "fileName": "activity.fit"
  },
  "activity": {
    "type": "running",
    "subType": "road",
    "date": "2026-07-19T00:00:00.000Z"
  },
  "summary": {
    "duration": { "value": 3600, "unit": "seconds" },
    "movingTime": { "value": 3540, "unit": "seconds" },
    "distance": { "value": 10000, "unit": "meters" },
    "calories": { "value": 700, "unit": "kcal" }
  }
}
```

## Commands

```bash
pnpm test
pnpm check
pnpm build
```

## MVP boundaries

This version intentionally does not include:

- training recommendations
- natural-language summaries
- an LLM integration
- a database
- a web UI
- user profiles or custom HR-zone calculation
- record-by-record time series in the JSON

The CLI uses Garmin's official JavaScript FIT SDK to decode files, then maps the decoded session, lap, zone, and record messages into one stable schema.
