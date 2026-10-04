//! Inert ELF/contract integrity only; no source proof or native admission.
use super::*;
use fe2o3_hsaco_finalize::{
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V5 as SCRATCH, NominalFinalizationErrorV5 as Error,
    derive_unfinalized_nominal_hsaco_v5, finalize_unfinalized_nominal_hsaco_v5,
    inspect_finalized_nominal_hsaco_v5, inspect_unfinalized_nominal_hsaco_v5,
};
use fe2o3_kernel_descriptor::*;
#[path = "../fixtures/nominal_v5_descriptor.rs"]
mod support;
use support::{free, substitute_cpu};

fn wire(target: &str, entries: usize) -> Vec<u8> {
    support::wires(target, entries, 2, None, "v5-inert").0
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
    if count > 0 {
        rename(&mut value, ".fe2o3.kd.v5");
    }
    value
}
fn rename(value: &mut Fixture, name: &str) {
    let offset = section_name_file_offset(&value.bytes, DESCRIPTOR_SECTION_INDEX);
    value.bytes[offset..offset + name.len()].copy_from_slice(name.as_bytes());
}

#[test]
fn both_targets_exact_digest_only_patch_reinspection_and_v2_retention() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for entries in [1, 2] {
            for hidden in [false, true] {
                let source = wire(target, entries);
                let raw = fixture(&source, target, entries, hidden, 1, &[]);
                let original = raw.bytes.clone();
                let finalized =
                    finalize_unfinalized_nominal_hsaco_v5(&raw.bytes, &source, SCRATCH, &mut free)
                        .unwrap();
                let mut hash = Sha256::new();
                hash.update(b"FE2O3/AMDHSA-CODE-OBJECT/V1\0");
                hash.update((raw.bytes.len() as u64).to_le_bytes());
                hash.update(&raw.bytes);
                let digest: [u8; 32] = hash.finalize().into();
                let mut expected = original.clone();
                let offset = finalized.location().digest_offset();
                expected[offset..offset + 32].copy_from_slice(&digest);
                assert_eq!(finalized.as_bytes(), expected);
                assert_eq!(finalized.digest().as_bytes(), &digest);
                assert_eq!(raw.bytes, original);
                assert_eq!(
                    derive_unfinalized_nominal_hsaco_v5(finalized.as_bytes(), SCRATCH, &mut free)
                        .unwrap(),
                    original
                );
                let inspected =
                    inspect_finalized_nominal_hsaco_v5(finalized.as_bytes(), SCRATCH, &mut free)
                        .unwrap();
                let table = inspected.descriptor_table();
                let input = decode_device_descriptor_table_v5(&source, &mut free).unwrap();
                assert_eq!(
                    table.device_target(),
                    DeviceTargetV1::parse(target).unwrap()
                );
                assert_eq!(table.kernel_count(), entries);
                assert_eq!(inspected.kernel_bindings().bindings().len(), entries);
                for i in 0..entries {
                    let kernel = table.kernel(i, &mut free).unwrap();
                    let contract = kernel.conditional_contract(&mut free).unwrap();
                    let original_kernel = input.kernel(i, &mut free).unwrap();
                    let original_contract =
                        original_kernel.conditional_contract(&mut free).unwrap();
                    assert_eq!(
                        contract.canonical_bytes(),
                        original_contract.canonical_bytes()
                    );
                    assert_eq!(contract.theorem().cpu_input_commitment, [17; 32]);
                }
                assert!(
                    !finalized.grants_launch_authority() && !inspected.grants_launch_authority()
                );
            }
        }
    }
}

#[test]
fn full_expected_source_binds_coherent_cpu_change_and_rejects_incoherent_change() {
    let source = wire(GENERAL_V3_TARGET, 1);
    let changed = substitute_cpu(&source, true);
    decode_device_descriptor_table_v5(&changed, &mut free).unwrap();
    let raw = fixture(&source, GENERAL_V3_TARGET, 1, false, 1, &[]);
    assert!(matches!(
        finalize_unfinalized_nominal_hsaco_v5(&raw.bytes, &changed, SCRATCH, &mut free),
        Err(Error::DescriptorSourceMismatch)
    ));
    let foreign = fixture(&changed, GENERAL_V3_TARGET, 1, false, 1, &[]);
    let a = finalize_unfinalized_nominal_hsaco_v5(&raw.bytes, &source, SCRATCH, &mut free).unwrap();
    let b = finalize_unfinalized_nominal_hsaco_v5(&foreign.bytes, &changed, SCRATCH, &mut free)
        .unwrap();
    assert_ne!(a.digest(), b.digest());
    let bad = substitute_cpu(&source, false);
    let bad_raw = fixture(&bad, GENERAL_V3_TARGET, 1, false, 1, &[]);
    assert!(matches!(
        inspect_unfinalized_nominal_hsaco_v5(&bad_raw.bytes, SCRATCH, &mut free),
        Err(Error::Wire(_))
    ));
    let mut corrupt = a.as_bytes().to_vec();
    corrupt[a.location().digest_offset()] ^= 1;
    assert!(inspect_finalized_nominal_hsaco_v5(&corrupt, SCRATCH, &mut free).is_err());
    assert!(inspect_unfinalized_nominal_hsaco_v5(a.as_bytes(), SCRATCH, &mut free).is_err());
}

