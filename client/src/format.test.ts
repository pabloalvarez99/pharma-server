import { describe, it, expect } from "vitest";
import {
  clp,
  num,
  fecha,
  toNumber,
  cleanRut,
  rutDigitVerifier,
  isValidRut,
  canonicalRut,
  formatRut,
  desgloseIva,
  parseCash,
  effectiveTender,
  vuelto,
  quickCashAmounts,
} from "./format";

// RUT verifier vectors computed by hand from the mód-11 rule (multiply the body
// right-to-left by the cycle 2,3,4,5,6,7,2,3…, sum, 11 − (sum mod 11); 11→0,
// 10→K) — i.e. an oracle independent of the implementation under test:
//   11111111 → sum 32, 32%11=10, 11−10=1   → "1"
//   12345678 → sum 138, 138%11=6, 11−6=5    → "5"
//   76123456 → sum 110, 110%11=0, →11→      → "0"  (UI placeholder -7 is bogus)
//    5126663 → sum 107, 107%11=8, 11−8=3    → "3"
//   40000000 → sum 12,  12%11=1,  11−1=10→  → "K"
const VALID: ReadonlyArray<[string, string]> = [
  ["11111111", "1"],
  ["12345678", "5"],
  ["76123456", "0"],
  ["5126663", "3"],
  ["40000000", "K"],
];

describe("rutDigitVerifier", () => {
  for (const [body, dv] of VALID) {
    it(`${body} → ${dv}`, () => {
      expect(rutDigitVerifier(body)).toBe(dv);
    });
  }
});

describe("isValidRut", () => {
  it("accepts a correct body+DV in any cosmetic format", () => {
    expect(isValidRut("11111111-1")).toBe(true);
    expect(isValidRut("11.111.111-1")).toBe(true);
    expect(isValidRut("12345678-5")).toBe(true);
    expect(isValidRut("40000000-k")).toBe(true); // lower-case K
    expect(isValidRut("40000000-K")).toBe(true);
  });

  it("rejects a wrong verifier digit", () => {
    // The UI placeholder 76123456-7 has the wrong DV (correct is 0).
    expect(isValidRut("76123456-7")).toBe(false);
    expect(isValidRut("12345678-9")).toBe(false);
    expect(isValidRut("11111111-2")).toBe(false);
  });

  it("rejects structurally invalid input", () => {
    expect(isValidRut("")).toBe(false);
    expect(isValidRut("abc")).toBe(false);
    expect(isValidRut("123-4")).toBe(false); // body too short (<7)
    expect(isValidRut("123456789-0")).toBe(false); // body too long (>8)
    expect(isValidRut("1234567K-K")).toBe(false); // non-digit in body
  });
});

describe("cleanRut", () => {
  it("strips dots/dash/spaces and upper-cases the verifier", () => {
    expect(cleanRut("76.123.456-0")).toBe("761234560");
    expect(cleanRut("40000000-k")).toBe("40000000K");
    expect(cleanRut("  12.345.678-5 ")).toBe("123456785");
  });
});

describe("canonicalRut", () => {
  it("renders the SII wire form NNNNNNNN-D without dots", () => {
    expect(canonicalRut("76.123.456-0")).toBe("76123456-0");
    expect(canonicalRut("12345678-5")).toBe("12345678-5");
    expect(canonicalRut("40000000-k")).toBe("40000000-K");
  });

  it("returns cleaned input unchanged when it cannot be split", () => {
    expect(canonicalRut("abc")).toBe("ABC");
  });
});

describe("formatRut", () => {
  it("renders the pretty NN.NNN.NNN-D form", () => {
    expect(formatRut("761234560")).toBe("76.123.456-0");
    expect(formatRut("12345678-5")).toBe("12.345.678-5");
    expect(formatRut("40000000-k")).toBe("40.000.000-K");
  });

  it("round-trips with canonicalRut on a valid RUT", () => {
    for (const [body, dv] of VALID) {
      const pretty = formatRut(`${body}${dv}`);
      expect(canonicalRut(pretty)).toBe(`${body}-${dv}`);
    }
  });
});

describe("desgloseIva (client↔server tax parity)", () => {
  // Vectors taken verbatim from crates/dte/src/emit.rs unit tests
  // (`desglose_exacto`, `desglose_redondeo_iva_absorbe`, `build_documento_factura`)
  // so a client/server drift in the IVA split fails here.
  it("splits an exact total (11900 → 10000 + 1900)", () => {
    expect(desgloseIva(11900)).toEqual({ neto: 10000, iva: 1900 });
  });

  it("makes the IVA absorb the rounding (1000 → 840 + 160)", () => {
    expect(desgloseIva(1000)).toEqual({ neto: 840, iva: 160 });
  });

  it("keeps neto + iva == afecto for arbitrary amounts", () => {
    for (const afecto of [0, 1, 19, 119, 990, 1190, 33991, 2883855, 16900]) {
      const { neto, iva } = desgloseIva(afecto);
      expect(neto + iva).toBe(afecto);
      expect(Number.isInteger(neto)).toBe(true);
      expect(Number.isInteger(iva)).toBe(true);
    }
  });

  it("zero affected total yields zero split", () => {
    expect(desgloseIva(0)).toEqual({ neto: 0, iva: 0 });
  });
});

