//! Deterministic JSON encoding for signing.
//!
//! Two nodes must hash *exactly* the same bytes for a signature to verify, so
//! we cannot rely on `serde_json`'s map ordering. [`canonical_bytes`] walks a
//! [`serde_json::Value`] and re-emits it with object keys sorted lexically and
//! no insignificant whitespace. Numbers/strings use serde_json's own minimal
//! encoding (sufficient for our integer/string payloads).

use serde_json::Value;

pub fn canonical_bytes(v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_value(v, &mut out);
    out
}

fn write_value(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Object(map) => {
            out.push(b'{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                // Key as a JSON string (serde handles escaping).
                let ks = serde_json::to_string(k).expect("string key");
                out.extend_from_slice(ks.as_bytes());
                out.push(b':');
                write_value(&map[*k], out);
            }
            out.push(b'}');
        }
        Value::Array(arr) => {
            out.push(b'[');
            for (i, e) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(e, out);
            }
            out.push(b']');
        }
        other => {
            // Scalars: serde_json's compact form is already deterministic.
            let s = serde_json::to_string(other).expect("scalar");
            out.extend_from_slice(s.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn key_order_is_stable() {
        let a = json!({ "b": 1, "a": 2, "c": { "z": 1, "y": 2 } });
        let b = json!({ "c": { "y": 2, "z": 1 }, "a": 2, "b": 1 });
        assert_eq!(canonical_bytes(&a), canonical_bytes(&b));
        assert_eq!(
            String::from_utf8(canonical_bytes(&a)).unwrap(),
            r#"{"a":2,"b":1,"c":{"y":2,"z":1}}"#
        );
    }
}
