use super::*;
use std::collections::{BTreeMap, BTreeSet};

fn parse(late: bool, case: Case, reverse: bool) -> Options {
    let ids = if reverse {
        ["0x3", "0x2", "0x1"]
    } else {
        ["0x1", "0x2", "0x3"]
    };
    let mut args = vec![
        if late {
            "--late-forward-window"
        } else {
            "--forward-window"
        }
        .to_owned(),
    ];
    args.extend(ids.map(str::to_owned));
    args.push(case.name().to_owned());
    options(&args).unwrap()
}

#[test]
fn forward_cli_is_separate_bounded_and_preserves_legacy_modes() {
    for late in [false, true] {
        for case in [Case::Overlap, Case::Packets] {
            for reverse in [false, true] {
                let parsed = parse(late, case, reverse);
                assert_eq!(parsed.forward_window, Some(late));
                assert!(!parsed.dispose_source);
                assert_eq!(parsed.case, case);
                assert_eq!(parsed.ids, if reverse { [3, 2, 1] } else { [1, 2, 3] });
            }
        }
    }
    for prefix in ["--forward-window", "--late-forward-window"] {
        for shape in ["disjoint", "duplicates", "4096", "--dispose-source"] {
            assert!(options(&[prefix, "0x1", "0x2", "0x3", shape].map(str::to_owned)).is_err());
        }
        for bad in ["0x0", "0x1", "0x2", "3", "0x10000000000000000"] {
            assert!(options(&[prefix, "0x1", "0x2", bad, "overlap"].map(str::to_owned)).is_err());
        }
        assert!(
            options(
                &[prefix, "--dispose-source", "0x1", "0x2", "0x3", "overlap"].map(str::to_owned)
            )
            .is_err()
        );
    }
    for case in ["disjoint", "overlap", "duplicates", "packets"] {
        let args = ["0x1", "0x2", "0x3", case].map(str::to_owned);
        assert_eq!(options(&args).unwrap().forward_window, None);
        let mut args = args.to_vec();
        args.insert(0, "--dispose-source".into());
        let legacy = options(&args).unwrap();
        assert_eq!(legacy.forward_window, None);
        assert!(legacy.dispose_source);
    }
}

#[test]
fn forward_window_crosses_frame_envelope_and_retains_independent_guards() {
    for case in [Case::Overlap, Case::Packets] {
        let layout = layout(case);
        let window = Window::new(&layout);
        assert!(FRAME_OFFSET < DESTINATION_BASE as usize && window.bytes > 0);
        assert!(
            FRAME_OFFSET + window.bytes > (DESTINATION_BASE + layout.destination_envelope) as usize
        );
        assert_eq!(FRAME_OFFSET + window.bytes + FRAME_SUFFIX, window.frame);
        assert!(TARGET_OFFSET > 0 && TARGET_OFFSET + window.bytes < window.target);
        assert_ne!(window.target, window.frame);
        assert_eq!(window.host, HOST_BASE + window.target + HOST_SUFFIX);
        for (list, source_base) in layout.lists.iter().zip(SOURCE_BASES) {
            assert!(source_base + layout.source_envelope < window.source as u64);
            assert!(DESTINATION_BASE + layout.destination_envelope < window.frame as u64);
            for segment in list {
                assert!(segment.source_offset + segment.byte_len <= layout.source_envelope);
                assert!(
                    segment.destination_offset + segment.byte_len <= layout.destination_envelope
                );
            }
        }
        let scalar = Gfx942ComputeXgmiPacketPlanV1::new(window.bytes as u64).unwrap();
        assert_eq!(scalar.count(), if case == Case::Packets { 3 } else { 1 });
        assert_eq!(
            (0..scalar.count())
                .map(|index| u64::from(scalar.packet(index).unwrap().bytes))
                .sum::<u64>(),
            window.bytes as u64
        );
        if case == Case::Packets {
            assert_eq!(scalar.packet(2).unwrap().bytes, 4403);
        }
    }
}

fn last_writer_byte(layout: &Layout, source: &[u8], offset: usize, round: usize) -> u8 {
    for index in (0..2).rev() {
        for segment in layout.lists[index].iter().rev() {
            let start = (DESTINATION_BASE + segment.destination_offset) as usize;
            if (start..start + segment.byte_len as usize).contains(&offset) {
                return source
                    [(SOURCE_BASES[index] + segment.source_offset) as usize + offset - start];
            }
        }
    }
    destination_sentinel(round)
}

