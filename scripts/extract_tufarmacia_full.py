#!/usr/bin/env python3
"""Extrae el catálogo COMPLETO de Tu Farmacia (Coquimbo) desde BACKUP_PRODUCTOS.txt.

A diferencia del extractor original (`extract_tufarmacia.py` — subset curado de
80 productos para la demo inicial), este script procesa TODOS los productos
ACTV con precio sugerido, deduplica por nombre canónico, e infiere principio
activo desde una lista extendida. Read-only sobre `build-and-deploy-webdev-asap`
(separación de repos por regla del proyecto: data sí, código no).

Salida: `demo-data/tufarmacia_full.csv` en el schema que acepta
`POST /api/v1/products/import` del backend pharma-server.

Columnas: sku,name,active_ingredient,cost_price,price,stock,external_id,laboratory
(Notar: la columna principal de precio es `price` para que matchee el header del
endpoint; `sale_price` también se acepta y se mapea — sólo emitimos `price`).
"""
import os
import re
import csv
import random
from datetime import date

SRC = r"C:\Users\Administrator\Documents\GitHub\build-and-deploy-webdev-asap\BACKUP_PRODUCTOS.txt"
OUT = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "demo-data",
    "tufarmacia_full.csv",
)

random.seed(2026)  # determinista

# Principios activos detectables por nombre. Lista extendida del extractor
# original con cefalosporinas, anticonceptivos, corticoides, antihipertensivos,
# IBPs adicionales, antidepresivos, diuréticos y bloqueadores cardiovasculares.
INGREDIENTS = [
    # Originales
    "paracetamol", "ibuprofeno", "amoxicilina", "acido acetilsalicilico",
    "acido mefenamico", "acido valproico", "loratadina", "omeprazol",
    "metformina", "losartan", "atorvastatina", "naproxeno", "clorfenamina",
    "cefadroxilo", "cloxacilina", "azitromicina", "ciprofloxacino",
    "diclofenaco", "ketoprofeno", "lansoprazol", "ranitidina", "sertralina",
    "fluoxetina", "clonazepam", "alprazolam", "salbutamol", "prednisona",
    "betametasona", "vitamina", "complejo b",
    # Extensión solicitada por la tarea
    "cefalexina", "levonorgestrel", "dexametasona", "captopril", "enalapril",
    "sildenafil", "esomeprazol", "montelukast", "simvastatina",
    "escitalopram", "amitriptilina", "clonidina", "furosemida",
    "espironolactona", "hidroclorotiazida", "metoprolol", "amlodipino",
    "valsartan", "irbesartan",
    # Comunes que aparecen en el catálogo y agregan cobertura
    "tramadol", "metoclopramida", "domperidona", "loperamida", "claritromicina",
    "eritromicina", "doxiciclina", "nitrofurantoina", "metronidazol",
    "fluconazol", "miconazol", "ketoconazol", "carvedilol", "bisoprolol",
    "atenolol", "warfarina", "acenocumarol", "levotiroxina", "insulina",
    "glibenclamida", "rosuvastatina", "pravastatina", "ezetimibe",
    "diazepam", "lorazepam", "zolpidem", "venlafaxina", "duloxetina",
    "risperidona", "quetiapina", "olanzapina", "haloperidol", "carbamazepina",
    "fenitoina", "gabapentina", "pregabalina", "topiramato", "lamotrigina",
    "morfina", "codeina", "fentanilo", "naloxona", "hidroxizina",
    "cetirizina", "desloratadina", "fexofenadina", "budesonida",
    "fluticasona", "mometasona", "tiotropio", "ipratropio",
    "esomeprazol", "pantoprazol", "rabeprazol", "famotidina",
    "bicalutamida", "tamsulosina", "finasterida",
]

# Detecta items ACTV con (precio $X,XXX) o ('-' sin precio).
# Layout de columnas en BACKUP_PRODUCTOS.txt:
#   ID(num)  DESCRIPCION  PVP_SUGERIDO  EST  CODIGOS_BARRA
# Ejemplo real:
#   "5779     ACIDO ACETILSALICILICO 100MG.100COM. MINTLAB    $13,490 ACTV   7800063112046"
LINE = re.compile(
    r"^(?P<id>\d+)\s+"
    r"(?P<desc>.+?)\s+"
    r"(?P<pvp>\$[\d,]+|-)\s+"
    r"(?P<est>ACTV|INAC|\S+)\s+"
    r"(?P<codes>[\d|]*)\s*$"
)

# Heurística mínima para laboratorio: última palabra MAYÚSCULAS de >=4 chars al
# final de DESCRIPCION (muchos registros traen marca/laboratorio al final).
LAB_TAIL = re.compile(r"\b([A-Z]{4,})\s*$")


