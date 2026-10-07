//! Data-only single-device configuration and output oracles, never runtime authority.
use std::ffi::OsString;

pub const ELEMENTS: usize = 65;
pub const GUARD_BYTES: usize = 64;
pub const COPY_BYTES: usize = 4096;
pub const FRAME_BYTES: usize = COPY_BYTES + 2 * GUARD_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Case {
    pub device: u64,
}

pub fn parse_case(args: &[OsString]) -> Result<Option<Case>, String> {
    if args
        .first()
        .is_none_or(|arg| arg != "--receipt-coexistence")
    {
        return Ok(None);
    }
    if args.len() != 2 {
        return Err("receipt coexistence requires exactly one GPU unique ID".into());
    }
    let digits = args[1]
        .to_str()
        .and_then(|text| text.strip_prefix("0x"))
        .ok_or("GPU unique ID requires a UTF-8 0x-prefixed hexadecimal value")?;
    if digits.is_empty() || digits.len() > 16 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("GPU unique ID must contain 1 to 16 hexadecimal digits".into());
    }
    let device = u64::from_str_radix(digits, 16).map_err(|error| error.to_string())?;
    if device == 0 {
        return Err("GPU unique ID must be nonzero".into());
    }
    Ok(Some(Case { device }))
}

pub fn source_frame() -> Vec<u8> {
    let mut bytes = vec![0x3c; FRAME_BYTES];
    for (index, byte) in bytes[GUARD_BYTES..GUARD_BYTES + COPY_BYTES]
        .iter_mut()
        .enumerate()
    {
        *byte = (index as u8).wrapping_mul(17).wrapping_add(11);
    }
    bytes
}

pub fn destination_frame() -> Vec<u8> {
    vec![0xa5; FRAME_BYTES]
}

pub fn check_results(fill: &[u32], source: &[u8], destination: &[u8]) -> Result<(), String> {
    if fill.len() != ELEMENTS || fill.iter().enumerate().any(|(i, value)| *value != i as u32) {
        return Err("generated fill content or extent differs".into());
    }
    let original = source_frame();
    let mut expected = destination_frame();
    expected[GUARD_BYTES..GUARD_BYTES + COPY_BYTES]
        .copy_from_slice(&original[GUARD_BYTES..GUARD_BYTES + COPY_BYTES]);
    if source != original || destination != expected {
        return Err("independent copy payload, source or guard bytes differ".into());
    }
    Ok(())
}

/// The fixture passes actual witness getters here; CPU tests use ordinary data.
pub struct ReceiptObservation {
    pub device: u64,
    pub copy_first: bool,
    pub packets: usize,
    pub identities: [[u8; 32]; 4],
}

pub struct Report {
    device: u64,
    identities: [[u8; 32]; 4],
}

impl Case {
    pub fn check_witness(self, observed: ReceiptObservation) -> Result<Report, String> {
        if observed.device != self.device
            || self.device == 0
            || !observed.copy_first
            || observed.packets != 1
            || observed.identities.iter().any(|hash| *hash == [0; 32])
            || observed.identities[0] == observed.identities[2]
            || observed.identities[1] == observed.identities[3]
        {
            return Err("missing or mismatched exact second-publication witness".into());
        }
        Ok(Report {
            device: self.device,
            identities: observed.identities,
        })
    }
}

