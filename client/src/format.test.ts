import { describe, it, expect } from "vitest";
import {
  clp,
  num,
  toNumber,
  cleanRut,
  rutDigitVerifier,
  isValidRut,
  canonicalRut,
  formatRut,
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