def infer_ingredient(name: str) -> str:
    """Devuelve el principio activo más largo que matchee (greedy by len).

    Greedy-by-length evita falsos positivos como `metformina` matcheando
    `metformina` cuando el nombre real es `metformina retard`.
    """
    low = name.lower()
    best = ""
    for ing in INGREDIENTS:
        if ing in low and len(ing) > len(best):
            best = ing
    return best


def infer_laboratory(desc: str) -> str:
    """Best-effort laboratorio desde el final del nombre."""
    m = LAB_TAIL.search(desc.strip())
    if not m:
        return ""
    cand = m.group(1)
    # Filtra unidades / formatos comunes que no son laboratorios.
    if cand in {
        "MG", "GR", "ML", "CC", "COM", "CAPS", "AMP", "FCO", "TAB",
        "SUSP", "JBE", "CR", "POM", "GEL", "INY", "EFER", "SOL",
        "DUO", "PACK", "PROMO",
    }:
        return ""
    return cand.title()


def canonical_key(desc: str) -> str:
    """Clave de dedupe: primeras 50 chars normalizadas (evita doble marca)."""
    s = re.sub(r"[^A-Z0-9 ]", " ", desc.upper())
    s = re.sub(r"\s+", " ", s).strip()
    return s[:50]


def parse_price(pvp: str) -> int:
    if pvp == "-" or not pvp:
        return 0
    return int(pvp.replace("$", "").replace(",", "").strip())


def main() -> int:
    rows = []
    seen_names = set()
    seen_barcodes = set()  # garantiza external_id único
    total_lines = 0
    actv = 0
    with_price = 0
    no_price = 0
    no_barcode = 0
    dupe = 0

    with open(SRC, encoding="utf-8", errors="replace") as f:
        for line in f:
            total_lines += 1
            m = LINE.match(line.rstrip("\r\n"))
            if not m:
                continue
            if m.group("est") != "ACTV":
                continue
            actv += 1

            price = parse_price(m.group("pvp"))
            if price <= 0:
                no_price += 1
                continue
            with_price += 1

            desc = re.sub(r"\s+", " ", m.group("desc").strip())
            if not desc:
                continue

            barcode = (m.group("codes") or "").split("|")[0].strip()
            # Algunos códigos son IDs internos cortos (6 chars) — los aceptamos.
            # Si no hay barcode útil, usamos TF<id> como external_id.
            if barcode and barcode.isdigit() and len(barcode) >= 6:
                external = barcode
            else:
                no_barcode += 1
                external = f"TF{m.group('id')}"

            if external in seen_barcodes:
                dupe += 1
                continue

            key = canonical_key(desc)
            if key in seen_names:
                dupe += 1
                continue
            seen_names.add(key)
            seen_barcodes.add(external)

            # Costo determinista por línea (semilla por id) → re-run estable.
            r = random.Random(int(m.group("id")))
            cost = round(price * r.uniform(0.55, 0.78))
            # Stock determinista, bias a valores realistas.
            stock = r.choice([0, 0, 3, 6, 10, 15, 24, 40, 60, 100])

            ing = infer_ingredient(desc)
            lab = infer_laboratory(desc)

            rows.append({
                "sku": external,
                "name": desc[:120],
                "active_ingredient": ing,
                "cost_price": cost,
                "price": price,
                "stock": stock,
                "external_id": external,
                "laboratory": lab,
            })

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", newline="", encoding="utf-8") as fout:
        w = csv.DictWriter(fout, fieldnames=[
            "sku", "name", "active_ingredient", "cost_price",
            "price", "stock", "external_id", "laboratory",
        ])
        w.writeheader()
        w.writerows(rows)

    print(f"== extract_tufarmacia_full ({date.today().isoformat()}) ==")
    print(f"  source         : {SRC}")
    print(f"  lines read     : {total_lines}")
    print(f"  ACTV products  : {actv}")
    print(f"  with price     : {with_price}")
    print(f"  no price (skip): {no_price}")
    print(f"  no barcode     : {no_barcode}  (asignado TF<id>)")
    print(f"  duplicates skip: {dupe}")
    print(f"  rows written   : {len(rows)}")
    print(f"  output         : {OUT}")
    # muestra
    for r in rows[:5]:
        ing = r["active_ingredient"] or "-"
        lab = r["laboratory"] or "-"
        print(
            f"  {r['external_id']:<14} "
            f"{r['name'][:50]:<50} "
            f"${r['price']:>7} stock={r['stock']:>3} "
            f"ing={ing[:18]:<18} lab={lab[:12]}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
