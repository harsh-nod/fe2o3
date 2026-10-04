//! Inert physical artifact integrity, not source proof or a worker transaction.
use super::*;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53, NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V89 as SCRATCH,
    NominalFinalizationErrorV89 as Error, derive_unfinalized_nominal_hsaco_v89,
    finalize_unfinalized_nominal_hsaco_v89, inspect_finalized_nominal_hsaco_v89,
    inspect_unfinalized_nominal_hsaco_v53, inspect_unfinalized_nominal_hsaco_v89,
};
use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
use fe2o3_kernel_descriptor::*;
#[path = "../../../fe2o3-kernel-descriptor/tests/support/mixed_v89.rs"]
mod mixed_fixture;
#[path = "../fixtures/nominal_v5_descriptor.rs"]
mod nominal_fixture;
use nominal_fixture::free;

fn wire(target: &str, count: usize) -> Vec<u8> {
    let (_, nominal) = nominal_fixture::wires(target, count, 2, None, "predicated-v89");
    let table = decode_device_descriptor_table_v3(&nominal, &mut free).unwrap();
    let descriptor = mixed_descriptor_subject_v26(&table, &mut free).unwrap();
    let contracts = (0..count)
        .map(|i| {
            let kernel = table.kernel(i, &mut free).unwrap();
            mixed_fixture::contract(MixedContractSubjectsV26 {
                kernel_id: *kernel.kernel_id().as_bytes(),
                source_semantic_identity: [2; 32],
                original_graph_identity: [3; 32],
                output_graph_identity: [4; 32],
                descriptor_identity: descriptor,
                original_root: (count - 1 - i) as u32,
                output_function: i as u32,
                source_rank: kernel.launch().rank(),
                index_width: 64,
                exact_grid: [256, 1, 1],
                source_argument_count: kernel.argument_count() as u32,
                generated_field_count: kernel.component_count() as u32,
                explicit_argument_bytes: kernel.abi_layout().explicit_argument_size(),
                kernarg_alignment: kernel.abi_layout().kernarg_segment_alignment(),
            })
        })
        .collect::<Vec<_>>();
    mixed_fixture::wire(&nominal, &contracts)
}

fn fixture(
    wire: &[u8],
    target: &str,
    entries: usize,
    hidden: bool,
    count: usize,
    extra: &[&str],
) -> Fixture {
    let mut value = super::nominal_v4::fixture_for_target(
        wire,
        entries,
        hidden,
        if hidden { 304 } else { 48 },
        |_| {},
        count,
        extra,
        target,
    );
    // The synthetic ELF string table initially holds one-digit version names.
    // Extend that table instead of overwriting its following string.
    let sections = u64::from_le_bytes(value.bytes[40..48].try_into().unwrap()) as usize;
    let index = u16::from_le_bytes(value.bytes[62..64].try_into().unwrap()) as usize;
    let header = sections + index * 64;
    let start =
        u64::from_le_bytes(value.bytes[header + 24..header + 32].try_into().unwrap()) as usize;
    let size =
        u64::from_le_bytes(value.bytes[header + 32..header + 40].try_into().unwrap()) as usize;
    let mut strings = value.bytes[start..start + size].to_vec();
    let name = strings.len();
    strings.extend_from_slice(b".fe2o3.kd.v89\0");
    let offset = value.bytes.len() as u64;
    value.bytes.extend_from_slice(&strings);
    value.bytes[header + 24..header + 32].copy_from_slice(&offset.to_le_bytes());
    value.bytes[header + 32..header + 40].copy_from_slice(&(strings.len() as u64).to_le_bytes());
    for &header in &value.descriptor_headers {
        value.bytes[header..header + 4].copy_from_slice(&(name as u32).to_le_bytes());
    }
    value
}

#[test]
fn mixed_v89_finalization_preserves_exact_guards_and_only_patches_digest() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for entries in [1, 2] {
            for hidden in [false, true] {
                let source = wire(target, entries);
                let raw = fixture(&source, target, entries, hidden, 1, &[]);
                let original = raw.bytes.clone();
                let finalized =
                    finalize_unfinalized_nominal_hsaco_v89(&raw.bytes, &source, SCRATCH, &mut free)
                        .unwrap();
                let mut hash = Sha256::new();
                hash.update(b"FE2O3/AMDHSA-CODE-OBJECT/V1\0");
                hash.update((original.len() as u64).to_le_bytes());
                hash.update(&original);
                let digest: [u8; 32] = hash.finalize().into();
                let mut expected = original.clone();
                let offset = finalized.location().digest_offset();
                expected[offset..offset + 32].copy_from_slice(&digest);
                assert_eq!(finalized.as_bytes(), expected);
                assert_eq!(raw.bytes, original);
                assert_eq!(
                    derive_unfinalized_nominal_hsaco_v89(finalized.as_bytes(), SCRATCH, &mut free)
                        .unwrap(),
                    original
                );
                let inspected =
                    inspect_finalized_nominal_hsaco_v89(finalized.as_bytes(), SCRATCH, &mut free)
                        .unwrap();
                let table = inspected.descriptor_table();
                let before = decode_mixed_descriptor_v89(&source, &mut free).unwrap();
                assert_eq!(table.kernel_count(), entries);
                assert_eq!(inspected.kernel_bindings().bindings().len(), entries);
                assert_eq!(
                    table.device_target(),
                    DeviceTargetV1::parse(target).unwrap()
                );
                for i in 0..entries {
                    let after = table.contract(i, &mut free).unwrap();
                    let before = before.contract(i, &mut free).unwrap();
                    assert_eq!(after.canonical_bytes(), before.canonical_bytes());
                    assert_eq!(after.identity(), before.identity());
                    assert_eq!(after.argument(1, &mut free).unwrap().writes, 0);
                    assert_eq!(after.occurrence_count(), 3);
                    for n in 0..3 {
                        assert_eq!(
                            after.occurrence_identity(n, &mut free).unwrap(),
                            before.occurrence_identity(n, &mut free).unwrap()
                        );
                        assert_eq!(
                            after
                                .occurrence(n, &mut free)
                                .unwrap()
                                .output_guard
                                .edge()
                                .is_some(),
                            n == 0
                        );
                    }
                }
                assert!(
                    !finalized.grants_launch_authority() && !inspected.grants_launch_authority()
                );
                assert_eq!(finalized.into_bytes(), expected);
            }
        }
    }
}

