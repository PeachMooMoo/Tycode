//! Converts to/from AWS's stupid document type
use std::collections::HashMap;

use aws_smithy_types::{Document, Number};
use serde_json::Value;

pub fn to_doc(v: serde_json::Value) -> Document {
    match v {
        Value::Null => Document::Null,
        Value::Bool(b) => Document::Bool(b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Document::from(i)
            } else if let Some(u) = n.as_u64() {
                Document::from(u)
            } else {
                Document::from(n.as_f64().unwrap())
            }
        }
        Value::String(s) => Document::String(s),
        Value::Array(a) => Document::Array(a.into_iter().map(to_doc).collect()),
        Value::Object(m) => Document::Object(
            m.into_iter()
                .map(|(k, v)| (k, to_doc(v)))
                .collect::<HashMap<_, _>>(),
        ),
    }
}

pub fn from_doc(doc: Document) -> Value {
    match doc {
        Document::Null => Value::Null,
        Document::Bool(b) => Value::Bool(b),

        Document::Number(n) => {
            let num = match n {
                Number::PosInt(pos) => serde_json::Number::from(pos),
                Number::NegInt(neg) => serde_json::Number::from(neg),
                Number::Float(f) => serde_json::Number::from_f64(f).expect("unable to convert f64"),
            };
            Value::Number(num)
        }

        Document::String(s) => Value::String(s),

        Document::Array(a) => Value::Array(a.into_iter().map(from_doc).collect()),

        Document::Object(m) => {
            // smithy uses std::collections::HashMap<String, Document>
            // serde_json uses serde_json::Map<String, Value>
            let mut map = serde_json::Map::with_capacity(m.len());
            for (k, v) in m {
                map.insert(k, from_doc(v));
            }
            Value::Object(map)
        }
    }
}
