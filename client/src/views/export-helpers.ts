// Shared, view-agnostic export helpers — the "sin lock-in" plumbing (ADR-0005
// invariant #4: the owner is the owner of their data, exportable as open CSV/JSON
// with no proprietary format). One place every view builds + triggers a download
// from, so a new surface gets export in a few lines and the escaping/encoding
// rules stay in a single tested spot.
//
// Reuses the RFC-4180 CSV primitives already proven by the inventory/reports
// exports (toCsv/csvField/exportFilename in stock-helpers) — this module adds the
// generic column-driven builder + the browser download trigger on top.
import { toCsv, exportFilename } from "./stock-helpers";

/** One CSV/JSON column: the es-CL `header` the operator reads, the machine `key`
 *  the JSON round-trips under, and `value` extracting the cell from a row. Money
 *  values must be returned as their raw Decimal STRING (never locale-formatted)
 *  so the file re-imports losslessly. */
export interface ExportColumn<T> {
  header: string;
  key: string;
  value: (row: T) => unknown;
}

/** A built export ready to download: the CSV + pretty JSON strings and the row
 *  `count`. Mirrors stock-helpers' ExportBundle shape (minus `truncated`, which
 *  only the capped catalog export needs). */
export interface ExportData {
  csv: string;
  json: string;
  count: number;
}

/** Build a CSV (es-CL headers) + pretty JSON (stable machine keys) from a column
 *  spec + rows. Pure + DOM-free → unit-testable; the view layers the download on
 *  top via {@link downloadExport}. */
export function buildExport<T>(columns: readonly ExportColumn<T>[], rows: readonly T[]): ExportData {
  const csv = toCsv(
    columns.map((c) => c.header),
    rows.map((r) => columns.map((c) => c.value(r))),
  );
  const json = JSON.stringify(
    rows.map((r) => {
      const o: Record<string, unknown> = {};
      for (const c of columns) o[c.key] = c.value(r);
      return o;
    }),
    null,
    2,
  );
  return { csv, json, count: rows.length };
}

/** Trigger a client-side file download (mirrors inventory.ts/reports.ts
 *  `downloadExport`; shared here so new views don't each re-roll the Blob dance).
 *  Guarded for non-DOM (test) environments — a no-op when `document` is absent. */
export function downloadExport(filename: string, mime: string, content: string): void {
  if (typeof document === "undefined") return;
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

/** Build the export for `rows` and download BOTH the CSV and JSON in one call,
 *  named `<prefix>-YYYY-MM-DD.{csv,json}`. The CSV is BOM-prefixed (`﻿`) so Excel
 *  es-CL opens it as UTF-8; the JSON stays clean. Returns the built {@link
 *  ExportData} so a caller/test can assert on it without a DOM. */
export function exportRows<T>(
  prefix: string,
  columns: readonly ExportColumn<T>[],
  rows: readonly T[],
): ExportData {
  const data = buildExport(columns, rows);
  const stem = exportFilename(prefix);
  downloadExport(`${stem}.csv`, "text/csv;charset=utf-8", `﻿${data.csv}`);
  downloadExport(`${stem}.json`, "application/json;charset=utf-8", data.json);
  return data;
}