impl Report {
    /// Emitted only after separate output, owned-shutdown and full-refund checks.
    pub fn json(&self) -> String {
        let hex = |bytes: [u8; 32]| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        format!(
            concat!(
                "{{\"schema\":\"fe2o3.genuine-receipt-coexistence.v1\",\"device\":\"{:#018x}\",",
                "\"artifact_profile\":\"legacy-conditional-fill\",\"elements\":65,\"copy_bytes\":4096,",
                "\"first_publication\":\"directional-copy\",\"copy_packets\":1,",
                "\"compute_receipt_sha256\":\"{}\",\"compute_membership_sha256\":\"{}\",",
                "\"copy_receipt_sha256\":\"{}\",\"copy_membership_sha256\":\"{}\",",
                "\"receipt_coexistence\":\"before-host-completion-consumption\",",
                "\"physical_overlap\":\"not-measured\",\"device_scope\":\"single-selected-device\",",
                "\"shutdown\":\"released\",\"result_credits\":\"fully-refunded\"}}"
            ),
            self.device,
            hex(self.identities[0]),
            hex(self.identities[1]),
            hex(self.identities[2]),
            hex(self.identities[3])
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_single_device_selector_has_no_roster_or_pair_fallback() {
        let parse =
            |args: &[&str]| parse_case(&args.iter().map(OsString::from).collect::<Vec<_>>());
        assert_eq!(parse(&[]), Ok(None));
        assert_eq!(parse(&["--roster", "0x1", "0x2"]), Ok(None));
        assert_eq!(
            parse(&["--receipt-coexistence", "0x75"]),
            Ok(Some(Case { device: 0x75 }))
        );
        for args in [
            vec!["--receipt-coexistence"],
            vec!["--receipt-coexistence", "0x0"],
            vec!["--receipt-coexistence", "1"],
            vec!["--receipt-coexistence", "0X1"],
            vec!["--receipt-coexistence", "0x"],
            vec!["--receipt-coexistence", "0x1 "],
            vec!["--receipt-coexistence", "0x10000000000000000"],
            vec!["--receipt-coexistence", "0x1", "0x2"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn full_payload_source_and_both_guard_regions_are_checked() {
        let fill: Vec<_> = (0..ELEMENTS as u32).collect();
        let source = source_frame();
        let mut destination = destination_frame();
        assert!(check_results(&fill, &source, &destination).is_err());
        destination[GUARD_BYTES..GUARD_BYTES + COPY_BYTES]
            .copy_from_slice(&source[GUARD_BYTES..GUARD_BYTES + COPY_BYTES]);
        assert!(check_results(&fill, &source, &destination).is_ok());
        for index in [
            0,
            GUARD_BYTES - 1,
            GUARD_BYTES,
            GUARD_BYTES + COPY_BYTES - 1,
            FRAME_BYTES - 1,
        ] {
            let mut corrupted = destination.clone();
            corrupted[index] ^= 1;
            assert!(check_results(&fill, &source, &corrupted).is_err());
            let mut corrupted = source.clone();
            corrupted[index] ^= 1;
            assert!(check_results(&fill, &corrupted, &destination).is_err());
        }
        assert!(check_results(&fill[..ELEMENTS - 1], &source, &destination).is_err());
        assert!(check_results(&fill, &source[..FRAME_BYTES - 1], &destination).is_err());
    }

    #[test]
    fn data_only_witness_oracle_rejects_scope_identity_order_and_packet_changes() {
        let case = Case { device: 7 };
        for mutation in 0..9 {
            let mut observation = ReceiptObservation {
                device: 7,
                copy_first: true,
                packets: 1,
                identities: [[1; 32], [2; 32], [3; 32], [4; 32]],
            };
            match mutation {
                0 => observation.device = 8,
                1 => observation.copy_first = false,
                2 => observation.packets = 0,
                3 => observation.packets = 2,
                4 => observation.identities[0] = [0; 32],
                5 => observation.identities[2] = observation.identities[0],
                6 => observation.identities[3] = observation.identities[1],
                7 => observation.identities[3] = [0; 32],
                _ => {}
            }
            let report = case.check_witness(observation);
            assert_eq!(report.is_ok(), mutation == 8);
            if let Ok(report) = report {
                assert!(
                    report
                        .json()
                        .contains("\"physical_overlap\":\"not-measured\"")
                );
                assert!(report.json().contains("legacy-conditional-fill"));
            }
        }
    }
}
