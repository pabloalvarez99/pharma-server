//! Canonical XML 1.0 (W3C REC `xml-c14n` 20010315, **inclusive**, sin
//! comentarios) — subtask **9.1.b.4**.
//!
//! ## Por qué
//!
//! El perfil XML-DSig del SII declara
//! `CanonicalizationMethod Algorithm=".../REC-xml-c14n-20010315"` para el
//! `<SignedInfo>`. Un validador detached (como el del SII) recanonicaliza el
//! `<SignedInfo>` y el `<Documento>` con C14N 1.0 **antes** de verificar el
//! digest y la firma. Si nosotros firmamos bytes que no son la forma canónica,
//! el SII recanonicaliza a bytes distintos y la firma no valida. La subtask
//! 9.1.b.2 firmó bytes "determinísticos pero no canónicos"; este módulo los
//! reemplaza por la forma canónica spec-correcta, manteniendo estable la API
//! pública de `sign.rs`.
//!
//! ## Alcance (lo que SÍ implementa)
//!
//! El XML que firmamos lo genera nuestro propio serializador (`xml::writer`,
//! `quick-xml`) + los `<Signature>`/`<SignedInfo>` que construye `sign.rs`:
//! **sin DTD, sin entidades externas, sin defaults de ATTLIST, UTF-8**. Sobre
//! ese subconjunto implementamos las reglas C14N 1.0 que aplican:
//!
//! - Orden de nodos namespace (default `xmlns` primero, luego por prefijo) y de
//!   atributos (clave primaria = URI de namespace, secundaria = nombre local).
//! - Remoción de declaraciones de namespace superfluas (una decl no se emite si
//!   un ancestro ya la emitió idéntica; `xmlns=""` sólo si cancela un default
//!   heredado no vacío). Reglas verificadas contra los ejemplos 3.3 del W3C.
//! - Elementos vacíos → par start-end (`<a/>` ⇒ `<a></a>`).
//! - Escape de contenido: `&`→`&amp;`, `<`→`&lt;`, `>`→`&gt;`, `#xD`→`&#xD;`.
//! - Escape de atributos: `&`→`&amp;`, `<`→`&lt;`, `"`→`&quot;`,
//!   `#x9`→`&#x9;`, `#xA`→`&#xA;`, `#xD`→`&#xD;` (`>` NO se escapa en atributos).
//! - Normalización de valor de atributo CDATA: whitespace literal → espacio;
//!   los char-refs conservan el carácter real.
//! - Comentarios eliminados; whitespace de contenido preservado tal cual.
//!
//! ## Fuera de alcance (justificado)
//!
//! Defaults de `<!ATTLIST>`, expansión de entidades de DTD, tipos NMTOKENS/ID
//! (colapso de whitespace por tipo) y conversión de encoding requieren un DTD,
//! que el DTE del SII **nunca** usa (ejemplos W3C 3.1/3.5/3.6/3.7/3.8). El
//! round-trip vivo contra `maullin.sii.cl` (9.1.l) queda bloqueado por
//! credenciales reales del SII; este módulo es offline-testable contra los
//! vectores publicados por el W3C que caen dentro del subconjunto sin DTD.

use crate::DteError;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::BTreeMap;

/// URI del namespace reservado `xml:` (atributos `xml:base`, `xml:space`).
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

fn err(msg: impl std::fmt::Display) -> DteError {
    DteError::XmlInvalid(format!("c14n: {msg}"))
}

