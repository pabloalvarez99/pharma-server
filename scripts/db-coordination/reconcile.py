#!/usr/bin/env python3
"""reconcile.py - web<->ERP catalog reconciliation report.

Joins the tu-farmacia.cl WEB catalog (Cloud SQL Postgres, dumped to CSV by
dump-web-catalog.{sh,ps1}) against the pharma-server ERP catalog (SurrealKV,
fetched live over the HTTP API) on the shared `external_id` (the product
barcode / Ecosur code).

Per ADR-0013 the WEB owns catalog + price; the ERP owns stock. This report
tells the operator where the two diverge so they can decide imports/repricing.

Zero external dependencies: Python 3 stdlib only (csv, json, urllib, argparse).

WEB CSV format (headerless — gcloud `export csv` emits no header):
    external_id, name, price, stock, active, updated_at

ERP source: GET {base}/api/v1/products?limit=500&offset=N  (paginated; the API
caps page size at 500 and defaults to 100, so we MUST page). Auth via
POST {base}/api/v1/login {tenant,email,password}.

Usage:
    python scripts/db-coordination/reconcile.py
    python scripts/db-coordination/reconcile.py --web demo-data/web_catalog.csv \
        --base http://127.0.0.1:8080 --tenant tufarmacia \
        --email admin@tufarmacia.cl --password tufarmacia2026 \
        --out docs/ops/db-coordination-report.md

If the ERP is unreachable you may instead point --erp-csv at a CSV produced by
GET /api/v1/products/export (header row, `external_id` column) — but note that
export endpoint is currently capped at 100 rows, so the live paged fetch is
preferred for the full catalog.

Password resolution order: --password flag > $PHARMA_PASSWORD env.
NEVER commit a password; pass it at runtime.
"""
from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import os
import sys
import urllib.error
import urllib.request
from decimal import Decimal, InvalidOperation

WEB_COLS = ["external_id", "name", "price", "stock", "active", "updated_at"]


# --------------------------------------------------------------------------- #
# loading
# --------------------------------------------------------------------------- #
def load_web(path):
    """Return {external_id: row-dict} from the headerless web export.

    Rows with empty external_id are skipped (cannot be reconciled). On a
    duplicate external_id the last row wins (export is ORDER BY external_id).
    """
    out = {}
    skipped_no_id = 0
    with open(path, newline="", encoding="utf-8") as fh:
        for raw in csv.reader(fh):
            if not raw or len(raw) < len(WEB_COLS):
                continue
            row = dict(zip(WEB_COLS, raw))
            ext = (row.get("external_id") or "").strip()
            if not ext:
                skipped_no_id += 1
                continue
            row["active"] = _as_bool(row.get("active"))
            out[ext] = row
    return out, skipped_no_id


def _as_bool(v):
    return str(v).strip().lower() in ("t", "true", "1", "yes")


def login(base, tenant, email, password):
    body = json.dumps({"tenant": tenant, "email": email, "password": password}).encode()
    req = urllib.request.Request(
        f"{base}/api/v1/login", data=body, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)["token"]


def fetch_erp(base, token, page=500):
    """Page /api/v1/products until a short page is returned. Returns list."""
    out = []
    offset = 0
    while True:
        url = f"{base}/api/v1/products?limit={page}&offset={offset}"
        req = urllib.request.Request(url, headers={"Authorization": f"Bearer {token}"})
        with urllib.request.urlopen(req, timeout=120) as r:
            batch = json.load(r)
        if not batch:
            break
        out.extend(batch)
        if len(batch) < page:
            break
        offset += page
    return out


def load_erp_csv(path):
    """Fallback: read ERP /products/export CSV (has header, `external_id`)."""
    out = []
    with open(path, newline="", encoding="utf-8") as fh:
        for row in csv.DictReader(fh):
            out.append(row)
    return out


def erp_index(rows):
    """Return {external_id: row} for ERP products that carry an external_id."""
    out = {}
    no_id = 0
    for p in rows:
        ext = (p.get("external_id") or "").strip()
        if not ext:
            no_id += 1
            continue
        out[ext] = p
    return out, no_id


# --------------------------------------------------------------------------- #
# price helpers
# --------------------------------------------------------------------------- #
def to_dec(v):
    try:
        return Decimal(str(v).strip())
    except (InvalidOperation, AttributeError):
        return None


def parse_ts(v):
    """Best-effort parse of an ISO-ish timestamp into aware datetime or None."""
    if not v:
        return None
    s = str(v).strip().replace(" ", "T", 1)
    if s.endswith("Z"):
        s = s[:-1] + "+00:00"
    # trim fractional seconds to 6 digits (Python <3.11 is picky)
    if "." in s:
        head, _, tail = s.partition(".")
        digits = ""
        rest = ""
        for i, c in enumerate(tail):
            if c.isdigit() and i < 6:
                digits += c
            elif not c.isdigit():
                rest = tail[i:]
                break
        s = f"{head}.{digits}{rest}" if digits else head + rest
    try:
        return dt.datetime.fromisoformat(s)
    except ValueError:
        return None


