use super::*;
use fe2o3_kernel_descriptor::*;
use sha2::Digest;
#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_v5.rs"]
mod fixture;

fn free(_: usize) -> Result<(), ()> {
    Ok(())
}
fn storage(bytes: &Vec<u8>) -> usize {
    compiler_descriptor_source_validation_storage_v5(bytes.capacity()).unwrap()
}
fn own(bytes: Vec<u8>) -> CompilerDescriptorSourceV5 {
    let prepaid = storage(&bytes);
    CompilerDescriptorSourceV5::from_owned_canonical_bytes(bytes, prepaid, &mut free).unwrap()
}
fn table_storage(source: &CompilerDescriptorSourceV5) -> usize {
    compiler_descriptor_source_table_storage_v5(source.canonical_bytes.capacity()).unwrap()
}

#[test]
fn typed_storage_formulas_keep_v4_layout_and_charge_only_v2_scratch_delta() {
    assert_eq!(
        DESCRIPTOR_TABLE_VIEW_STORAGE_V5,
        DESCRIPTOR_TABLE_VIEW_STORAGE_V4
    );
    assert_eq!(
        size_of::<DeviceDescriptorTableInputV5<'static>>(),
        size_of::<DeviceDescriptorTableInputV4<'static>>()
    );
    assert_eq!(
        size_of::<KernelDescriptorRefV5<'static, 'static>>(),
        size_of::<KernelDescriptorRefV4<'static, 'static>>()
    );
    let delta = CONDITIONAL_INVOCATION_CODEC_STORAGE_V2 - CONDITIONAL_INVOCATION_CODEC_STORAGE_V1;
    assert_eq!(
        DESCRIPTOR_QUERY_STORAGE_V5,
        DESCRIPTOR_QUERY_STORAGE_V4 + delta
    );
    assert_eq!(
        DESCRIPTOR_READER_SCRATCH_STORAGE_V5,
        DESCRIPTOR_READER_SCRATCH_STORAGE_V4 + delta
    );
    assert_eq!(
        DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V5,
        DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V4 + delta
    );
    assert_eq!(
        COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V5,
        crate::COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V4
    );
    assert_eq!(
        COMPILER_DESCRIPTOR_SOURCE_HASH_STORAGE_V5,
        crate::COMPILER_DESCRIPTOR_SOURCE_HASH_STORAGE_V4
    );
    assert_eq!(
        COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V5,
        crate::COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V4 + delta
    );
    assert_eq!(
        COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V5,
        crate::COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V4 + delta
    );
    assert_eq!(COMPILER_DESCRIPTOR_SECTION_NAME_V5, ".fe2o3.kd.v5");
}

#[test]
fn owned_multi_entry_transport_retains_exact_backing_without_authority() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let mut bytes = fixture::wire(target, 2, 2);
        bytes.reserve(97);
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let expected = bytes.clone();
        let source = own(bytes);
        assert_eq!(source.canonical_bytes().as_ptr(), pointer);
        assert_eq!(source.canonical_bytes.capacity(), capacity);
        assert_eq!(source.canonical_bytes(), expected);
        assert_eq!(
            source.storage().retained_storage(),
            size_of::<CompilerDescriptorSourceV5>() + capacity
        );
        assert_eq!(source.identity().byte_len(), expected.len() as u64);
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V5\0");
        hash.update((expected.len() as u64).to_le_bytes());
        hash.update(&expected);
        let digest: [u8; 32] = hash.finalize().into();
        assert_eq!(source.identity().sha256(), &digest);
        assert!(!source.authenticates_compiler_origin());
        assert!(!source.grants_link_authority());
        assert!(!source.grants_load_authority());
        assert!(!source.grants_launch_authority());
        let table = source.table(table_storage(&source), &mut free).unwrap();
        assert_eq!(table.canonical_bytes().as_ptr(), pointer);
        assert_eq!(table.kernel_count(), 2);
        for i in 0..2 {
            let kernel = table.kernel(i, &mut free).unwrap();
            let contract = kernel.conditional_contract(&mut free).unwrap();
            assert_eq!(contract.read_count(), 2);
            assert_eq!(
                &contract.subjects().kernel_id,
                kernel.kernel_id().as_bytes()
            );
        }
        source
            .revalidate(storage(&source.canonical_bytes), &mut free)
            .unwrap();
    }
}

