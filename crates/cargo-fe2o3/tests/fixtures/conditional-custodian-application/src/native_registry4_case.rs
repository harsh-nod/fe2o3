//! Inert arguments/output oracle for the explicit four-original registry caller.
use std::ffi::OsString;

#[path = "native_registry4_case/observation.rs"]
pub mod observation;

pub const OUTPUT_ELEMENTS: [usize; 4] = [1, 37, 65, 129];
pub const GRID_X: [u32; 4] = [64, 64, 128, 192];
pub const SOURCE_BYTES: u64 = 4 * (1 + 37 + 65 + 129);

#[derive(Debug, Eq, PartialEq)]
pub struct Case {
    pub producer_source: String,
    pub device: u64,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    parse_profile(args, "--native-v5-registry4")
}

pub fn parse_repeat2(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    parse_profile(args, "--native-v5-registry4-repeat2")
}

pub(super) fn parse_profile(
    args: impl IntoIterator<Item = OsString>,
    profile: &str,
) -> Result<Case, String> {
    let mut args = args.into_iter();
    let mut next = || {
        args.next()
            .ok_or("incomplete native Registry4 arguments")?
            .into_string()
            .map_err(|_| "native Registry4 arguments must be UTF-8")
    };
    if next()? != profile || next()? != "--producer-source" {
        return Err(format!(
            "registry fixture requires {profile} --producer-source"
        ));
    }
    let producer_source = next()?;
    if producer_source.is_empty()
        || producer_source.len() > 4096
        || producer_source.as_bytes().contains(&0)
        || next()? != "--device"
    {
        return Err("registry fixture requires exact source spelling and one device".into());
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
        .ok_or("registry device requires 0x and 16 lowercase hex digits")?;
    let device = u64::from_str_radix(digits, 16).map_err(|error| error.to_string())?;
    if device == 0 || args.next().is_some() {
        return Err("registry fixture requires one nonzero device and no extra arguments".into());
    }
    Ok(Case {
        producer_source,
        device,
    })
}

pub fn check_output(member: usize, output: &[u32]) -> Result<(), String> {
    let expected = OUTPUT_ELEMENTS
        .get(member)
        .ok_or("unknown registry member")?;
    if output.len() != *expected
        || output
            .iter()
            .enumerate()
            .any(|(i, value)| *value != i as u32)
    {
        return Err(format!(
            "registry output {member} differs from exact closed fill"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat2_marker_does_not_fall_back_to_single_cycle() {
        let args = [
            "--native-v5-registry4-repeat2",
            "--producer-source",
            "src/lib.rs",
            "--device",
            "0x000000000000abcd",
        ];
        assert!(parse(args.map(OsString::from)).is_err());
        assert_eq!(
            parse_repeat2(args.map(OsString::from)).unwrap().device,
            0xabcd
        );
        let mut single = args;
        single[0] = "--native-v5-registry4";
        assert!(parse_repeat2(single.map(OsString::from)).is_err());
    }

    #[test]
    fn explicit_registry_family_source_and_device_are_required() {
        let valid = [
            "--native-v5-registry4",
            "--producer-source",
            "src/lib.rs",
            "--device",
            "0x000000000000abcd",
        ];
        let parse_strings = |args: &[&str]| parse(args.iter().map(OsString::from));
        assert_eq!(parse_strings(&valid).unwrap().device, 0xabcd);
        for count in 0..valid.len() {
            assert!(parse_strings(&valid[..count]).is_err());
        }
        for marker in ["--native-v5", "--native-v5-roster", "--receipt-coexistence"] {
            let mut changed = valid;
            changed[0] = marker;
            assert!(parse_strings(&changed).is_err());
        }
        for source in ["", "a\0b", &"a".repeat(4097)] {
            let mut changed = valid;
            changed[2] = source;
            assert!(parse_strings(&changed).is_err());
        }
        for device in ["0xabcd", "0x0000000000000000", "0x000000000000ABCD"] {
            let mut changed = valid;
            changed[4] = device;
            assert!(parse_strings(&changed).is_err());
        }
        let mut extra = valid.to_vec();
        extra.push("fallback");
        assert!(parse_strings(&extra).is_err());
    }

    #[test]
    fn four_distinct_extents_use_padded_full64_grids_and_exact_values() {
        let mut bytes = 0_u64;
        for (member, count) in OUTPUT_ELEMENTS.into_iter().enumerate() {
            assert!(count > 0 && count <= GRID_X[member] as usize);
            assert!(GRID_X[member].is_multiple_of(64));
            let mut output: Vec<_> = (0..count as u32).collect();
            check_output(member, &output).unwrap();
            output[count - 1] = u32::MAX;
            assert!(check_output(member, &output).is_err());
            output.pop();
            assert!(check_output(member, &output).is_err());
            bytes += count as u64 * 4;
        }
        assert_eq!(bytes, SOURCE_BYTES);
        assert!(check_output(4, &[]).is_err());
    }
}
