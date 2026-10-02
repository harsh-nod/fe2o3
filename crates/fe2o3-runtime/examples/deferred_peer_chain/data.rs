use super::*;

pub(super) const ELEMENTS: usize = 65_536;
pub(super) const BYTES: usize = ELEMENTS * 4;
pub(super) const PAGE: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReturnLayout {
    pub source_offset: usize,
    pub returned_offset: usize,
    pub host_offset: usize,
    pub copy_bytes: usize,
    pub returned_bytes: usize,
    pub host_bytes: usize,
}

impl ReturnLayout {
    pub fn new(window: bool) -> Self {
        if window {
            Self {
                source_offset: 20,
                returned_offset: 131,
                host_offset: 97,
                copy_bytes: BYTES - 64,
                returned_bytes: BYTES + 769,
                host_bytes: BYTES + 1153,
            }
        } else {
            Self {
                source_offset: 0,
                returned_offset: 0,
                host_offset: 0,
                copy_bytes: BYTES,
                returned_bytes: BYTES,
                host_bytes: BYTES,
            }
        }
    }

    pub fn returned_initial(self) -> Vec<u8> {
        if self == Self::new(false) {
            filled(-23.0)
        } else {
            vec![0xa5; self.returned_bytes]
        }
    }
}

pub(super) fn expected_window_output() -> [Vec<u8>; 3] {
    let layout = ReturnLayout::new(true);
    let source = expected_d();
    let copied = &source[layout.source_offset..layout.source_offset + layout.copy_bytes];
    let mut returned = layout.returned_initial();
    returned[layout.returned_offset..layout.returned_offset + layout.copy_bytes]
        .copy_from_slice(copied);
    let mut host = vec![0x5a; layout.host_bytes];
    host[layout.host_offset..layout.host_offset + layout.copy_bytes].copy_from_slice(copied);
    [source, returned, host]
}

pub(super) fn verify_window_output(snapshots: &[Vec<u8>; 3]) -> ResultV1<String> {
    if snapshots != &expected_window_output() {
        return Err("complete independent D/E/host window oracle differs".into());
    }
    let mut framed = b"fe2o3.pending-compute-return-window.v1\0".to_vec();
    for bytes in snapshots {
        framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        framed.extend_from_slice(bytes);
    }
    Ok(digest(&framed))
}

pub(super) fn unique_ids(arguments: &[String]) -> ResultV1<[u64; 2]> {
    if arguments.len() != 2 {
        return Err(USAGE.into());
    }
    let mut ids = [0; 2];
    for (index, argument) in arguments.iter().enumerate() {
        let hex = argument
            .strip_prefix("0x")
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(USAGE)?;
        ids[index] = u64::from_str_radix(hex, 16).map_err(|_| USAGE)?;
    }
    if ids.contains(&0) || ids[0] == ids[1] {
        return Err(USAGE.into());
    }
    Ok(ids)
}

pub(super) fn filled(value: f32) -> Vec<u8> {
    value.to_le_bytes().repeat(ELEMENTS)
}

pub(super) fn inputs() -> [Vec<u8>; 4] {
    let a = (0..ELEMENTS)
        .flat_map(|index| (((index & 63) as f32) * 0.25).to_le_bytes())
        .collect();
    let b = (0..ELEMENTS)
        .flat_map(|index| (((index & 31) as f32) * 0.5).to_le_bytes())
        .collect();
    [a, b, filled(0.25), filled(0.5)]
}

pub(super) fn expected_c() -> Vec<u8> {
    (0..ELEMENTS)
        .flat_map(|index| {
            (((index & 63) as f32) * 0.25 + ((index & 31) as f32) * 0.5).to_le_bytes()
        })
        .collect()
}

// All terms are exactly representable quarters; no authority output is imported.
pub(super) fn expected_d() -> Vec<u8> {
    (0..ELEMENTS)
        .flat_map(|index| (((index & 63) as f32) * 0.25 + (index & 31) as f32).to_le_bytes())
        .collect()
}

pub(super) fn digest(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut text, "{byte:02x}").expect("String formatting");
    }
    text
}

pub(super) fn verify_output(bytes: &[u8]) -> ResultV1<String> {
    if BYTES.div_ceil(PAGE) * PAGE != BYTES || bytes.len() != BYTES {
        return Err("full page-aligned output extent differs".into());
    }
    let expected = expected_d();
    if bytes != expected {
        return Err("complete independent output oracle differs".into());
    }
    Ok(digest(bytes))
}

pub(super) struct Receipts<I = RuntimeSubmissionIdV1> {
    observations: [Option<(I, RuntimeCompletionStatusV1)>; 4],
    deliveries: [usize; 4],
}