/// Canonicaliza un fragmento XML (el elemento ápice + descendientes) a su forma
/// **Canonical XML 1.0 inclusiva sin comentarios**.
///
/// El fragmento se trata como un sub-árbol auto-contenido: las declaraciones de
/// namespace en alcance del ápice deben venir ya materializadas en el string
/// (es lo que hace `sign.rs` al construir `<SignedInfo xmlns="...">`), porque un
/// substring no conserva los namespaces de sus ancestros.
pub fn canonicalize(xml: &str) -> Result<String, DteError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false); // whitespace de contenido se preserva

    let mut out = String::with_capacity(xml.len());
    // Pila de scopes de namespace: cada frame = decls (prefijo, uri) del elemento
    // (prefijo "" = default `xmlns`). Incluye decls superfluas (siguen en scope).
    let mut ns_stack: Vec<Vec<(String, String)>> = Vec::new();
    let mut depth: i32 = 0;

    loop {
        match reader
            .read_event()
            .map_err(|e| err(format!("parse: {e}")))?
        {
            Event::Start(e) => {
                write_start(&mut out, e.name().as_ref(), e.attributes(), &mut ns_stack)?;
                depth += 1;
            }
            Event::Empty(e) => {
                let name = e.name().as_ref().to_vec();
                write_start(&mut out, &name, e.attributes(), &mut ns_stack)?;
                out.push_str("</");
                out.push_str(std::str::from_utf8(&name).map_err(err)?);
                out.push('>');
                ns_stack.pop();
            }
            Event::End(e) => {
                out.push_str("</");
                out.push_str(std::str::from_utf8(e.name().as_ref()).map_err(err)?);
                out.push('>');
                ns_stack.pop();
                depth -= 1;
            }
            Event::Text(e) => {
                if depth > 0 {
                    let raw = std::str::from_utf8(&e.into_inner())
                        .map_err(err)?
                        .to_owned();
                    out.push_str(&text_content(&raw)?);
                }
                // depth == 0 → whitespace fuera del elemento documento: se descarta.
            }
            Event::CData(e) => {
                if depth > 0 {
                    let raw = std::str::from_utf8(&e.into_inner())
                        .map_err(err)?
                        .to_owned();
                    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
                    out.push_str(&escape_content(&normalized));
                }
            }
            Event::PI(e) => {
                // C14N conserva PIs (3.1). Edge: el DTE no emite PIs en contenido.
                if depth > 0 {
                    let body = std::str::from_utf8(&e.into_inner())
                        .map_err(err)?
                        .to_owned();
                    out.push_str("<?");
                    out.push_str(&body);
                    out.push_str("?>");
                }
            }
            // Declaración XML, DOCTYPE y comentarios se eliminan.
            Event::Decl(_) | Event::DocType(_) | Event::Comment(_) => {}
            Event::Eof => break,
        }
    }
    Ok(out)
}

/// Mapa efectivo prefijo→uri en el scope actual (fold de toda la pila).
fn effective_ns(stack: &[Vec<(String, String)>]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for frame in stack {
        for (prefix, uri) in frame {
            map.insert(prefix.clone(), uri.clone());
        }
    }
    map
}

/// Emite el start-tag canónico de un elemento y empuja su frame de namespaces.
fn write_start(
    out: &mut String,
    name: &[u8],
    attrs: quick_xml::events::attributes::Attributes,
    ns_stack: &mut Vec<Vec<(String, String)>>,
) -> Result<(), DteError> {
    let qname = std::str::from_utf8(name).map_err(err)?.to_owned();

    // Separar declaraciones de namespace de atributos normales.
    let mut decls: Vec<(String, String)> = Vec::new();
    // (ns_uri, local, qname, valor_escapeado)
    let mut plain: Vec<(String, String, String, String)> = Vec::new();

    let effective = effective_ns(ns_stack);

    for attr in attrs {
        let attr = attr.map_err(|e| err(format!("atributo: {e}")))?;
        let key = std::str::from_utf8(attr.key.as_ref())
            .map_err(err)?
            .to_owned();
        let raw = std::str::from_utf8(&attr.value).map_err(err)?.to_owned();
        if key == "xmlns" {
            decls.push((String::new(), decode_refs(&raw)?));
        } else if let Some(prefix) = key.strip_prefix("xmlns:") {
            decls.push((prefix.to_owned(), decode_refs(&raw)?));
        } else {
            plain.push((key, raw, String::new(), String::new()));
        }
    }

    // Decls efectivas tras aplicar las de este elemento (para resolver el URI de
    // los atributos prefijados al ordenarlos).
    let mut eff_after = effective.clone();
    for (prefix, uri) in &decls {
        eff_after.insert(prefix.clone(), uri.clone());
    }

    // Resolver (ns_uri, local) de cada atributo normal para el orden canónico.
    let mut attrs_out: Vec<(String, String, String, String)> = Vec::new();
    for (key, raw, _, _) in plain {
        let (ns_uri, local) = match key.split_once(':') {
            Some(("xml", local)) => (XML_NS.to_owned(), local.to_owned()),
            Some((prefix, local)) => {
                let uri = eff_after
                    .get(prefix)
                    .ok_or_else(|| err(format!("prefijo de atributo sin declarar: {prefix}")))?;
                (uri.clone(), local.to_owned())
            }
            // Atributo sin prefijo: NO está en el default namespace (C14N).
            None => (String::new(), key.clone()),
        };
        let value = escape_attr(&attr_value(&raw)?);
        attrs_out.push((ns_uri, local, key, value));
    }
    // Orden: clave primaria URI de namespace, secundaria nombre local.
    attrs_out.sort_by(|a, b| (a.0.as_str(), a.1.as_str()).cmp(&(b.0.as_str(), b.1.as_str())));

    // Namespaces a renderizar: omitir decls superfluas (idénticas a un ancestro).
    let mut render: Vec<(String, String)> = Vec::new();
    for (prefix, uri) in &decls {
        let inherited = effective.get(prefix).map(String::as_str).unwrap_or("");
        if uri != inherited {
            render.push((prefix.clone(), uri.clone()));
        }
    }
    // Orden: default `xmlns` primero, luego por prefijo lexicográfico.
    render.sort_by(|a, b| match (a.0.is_empty(), b.0.is_empty()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.cmp(&b.0),
    });

    out.push('<');
    out.push_str(&qname);
    for (prefix, uri) in &render {
        if prefix.is_empty() {
            out.push_str(" xmlns=\"");
        } else {
            out.push_str(" xmlns:");
            out.push_str(prefix);
            out.push_str("=\"");
        }
        out.push_str(&escape_attr(uri));
        out.push('"');
    }
    for (_, _, key, value) in &attrs_out {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        out.push_str(value);
        out.push('"');
    }
    out.push('>');

    // El frame guarda TODAS las decls (las superfluas siguen en scope).
    ns_stack.push(decls);
    Ok(())
}