describe("money helpers", () => {
  it("clp formats CLP integers and tolerates strings/garbage", () => {
    expect(clp(1190)).toBe("$1.190");
    expect(clp("1190")).toBe("$1.190");
    expect(clp("not-a-number")).toBe("$0");
  });

  it("num groups with es-CL separators", () => {
    expect(num(76123456)).toBe("76.123.456");
  });

  it("toNumber parses a Decimal string and floors garbage to 0", () => {
    expect(toNumber("1190.50")).toBe(1190.5);
    expect(toNumber("garbage")).toBe(0);
    expect(toNumber(42)).toBe(42);
  });
});

describe("parseCash", () => {
  it("strips currency cosmetics to an integer", () => {
    expect(parseCash("$10.000")).toBe(10000);
    expect(parseCash("10000")).toBe(10000);
    expect(parseCash(" 5.000 CLP ")).toBe(5000);
  });

  it("floors empty / garbage to 0 (never NaN)", () => {
    expect(parseCash("")).toBe(0);
    expect(parseCash("abc")).toBe(0);
    expect(parseCash("$")).toBe(0);
  });

  it("a stray minus cannot yield a negative tender", () => {
    expect(parseCash("-500")).toBe(500);
  });
});

describe("effectiveTender", () => {
  it("sends the tendered amount when it covers the total", () => {
    expect(effectiveTender(10000, 8990)).toBe(10000);
    expect(effectiveTender(8990, 8990)).toBe(8990); // exact
  });

  it("falls back to the exact total on a short tender (never sends less)", () => {
    expect(effectiveTender(5000, 8990)).toBe(8990);
    expect(effectiveTender(0, 8990)).toBe(8990);
  });
});

describe("vuelto", () => {
  it("reports change due when cash covers the total", () => {
    expect(vuelto(10000, 8990)).toEqual({ kind: "ok", amount: 1010 });
    expect(vuelto(8990, 8990)).toEqual({ kind: "ok", amount: 0 });
  });

  it("reports a positive shortfall when cash falls short", () => {
    expect(vuelto(5000, 8990)).toEqual({ kind: "short", amount: 3990 });
  });

  it("shows nothing with no cash or an empty cart", () => {
    expect(vuelto(0, 8990)).toEqual({ kind: "none", amount: 0 });
    expect(vuelto(10000, 0)).toEqual({ kind: "none", amount: 0 });
  });

  it("amount is always non-negative — the sign lives in kind", () => {
    for (const [r, t] of [
      [0, 0],
      [1, 99999],
      [99999, 1],
      [4990, 5000],
    ] as const) {
      expect(vuelto(r, t).amount).toBeGreaterThanOrEqual(0);
    }
  });
});

describe("fecha", () => {
  it("renders es-CL dd-mm-yyyy with a full 4-digit year", () => {
    // Noon-anchored (the server's date contract) so it never slips a day in CL TZ.
    expect(fecha("2026-06-16T12:00:00Z")).toBe("16-06-2026");
    expect(fecha("2026-01-05T12:00:00Z")).toBe("05-01-2026");
  });

  it("is consistent across views (no 2-digit-year drift)", () => {
    // The whole point of consolidating: one canonical string everywhere.
    const s = fecha("2026-12-31T12:00:00Z");
    expect(s).toBe("31-12-2026");
    expect(s).not.toMatch(/-\d{2}$/); // not `…-26`
  });

  it("echoes unparseable input verbatim rather than 'Invalid Date'", () => {
    expect(fecha("not-a-date")).toBe("not-a-date");
    expect(fecha("")).toBe("");
  });
});

describe("quickCashAmounts", () => {
  it("offers the exact total first, then the next round bills", () => {
    expect(quickCashAmounts(8990)).toEqual([8990, 9000, 10000]);
  });

  it("dedups when the total is already a round bill", () => {
    expect(quickCashAmounts(10000)).toEqual([10000]);
    // 5000 collapses the 1k/5k tiers but the 10k tier still rounds up.
    expect(quickCashAmounts(5000)).toEqual([5000, 10000]);
  });

  it("ascending, positive, capped at 4", () => {
    const a = quickCashAmounts(1234);
    expect(a).toEqual([1234, 2000, 5000, 10000]);
    expect(a.length).toBeLessThanOrEqual(4);
    expect([...a].sort((x, y) => x - y)).toEqual(a);
  });

  it("empty for a non-positive total", () => {
    expect(quickCashAmounts(0)).toEqual([]);
    expect(quickCashAmounts(-5)).toEqual([]);
  });
});
