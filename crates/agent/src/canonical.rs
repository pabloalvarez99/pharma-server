//! Deterministic JSON encoding for signing.
//!
//! Two nodes must hash *exactly* the same bytes for a signature to verify, so
//! we cannot rely on `serde_json`'s map ordering. [`canonical_bytes`] walks a
//! [`serde_json::Value`] and re-emits it with object keys sorted lexically and
//! no insignificant whitespace. Numbers/strings use serde_json's own minimal
//! encoding (sufficient for our integer/string payloads).
//!
//! Returns [`Result`] so a malformed scalar (e.g. a non-finite `f64` that
//! `serde_json` refuses to encode) surfaces as [`AgentError::Canonical`]
//! instead of panicking — see audit on `unwrap/expect` removal.

use serde_json::Value;

use crate::error::{AgentError, Result};

pub fn canonical_bytes(v: &Value) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    write_value(v, &mut out)?;
    Ok(out)
}

fn write_value(v: &Value, out: &mut Vec<u8>) -> Result<()> {
    match v {
        Value::Object(map) => {
            out.push(b'{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                // Key as a JSON string (serde handles escaping). Failure here
                // would mean a key with non-UTF-8 bytes, which is unreachable
                // for serde_json::Map<String, _> but we surface it instead of
                // panicking just in case.
                let ks = serde_json::to_string(k)
                    .map_err(|e| AgentError::Canonical(format!("key encode: {e}")))?;
                out.extend_from_slice(ks.as_bytes());
                out.push(b':');
                write_value(&map[*k], out)?;
            }
            out.push(b'}');
        }
        Value::Array(arr) => {
            out.push(b'[');
            for (i, e) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(e, out)?;
            }
            out.push(b']');
        }
        other => {
            // Scalars: serde_json's compact form is already deterministic.
            let s = serde_json::to_string(other)
                .map_err(|e| AgentError::Canonical(format!("scalar encode: {e}")))?;
            out.extend_from_slice(s.as_bytes());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_is_stable() -> Result<()> {
        let a = json!({ "b": 1, "a": 2, "c": { "z": 1, "y": 2 } });
        let b = json!({ "c": { "y": 2, "z": 1 }, "a": 2, "b": 1 });
        assert_eq!(canonical_bytes(&a)?, canonical_bytes(&b)?);
        assert_eq!(
            String::from_utf8(canonical_bytes(&a)?)
                .map_err(|e| AgentError::Canonical(e.to_string()))?,
            r#"{"a":2,"b":1,"c":{"y":2,"z":1}}"#
        );
        Ok(())
    }
}