#[test]
fn owned_transport_preserves_mixed_alignment_repeated_reads_without_authority() {
    let source = own(fixture::wire_custom(
        "gfx950:xnack-",
        1,
        1,
        fixture::mixed_read_alignments,
    ));
    assert!(!source.authenticates_compiler_origin());
    assert!(!source.grants_link_authority());
    assert!(!source.grants_load_authority());
    assert!(!source.grants_launch_authority());
    let table = source.table(table_storage(&source), &mut free).unwrap();
    let kernel = table.kernel(0, &mut free).unwrap();
    let contract = kernel.conditional_contract(&mut free).unwrap();
    let mut reads = contract.reads();
    for alignment in [4, 1] {
        let read = reads.next(&mut free).unwrap().unwrap();
        assert_eq!(
            (read.argument, read.element_bytes, read.alignment),
            (0, 4, alignment)
        );
    }
    assert_eq!(reads.next(&mut free).unwrap(), None);
    source
        .revalidate(storage(&source.canonical_bytes), &mut free)
        .unwrap();
}

#[test]
fn actual_capacity_and_all_storage_boundaries_are_checked_before_work() {
    let mut bytes = fixture::wire("gfx942:xnack-", 1, 0);
    bytes.reserve(4096);
    let exact = storage(&bytes);
    let mut called = false;
    assert!(matches!(
        CompilerDescriptorSourceV5::from_owned_canonical_bytes(bytes, exact - 1, &mut |_| {
            called = true;
            Ok::<(), ()>(())
        }),
        Err(CompilerDescriptorSourceErrorV5::Storage { .. })
    ));
    assert!(!called);
    assert!(compiler_descriptor_source_retained_storage_v5(usize::MAX).is_none());
    assert!(compiler_descriptor_source_validation_storage_v5(usize::MAX).is_none());
    assert!(compiler_descriptor_source_table_storage_v5(usize::MAX).is_none());
    let source = own(fixture::wire("gfx942:xnack-", 1, 0));
    assert!(matches!(
        source.table(table_storage(&source) - 1, &mut free),
        Err(CompilerDescriptorSourceErrorV5::Storage { .. })
    ));
    assert!(matches!(
        source.revalidate(storage(&source.canonical_bytes) - 1, &mut free),
        Err(CompilerDescriptorSourceErrorV5::Storage { .. })
    ));
}

#[test]
fn malformed_finalized_and_older_families_are_not_compiler_sources() {
    let bytes = fixture::wire("gfx942:xnack-", 1, 2);
    let mut finalized = bytes.clone();
    finalized[CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V5] = 1;
    let prepaid = storage(&finalized);
    assert!(matches!(
        CompilerDescriptorSourceV5::from_owned_canonical_bytes(finalized, prepaid, &mut free),
        Err(CompilerDescriptorSourceErrorV5::FinalizedDigest)
    ));
    for version in [1_u16, 3, 4] {
        let mut bad = bytes.clone();
        bad[8..10].copy_from_slice(&version.to_le_bytes());
        let prepaid = storage(&bad);
        assert!(
            CompilerDescriptorSourceV5::from_owned_canonical_bytes(bad, prepaid, &mut free)
                .is_err()
        );
    }
    let mut bad = bytes.clone();
    bad.pop();
    let prepaid = storage(&bad);
    assert!(
        CompilerDescriptorSourceV5::from_owned_canonical_bytes(bad, prepaid, &mut free).is_err()
    );
    let prepaid_v3 =
        crate::compiler_descriptor_source_validation_storage_v3(bytes.capacity()).unwrap();
    assert!(
        crate::CompilerDescriptorSourceV3::from_owned_canonical_bytes(bytes, prepaid_v3, &mut free)
            .is_err()
    );
    fixture::with_input("gfx942:xnack-", 1, 2, |input| {
        let mut v3 =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut v3, &mut free).unwrap();
        let prepaid = storage(&v3);
        assert!(
            CompilerDescriptorSourceV5::from_owned_canonical_bytes(v3, prepaid, &mut free).is_err()
        );
    });
}

#[test]
fn revalidation_detects_cached_identity_mismatch() {
    let mut source = own(fixture::wire("gfx942:xnack-", 1, 0));
    source.identity.sha256[0] ^= 1;
    assert!(matches!(
        source.revalidate(storage(&source.canonical_bytes), &mut free),
        Err(CompilerDescriptorSourceErrorV5::IdentityMismatch)
    ));
}

