use std::collections::HashSet;

use fe2o3_device::{Fp8E4M3Fnuz, Fp8E4M3Fnuzx4, Fp8E5M2Fnuz, Fp8E5M2Fnuzx4};

const GOLDEN: &str = include_str!("fixtures/fp8_gfx942_rocm.golden");

fn parse_u8(value: &str) -> u8 {
    u8::from_str_radix(value, 16).unwrap_or_else(|error| panic!("invalid u8 {value}: {error}"))
}

fn parse_u32(value: &str) -> u32 {
    u32::from_str_radix(value, 16).unwrap_or_else(|error| panic!("invalid u32 {value}: {error}"))
}

fn metadata(key: &str) -> &'static str {
    GOLDEN
        .lines()
        .find_map(|line| {
            let mut fields = line.split_ascii_whitespace();
            match (fields.next(), fields.next(), fields.next(), fields.next()) {
                (Some("meta"), Some(found), Some(value), None) if found == key => Some(value),
                _ => None,
            }
        })
        .unwrap_or_else(|| panic!("missing metadata {key}"))
}

#[test]
fn golden_identifies_the_exact_gfx942_toolchain_contract() {
    assert_eq!(metadata("schema"), "fe2o3-fp8-gfx942-golden-v1");
    assert_eq!(metadata("target"), "gfx942");
    assert_eq!(metadata("rocm-release"), "7.2.4");
    assert_eq!(metadata("hip-version"), "7.2.53211-97f5574fe2");
    assert_eq!(metadata("clang-version"), "22.0.0git");
    assert_eq!(
        metadata("clang-revision"),
        "f58b06dce1f9c15707c5f808fd002e18c2accf7e"
    );
    assert_eq!(metadata("rounding"), "rne");
    assert_eq!(metadata("saturation"), "satfinite");
    assert_eq!(metadata("fnuz-nan-f32"), "ffc00000");
    assert_eq!(metadata("widening-cases"), "512");
    assert_eq!(metadata("narrowing-cases"), "2092");
    assert_eq!(metadata("packed-cases"), "14");

    for digest in [metadata("oracle-sha256"), metadata("generator-sha256")] {
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}

#[test]
fn every_widening_matches_the_native_gfx942_golden() {
    let mut seen_e4 = [false; 256];
    let mut seen_e5 = [false; 256];
    let mut count = 0;

    for line in GOLDEN.lines().filter(|line| line.starts_with("widen ")) {
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        assert_eq!(fields.len(), 4, "malformed record: {line}");
        let raw = parse_u8(fields[2]);
        let expected = parse_u32(fields[3]);
        let (seen, actual) = match fields[1] {
            "e4" => (&mut seen_e4, Fp8E4M3Fnuz::from_bits(raw).to_f32().to_bits()),
            "e5" => (&mut seen_e5, Fp8E5M2Fnuz::from_bits(raw).to_f32().to_bits()),
            format => panic!("unknown widening format {format}"),
        };
        assert!(!seen[raw as usize], "duplicate widening record: {line}");
        seen[raw as usize] = true;
        assert_eq!(actual, expected, "widening mismatch: {line}");
        count += 1;
    }

    assert_eq!(count, 512);
    assert!(seen_e4.into_iter().all(|seen| seen));
    assert!(seen_e5.into_iter().all(|seen| seen));
}

#[test]
fn every_narrowing_case_matches_the_native_gfx942_golden() {
    let mut labels = HashSet::new();
    let mut per_format = [0_usize; 2];
    let mut categories = [[0_usize; 3]; 2];

    for line in GOLDEN.lines().filter(|line| line.starts_with("narrow ")) {
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        assert_eq!(fields.len(), 5, "malformed record: {line}");
        let input = f32::from_bits(parse_u32(fields[3]));
        let expected = parse_u8(fields[4]);
        let (format_index, actual) = match fields[1] {
            "e4" => (0, Fp8E4M3Fnuz::from_f32(input).to_bits()),
            "e5" => (1, Fp8E5M2Fnuz::from_f32(input).to_bits()),
            format => panic!("unknown narrowing format {format}"),
        };
        assert!(
            labels.insert((fields[1], fields[2])),
            "duplicate narrowing label: {line}"
        );
        let category = if fields[2].starts_with("exact-") {
            0
        } else if fields[2].starts_with("boundary-") {
            1
        } else if fields[2].starts_with("exception-") {
            2
        } else {
            panic!("unknown narrowing category: {line}");
        };
        per_format[format_index] += 1;
        categories[format_index][category] += 1;
        assert_eq!(actual, expected, "narrowing mismatch: {line}");
    }

    assert_eq!(per_format, [1046, 1046]);
    assert_eq!(categories, [[256, 762, 28], [256, 762, 28]]);
}

#[test]
fn every_packed_lane_case_matches_the_native_gfx942_golden() {
    let mut labels = HashSet::new();
    let mut per_format = [0_usize; 2];

    for line in GOLDEN.lines().filter(|line| line.starts_with("pack ")) {
        let fields: Vec<_> = line.split_ascii_whitespace().collect();
        assert_eq!(fields.len(), 8, "malformed record: {line}");
        let lanes = [
            parse_u8(fields[3]),
            parse_u8(fields[4]),
            parse_u8(fields[5]),
            parse_u8(fields[6]),
        ];
        let expected = parse_u32(fields[7]);
        let (format_index, actual, unpacked) = match fields[1] {
            "e4" => {
                let packed = Fp8E4M3Fnuzx4::from_array(lanes.map(Fp8E4M3Fnuz::from_bits));
                (
                    0,
                    packed.to_bits(),
                    packed.to_array().map(Fp8E4M3Fnuz::to_bits),
                )
            }
            "e5" => {
                let packed = Fp8E5M2Fnuzx4::from_array(lanes.map(Fp8E5M2Fnuz::from_bits));
                (
                    1,
                    packed.to_bits(),
                    packed.to_array().map(Fp8E5M2Fnuz::to_bits),
                )
            }
            format => panic!("unknown packed format {format}"),
        };
        assert!(
            labels.insert((fields[1], fields[2])),
            "duplicate packed label: {line}"
        );
        per_format[format_index] += 1;
        assert_eq!(actual, expected, "packed mismatch: {line}");
        assert_eq!(unpacked, lanes, "unpacked lane mismatch: {line}");
    }

    assert_eq!(per_format, [7, 7]);
}