/// Resuelve una referencia de entidad/carácter (`amp` `lt` `gt` `quot` `apos` o
/// `#NNN` / `#xHH`) a su carácter. `name` es el texto entre `&` y `;`.
fn resolve_ref(name: &str) -> Result<char, DteError> {
    if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
        let cp = u32::from_str_radix(hex, 16)
            .map_err(|_| err(format!("char-ref hex inválido: &{name};")))?;
        return char::from_u32(cp).ok_or_else(|| err(format!("codepoint inválido: &{name};")));
    }
    if let Some(dec) = name.strip_prefix('#') {
        let cp: u32 = dec
            .parse()
            .map_err(|_| err(format!("char-ref decimal inválido: &{name};")))?;
        return char::from_u32(cp).ok_or_else(|| err(format!("codepoint inválido: &{name};")));
    }
    match name {
        "amp" => Ok('&'),
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "quot" => Ok('"'),
        "apos" => Ok('\''),
        other => Err(err(format!("entidad no soportada (sin DTD): &{other};"))),
    }
}

/// Expande char/entity-refs de un string (sin normalizar whitespace). Para URIs
/// de namespace y como paso de decodificación de contenido.
fn decode_refs(s: &str) -> Result<String, DteError> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        if c == '&' {
            let rest = &s[i + 1..];
            let end = rest.find(';').ok_or_else(|| err("referencia sin ';'"))?;
            out.push(resolve_ref(&rest[..end])?);
            // avanzar el iterador más allá del ';'
            for _ in 0..rest[..=end].chars().count() {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

/// Normaliza un valor de atributo tipo CDATA (XML §3.3.3 + line-norm): los
/// endings de línea literales y el whitespace literal pasan a espacio; los
/// char-refs conservan el carácter real (que luego `escape_attr` puede re-escapar).
fn attr_value(raw: &str) -> Result<String, DteError> {
    let raw = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '&' => {
                let rest = &raw[i + 1..];
                let end = rest.find(';').ok_or_else(|| err("referencia sin ';'"))?;
                out.push(resolve_ref(&rest[..end])?);
                for _ in 0..rest[..=end].chars().count() {
                    chars.next();
                }
            }
            ' ' | '\t' | '\n' => out.push(' '),
            other => out.push(other),
        }
    }
    Ok(out)
}

/// Contenido textual: line-norm de CR literales, expansión de refs, luego escape
/// canónico de contenido.
fn text_content(raw: &str) -> Result<String, DteError> {
    let raw = raw.replace("\r\n", "\n").replace('\r', "\n");
    let decoded = decode_refs(&raw)?;
    Ok(escape_content(&decoded))
}

/// Escape de contenido C14N: `&` `<` `>` y `#xD`. Tab y `#xA` se preservan.
fn escape_content(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#xD;"),
            other => out.push(other),
        }
    }
    out
}

