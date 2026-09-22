use super::*;
use fe2o3_functional_proof::FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_lower_mir_kernel::ProductionSourceLaunchInputV1 as Launch;

const WORK: usize = 10_000_000;
const STORAGE: usize = 10_000_000;
const FLOOR: usize = 37;

fn sample(
    erased: Option<&[u8]>,
    budget: &mut Budget<'_>,
) -> Result<(Vec<u8>, NativeCompilerSourcePacketStorageV1), E> {
    let mut other_binding = [7; 32];
    other_binding[31] = 8;
    let launches = [
        ProductionSourceLaunchRootInputV1::new(
            " logical/name ",
            [7; 32],
            Launch::new(2, None, [0, u32::MAX, 3]),
        ),
        ProductionSourceLaunchRootInputV1::new(
            "other",
            other_binding,
            Launch::new(3, Some([1, 1, 1]), [9, 0, 1]),
        ),
    ];
    let row = NativeCompilerStagingCommitmentV1 {
        receipt: [1; 32],
        effect: [2; 32],
        signer: [3; 32],
        execution: [4; 32],
        toolchain: [[5; 32], [6; 32], [7; 32], [8; 32], [9; 32]],
    };
    let mut other = row;
    other.receipt = [10; 32];
    let commitments = [row, other];
    let staging = [
        NativeCompilerRootStagingV1 {
            semantic_root: 9,
            commitments: &commitments,
        },
        NativeCompilerRootStagingV1 {
            semantic_root: 4,
            commitments: &commitments[1..],
        },
    ];
    let signatures = [
        Signature::from_untrusted_parts(
            [11; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
            [12; 32],
        ),
        Signature::from_untrusted_parts(
            [13; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
            [14; 32],
        ),
    ];
    let ranked = [
        NativeCompilerRankedRecipeRootV1 {
            semantic_root: 9,
            launch_rank: 1,
            recipe_bytes: b"recipe one",
            source_rows_bytes: b"rows one",
            ranked_ir: "diagnostic\ntext",
            effect_receipts: &signatures,
        },
        NativeCompilerRankedRecipeRootV1 {
            semantic_root: 4,
            launch_rank: 2,
            recipe_bytes: b"recipe two",
            source_rows_bytes: b"rows two",
            ranked_ir: "other text",
            effect_receipts: &signatures[1..],
        },
    ];
    encode_native_compiler_source_packet_v1(
        NativeCompilerRankedRecipeSourceProofInputsV1 {
            source: NativeCompilerSourceProofInputsV1 {
                semantic_mir: b"mir",
                native_module: b"native",
                middle_end_roster: b"middle",
                correspondence_roster: b"correspondence",
                verus_roster: b"verus",
                launch_inputs: &launches,
                staging_roots: &staging,
            },
            ranked_roots: &ranked,
        },
        erased,
        budget,
    )
}

#[test]
fn complete_packet_losslessly_preserves_independent_fields_and_order() {
    for erased in [None, Some(&b"actual E"[..])] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let (bytes, storage) = sample(erased, &mut budget).unwrap();
        assert_eq!(storage.retained_storage(), bytes.capacity());
        assert_eq!(&bytes[..12], b"F2NSRC1\0\x01\0\0\0");
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize,
            bytes.len()
        );
        assert_eq!(bytes[16], if erased.is_some() { 2 } else { 1 });
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        recipe_source::recipe_scope(&mut budget, |budget| {
            let packet = decode::decode(&bytes, budget)?;
            let metadata = size_of::<Packet<'_>>()
                + packet.launches.capacity() * size_of::<ProductionSourceLaunchRootInputV1<'_>>()
                + packet.staging.capacity() * size_of::<StagingRoot>()
                + packet.ranked.capacity() * size_of::<RankedRoot<'_>>()
                + packet
                    .staging
                    .iter()
                    .map(|root| {
                        root.commitments.capacity() * size_of::<NativeCompilerStagingCommitmentV1>()
                    })
                    .sum::<usize>()
                + packet
                    .ranked
                    .iter()
                    .map(|root| root.signatures.capacity() * size_of::<Signature>())
                    .sum::<usize>();
            assert_eq!(budget.storage(), floor + metadata);
            assert_eq!(packet.erased, erased);
            assert_eq!(
                packet.frames,
                [
                    &b"mir"[..],
                    b"native",
                    b"middle",
                    b"correspondence",
                    b"verus"
                ]
            );
            assert_eq!(packet.launches[0].logical_name(), " logical/name ");
            assert_eq!(
                packet.launches[0].launch(),
                Launch::new(2, None, [0, u32::MAX, 3])
            );
            assert_eq!(
                packet.launches[1].launch(),
                Launch::new(3, Some([1, 1, 1]), [9, 0, 1])
            );
            assert_eq!(
                packet.launches[0].kernel_binding()[..8],
                packet.launches[1].kernel_binding()[..8]
            );
            assert_ne!(
                packet.launches[0].kernel_binding(),
                packet.launches[1].kernel_binding()
            );
            assert_eq!(
                (
                    packet.staging[0].semantic_root,
                    packet.staging[1].semantic_root
                ),
                (9, 4)
            );
            for (index, digest) in packet.staging[0].commitments[0]
                .digests()
                .iter()
                .enumerate()
            {
                assert_eq!(digest.as_bytes(), &[index as u8 + 1; 32]);
            }
            assert_eq!(packet.staging[0].commitments[1].receipt, [10; 32]);
            assert_eq!(
                (
                    packet.ranked[0].semantic_root,
                    packet.ranked[1].semantic_root
                ),
                (9, 4)
            );
            assert_eq!(
                (packet.ranked[0].launch_rank, packet.ranked[1].launch_rank),
                (1, 2)
            );
            assert_eq!(
                packet.ranked[0].signatures[0].wire(),
                &[11; FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2]
            );
            assert_eq!(packet.ranked[0].signatures[0].verifying_key(), &[12; 32]);
            assert_eq!(
                packet.ranked[0].signatures[1],
                packet.ranked[1].signatures[0]
            );
            assert_eq!(packet.ranked[1].signatures[0].verifying_key(), &[14; 32]);
            let (encoded, _) = packet.replay(budget, |inputs, budget| {
                encode_native_compiler_source_packet_v1(inputs, packet.erased, budget)
            })?;
            assert_eq!(encoded, bytes);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

fn decode_only(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(), E> {
    recipe_source::recipe_scope(budget, |budget| {
        drop(decode::decode(bytes, budget)?);
        Ok(())
    })
}

#[test]
fn complete_packet_rejects_truncation_lengths_tags_utf8_and_trailing_bytes() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (bytes, _) = sample(None, &mut budget).unwrap();
    let mut corruptions = Vec::new();
    for end in 0..bytes.len() {
        let mut truncated = bytes[..end].to_vec();
        if end >= 16 {
            truncated[12..16].copy_from_slice(&(end as u32).to_le_bytes());
        }
        corruptions.push(truncated);
    }
    for offset in 0..17 {
        let mut changed = bytes.clone();
        changed[offset] ^= 0x80;
        corruptions.push(changed);
    }
    let launch_count = 17 + 6 * 4 + 3 + 6 + 6 + 14 + 5;
    for offset in [17, launch_count, launch_count + 4] {
        let mut changed = bytes.clone();
        changed[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        corruptions.push(changed);
    }
    for (offset, value) in [
        (launch_count + 8, 0xff),
        (launch_count + 8 + 14 + 32 + 1, 2),
    ] {
        let mut changed = bytes.clone();
        changed[offset] = value;
        corruptions.push(changed);
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    let len = trailing.len() as u32;
    trailing[12..16].copy_from_slice(&len.to_le_bytes());
    corruptions.push(trailing);
    for (index, changed) in corruptions.iter().enumerate() {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(
            decode_only(changed, &mut budget).is_err(),
            "corruption {index}"
        );
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
    assert!(sample(Some(&[]), &mut budget).is_err());
    assert!(
        decode_only(
            &vec![0; MAX_NATIVE_COMPILER_SOURCE_PACKET_BYTES_V1 + 1],
            &mut budget
        )
        .is_err()
    );
}

#[test]
fn complete_packet_exact_and_one_short_resource_limits_restore_floor() {
    for encode in [true, false] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let (bytes, _) = sample(None, &mut budget).unwrap();
        let run = |budget: &mut Budget<'_>| {
            if encode {
                sample(None, budget).map(drop)
            } else {
                decode_only(&bytes, budget)
            }
        };
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        run(&mut budget).unwrap();
        let used = budget.work();
        let peak = budget.peak_storage();
        if encode {
            assert_eq!(peak, FLOOR + bytes.capacity());
        }
        for (work_limit, storage_limit, succeeds) in [
            (used, peak, true),
            (used - 1, peak, false),
            (used, peak - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            assert!(budget.reserve_storage(usize::MAX).is_err());
            assert_eq!(run(&mut budget).is_ok(), succeeds);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
        }
    }
}

#[test]
fn complete_packet_valid_framing_at_aggregate_limit_and_one_over() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (small, _) = sample(None, &mut budget).unwrap();
    let maximum_mir = MAX_NATIVE_COMPILER_SOURCE_PACKET_BYTES_V1 - small.len() + 3;
    let mir = vec![0; maximum_mir + 1];
    let mut exact = None;
    for (length, succeeds) in [(maximum_mir, true), (maximum_mir + 1, false)] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = recipe_source::recipe_scope(&mut budget, |budget| {
            let packet = decode::decode(&small, budget)?;
            packet.replay(budget, |inputs, budget| {
                encode_native_compiler_source_packet_v1(
                    NativeCompilerRankedRecipeSourceProofInputsV1 {
                        source: NativeCompilerSourceProofInputsV1 {
                            semantic_mir: &mir[..length],
                            ..inputs.source
                        },
                        ranked_roots: inputs.ranked_roots,
                    },
                    None,
                    budget,
                )
            })
        });
        if succeeds {
            let (bytes, _) = result.unwrap();
            assert_eq!(bytes.len(), MAX_NATIVE_COMPILER_SOURCE_PACKET_BYTES_V1);
            exact = Some(bytes);
        } else {
            assert!(matches!(
                result,
                Err(E::PacketWire("aggregate source packet limit"))
            ));
        }
        assert_eq!(budget.storage(), FLOOR);
    }
    let mut bytes = exact.unwrap();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    decode_only(&bytes, &mut budget).unwrap();
    bytes.insert(21, 0);
    let length = bytes.len() as u32;
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
    bytes[17..21].copy_from_slice(&((maximum_mir + 1) as u32).to_le_bytes());
    assert!(matches!(
        decode_only(&bytes, &mut budget),
        Err(E::PacketWire("aggregate source packet limit"))
    ));
}

#[test]
fn complete_packet_unwind_drops_metadata_before_restoring_inherited_floor() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (bytes, _) = sample(None, &mut budget).unwrap();
    budget.reserve_storage(FLOOR).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let accepted = budget.work();
    let identity = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        recipe_source::recipe_scope::<()>(&mut budget, |budget| {
            let packet = decode::decode(&bytes, budget)?;
            packet.replay(budget, |inputs, budget| {
                assert_eq!(inputs.ranked_roots.len(), 2);
                assert!(budget.storage() > FLOOR);
                std::panic::panic_any(0x712_u32)
            })
        })
    }));
    assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 0x712);
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work() > accepted);
    assert!(budget.peak_storage() > FLOOR);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    assert!(budget.work_ledger_identity_v1() == identity);
}