impl<I: Copy + Eq> Receipts<I> {
    pub fn new() -> Self {
        Self {
            observations: [None; 4],
            deliveries: [0; 4],
        }
    }
    pub fn record(&mut self, index: usize, id: I, status: RuntimeCompletionStatusV1) {
        self.deliveries[index] = self.deliveries[index].saturating_add(1);
        self.observations[index] = Some((id, status));
    }
    pub fn succeeded(&self, ids: &[I; 4]) -> bool {
        ids.iter().enumerate().all(|(index, &id)| {
            self.deliveries[index] == 1
                && self.observations[index] == Some((id, RuntimeCompletionStatusV1::Succeeded))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_window_checks_all_three_full_extents_and_guard_boundaries() {
        let layout = ReturnLayout::new(true);
        assert_eq!(layout.source_offset + layout.copy_bytes, BYTES - 44);
        assert_eq!(
            layout.returned_offset + layout.copy_bytes,
            layout.returned_bytes - 702
        );
        assert_eq!(
            layout.host_offset + layout.copy_bytes,
            layout.host_bytes - 1120
        );
        let snapshots = expected_window_output();
        assert!(verify_window_output(&snapshots).is_ok());
        for (index, offset) in [
            layout.source_offset,
            layout.returned_offset,
            layout.host_offset,
        ]
        .into_iter()
        .enumerate()
        {
            for byte in [
                0,
                offset - 1,
                offset,
                offset + layout.copy_bytes - 1,
                offset + layout.copy_bytes,
                snapshots[index].len() - 1,
            ] {
                let mut corrupt = snapshots.clone();
                corrupt[index][byte] ^= 1;
                assert!(verify_window_output(&corrupt).is_err());
            }
            let mut truncated = snapshots.clone();
            truncated[index].pop();
            assert!(verify_window_output(&truncated).is_err());
            let mut extended = snapshots.clone();
            extended[index].push(0);
            assert!(verify_window_output(&extended).is_err());
        }
        let mut absent = snapshots.clone();
        absent[0] = expected_c();
        assert!(verify_window_output(&absent).is_err());
        absent = snapshots.clone();
        absent[1] = layout.returned_initial();
        assert!(verify_window_output(&absent).is_err());
        absent = snapshots;
        absent[2].fill(0x5a);
        assert!(verify_window_output(&absent).is_err());
    }

    #[test]
    fn cli_preserves_two_explicit_device_identities_and_rejects_ambiguity() {
        assert_eq!(unique_ids(&["0x2".into(), "0x1".into()]).unwrap(), [2, 1]);
        for arguments in [
            vec![],
            vec!["0x1"],
            vec!["0x1", "0x2", "0x3"],
            vec!["0x0", "0x1"],
            vec!["0x1", "0x01"],
            vec!["1", "0x2"],
            vec!["0x", "0x2"],
            vec!["0x10000000000000000", "0x2"],
            vec!["--queued", "0x2"],
        ] {
            assert!(
                unique_ids(&arguments.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err()
            );
        }
    }

    #[test]
    fn independent_inputs_oracle_and_full_write_extent_match_unchanged_r57_v2() {
        let admitted = admit_gfx942_r57_n3_qualification_v2().unwrap();
        let fixture = admitted.host_buffers().unwrap();
        let [a, b, c, d] = inputs();
        assert_eq!(a, fixture.a());
        assert_eq!(b, fixture.b());
        assert_eq!(c, fixture.c_initial());
        assert_eq!(d, fixture.d_initial());
        assert_eq!(expected_c(), fixture.expected_c());
        assert_eq!(expected_d(), fixture.expected_d());
        assert_eq!(BYTES, GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1);
        assert_eq!(ELEMENTS, GFX942_R57_N3_QUALIFICATION_ELEMENTS_V1);
        assert_eq!(BYTES % PAGE, 0);
        assert_eq!(
            GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1[2].access,
            RuntimeAccessV1::Write
        );
    }

    #[test]
    fn full_output_rejects_initial_sentinels_missing_compute_corruption_and_wrong_extent() {
        let expected = expected_d();
        assert_eq!(verify_output(&expected).unwrap(), digest(&expected));
        for wrong in [filled(0.5), filled(-1.0), filled(-23.0), expected_c()] {
            assert!(verify_output(&wrong).is_err());
        }
        for index in [0, BYTES / 2, BYTES - 1] {
            let mut wrong = expected.clone();
            wrong[index] ^= 1;
            assert!(verify_output(&wrong).is_err());
        }
        assert!(verify_output(&expected[..BYTES - 1]).is_err());
        let mut extra = expected;
        extra.push(0);
        assert!(verify_output(&extra).is_err());
    }

    #[test]
    fn callbacks_require_all_four_original_ids_once_and_success() {
        let ids = [11_u64, 12, 13, 14];
        let mut receipts = Receipts::new();
        for (index, &id) in ids.iter().enumerate() {
            receipts.record(index, id, RuntimeCompletionStatusV1::Succeeded);
        }
        assert!(receipts.succeeded(&ids));
        assert!(!receipts.succeeded(&[11, 12, 13, 15]));
        receipts.observations[1] = Some((12, RuntimeCompletionStatusV1::QuiescentWithoutResult));
        assert!(!receipts.succeeded(&ids));
        receipts.observations[1] = Some((12, RuntimeCompletionStatusV1::Succeeded));
        receipts.record(1, 12, RuntimeCompletionStatusV1::Succeeded);
        assert!(!receipts.succeeded(&ids));
    }
}