/// Escape de valor de atributo C14N: `&` `<` `"` y `#x9`/`#xA`/`#xD`. `>` y `'`
/// NO se escapan.
fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#x9;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// W3C C14N 1.0 §3.2 — whitespace en contenido del documento. Input y forma
    /// canónica son idénticos: el whitespace de contenido se preserva tal cual.
    #[test]
    fn w3c_3_2_whitespace_preserved() {
        let input = "<doc>\n   <clean>   </clean>\n   <dirty>   A   B   </dirty>\n   <mixed>\n      A\n      <clean>   </clean>\n      B\n      <dirty>   A   B   </dirty>\n      C\n   </mixed>\n</doc>";
        assert_eq!(canonicalize(input).unwrap(), input);
    }

    /// W3C C14N 1.0 §3.3 — start/end tags: orden de atributos y namespaces,
    /// remoción de decls superfluas, elementos vacíos → par start-end.
    ///
    /// Adaptado: se quita el DOCTYPE/ATTLIST (el DTE no usa DTD), por lo que e9
    /// NO recibe el atributo `attr="default"` que el ejemplo original deriva del
    /// ATTLIST. Todo lo demás (sorting + namespaces) es idéntico al vector W3C.
    #[test]
    fn w3c_3_3_attr_and_namespace_ordering() {
        let input = "<doc>\n   <e1   />\n   <e2   ></e2>\n   <e3   name = \"elem3\"   id=\"elem3\"   />\n   <e4   name=\"elem4\"   id=\"elem4\"   ></e4>\n   <e5 a:attr=\"out\" b:attr=\"sorted\" attr2=\"all\" attr=\"I'm\"\n       xmlns:b=\"http://www.ietf.org\"\n       xmlns:a=\"http://www.w3.org\"\n       xmlns=\"http://example.org\"/>\n   <e6 xmlns=\"\" xmlns:a=\"http://www.w3.org\">\n      <e7 xmlns=\"http://www.ietf.org\">\n         <e8 xmlns=\"\" xmlns:a=\"http://www.w3.org\">\n            <e9 xmlns=\"\" xmlns:a=\"http://www.ietf.org\"/>\n         </e8>\n      </e7>\n   </e6>\n</doc>";
        let expected = "<doc>\n   <e1></e1>\n   <e2></e2>\n   <e3 id=\"elem3\" name=\"elem3\"></e3>\n   <e4 id=\"elem4\" name=\"elem4\"></e4>\n   <e5 xmlns=\"http://example.org\" xmlns:a=\"http://www.w3.org\" xmlns:b=\"http://www.ietf.org\" attr=\"I'm\" attr2=\"all\" b:attr=\"sorted\" a:attr=\"out\"></e5>\n   <e6 xmlns:a=\"http://www.w3.org\">\n      <e7 xmlns=\"http://www.ietf.org\">\n         <e8 xmlns=\"\">\n            <e9 xmlns:a=\"http://www.ietf.org\"></e9>\n         </e8>\n      </e7>\n   </e6>\n</doc>";
        assert_eq!(canonicalize(input).unwrap(), expected);
    }

    #[test]
    fn empty_element_becomes_start_end_pair() {
        assert_eq!(canonicalize("<a/>").unwrap(), "<a></a>");
        assert_eq!(canonicalize("<a></a>").unwrap(), "<a></a>");
        assert_eq!(canonicalize("<a b=\"1\"/>").unwrap(), "<a b=\"1\"></a>");
    }

    #[test]
    fn comments_are_removed() {
        assert_eq!(canonicalize("<a>x<!-- c -->y</a>").unwrap(), "<a>xy</a>");
    }

    #[test]
    fn content_escaping() {
        // CDATA expandido + escape de < & > en contenido.
        assert_eq!(
            canonicalize("<a><![CDATA[<&>]]></a>").unwrap(),
            "<a>&lt;&amp;&gt;</a>"
        );
        // char-ref CR (#xD) en contenido se escapa a &#xD;.
        assert_eq!(canonicalize("<a>x&#13;y</a>").unwrap(), "<a>x&#xD;y</a>");
    }

    #[test]
    fn attribute_escaping() {
        // " se escapa a &quot;; & a &amp;; < a &lt;; > NO se escapa en atributo.
        assert_eq!(
            canonicalize("<a b='&quot;&amp;&lt;&gt;'></a>").unwrap(),
            "<a b=\"&quot;&amp;&lt;>\"></a>"
        );
        // whitespace por char-ref (#x9/#xA/#xD) se escapa; ' no se escapa.
        assert_eq!(
            canonicalize("<a b='&#9;&#10;&#13;'></a>").unwrap(),
            "<a b=\"&#x9;&#xA;&#xD;\"></a>"
        );
    }

    #[test]
    fn cdata_attribute_value_normalization() {
        // Whitespace literal (tab real) en valor de atributo CDATA → espacio.
        assert_eq!(
            canonicalize("<a b=\"x\ty\"></a>").unwrap(),
            "<a b=\"x y\"></a>"
        );
    }

    #[test]
    fn idempotent() {
        let once = canonicalize("<a z=\"1\" m=\"2\"><b/></a>").unwrap();
        assert_eq!(canonicalize(&once).unwrap(), once);
    }

    /// El valor real de C14N para el SII: dos serializaciones equivalentes que
    /// sólo difieren en el orden de atributos canonicalizan a los mismos bytes.
    #[test]
    fn attribute_reorder_is_canonically_equal() {
        let a = canonicalize("<x m=\"1\" a=\"2\" z=\"3\"></x>").unwrap();
        let b = canonicalize("<x z=\"3\" a=\"2\" m=\"1\"></x>").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, "<x a=\"2\" m=\"1\" z=\"3\"></x>");
    }
}
