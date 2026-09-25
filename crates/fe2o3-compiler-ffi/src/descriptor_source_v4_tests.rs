use super::*;
use fe2o3_kernel_descriptor::*;
use sha2::Digest;
#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_v4.rs"]
mod fixture;

fn free(_: usize) -> Result<(), ()> {
    Ok(())
}
fn storage(bytes: &Vec<u8>) -> usize {
    compiler_descriptor_source_validation_storage_v4(bytes.capacity()).unwrap()
}
fn own(bytes: Vec<u8>) -> CompilerDescriptorSourceV4 {
    let prepaid = storage(&bytes);
    CompilerDescriptorSourceV4::from_owned_canonical_bytes(bytes, prepaid, &mut free).unwrap()
}
fn table_storage(source: &CompilerDescriptorSourceV4) -> usize {
    compiler_descriptor_source_table_storage_v4(source.canonical_bytes.capacity()).unwrap()
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
            size_of::<CompilerDescriptorSourceV4>() + capacity
        );
        assert_eq!(source.identity().byte_len(), expected.len() as u64);
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V4\0");
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
        CompilerDescriptorSourceV4::from_owned_canonical_bytes(bytes, exact - 1, &mut |_| {
            called = true;
            Ok::<(), ()>(())
        }),
        Err(CompilerDescriptorSourceErrorV4::Storage { .. })
    ));
    assert!(!called);
    assert!(compiler_descriptor_source_retained_storage_v4(usize::MAX).is_none());
    assert!(compiler_descriptor_source_validation_storage_v4(usize::MAX).is_none());
    assert!(compiler_descriptor_source_table_storage_v4(usize::MAX).is_none());
    let source = own(fixture::wire("gfx942:xnack-", 1, 0));
    assert!(matches!(
        source.table(table_storage(&source) - 1, &mut free),
        Err(CompilerDescriptorSourceErrorV4::Storage { .. })
    ));
    assert!(matches!(
        source.revalidate(storage(&source.canonical_bytes) - 1, &mut free),
        Err(CompilerDescriptorSourceErrorV4::Storage { .. })
    ));
}

#[test]
fn malformed_finalized_and_older_families_are_not_compiler_sources() {
    let bytes = fixture::wire("gfx942:xnack-", 1, 2);
    let mut finalized = bytes.clone();
    finalized[CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V4] = 1;
    let prepaid = storage(&finalized);
    assert!(matches!(
        CompilerDescriptorSourceV4::from_owned_canonical_bytes(finalized, prepaid, &mut free),
        Err(CompilerDescriptorSourceErrorV4::FinalizedDigest)
    ));
    for version in [1_u16, 3, 5] {
        let mut bad = bytes.clone();
        bad[8..10].copy_from_slice(&version.to_le_bytes());
        let prepaid = storage(&bad);
        assert!(
            CompilerDescriptorSourceV4::from_owned_canonical_bytes(bad, prepaid, &mut free)
                .is_err()
        );
    }
    let mut bad = bytes.clone();
    bad.pop();
    let prepaid = storage(&bad);
    assert!(
        CompilerDescriptorSourceV4::from_owned_canonical_bytes(bad, prepaid, &mut free).is_err()
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
            CompilerDescriptorSourceV4::from_owned_canonical_bytes(v3, prepaid, &mut free).is_err()
        );
    });
}

#[test]
fn revalidation_detects_cached_identity_mismatch() {
    let mut source = own(fixture::wire("gfx942:xnack-", 1, 0));
    source.identity.sha256[0] ^= 1;
    assert!(matches!(
        source.revalidate(storage(&source.canonical_bytes), &mut free),
        Err(CompilerDescriptorSourceErrorV4::IdentityMismatch)
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
    CompilerDescriptorSourceV4::from_owned_canonical_bytes(bytes, prepaid, &mut |_| {
        calls += 1;
        Ok::<(), ()>(())
    })
    .unwrap();
    for deny in 0..calls {
        let mut seen = 0;
        let bytes = source.canonical_bytes.clone();
        let prepaid = storage(&bytes);
        assert!(
            CompilerDescriptorSourceV4::from_owned_canonical_bytes(bytes, prepaid, &mut |_| {
                seen += 1;
                if seen == deny + 1 { Err(()) } else { Ok(()) }
            })
            .is_err()
        );
    }
}
