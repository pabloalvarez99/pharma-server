#!/usr/bin/env python3
"""Extract customers from a Supabase auth users CSV export.

Reads the file produced by Supabase's "List All Auth Users" snippet and emits
a JSON file shaped for `POST /api/v1/admin/import-customers`:

    { "customers": [ { "email", "name", "phone?", "external_id" }, ... ] }

The tricky part is `raw_user_meta_data`: Supabase wraps the JSON in double
quotes and doubles internal `"` chars (`""provider""`). Python's stdlib
`csv` module already understands this dialect, so we let it do the dequoting
and then `json.loads` the result.

Usage:
    python extract_customers_supabase.py <input.csv> [output.json]

Defaults output path to `demo-data/tufarmacia_customers.json` relative to
the repo root (caller's CWD).
"""

from __future__ import annotations

import csv
import json
import sys
from pathlib import Path


def parse_meta(blob: str | None) -> dict:
    """Parse `raw_user_meta_data` cell — handles missing/null/empty/JSON."""
    if not blob or blob.strip().lower() in {"null", ""}:
        return {}
    # The csv module already removed the surrounding quotes and unescaped
    # `""` -> `"`, so the cell value is already valid JSON. Be defensive in
    # case a row is malformed.
    try:
        parsed = json.loads(blob)
    except json.JSONDecodeError:
        return {}
    return parsed if isinstance(parsed, dict) else {}


def build_name(meta: dict, fallback_email: str) -> str:
    """Compose a display name from meta. Falls back to the email local part."""
    name = (meta.get("name") or "").strip()
    surname = (meta.get("surname") or "").strip()
    if name and surname:
        return f"{name} {surname}"
    if name:
        return name
    if surname:
        return surname
    # Use the email local part as a last resort so we never insert empty names.
    return fallback_email.split("@", 1)[0]


def extract(input_path: Path) -> list[dict]:
    customers: list[dict] = []
    with input_path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        for row in reader:
            email = (row.get("email") or "").strip().lower()
            if not email or "@" not in email:
                # Skip rows without a usable email — they cannot be upserted.
                continue
            meta = parse_meta(row.get("raw_user_meta_data"))
            customer: dict = {
                "email": email,
                "name": build_name(meta, email),
                "external_id": (row.get("id") or "").strip() or None,
            }
            phone = (meta.get("phone") or row.get("phone") or "").strip()
            if phone and phone.lower() != "null":
                customer["phone"] = phone
            # Drop None external_id so the JSON stays compact.
            if customer["external_id"] is None:
                customer.pop("external_id")
            customers.append(customer)
    return customers


def main() -> int:
    if len(sys.argv) < 2:
        print(
            "usage: extract_customers_supabase.py <input.csv> [output.json]",
            file=sys.stderr,
        )
        return 2
    input_path = Path(sys.argv[1])
    output_path = (
        Path(sys.argv[2])
        if len(sys.argv) >= 3
        else Path("demo-data/tufarmacia_customers.json")
    )
    if not input_path.is_file():
        print(f"error: input csv not found: {input_path}", file=sys.stderr)
        return 1

    customers = extract(input_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8") as f:
        json.dump({"customers": customers}, f, ensure_ascii=False, indent=2)
        f.write("\n")
    print(
        f"extracted {len(customers)} customers from {input_path} -> {output_path}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