#[test]
fn coherent_v2_contract_substitution_still_changes_the_retained_source() {
    let mut source = own(fixture::wire("gfx942:xnack-", 1, 0));
    let start = fixture::contract_start(&source.canonical_bytes);
    source.canonical_bytes[start + 576] ^= 0x40;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V2/LE/SHARED-IEEE/CPU-SOURCE\0");
    for offset in [92, 352, 448, 480, 512, 544, 576] {
        hash.update(&source.canonical_bytes[start + offset..start + offset + 32]);
    }
    source.canonical_bytes[start + 320..start + 352].copy_from_slice(&hash.finalize());
    fixture::repair_identity(&mut source.canonical_bytes);
    // This remains canonical inert content, but no longer the retained source.
    assert!(source.table(table_storage(&source), &mut free).is_ok());
    assert!(matches!(
        source.revalidate(storage(&source.canonical_bytes), &mut free),
        Err(CompilerDescriptorSourceErrorV5::IdentityMismatch)
    ));
}

#[test]
fn construction_table_and_revalidation_propagate_every_work_denial() {
    let source = own(fixture::wire("gfx942:xnack-", 1, 0));
    let mut calls = 0;
    source
        .revalidate(storage(&source.canonical_bytes), &mut |_| {
            calls += 1;
            Ok::<(), ()>(())
        })
        .unwrap();
    for deny in 0..calls {
        let mut seen = 0;
        assert!(
            source
                .revalidate(storage(&source.canonical_bytes), &mut |_| {
                    seen += 1;
                    if seen == deny + 1 { Err(()) } else { Ok(()) }
                })
                .is_err()
        );
    }
    let mut calls = 0;
    source
        .table(table_storage(&source), &mut |_| {
            calls += 1;
            Ok::<(), ()>(())
        })
        .unwrap();
    for deny in 0..calls {
        let mut seen = 0;
        assert!(
            source
                .table(table_storage(&source), &mut |_| {
                    seen += 1;
                    if seen == deny + 1 { Err(()) } else { Ok(()) }
                })
                .is_err()
        );
    }
    let mut calls = 0;
    let bytes = source.canonical_bytes.clone();
    let prepaid = storage(&bytes);
    CompilerDescriptorSourceV5::from_owned_canonical_bytes(bytes, prepaid, &mut |_| {
        calls += 1;
        Ok::<(), ()>(())
    })
    .unwrap();
    for deny in 0..calls {
        let mut seen = 0;
        let bytes = source.canonical_bytes.clone();
        let prepaid = storage(&bytes);
        assert!(
            CompilerDescriptorSourceV5::from_owned_canonical_bytes(bytes, prepaid, &mut |_| {
                seen += 1;
                if seen == deny + 1 { Err(()) } else { Ok(()) }
            })
            .is_err()
        );
    }
}