# --------------------------------------------------------------------------- #
# reconcile
# --------------------------------------------------------------------------- #
def reconcile(web, erp):
    web_ids = set(web)
    erp_ids = set(erp)
    both = web_ids & erp_ids
    web_only = web_ids - erp_ids
    erp_only = erp_ids - web_ids

    price_div = []  # dicts: ext, name, web_price, erp_price, diff, newer
    for ext in both:
        w = web[ext]
        e = erp[ext]
        wp = to_dec(w.get("price"))
        ep = to_dec(e.get("price"))
        if wp is None or ep is None or wp == ep:
            continue
        wt = parse_ts(w.get("updated_at"))
        et = parse_ts(e.get("updated_at"))
        newer = "?"
        if wt and et:
            newer = "web" if wt > et else "erp" if et > wt else "equal"
        price_div.append(
            {
                "ext": ext,
                "name": (e.get("name") or w.get("name") or "")[:48],
                "web_price": f"{wp:.2f}",
                "erp_price": f"{ep:.2f}",
                "diff": f"{(ep - wp):+.2f}",
                "newer": newer,
            }
        )
    price_div.sort(key=lambda d: abs(Decimal(d["diff"])), reverse=True)
    return {
        "both": both,
        "web_only": web_only,
        "erp_only": erp_only,
        "price_div": price_div,
    }


# --------------------------------------------------------------------------- #
# report
# --------------------------------------------------------------------------- #
def count_active_web(web, ids):
    return sum(1 for i in ids if web[i].get("active"))


def md_table(headers, rows):
    out = ["| " + " | ".join(headers) + " |", "|" + "|".join(["---"] * len(headers)) + "|"]
    for r in rows:
        out.append("| " + " | ".join(str(c) for c in r) + " |")
    return "\n".join(out)


def write_report(path, web, erp, res, meta):
    both, web_only = res["both"], res["web_only"]
    erp_only, price_div = res["erp_only"], res["price_div"]
    now = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")

    web_active = sum(1 for v in web.values() if v.get("active"))
    newer_web = sum(1 for d in price_div if d["newer"] == "web")
    newer_erp = sum(1 for d in price_div if d["newer"] == "erp")
    newer_unk = len(price_div) - newer_web - newer_erp

    lines = []
    lines.append("# Web <-> ERP catalog reconciliation report")
    lines.append("")
    lines.append(f"_Generated: {now} by `scripts/db-coordination/reconcile.py`._")
    lines.append("")
    lines.append(
        "Join key: `external_id` (shared barcode / Ecosur code). Canonical-truth "
        "policy (ADR-0013): **WEB owns catalog + price**, **ERP owns stock**."
    )
    lines.append("")
    lines.append("## Sources")
    lines.append("")
    lines.append(md_table(
        ["Side", "Source", "Total rows", "With external_id", "Active"],
        [
            ["WEB", meta["web_src"], meta["web_total"], len(web), web_active],
            ["ERP", meta["erp_src"], meta["erp_total"], len(erp), "n/a (all active)"],
        ],
    ))
    if meta.get("web_skipped_no_id") or meta.get("erp_skipped_no_id"):
        lines.append("")
        lines.append(
            f"> WEB rows dropped for empty external_id: {meta.get('web_skipped_no_id', 0)} · "
            f"ERP rows dropped for empty external_id: {meta.get('erp_skipped_no_id', 0)}"
        )
    lines.append("")
    lines.append("## Overlap (by external_id)")
    lines.append("")
    lines.append(md_table(
        ["Bucket", "Count", "Of which web-active"],
        [
            ["Both (in web AND erp)", len(both), count_active_web(web, both)],
            ["WEB only (missing from ERP -> import?)", len(web_only), count_active_web(web, web_only)],
            ["ERP only (not on web -> publish or retire?)", len(erp_only), "n/a"],
        ],
    ))
    lines.append("")
    lines.append("## Price divergence (same external_id, different price)")
    lines.append("")
    lines.append(
        f"**{len(price_div)}** matched products differ in price. "
        f"Newer side (by `updated_at`): web={newer_web}, erp={newer_erp}, unknown={newer_unk}. "
        "Per ADR-0013 web price is canonical; rows where ERP is newer usually mean a "
        "POS-side manual price edit that should be reconciled back to web."
    )
    lines.append("")
    top = price_div[: meta.get("top", 40)]
    if top:
        lines.append(f"Top {len(top)} by absolute difference (erp - web):")
        lines.append("")
        lines.append(md_table(
            ["external_id", "name", "web_price", "erp_price", "diff", "newer"],
            [[d["ext"], d["name"], d["web_price"], d["erp_price"], d["diff"], d["newer"]] for d in top],
        ))
    else:
        lines.append("_No price divergences among matched products._")
    lines.append("")
    lines.append("## WEB-only active products (candidates to import into ERP)")
    lines.append("")
    wo_active = sorted(
        (web[i] for i in web_only if web[i].get("active")),
        key=lambda r: r.get("external_id", ""),
    )
    lines.append(f"{len(wo_active)} active web products are absent from the ERP.")
    if wo_active:
        lines.append("")
        sample = wo_active[: meta.get("top", 40)]
        lines.append(f"First {len(sample)}:")
        lines.append("")
        lines.append(md_table(
            ["external_id", "name", "price"],
            [[r["external_id"], (r.get("name") or "")[:48], r.get("price")] for r in sample],
        ))
    lines.append("")
    lines.append("## How to act on this report")
    lines.append("")
    lines.append(
        "- **WEB-only active -> ERP**: export those rows and POST to "
        "`/api/v1/products/import` (CSV, columns `name,price,external_id,...`). "
        "See `docs/ops/db-coordination.md`.\n"
        "- **Price divergence, web newer**: pull web price into ERP (import / "
        "`bulk-price`).\n"
        "- **Price divergence, erp newer**: a POS edit; push back to web admin so "
        "web stays canonical.\n"
        "- **ERP-only**: either publish to web (it becomes catalog truth) or retire "
        "in ERP. ERP-only is expected for the full 22k Ecosur backup vs the ~1.5k "
        "curated web catalog."
    )
    lines.append("")

    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines))
    return path