#[test]
fn mixed_v89_finalization_rejects_foreign_source_and_tampered_final_digest() {
    let source = wire(GENERAL_V3_TARGET, 1);
    let raw = fixture(&source, GENERAL_V3_TARGET, 1, false, 1, &[]);
    let table = decode_mixed_descriptor_v89(&source, &mut free).unwrap();
    let mut subjects = *table.contract(0, &mut free).unwrap().subjects();
    subjects.source_semantic_identity[0] ^= 1;
    let foreign = mixed_fixture::wire(
        table.nominal_canonical_bytes(),
        &[mixed_fixture::contract(subjects)],
    );
    assert!(matches!(
        finalize_unfinalized_nominal_hsaco_v89(&raw.bytes, &foreign, SCRATCH, &mut free),
        Err(Error::DescriptorSourceMismatch)
    ));
    let good =
        finalize_unfinalized_nominal_hsaco_v89(&raw.bytes, &source, SCRATCH, &mut free).unwrap();
    let mut corrupt = good.as_bytes().to_vec();
    corrupt[good.location().digest_offset()] ^= 1;
    assert!(inspect_finalized_nominal_hsaco_v89(&corrupt, SCRATCH, &mut free).is_err());
    assert!(inspect_unfinalized_nominal_hsaco_v89(good.as_bytes(), SCRATCH, &mut free).is_err());
    assert!(inspect_finalized_nominal_hsaco_v89(&raw.bytes, SCRATCH, &mut free).is_err());
}

#[test]
fn mixed_v89_finalization_rejects_missing_duplicate_and_competing_sections() {
    let source = wire(GENERAL_V3_TARGET, 1);
    for count in [0, 2] {
        let raw = fixture(&source, GENERAL_V3_TARGET, 1, false, count, &[]);
        let error =
            inspect_unfinalized_nominal_hsaco_v89(&raw.bytes, SCRATCH, &mut free).unwrap_err();
        assert!(match count {
            0 => matches!(
                error,
                Error::Artifact(FinalizationError::MissingDescriptorSection)
            ),
            _ => matches!(
                error,
                Error::Artifact(FinalizationError::DuplicateDescriptorSection)
            ),
        });
        assert!(error.to_string().contains(".fe2o3.kd.v89"));
    }
    for name in [
        ".fe2o3.kd.v1",
        ".fe2o3.kd.v3",
        ".fe2o3.kd.v5",
        ".fe2o3.kd.v53",
        ".fe2o3.kd.v90",
    ] {
        let mut raw = fixture(&source, GENERAL_V3_TARGET, 1, false, 1, &[name]);
        for swap in [false, true] {
            if swap {
                for i in 0..64 {
                    raw.bytes
                        .swap(raw.descriptor_headers[0] + i, raw.extra_headers[0] + i);
                }
            }
            assert!(inspect_unfinalized_nominal_hsaco_v89(&raw.bytes, SCRATCH, &mut free).is_err());
        }
    }
    let raw = fixture(&source, GENERAL_V3_TARGET, 1, false, 1, &[]);
    assert!(
        inspect_unfinalized_nominal_hsaco_v53(
            &raw.bytes,
            NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53,
            &mut free
        )
        .is_err()
    );
}

#[test]
fn mixed_v89_finalization_requires_exact_work_and_complete_scratch() {
    let source = wire(GENERAL_V3_TARGET, 2);
    let raw = fixture(&source, GENERAL_V3_TARGET, 2, true, 1, &[]);
    let mut work = 0usize;
    let expected = finalize_unfinalized_nominal_hsaco_v89(&raw.bytes, &source, SCRATCH, &mut |n| {
        work += n;
        Ok::<_, &'static str>(())
    })
    .unwrap();
    for remaining in [work, work - 1] {
        let mut left = remaining;
        let result =
            finalize_unfinalized_nominal_hsaco_v89(&raw.bytes, &source, SCRATCH, &mut |n| {
                left = left.checked_sub(n).ok_or("work exhausted")?;
                Ok::<_, &'static str>(())
            });
        if remaining == work {
            assert_eq!(result.unwrap().as_bytes(), expected.as_bytes());
            assert_eq!(left, 0);
        } else {
            assert!(result.is_err());
        }
    }
    assert!(
        matches!(finalize_unfinalized_nominal_hsaco_v89(&raw.bytes, &source, SCRATCH - 1, &mut free), Err(Error::Scratch { required: SCRATCH, prepaid }) if prepaid == SCRATCH - 1)
    );
    let original = raw.bytes.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = finalize_unfinalized_nominal_hsaco_v89(
                &raw.bytes,
                &source,
                SCRATCH,
                &mut |_| -> Result<(), &'static str> { panic!("work callback panic") },
            );
        }))
        .is_err()
    );
    assert_eq!(raw.bytes, original);
}