#[test]
fn v4_original_construction_table_revalidation_debits_and_extents_are_exact() {
    let mut bytes = fixture::v4::wire("gfx942:xnack-", 1, 0);
    bytes.reserve(97);
    let capacity = bytes.capacity();
    let mut expected = vec![1];
    decode_device_descriptor_table_v4(&bytes, &mut |w| {
        expected.push(w);
        Ok::<_, ()>(())
    })
    .unwrap();
    expected.push(32);
    let table_trace = expected.clone();
    expected.push(b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V4\0".len() + 8 + bytes.len() + 128);
    let expected_bytes = bytes.clone();
    let mut actual = Vec::new();
    let prepaid = crate::compiler_descriptor_source_validation_storage_v4(capacity).unwrap();
    let source =
        crate::CompilerDescriptorSourceV4::from_owned_canonical_bytes(bytes, prepaid, &mut |w| {
            actual.push(w);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        source.storage().retained_storage(),
        size_of::<crate::CompilerDescriptorSourceV4>() + capacity
    );
    assert_eq!(source.canonical_bytes(), expected_bytes);
    assert_eq!(
        prepaid,
        source.storage().retained_storage()
            + crate::COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V4
    );
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V4\0");
    hash.update((expected_bytes.len() as u64).to_le_bytes());
    hash.update(&expected_bytes);
    let digest: [u8; 32] = hash.finalize().into();
    assert_eq!(source.identity().sha256(), &digest);
    actual.clear();
    source
        .table(
            crate::compiler_descriptor_source_table_storage_v4(capacity).unwrap(),
            &mut |w| {
                actual.push(w);
                Ok::<_, ()>(())
            },
        )
        .unwrap();
    assert_eq!(actual, table_trace);
    expected.push(size_of::<crate::CompilerDescriptorSourceIdentityV4>() + 1);
    actual.clear();
    source
        .revalidate(prepaid, &mut |w| {
            actual.push(w);
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(actual, expected);
    for deny in 0..expected.len() {
        actual.clear();
        let err = source
            .revalidate(prepaid, &mut |w| {
                actual.push(w);
                if actual.len() == deny + 1 {
                    Err(deny)
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        match err {
            crate::CompilerDescriptorSourceErrorV4::Work(index)
            | crate::CompilerDescriptorSourceErrorV4::Wire(DescriptorWireErrorV4::Nominal(
                DescriptorWireErrorV3::Work(index),
            ))
            | crate::CompilerDescriptorSourceErrorV4::Wire(DescriptorWireErrorV4::Contract(
                ConditionalInvocationWireErrorV1::Work(index),
            )) => assert_eq!(index, deny),
            other => panic!("wrong error adapter: {other:?}"),
        }
        assert_eq!(actual, expected[..=deny]);
    }
}

#[derive(Default)]
struct CallbackAccount {
    remaining: usize,
    work: usize,
    denials: usize,
}
impl CallbackAccount {
    fn charge(&mut self, work: usize) -> Result<(), &'static str> {
        if self.remaining < work {
            self.denials += 1;
            return Err("original account denied");
        }
        self.remaining -= work;
        self.work += work;
        Ok(())
    }
}

#[test]
fn original_callback_account_exact_one_short_and_retry_history_are_preserved() {
    // This API accepts an inert prepaid extent, not an authenticated Budget.
    // Pin the actual caller callback's state, without claiming ledger admission.
    let source = own(fixture::wire("gfx942:xnack-", 1, 0));
    let prepaid = storage(&source.canonical_bytes);
    let mut total = 0;
    source
        .revalidate(prepaid, &mut |w| {
            total += w;
            Ok::<_, ()>(())
        })
        .unwrap();
    for limit in [total, total - 1] {
        let mut account = CallbackAccount {
            remaining: limit,
            ..Default::default()
        };
        let result = source.revalidate(prepaid, &mut |w| account.charge(w));
        if limit == total {
            assert!(result.is_ok());
            assert_eq!(account.remaining, 0);
            assert_eq!(account.work, total);
            assert_eq!(account.denials, 0);
        } else {
            assert!(matches!(
                result,
                Err(CompilerDescriptorSourceErrorV5::Work(
                    "original account denied"
                ))
            ));
            assert_eq!(account.denials, 1);
            let before = account.work;
            assert!(
                source
                    .revalidate(prepaid, &mut |w| account.charge(w))
                    .is_err()
            );
            assert_eq!(account.denials, 2);
            assert!(account.work >= before);
            assert_eq!(account.work + account.remaining, limit);
        }
    }
}

#[test]
fn callback_unwind_keeps_owner_bytes_and_consumed_work() {
    let source = own(fixture::wire("gfx950:xnack-", 1, 1));
    let identity = source.identity();
    let pointer = source.canonical_bytes().as_ptr();
    let mut trace = Vec::new();
    source
        .revalidate(storage(&source.canonical_bytes), &mut |w| {
            trace.push(w);
            Ok::<_, ()>(())
        })
        .unwrap();
    for stop in [0, trace.len() / 2, trace.len() - 1] {
        let mut seen = Vec::new();
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = source.revalidate(storage(&source.canonical_bytes), &mut |w| {
                seen.push(w);
                if seen.len() == stop + 1 {
                    panic!("original callback unwind");
                }
                Ok::<_, ()>(())
            });
        }));
        assert!(failed.is_err());
        assert_eq!(seen, trace[..=stop]);
        assert_eq!(source.identity(), identity);
        assert_eq!(source.canonical_bytes().as_ptr(), pointer);
        source
            .revalidate(storage(&source.canonical_bytes), &mut free)
            .unwrap();
    }
}

#[test]
fn v4_owner_cannot_accept_v5_or_relabel_v2_as_v1() {
    let bytes = fixture::wire("gfx942:xnack-", 1, 0);
    let prepaid =
        crate::compiler_descriptor_source_validation_storage_v4(bytes.capacity()).unwrap();
    assert!(
        crate::CompilerDescriptorSourceV4::from_owned_canonical_bytes(
            bytes.clone(),
            prepaid,
            &mut free
        )
        .is_err()
    );
    let mut relabeled = bytes;
    relabeled[8..10].copy_from_slice(&4u16.to_le_bytes());
    let prepaid =
        crate::compiler_descriptor_source_validation_storage_v4(relabeled.capacity()).unwrap();
    assert!(matches!(
        crate::CompilerDescriptorSourceV4::from_owned_canonical_bytes(
            relabeled, prepaid, &mut free
        ),
        Err(crate::CompilerDescriptorSourceErrorV4::Wire(
            DescriptorWireErrorV4::Contract(_)
        ))
    ));
}
