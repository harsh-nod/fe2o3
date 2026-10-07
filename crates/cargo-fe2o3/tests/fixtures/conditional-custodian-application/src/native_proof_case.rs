//! Explicit CPU-only native V5 proof qualification, never a GPU result.
use std::ffi::OsString;

pub const SCHEMA: &str = "fe2o3.native-conditional-proof-only.v1";
pub const FIELDS: &str = concat!(
    "\"target\":\"gfx942:xnack-\",\"proof_only\":true,\"gpu_execution\":false,",
    "\"native_launches\":0,\"native_copies\":0"
);

#[derive(Debug, Eq, PartialEq)]
pub struct Case {
    pub producer_source: String,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    let mut args = args.into_iter();
    let mut next = || {
        args.next()
            .ok_or_else(|| "incomplete explicit native proof-only arguments".to_owned())?
            .into_string()
            .map_err(|_| "native proof-only arguments must be UTF-8".to_owned())
    };
    if next()? != "--native-v5-proof-only" || next()? != "--producer-source" {
        return Err("proof-only fixture requires --native-v5-proof-only --producer-source".into());
    }
    let producer_source = next()?;
    if producer_source.is_empty()
        || producer_source.len() > 4096
        || producer_source
            .bytes()
            .any(|b| matches!(b, 0 | b'\n' | b'\r'))
        || args.next().is_some()
    {
        return Err(
            "proof-only fixture requires one exact source spelling and no device arguments".into(),
        );
    }
    Ok(Case { producer_source })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Result<Case, String> {
        parse(values.iter().map(OsString::from))
    }

    #[test]
    fn explicit_proof_only_preserves_source_spelling_and_refuses_other_modes() {
        let valid = [
            "--native-v5-proof-only",
            "--producer-source",
            "./src/../src/lib.rs",
        ];
        assert_eq!(args(&valid).unwrap().producer_source, valid[2]);
        for n in 0..valid.len() {
            assert!(args(&valid[..n]).is_err());
        }
        for mode in [
            "--native-v5",
            "--native-v5-roster",
            "--receipt-coexistence",
            "",
        ] {
            let mut changed = valid;
            changed[0] = mode;
            assert!(args(&changed).is_err());
        }
        for extra in ["--device", "--devices", "--transport", "ignored"] {
            let mut changed = valid.to_vec();
            changed.push(extra);
            assert!(args(&changed).is_err());
        }
        for spelling in ["", "a\0b", "a\nb", "a\rb"] {
            let mut changed = valid;
            changed[2] = spelling;
            assert!(args(&changed).is_err());
        }
        assert!(args(&[valid[0], valid[1], &"a".repeat(4097)]).is_err());
        assert!(args(&[valid[0], valid[1], &"a".repeat(4096)]).is_ok());
    }

    #[test]
    fn proof_only_schema_has_no_runtime_settlement_or_credit_claim() {
        assert_eq!(SCHEMA, "fe2o3.native-conditional-proof-only.v1");
        for forbidden in ["shutdown", "credits", "settled", "devices", "overlap"] {
            assert!(!FIELDS.contains(forbidden));
        }
        assert!(FIELDS.contains("\"gpu_execution\":false"));
        assert!(FIELDS.contains("\"native_launches\":0"));
        assert!(FIELDS.contains("\"native_copies\":0"));
    }
}
