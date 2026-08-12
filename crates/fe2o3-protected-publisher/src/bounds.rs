use thiserror::Error;

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_JWT_BYTES: usize = 16 * 1024;
pub const MAX_JWT_SEGMENT_BYTES: usize = 12 * 1024;
pub const MAX_JWKS_BYTES: usize = 256 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 512 * 1024;
pub const MAX_RECEIPT_BYTES: usize = 256 * 1024;
pub const MAX_HTTP_HEADER_BYTES: usize = 32 * 1024;
pub const MAX_HTTP_HEADERS: usize = 32;
pub const MAX_JSON_DEPTH: usize = 32;
pub const MAX_JSON_KEYS: usize = 512;
pub const MAX_JSON_TOKENS: usize = 4096;
pub const MAX_JSON_STRING_BYTES: usize = 4096;
pub const MAX_JWKS_KEYS: usize = 16;
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;
pub const MAX_PRIVATE_KEY_BYTES: usize = 16 * 1024;
pub const MAX_OIDC_LIFETIME_SECS: i64 = 10 * 60;
pub const MAX_CLOCK_SKEW_SECS: i64 = 5 * 60;
pub const RECEIPT_LIFETIME_SECS: i64 = 60 * 60;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BoundError {
    #[error("input exceeds its byte bound")]
    Bytes,
    #[error("JSON nesting exceeds its bound")]
    Depth,
    #[error("JSON member count exceeds its bound")]
    Keys,
    #[error("JSON token work exceeds its bound")]
    Work,
    #[error("JSON string exceeds its bound")]
    String,
}

pub fn preflight_json(raw: &[u8], byte_limit: usize) -> Result<(), BoundError> {
    if raw.len() > byte_limit {
        return Err(BoundError::Bytes);
    }

    let mut depth = 0usize;
    let mut keys = 0usize;
    let mut tokens = 0usize;
    let mut string_bytes = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut previous_token = false;

    for &byte in raw {
        if in_string {
            string_bytes = string_bytes.checked_add(1).ok_or(BoundError::Work)?;
            if string_bytes > MAX_JSON_STRING_BYTES {
                return Err(BoundError::String);
            }
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }

        match byte {
            b'"' => {
                in_string = true;
                string_bytes = 0;
                tokens = tokens.checked_add(1).ok_or(BoundError::Work)?;
            }
            b'{' | b'[' => {
                depth = depth.checked_add(1).ok_or(BoundError::Depth)?;
                if depth > MAX_JSON_DEPTH {
                    return Err(BoundError::Depth);
                }
                tokens = tokens.checked_add(1).ok_or(BoundError::Work)?;
            }
            b'}' | b']' => {
                depth = depth.checked_sub(1).ok_or(BoundError::Depth)?;
                tokens = tokens.checked_add(1).ok_or(BoundError::Work)?;
            }
            b':' => {
                keys = keys.checked_add(1).ok_or(BoundError::Keys)?;
                if keys > MAX_JSON_KEYS {
                    return Err(BoundError::Keys);
                }
                tokens = tokens.checked_add(1).ok_or(BoundError::Work)?;
            }
            b',' => tokens = tokens.checked_add(1).ok_or(BoundError::Work)?,
            b' ' | b'\n' | b'\r' | b'\t' => previous_token = false,
            _ if !previous_token => {
                tokens = tokens.checked_add(1).ok_or(BoundError::Work)?;
                previous_token = true;
            }
            _ => {}
        }
        if tokens > MAX_JSON_TOKENS {
            return Err(BoundError::Work);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_byte_boundary_is_enforced() {
        assert!(preflight_json(b"{}", 2).is_ok());
        assert_eq!(preflight_json(b"{}", 1), Err(BoundError::Bytes));
    }

    #[test]
    fn deep_and_member_heavy_inputs_reject() {
        let deep = format!(
            "{}0{}",
            "[".repeat(MAX_JSON_DEPTH + 1),
            "]".repeat(MAX_JSON_DEPTH + 1)
        );
        assert_eq!(
            preflight_json(deep.as_bytes(), deep.len()),
            Err(BoundError::Depth)
        );
        let many = format!(
            "{{{}}}",
            (0..=MAX_JSON_KEYS)
                .map(|i| format!("\"k{i}\":0"))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(
            preflight_json(many.as_bytes(), many.len()),
            Err(BoundError::Keys)
        );
    }
}
