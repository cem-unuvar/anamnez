//! Deterministic JSON canonicalization for `audit_log.metadata`.
//!
//! Rules:
//! - object keys lexicographically sorted at every depth
//! - no whitespace
//! - strings serialized via `serde_json`'s compact form (handles escapes)
//! - numbers via `serde_json`'s compact form
//! - output is UTF-8 bytes

use serde_json::Value as JsonValue;

/// Emit the canonical UTF-8 byte representation of `value` for hashing.
#[must_use]
pub fn to_canonical_bytes(value: &JsonValue) -> Vec<u8> {
    let mut out = Vec::new();
    write_value(value, &mut out);
    out
}

fn write_value(v: &JsonValue, out: &mut Vec<u8>) {
    match v {
        JsonValue::Null => out.extend_from_slice(b"null"),
        JsonValue::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        JsonValue::Number(n) => out.extend_from_slice(n.to_string().as_bytes()),
        JsonValue::String(s) => {
            // serde_json::to_string handles JSON escapes correctly.
            let encoded = serde_json::to_string(s).expect("string to JSON cannot fail");
            out.extend_from_slice(encoded.as_bytes());
        }
        JsonValue::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(item, out);
            }
            out.push(b']');
        }
        JsonValue::Object(map) => {
            // Collect keys, sort lexicographically by UTF-8 bytes.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push(b'{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                let key_encoded = serde_json::to_string(*k).expect("string key encode");
                out.extend_from_slice(key_encoded.as_bytes());
                out.push(b':');
                write_value(&map[*k], out);
            }
            out.push(b'}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn null_bool_number() {
        assert_eq!(to_canonical_bytes(&json!(null)), b"null");
        assert_eq!(to_canonical_bytes(&json!(true)), b"true");
        assert_eq!(to_canonical_bytes(&json!(42)), b"42");
    }

    #[test]
    fn keys_sorted_at_every_depth() {
        let v = json!({ "z": 1, "a": 2, "m": { "y": 3, "b": 4 } });
        let bytes = to_canonical_bytes(&v);
        let s = std::str::from_utf8(&bytes).unwrap();
        assert_eq!(s, r#"{"a":2,"m":{"b":4,"y":3},"z":1}"#);
    }

    #[test]
    fn arrays_preserve_order() {
        let v = json!([3, 1, 2]);
        let s = std::str::from_utf8(&to_canonical_bytes(&v))
            .unwrap()
            .to_owned();
        assert_eq!(s, "[3,1,2]");
    }

    #[test]
    fn deterministic_across_input_orderings() {
        let a = json!({ "x": 1, "y": 2 });
        let b: serde_json::Value = serde_json::from_str(r#"{"y":2,"x":1}"#).unwrap();
        assert_eq!(to_canonical_bytes(&a), to_canonical_bytes(&b));
    }
}
