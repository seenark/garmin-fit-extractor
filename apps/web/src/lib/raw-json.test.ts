import { expect, test } from "bun:test";

import { formatRawJson } from "./raw-json";

test("formatRawJson matches the raw view's visible two-space JSON", () => {
  const records = [
    { kind: "session", fields: [{ name: "sport", value: "running" }] },
  ];

  expect(formatRawJson(records)).toBe(`[
  {
    "kind": "session",
    "fields": [
      {
        "name": "sport",
        "value": "running"
      }
    ]
  }
]`);
});
