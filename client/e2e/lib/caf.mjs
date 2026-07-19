// Synthetic CAF generator for the DTE lifecycle E2E.
//
// A real CAF is the folio-authorization XML the SII hands an emisor: it carries
// the folio range AND the RSA private key (RSASK) the emisor uses to sign each
// document's TED (timbre). We can't reach the live SII offline, but the server
// only needs a *structurally valid* CAF whose RSASK is a usable RSA key — the
// TED signature is self-consistent (crates/dte/src/timbre.rs verifies DD against
// the CAF's own RSAPUBK; the SII <FRMA> over the CAF itself is never re-checked
// offline). So we mint one here with node:crypto, byte-for-byte in the shape
// crates/dte/src/caf.rs::parse_xml + timbre::extract_pem expect:
//   <AUTORIZACION><CAF version="1.0"><DA>… RE/TD/RNG(D,H)/FA …</DA>
//     <FRMA>placeholder</FRMA></CAF><RSASK>PKCS#1 PEM</RSASK>
//     <RSAPUBK>SPKI PEM</RSAPUBK></AUTORIZACION>
// Mirrors crates/dte/tests/common/mod.rs::caf_xml_synthetic (RSA 1024 for speed;
// the real SII uses 2048 — irrelevant to the offline signature roundtrip).

import { generateKeyPairSync } from "node:crypto";
import { writeFileSync } from "node:fs";
import { join } from "node:path";

/** Mint a synthetic CAF XML for `tipo` over folios `desde..hasta` and write it
 *  into `dir`. Returns the file path for `pharma caf import`. */
export function writeCaf(dir, { rut, tipo, desde, hasta }) {
  const { privateKey, publicKey } = generateKeyPairSync("rsa", {
    modulusLength: 1024,
    publicKeyEncoding: { type: "spki", format: "pem" }, // matches RSAPUBK (to_public_key_pem)
    privateKeyEncoding: { type: "pkcs1", format: "pem" }, // matches RSASK (from_pkcs1_pem)
  });
  const xml = `<AUTORIZACION>
<CAF version="1.0">
<DA>
<RE>${rut}</RE>
<RS>FARMACIA TEST SPA</RS>
<TD>${tipo}</TD>
<RNG><D>${desde}</D><H>${hasta}</H></RNG>
<FA>2026-01-01</FA>
<RSAPK><M>placeholder</M><E>Aw==</E></RSAPK>
<IDK>100</IDK>
</DA>
<FRMA algoritmo="SHA1withRSA">PLACEHOLDER_SII_SIGNATURE</FRMA>
</CAF>
<RSASK>${privateKey.trim()}</RSASK>
<RSAPUBK>${publicKey.trim()}</RSAPUBK>
</AUTORIZACION>`;
  const path = join(dir, `caf-${tipo}-${desde}-${hasta}.xml`);
  writeFileSync(path, xml, "utf8");
  return path;
}
