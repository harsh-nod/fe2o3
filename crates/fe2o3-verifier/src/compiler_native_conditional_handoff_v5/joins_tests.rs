//! Inert coordinate mutations, not successful conditional-source reconstruction.
use super::*;
use fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1 as LLVM_LAYOUT;
use fe2o3_compiler_lineage::canonical_semantic_target_layout_transcript_v1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn view(profile: Profile) -> Layout<'static> {
    Layout {
        rustc_llvm_target: profile.rustc_target(),
        live_rustc_data_layout: LLVM_LAYOUT,
        default_pointer_width_bits: 64,
        target_cpu: profile.cpu(),
        target_features: profile.rustc_features(),
    }
}
fn layout_bytes(v: Layout<'_>) -> Box<[u8]> {
    canonical_semantic_target_layout_transcript_v1(
        v.rustc_llvm_target,
        v.live_rustc_data_layout,
        v.default_pointer_width_bits,
        v.target_cpu,
        v.target_features,
    )
    .unwrap()
}
fn expected(profile: Profile) -> Lowering {
    let axis = |n| Coordinate::new([n; 32], u64::from(n)).unwrap();
    Lowering {
        final_native: Subject::new([1; 32], 11, [2; 32], 22).unwrap(),
        carrier: axis(3),
        descriptor: axis(4),
        pre_descriptor_llvm: axis(5),
        final_llvm: axis(6),
        module_handoff: axis(7),
        profile,
    }
}

#[test]
fn conditional_native_recovery_layout_exact_both_profiles_and_all_field_mutations() {
    let mutations: &[fn(&mut Layout<'_>)] = &[
        |v| v.rustc_llvm_target = "foreign-llvm-target",
        |v| v.live_rustc_data_layout = "e-p:32:32",
        |v| v.default_pointer_width_bits = 32,
        |v| v.target_cpu = "foreign-cpu",
        |v| v.target_features = "+wavefrontsize32",
    ];
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let base = view(profile);
        let wire = layout_bytes(base);
        let digest: [u8; 32] = Sha256::digest(&wire).into();
        for mutation in 0..9 {
            let mut actual = base;
            let mut source = digest;
            let mut invocation = profile.device_target();
            if (1..=5).contains(&mutation) {
                mutations[mutation - 1](&mut actual);
            }
            if mutation == 6 {
                source[0] ^= 1;
            }
            if mutation == 7 {
                invocation = "gfx900";
            }
            let bytes = layout_bytes(actual);
            let n = bytes.len() + usize::from(mutation == 8);
            let mut work = Work::new(usize::MAX);
            let mut b = Budget::new(&mut work, MAX_STORAGE);
            b.reserve_storage(19 + wire.len() + bytes.len()).unwrap();
            let floor = b.storage();
            let ledger = b.work_ledger_identity_v1();
            let result = layout(actual, n, &source, invocation, profile, &mut b);
            if mutation == 0 {
                result.unwrap();
                assert_eq!(b.storage(), floor);
            } else {
                assert!(result.is_err());
                assert!(b.storage() > floor);
            }
            assert!(b.work_ledger_identity_v1() == ledger);
        }
    }
}

#[test]
fn conditional_native_recovery_layout_exact_one_short_original_account_and_denials() {
    let profile = Profile::Gfx950;
    let v = view(profile);
    let bytes = layout_bytes(v);
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let floor = bytes.len() + 19;
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, MAX_STORAGE);
    b.reserve_storage(floor).unwrap();
    layout(
        v,
        bytes.len(),
        &digest,
        profile.device_target(),
        profile,
        &mut b,
    )
    .unwrap();
    let (required_work, peak) = (b.work(), b.peak_storage());
    assert_eq!(
        required_work,
        1 + bytes.len() * 4 + profile.device_target().len() + 512
    );
    for (work_limit, storage_limit, success) in [
        (required_work, peak, true),
        (required_work - 1, peak, false),
        (required_work, peak - 1, false),
    ] {
        let mut w = Work::new(work_limit);
        let mut b = Budget::new(&mut w, storage_limit);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = layout(
            v,
            bytes.len(),
            &digest,
            profile.device_target(),
            profile,
            &mut b,
        );
        assert_eq!(result.is_ok(), success);
        assert!(b.work_ledger_identity_v1() == ledger);
        if success {
            assert_eq!(b.storage(), floor);
        } else {
            assert!(b.failed_work().is_some() || b.failed_storage().is_some());
        }
    }
}

#[test]
fn conditional_native_recovery_lowering_all_coordinates_and_profile_mandatory() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let base = expected(profile);
        for mutation in 0..16 {
            let mut actual = base;
            match mutation {
                0 => {}
                1 => actual.final_native = Subject::new([8; 32], 11, [2; 32], 22).unwrap(),
                2 => actual.final_native = Subject::new([1; 32], 12, [2; 32], 22).unwrap(),
                3 => actual.final_native = Subject::new([1; 32], 11, [8; 32], 22).unwrap(),
                4 => actual.final_native = Subject::new([1; 32], 11, [2; 32], 23).unwrap(),
                5..=14 => {
                    let field = match (mutation - 5) / 2 {
                        0 => &mut actual.carrier,
                        1 => &mut actual.descriptor,
                        2 => &mut actual.pre_descriptor_llvm,
                        3 => &mut actual.final_llvm,
                        _ => &mut actual.module_handoff,
                    };
                    *field = if (mutation - 5) % 2 == 0 {
                        Coordinate::new([91; 32], field.byte_len()).unwrap()
                    } else {
                        Coordinate::new(field.sha256(), field.byte_len() + 1).unwrap()
                    };
                }
                _ => {
                    actual.profile = if profile == Profile::Gfx942 {
                        Profile::Gfx950
                    } else {
                        Profile::Gfx942
                    }
                }
            }
            let mut w = Work::new(size_of::<Lowering>());
            let mut b = Budget::new(&mut w, MAX_STORAGE);
            b.reserve_storage(2 * size_of::<Lowering>()).unwrap();
            let ledger = b.work_ledger_identity_v1();
            assert_eq!(lowering(actual, base, &mut b).is_ok(), mutation == 0);
            assert_eq!(b.work(), size_of::<Lowering>());
            assert!(b.work_ledger_identity_v1() == ledger);
        }
    }
}

#[test]
fn conditional_native_recovery_raw_prefix_and_descriptor_coordinates_are_not_receipt_domains() {
    let bytes = b"inert complete preimage";
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, MAX_STORAGE);
    b.reserve_storage(bytes.len() + size_of::<Sha256>() + size_of::<Coordinate>())
        .unwrap();
    let id = raw(bytes, &mut b).unwrap();
    assert_eq!(id.sha256(), <[u8; 32]>::from(Sha256::digest(bytes)));
    assert_eq!(id.byte_len(), bytes.len() as u64);
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/COMPILER-DESCRIPTOR-SOURCE/V5\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    assert_ne!(id.sha256(), <[u8; 32]>::from(hash.finalize()));
    assert_eq!(b.work(), bytes.len() + 160);
}