# --------------------------------------------------------------------------- #
def main(argv=None):
    ap = argparse.ArgumentParser(description="web<->ERP catalog reconciliation")
    ap.add_argument("--web", default="demo-data/web_catalog.csv")
    ap.add_argument("--base", default="http://127.0.0.1:8080")
    ap.add_argument("--tenant", default="tufarmacia")
    ap.add_argument("--email", default="admin@tufarmacia.cl")
    ap.add_argument("--password", default=os.environ.get("PHARMA_PASSWORD"))
    ap.add_argument("--erp-csv", help="use this ERP CSV instead of the live API")
    ap.add_argument("--out", default="docs/ops/db-coordination-report.md")
    ap.add_argument("--top", type=int, default=40, help="rows per sample table")
    args = ap.parse_args(argv)

    if not os.path.exists(args.web):
        sys.exit(
            f"ERROR: web catalog not found: {args.web}\n"
            "Run scripts/db-coordination/dump-web-catalog.{sh,ps1} first."
        )

    print(f"==> Loading web catalog: {args.web}")
    web, web_skipped = load_web(args.web)
    web_total = len(web) + web_skipped
    print(f"    {web_total} rows, {len(web)} with external_id ({web_skipped} skipped)")

    erp_skipped = 0
    if args.erp_csv:
        print(f"==> Loading ERP catalog from CSV: {args.erp_csv}")
        erp_rows = load_erp_csv(args.erp_csv)
        erp_src = args.erp_csv
    else:
        if not args.password:
            sys.exit("ERROR: ERP password required (--password or $PHARMA_PASSWORD)")
        print(f"==> Logging into ERP {args.base} (tenant={args.tenant})")
        try:
            token = login(args.base, args.tenant, args.email, args.password)
        except (urllib.error.URLError, OSError) as e:
            sys.exit(
                f"ERROR: cannot reach ERP at {args.base}: {e}\n"
                "Start the server or pass --erp-csv with a /products/export dump."
            )
        print("==> Fetching ERP products (paged, limit=500)...")
        erp_rows = fetch_erp(args.base, token)
        erp_src = f"{args.base}/api/v1/products (paged)"
    erp_total = len(erp_rows)
    print(f"    {erp_total} ERP products fetched")

    erp, erp_skipped = erp_index(erp_rows)
    res = reconcile(web, erp)

    meta = {
        "web_src": args.web,
        "erp_src": erp_src,
        "web_total": web_total,
        "erp_total": erp_total,
        "web_skipped_no_id": web_skipped,
        "erp_skipped_no_id": erp_skipped,
        "top": args.top,
    }
    out = write_report(args.out, web, erp, res, meta)
    print(
        f"==> both={len(res['both'])} web_only={len(res['web_only'])} "
        f"erp_only={len(res['erp_only'])} price_div={len(res['price_div'])}"
    )
    print(f"==> Report written: {out}")


if __name__ == "__main__":
    main()