#[test]
fn forward_oracle_matches_independent_last_writer_and_complete_target_host_bytes() {
    for case in [Case::Overlap, Case::Packets] {
        let layout = layout(case);
        let window = Window::new(&layout);
        let mut hashes = Vec::new();
        for round in 0..ROUNDS {
            let source = source_bytes(window, round);
            let [frame, target, host] = oracle(&layout, window, &source, round);
            for (offset, &byte) in frame.iter().enumerate() {
                assert_eq!(byte, last_writer_byte(&layout, &source, offset, round));
            }
            for (offset, &byte) in target.iter().enumerate() {
                let wanted = if (TARGET_OFFSET..TARGET_OFFSET + window.bytes).contains(&offset) {
                    frame[FRAME_OFFSET + offset - TARGET_OFFSET]
                } else {
                    target_sentinel(round)
                };
                assert_eq!(byte, wanted);
            }
            for (offset, &byte) in host.iter().enumerate() {
                let wanted = if (HOST_BASE..HOST_BASE + window.target).contains(&offset) {
                    target[offset - HOST_BASE]
                } else {
                    host_sentinel(round)
                };
                assert_eq!(byte, wanted);
            }
            assert_eq!(target[TARGET_OFFSET], destination_sentinel(round));
            assert_eq!(
                target[TARGET_OFFSET + window.bytes - 1],
                destination_sentinel(round)
            );
            let parts = [source, frame, target, host];
            let value = forward_digest(&parts);
            let mut independent = Vec::from(FORWARD_DOMAIN);
            for bytes in &parts {
                independent.extend((bytes.len() as u64).to_le_bytes());
                independent.extend(bytes);
            }
            let expected: String = Sha256::digest(independent)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(value, expected);
            hashes.push(value);
        }
        assert_ne!(hashes[0], hashes[1]);
    }
}

#[test]
fn forward_report_has_exact_separate_schema_counts_and_publication_scope() {
    let expected_keys: BTreeSet<_> = "schema authority transport devices unique_ids case late contexts rounds kernels modules allocations streams source_bytes frame_bytes target_bytes list_source_offsets list_destination_offset source_envelope destination_envelope descriptors_per_round list_copied_bytes_per_round packets_per_round peer_source_offset peer_target_offset peer_bytes readback_source_offset readback_bytes host_offset host_suffix host_bytes lists scalar_peers d2h_copies native_counter completion_receipts callbacks admission dependency events descriptor_snapshot progress publication_observed publication_identity native_at_peer_admission retained_at_peer_admission source frame initialized_complement target_guards host_guards digest round_sha256 payloads_changed allocations_reused results source_disposal drain cleanup physical_overlap performance_acceptance formal_refinement".split_whitespace().collect();
    for late in [false, true] {
        for case in [Case::Overlap, Case::Packets] {
            let options = parse(late, case, true);
            let layout = layout(case);
            let window = Window::new(&layout);
            let hashes = ["a".repeat(64), "b".repeat(64)];
            let line = report(&options, &layout, window, late, &hashes);
            let mut fields = BTreeMap::new();
            let mut words = line.split_whitespace();
            assert_eq!(words.next(), Some("PASS"));
            for word in words {
                let (key, value) = word.split_once('=').unwrap();
                assert!(fields.insert(key, value).is_none());
            }
            assert_eq!(
                fields.keys().copied().collect::<BTreeSet<_>>(),
                expected_keys
            );
            assert_eq!(fields["schema"], "fe2o3.segment-frame-peer-window.v1");
            assert_eq!(fields["authority"], "production-deny-all");
            assert_eq!(
                fields["unique_ids"],
                "0x0000000000000003,0x0000000000000002,0x0000000000000001"
            );
            assert_eq!(fields["native_counter"], "0,3,6");
            assert_eq!(fields["completion_receipts"], "8");
            assert_eq!(fields["lists"], "4");
            assert_eq!(fields["scalar_peers"], "2");
            assert_eq!(fields["d2h_copies"], "2");
            assert_eq!(fields["readback_source_offset"], "0");
            assert_eq!(fields["readback_bytes"], window.target.to_string());
            assert_eq!(
                fields["packets_per_round"],
                if case == Case::Packets {
                    "3,3,3"
                } else {
                    "2,2,1"
                }
            );
            assert_eq!(
                fields["publication_observed"],
                if late { "true" } else { "false" }
            );
            assert_eq!(
                fields["publication_identity"],
                if late {
                    "ordered-roster-inference"
                } else {
                    "not-sampled"
                }
            );
            assert_eq!(
                fields["native_at_peer_admission"],
                if late { "1,4" } else { "0,3" }
            );
            assert_eq!(
                fields["retained_at_peer_admission"],
                if late { "1,1" } else { "0,0" }
            );
            assert_eq!(fields["performance_acceptance"], "false");
            assert_eq!(fields["formal_refinement"], "false");
        }
    }
}