#[test]
fn missing_duplicate_foreign_and_mixed_sections_refuse_without_fallback() {
    let source = wire(GENERAL_V3_TARGET, 1);
    for count in [0, 2] {
        let raw = fixture(&source, GENERAL_V3_TARGET, 1, false, count, &[]);
        let error =
            inspect_unfinalized_nominal_hsaco_v5(&raw.bytes, SCRATCH, &mut free).unwrap_err();
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
        assert!(error.to_string().contains(".fe2o3.kd.v5"));
    }
    for name in [
        ".fe2o3.kd.v1",
        ".fe2o3.kd.v3",
        ".fe2o3.kd.v4",
        ".fe2o3.kd.v6",
    ] {
        let mut raw = fixture(&source, GENERAL_V3_TARGET, 1, false, 1, &[]);
        rename(&mut raw, name);
        assert!(inspect_unfinalized_nominal_hsaco_v5(&raw.bytes, SCRATCH, &mut free).is_err());
        let mut mixed = fixture(&source, GENERAL_V3_TARGET, 1, false, 1, &[name]);
        for swap in [false, true] {
            if swap {
                for i in 0..64 {
                    mixed
                        .bytes
                        .swap(mixed.descriptor_headers[0] + i, mixed.extra_headers[0] + i);
                }
            }
            assert!(
                inspect_unfinalized_nominal_hsaco_v5(&mixed.bytes, SCRATCH, &mut free).is_err()
            );
        }
    }
    for version in [1u16, 3, 4, 6, 0x0105] {
        let mut changed = source.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        let raw = fixture(&changed, GENERAL_V3_TARGET, 1, false, 1, &[]);
        assert!(matches!(
            inspect_unfinalized_nominal_hsaco_v5(&raw.bytes, SCRATCH, &mut free),
            Err(Error::Wire(_))
        ));
    }
    let mut old = support::descriptor::v4::wire(GENERAL_V3_TARGET, 1, 2);
    old[8..10].copy_from_slice(&5u16.to_le_bytes());
    let raw = fixture(&old, GENERAL_V3_TARGET, 1, false, 1, &[]);
    assert!(matches!(
        inspect_unfinalized_nominal_hsaco_v5(&raw.bytes, SCRATCH, &mut free),
        Err(Error::Wire(_))
    ));
}

#[test]
fn shared_physical_checker_still_rejects_target_abi_launch_and_metadata_disagreement() {
    let source = wire(GENERAL_V3_TARGET, 1);
    let wrong_target = fixture(&source, "gfx950:xnack-", 1, false, 1, &[]);
    assert!(inspect_unfinalized_nominal_hsaco_v5(&wrong_target.bytes, SCRATCH, &mut free).is_err());
    for mode in 0..4 {
        let mut raw = super::nominal_v4::fixture_for_target(
            &source,
            1,
            false,
            48,
            |kernel| match mode {
                0 => set_field(kernel, ".kernarg_segment_align", 16.into()),
                1 => set_field(kernel, ".max_flat_workgroup_size", 128.into()),
                2 => set_field(kernel, ".wavefront_size", 32.into()),
                _ => {
                    let Value::Array(args) = field_mut(kernel, ".args") else {
                        panic!("args")
                    };
                    set_field(&mut args[0], ".access", "write_only".into());
                }
            },
            1,
            &[],
            GENERAL_V3_TARGET,
        );
        rename(&mut raw, ".fe2o3.kd.v5");
        assert!(
            inspect_unfinalized_nominal_hsaco_v5(&raw.bytes, SCRATCH, &mut free).is_err(),
            "mode {mode}"
        );
    }
}

#[test]
fn exact_scratch_work_every_callback_denial_and_unwind_never_mutate_caller_bytes() {
    let source = wire(GENERAL_V3_TARGET, 1);
    let raw = fixture(&source, GENERAL_V3_TARGET, 1, false, 1, &[]);
    let before = raw.bytes.clone();
    let mut trace = Vec::new();
    finalize_unfinalized_nominal_hsaco_v5(&raw.bytes, &source, SCRATCH, &mut |n| {
        trace.push(n);
        Ok::<_, usize>(())
    })
    .unwrap();
    let total = trace.iter().sum::<usize>();
    for (scratch, allowance) in [(SCRATCH, total), (SCRATCH, total - 1), (SCRATCH - 1, total)] {
        let mut remaining = allowance;
        let mut calls = 0;
        let result =
            finalize_unfinalized_nominal_hsaco_v5(&raw.bytes, &source, scratch, &mut |n| {
                calls += 1;
                remaining = remaining.checked_sub(n).ok_or("denied")?;
                Ok::<_, &'static str>(())
            });
        assert_eq!(result.is_ok(), scratch == SCRATCH && allowance == total);
        if result.is_ok() {
            assert_eq!(remaining, 0);
        }
        if scratch < SCRATCH {
            assert_eq!(calls, 0);
            assert!(matches!(
                result,
                Err(Error::Scratch {
                    required: SCRATCH,
                    ..
                })
            ));
        }
    }
    for deny in 0..trace.len() {
        let mut seen = Vec::new();
        let result =
            finalize_unfinalized_nominal_hsaco_v5(&raw.bytes, &source, SCRATCH, &mut |n| {
                seen.push(n);
                if seen.len() == deny + 1 {
                    Err(deny)
                } else {
                    Ok(())
                }
            });
        assert!(result.is_err());
        assert_eq!(seen, trace[..=deny]);
    }
    for panic_at in [0, trace.len() - 1] {
        let mut calls = 0;
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                finalize_unfinalized_nominal_hsaco_v5(&raw.bytes, &source, SCRATCH, &mut |_| {
                    let current = calls;
                    calls += 1;
                    assert_ne!(current, panic_at, "inert callback unwind");
                    free(0)
                })
            }))
            .is_err()
        );
        assert_eq!(calls, panic_at + 1);
    }
    assert_eq!(raw.bytes, before);
}
