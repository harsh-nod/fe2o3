//! Inert closed-count oracle; it does not qualify native execution or ordering.
use super::native_registry4_case::{Case, parse_profile};
use std::ffi::OsString;

pub const OUTPUT_ELEMENTS: [usize; 16] = [
    1, 65, 129, 193, 257, 321, 385, 449, 513, 577, 641, 705, 769, 833, 897, 961,
];
pub const GRID_X: [u32; 16] = [
    64, 128, 192, 256, 320, 384, 448, 512, 576, 640, 704, 768, 832, 896, 960, 1024,
];
pub const SOURCE_BYTES: u64 = 30_784;

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    parse_profile(args, "--native-v5-registry16")
}
pub fn check_output(member: usize, output: &[u32]) -> Result<(), String> {
    let expected = OUTPUT_ELEMENTS
        .get(member)
        .ok_or("unknown registry16 member")?;
    if output.len() != *expected
        || output
            .iter()
            .enumerate()
            .any(|(i, value)| *value != i as u32)
    {
        return Err(format!("registry16 original output {member} mismatch"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry16_marker_never_accepts_four_or_repeat2_fallback() {
        let mut args = [
            "--native-v5-registry16",
            "--producer-source",
            "src/lib.rs",
            "--device",
            "0x000000000000abcd",
        ];
        assert_eq!(parse(args.map(OsString::from)).unwrap().device, 0xabcd);
        assert!(super::super::native_registry4_case::parse(args.map(OsString::from)).is_err());
        for marker in [
            "--native-v5",
            "--native-v5-registry4",
            "--native-v5-registry4-repeat2",
        ] {
            args[0] = marker;
            assert!(parse(args.map(OsString::from)).is_err());
        }
    }
    #[test]
    fn registry16_exact_extents_bytes_and_padded_full64_grid() {
        let mut total = 0;
        for (member, count) in OUTPUT_ELEMENTS.into_iter().enumerate() {
            assert_eq!(count, 1 + 64 * member);
            assert_eq!(GRID_X[member] as usize, (member + 1) * 64);
            assert!(count <= GRID_X[member] as usize);
            let mut output: Vec<_> = (0..count as u32).collect();
            check_output(member, &output).unwrap();
            output[count - 1] = u32::MAX;
            assert!(check_output(member, &output).is_err());
            output.pop();
            assert!(check_output(member, &output).is_err());
            total += count as u64 * 4;
        }
        assert_eq!(total, SOURCE_BYTES);
        assert!(check_output(16, &[]).is_err());
    }
}
