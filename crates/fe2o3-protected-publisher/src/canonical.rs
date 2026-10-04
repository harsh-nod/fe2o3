use std::collections::BTreeMap;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Number, Value};

use crate::bounds::{BoundError, preflight_json, preflight_json_with_string_limit};

#[derive(Debug, thiserror::Error)]
pub enum CanonicalError {
    #[error("bounded JSON preflight failed")]
    Bound(#[from] BoundError),
    #[error("JSON is malformed or contains duplicate members")]
    Json,
    #[error("JSON is not canonical")]
    NonCanonical,
}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct UniqueVisitor;

        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a duplicate-free JSON value")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Bool(value)))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(Number::from(value))))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Number(Number::from(value))))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Number::from_f64(value)
                    .map(|number| UniqueValue(Value::Number(number)))
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(UniqueValue(Value::String(value.to_owned())))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = sequence.next_element()? {
                    values.try_reserve(1).map_err(de::Error::custom)?;
                    values.push(value);
                }
                Ok(UniqueValue(Value::Array(values)))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((key, UniqueValue(value))) =
                    map.next_entry::<String, UniqueValue>()?
                {
                    if values.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate JSON member"));
                    }
                }
                Ok(UniqueValue(Value::Object(
                    values.into_iter().collect::<Map<_, _>>(),
                )))
            }
        }

        deserializer.deserialize_any(UniqueVisitor)
    }
}

pub fn parse_unique(raw: &[u8], limit: usize) -> Result<Value, CanonicalError> {
    preflight_json(raw, limit)?;
    parse_unique_preflighted(raw)
}

fn parse_unique_preflighted(raw: &[u8]) -> Result<Value, CanonicalError> {
    let mut deserializer = serde_json::Deserializer::from_slice(raw);
    let UniqueValue(value) =
        UniqueValue::deserialize(&mut deserializer).map_err(|_| CanonicalError::Json)?;
    deserializer.end().map_err(|_| CanonicalError::Json)?;
    Ok(value)
}

pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, CanonicalError> {
    let mut bytes = serde_json::to_vec(value).map_err(|_| CanonicalError::Json)?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn parse_canonical(raw: &[u8], limit: usize) -> Result<Value, CanonicalError> {
    let value = parse_unique(raw, limit)?;
    if canonical_bytes(&value)? != raw {
        return Err(CanonicalError::NonCanonical);
    }
    Ok(value)
}

pub(crate) fn parse_canonical_with_string_limit(
    raw: &[u8],
    byte_limit: usize,
    string_byte_limit: usize,
) -> Result<Value, CanonicalError> {
    preflight_json_with_string_limit(raw, byte_limit, string_byte_limit)?;
    let value = parse_unique_preflighted(raw)?;
    if canonical_bytes(&value)? != raw {
        return Err(CanonicalError::NonCanonical);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicates_and_noncanonical_order() {
        assert!(matches!(
            parse_canonical(b"{\"a\":1,\"a\":2}\n", 64),
            Err(CanonicalError::Json)
        ));
        assert!(matches!(
            parse_canonical(b"{\"b\":2,\"a\":1}\n", 64),
            Err(CanonicalError::NonCanonical)
        ));
        assert!(parse_canonical(b"{\"a\":1,\"b\":2}\n", 64).is_ok());
    }
}
