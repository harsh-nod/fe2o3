//! Explicit native V5 single-device qualification, never the legacy pair mode.
use std::ffi::OsString;

pub const OUTPUT_ELEMENTS: [usize; 2] = [64, 37];

#[derive(Debug, Eq, PartialEq)]
pub struct Case {
    pub producer_source: String,
    pub device: u64,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    let mut args = args.into_iter();
    let mut next = || {
        args.next()
            .ok_or_else(|| "incomplete explicit native V5 arguments".to_owned())?
            .into_string()
            .map_err(|_| "native V5 arguments must be UTF-8".to_owned())
    };
    if next()? != "--native-v5" || next()? != "--producer-source" {
        return Err("native fixture requires --native-v5 --producer-source".into());
    }
    let source = next()?;
    if source.is_empty() || source.len() > 4096 || source.as_bytes().contains(&0) {
        return Err("native fixture source spelling is empty or oversized".into());
    }
    if next()? != "--device" {
        return Err("native fixture requires exactly one explicit device".into());
    }
    let device = next()?;
    let digits = device
        .strip_prefix("0x")
        .filter(|digits| {
            digits.len() == 16
                && digits
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        })
        .ok_or("native device requires 0x and 16 lowercase hex digits")?;
    let device = u64::from_str_radix(digits, 16).map_err(|error| error.to_string())?;
    if device == 0 || args.next().is_some() {
        return Err("native fixture requires one nonzero device and no extra arguments".into());
    }
    Ok(Case {
        producer_source: source,
        device,
    })
}

pub fn check_output(invocation: usize, output: &[u32]) -> Result<(), String> {
    let expected = OUTPUT_ELEMENTS
        .get(invocation)
        .ok_or("unknown native invocation")?;
    if output.len() != *expected
        || output
            .iter()
            .enumerate()
            .any(|(i, value)| *value != i as u32)
    {
        return Err(format!(
            "native output {invocation} differs from the exact closed fill"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Result<Case, String> {
        parse(values.iter().map(OsString::from))
    }

    #[test]
    fn native_v5_requires_explicit_source_and_canonical_single_uid() {
        let valid = [
            "--native-v5",
            "--producer-source",
            "src/lib.rs",
            "--device",
            "0x000000000000abcd",
        ];
        assert_eq!(
            args(&valid).unwrap(),
            Case {
                producer_source: "src/lib.rs".into(),
                device: 0xabcd
            }
        );
        for count in 0..valid.len() {
            assert!(args(&valid[..count]).is_err());
        }
        for uid in [
            "0xabcd",
            "0x000000000000ABCD",
            "0x0000000000000000",
            "0X000000000000abcd",
            "000000000000abcd",
            "0x000000000000abcd ",
        ] {
            let mut changed = valid;
            changed[4] = uid;
            assert!(args(&changed).is_err());
        }
        let mut changed = valid;
        changed[0] = "--receipt-coexistence";
        assert!(args(&changed).is_err());
        let mut extra = valid.to_vec();
        extra.push("legacy");
        assert!(args(&extra).is_err());
        let mut changed = valid;
        changed[2] = "";
        assert!(args(&changed).is_err());
        let mut changed = valid;
        changed[2] = "a\0b";
        assert!(args(&changed).is_err());
    }

    #[test]
    fn native_v5_output_oracle_rejects_extent_and_value_substitution() {
        for (invocation, count) in OUTPUT_ELEMENTS.into_iter().enumerate() {
            let mut output: Vec<_> = (0..count as u32).collect();
            check_output(invocation, &output).unwrap();
            output[count - 1] = u32::MAX;
            assert!(check_output(invocation, &output).is_err());
            output.pop();
            assert!(check_output(invocation, &output).is_err());
        }
        assert!(check_output(2, &[]).is_err());
    }
}
