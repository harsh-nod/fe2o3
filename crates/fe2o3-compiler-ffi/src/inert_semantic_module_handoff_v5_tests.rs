//! Inert outer-content fixtures, not admitted conditional history/source/proofs.
#![cfg(test)]
use super::{native_v5::*, tests_wire_adversarial as fixture, *};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_compiler_lineage::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
const LIMIT: usize = MAX_NATIVE_CONDITIONAL_STORAGE_V1;

fn capsule(profile: Profile, seed: u8) -> InertProductionSemanticCapsuleV5 {
    let module = fixture::module_handoff(seed, profile.device_target());
    let original = fixture::capsule(seed, profile.device_target(), module.module_bytes());
    let ol = NativeConditionalOutputLayoutV1::new::<()>(7, 7, 10).unwrap();
    let cl = NativeConditionalCarrierLayoutV1::new::<()>(ol.encoded_len(), 6).unwrap();
    let mut carrier = vec![0; cl.encoded_len()];
    let output = &mut carrier[cl.output_range()];
    output[ol.history_range()].copy_from_slice(b"history");
    output[ol.catalog_range()].copy_from_slice(b"catalog");
    output[ol.descriptor_range()].copy_from_slice(b"descriptor");
    seal_native_conditional_output_v1(ol, output, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    carrier[cl.source_range()].copy_from_slice(b"source");
    let carrier_id =
        seal_native_conditional_carrier_v1(cl, &mut carrier, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    let axis = |hash, len| TargetLineageIdentityV3::new(hash, len).unwrap();
    let lowering = InertNativeLoweringAssociationV1::new(NativeLoweringAssociationInputsV1 {
        final_native: InertNativeNeutralSubjectV1::new([1; 32], 7, [2; 32], 7).unwrap(),
        carrier: axis(*carrier_id.sha256(), carrier_id.byte_len()),
        descriptor: axis(Sha256::digest(b"descriptor").into(), 10),
        pre_descriptor_llvm: axis([3; 32], 1),
        final_llvm: axis(
            *module.module_identity().sha256(),
            module.module_identity().byte_len(),
        ),
        module_handoff: axis(*module.identity().sha256(), module.identity().byte_len()),
        profile,
    });
    let invocation = fe2o3_rustc_invocation::encode_descriptor_v3(original.invocation()).unwrap();
    let target_layout = canonical_semantic_target_layout_transcript_v1(
        profile.rustc_target(),
        "layout",
        64,
        profile.cpu(),
        profile.rustc_features(),
    )
    .unwrap();
    let final_module = InertFinalCompilerModuleCommitmentV3::from_handoff(&module).unwrap();
    let input = NativeConditionalMetadataInputV1 {
        invocation: &invocation,
        rustc_inventory: original
            .receipts()
            .rustc_identity_inventory()
            .canonical_preimage(),
        rustc_preflight: original
            .receipts()
            .rustc_preflight_plan()
            .canonical_preimage(),
        semantic_target_layout: &target_layout,
        native_lowering: lowering.canonical_bytes(),
        final_module_commitment: final_module.canonical_bytes(),
    };
    let ml = NativeConditionalMetadataLayoutV1::new::<()>(input).unwrap();
    // Public synthetic policies exercise framing only, never production admission.
    let roots = [NativeConditionalPolicyRootInputV1 {
        semantic_root: 9,
        kernel_binding: [1; 32],
        effect_signers: &[[2; 32]],
        effect_toolchain: [[3; 32]; 5],
        formula_verifying_key: [4; 32],
        formula_toolchain: [[5; 32]; 5],
        formula_boundary: 1,
    }];
    let roster = encode_native_conditional_policy_roster_v1(
        NativeConditionalPolicyRosterInputV1 {
            source_packet: b"source",
            roots: &roots,
        },
        LIMIT,
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    let ml2 = NativeConditionalMetadataLayoutV2::new::<()>(ml.encoded_len(), roster.len()).unwrap();
    let l = InertProductionSemanticCapsuleLayoutV5::new::<()>(ml2.encoded_len(), carrier.len())
        .unwrap();
    let mut bytes = vec![0; l.encoded_len()];
    let envelope = &mut bytes[l.metadata_range()];
    let m = &mut envelope[ml2.metadata_range()];
    for (range, payload) in [
        (ml.invocation_range(), input.invocation),
        (ml.rustc_inventory_range(), input.rustc_inventory),
        (ml.rustc_preflight_range(), input.rustc_preflight),
        (
            ml.semantic_target_layout_range(),
            input.semantic_target_layout,
        ),
        (ml.native_lowering_range(), input.native_lowering),
        (
            ml.final_module_commitment_range(),
            input.final_module_commitment,
        ),
    ] {
        m[range].copy_from_slice(payload);
    }
    seal_native_conditional_metadata_v1(ml, m, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    envelope[ml2.policy_roster_range()].copy_from_slice(&roster);
    seal_native_conditional_metadata_v2(ml2, envelope, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    bytes[l.carrier_range()].copy_from_slice(&carrier);
    seal_inert_production_semantic_capsule_v5(l, &mut bytes, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    InertProductionSemanticCapsuleV5::decode_owned(bytes).unwrap()
}
fn wire(
    cap: &InertProductionSemanticCapsuleV5,
    module: &CompilerModuleHandoffV2,
) -> (InertSemanticCompilerModuleHandoffLayoutV5, Vec<u8>) {
    let l = InertSemanticCompilerModuleHandoffLayoutV5::new(
        cap.canonical_bytes().len(),
        module.canonical_bytes().len(),
    )
    .unwrap();
    let mut b = vec![0; l.encoded_len()];
    b[l.capsule_range()].copy_from_slice(cap.canonical_bytes());
    b[l.module_handoff_range()].copy_from_slice(module.canonical_bytes());
    seal_inert_semantic_compiler_module_handoff_v5(
        l,
        &mut b,
        cap.identity(),
        module.identity(),
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    (l, b)
}

#[test]
fn conditional_outer_v5_shared_capacity_ranges_and_both_profiles() {
    for p in [Profile::Gfx942, Profile::Gfx950] {
        let cap = capsule(p, 3);
        let module = fixture::module_handoff(3, p.device_target());
        let (l, b) = wire(&cap, &module);
        assert_eq!(
            preflight_inert_semantic_compiler_module_handoff_v5(&cap, &module).unwrap(),
            l
        );
        let mut backing = Vec::with_capacity(b.len() + 4096);
        backing.extend_from_slice(&[1; 13]);
        backing.extend_from_slice(&b);
        backing.extend_from_slice(&[2; 17]);
        let capacity = backing.capacity();
        let backing = Arc::new(backing);
        let weak = Arc::downgrade(&backing);
        let owner = InertSemanticCompilerModuleHandoffV5::decode_shared_vec(
            backing.clone(),
            13..13 + b.len(),
        )
        .unwrap();
        assert_eq!(owner.backing_capacity(), capacity);
        assert_eq!(owner.module_handoff().backing_capacity(), capacity);
        assert_eq!(owner.canonical_bytes().as_ptr(), backing[13..].as_ptr());
        assert_eq!(
            owner.capsule().canonical_bytes().as_ptr(),
            backing[13 + l.capsule_range().start..].as_ptr()
        );
        assert_eq!(
            owner.module_handoff().canonical_bytes().as_ptr(),
            backing[13 + l.module_handoff_range().start..].as_ptr()
        );
        assert_eq!(owner.capsule().history_bytes(), b"history");
        assert_eq!(owner.capsule().source_packet_bytes(), b"source");
        let roster = read_native_conditional_policy_roster_v1(
            owner.capsule().policy_roster_bytes(),
            LIMIT,
            |_| Ok::<_, ()>(()),
        )
        .unwrap();
        assert_eq!(roster.source_packet_len(), 6);
        assert_eq!(roster.roots().next().unwrap().semantic_root(), 9);
        assert!(!owner.grants_authority());
        assert!(!owner.capsule().grants_authority());
        let final_receipt = owner.capsule().final_compiler_module_commitment();
        assert_eq!(
            final_receipt,
            &InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(
                InertFinalCompilerModuleCommitmentV3::from_handoff(owner.module_handoff())
                    .unwrap()
                    .canonical_bytes(),
            )
            .unwrap(),
        );
        assert_eq!(
            final_receipt.canonical_preimage().as_ptr(),
            owner.capsule().final_module_commitment_bytes().as_ptr(),
        );
        drop(backing);
        assert!(weak.upgrade().is_some());
        drop(owner);
        assert!(weak.upgrade().is_none());
        let ptr = b.as_ptr();
        let owner = InertSemanticCompilerModuleHandoffV5::decode_owned(b).unwrap();
        assert_eq!(owner.canonical_bytes().as_ptr(), ptr);
    }
}

#[test]
fn conditional_outer_v5_golden_header_pair_domains_and_determinism() {
    let cap = capsule(Profile::Gfx942, 3);
    let m = fixture::module_handoff(3, Profile::Gfx942.device_target());
    let (_, b) = wire(&cap, &m);
    let end = b.len() - 32;
    let pair_start = end - 132;
    assert_eq!(&b[..12], b"F2O3IHV5\x05\0\0\0");
    assert_eq!(&b[20..24], &[0; 4]);
    assert_eq!(
        &b[pair_start..pair_start + 20],
        b"F2O3PBV5\x05\0\0\0\x84\0\0\0\0\0\0\0"
    );
    assert_eq!(
        &b[pair_start + 20..pair_start + 52],
        cap.identity().sha256()
    );
    assert_eq!(&b[pair_start + 60..pair_start + 92], m.identity().sha256());
    assert_eq!(
        &b[end - 32..end],
        derive_identity_sha256(
            b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V5\0",
            &b[pair_start..end - 32]
        )
        .unwrap()
    );
    assert_eq!(
        &b[end..],
        derive_identity_sha256(
            b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V5\0",
            &b[..end]
        )
        .unwrap()
    );
    assert_eq!(b, wire(&cap, &m).1);
    let other = capsule(Profile::Gfx942, 4);
    let m4 = fixture::module_handoff(4, Profile::Gfx942.device_target());
    assert_ne!(b, wire(&other, &m4).1);
}

#[test]
fn conditional_outer_v5_caps_and_decode_schedule_preserve_v4_terms() {
    assert_eq!(
        MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V5,
        MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3
    );
    for n in [
        206,
        1024,
        262338,
        524288,
        16777216,
        MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V5,
    ] {
        let old = native_v4::inert_semantic_compiler_module_handoff_decode_work_v4(n).unwrap();
        assert_eq!(
            old,
            8 * n
                + 128 * n.min(262338)
                + 128 * n.min(524288)
                + 320 * n.min(16777216)
                + 4096 * (n / 5).min(16384)
                + 7900672
        );
        assert_eq!(
            inert_semantic_compiler_module_handoff_decode_work_v5(n).unwrap(),
            old + 12 * n + 36608
        );
        assert_eq!(
            inert_semantic_compiler_module_handoff_decode_work_v5(n).unwrap(),
            20 * n
                + 128 * n.min(262338)
                + 128 * n.min(524288)
                + 320 * n.min(16777216)
                + 4096 * (n / 5).min(16384)
                + 7937280
        );
    }
    for n in [
        0,
        205,
        usize::MAX,
        MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V5 + 1,
    ] {
        assert!(inert_semantic_compiler_module_handoff_decode_work_v5(n).is_err());
    }
    assert!(
        INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5
            > INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V3
    );
    assert_eq!(
        INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5,
        INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V3
            + INERT_PRODUCTION_SEMANTIC_CAPSULE_WORKING_STORAGE_V5
            + INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_SEAL_STORAGE_V5
            + std::mem::size_of::<InertSemanticCompilerModuleHandoffV5>()
            + std::mem::size_of::<native::Finished>(),
    );
    for (a, b) in [
        (0, 1),
        (1, 0),
        (usize::MAX, 1),
        (1, usize::MAX),
        (MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V5 + 1, 1),
        (1, MAX_COMPILER_MODULE_HANDOFF_BYTES_V2 + 1),
    ] {
        assert!(InertSemanticCompilerModuleHandoffLayoutV5::new(a, b).is_err());
    }
}

#[test]
fn conditional_outer_v5_exact_work_one_short_and_unwind_before_mutation() {
    let cap = capsule(Profile::Gfx942, 3);
    let m = fixture::module_handoff(3, Profile::Gfx942.device_target());
    let (l, original) = wire(&cap, &m);
    let work = l.encoded_len()
        + b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V5\0".len()
        + b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V5\0".len()
        + 16
        + 256
        + 100
        + 3 * 132
        + 2 * 40;
    for limit in [work - 1, work] {
        let mut b = original.clone();
        b[..40].fill(17);
        let before = b.clone();
        let mut calls = 0;
        let r = seal_inert_semantic_compiler_module_handoff_v5(
            l,
            &mut b,
            cap.identity(),
            m.identity(),
            |n| {
                calls += 1;
                assert_eq!(n, work);
                if n > limit { Err("work") } else { Ok(()) }
            },
        );
        assert_eq!(calls, 1);
        if limit < work {
            assert!(matches!(
                r,
                Err(InertSemanticCompilerModuleHandoffErrorV5::Charge("work"))
            ));
            assert_eq!(b, before);
        } else {
            assert!(r.is_ok());
            assert_eq!(b, original);
        }
    }
    let mut b = original.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = seal_inert_semantic_compiler_module_handoff_v5::<()>(
                l,
                &mut b,
                cap.identity(),
                m.identity(),
                |_| panic!("work"),
            );
        }))
        .is_err()
    );
    assert_eq!(b, original);
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("destructor");
        }
    }
    let bomb = Bomb;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = seal_inert_semantic_compiler_module_handoff_v5(
                l,
                &mut b,
                cap.identity(),
                m.identity(),
                move |_| {
                    let _ = &bomb;
                    Ok::<_, ()>(())
                },
            );
        }))
        .is_err()
    );
    assert_eq!(b, original);
}

#[test]
fn conditional_outer_v5_wrong_target_or_commitment_is_not_fixed_by_resealing() {
    let cap = capsule(Profile::Gfx942, 3);
    for (target, seed, target_error) in [(Profile::Gfx950, 3, true), (Profile::Gfx942, 4, false)] {
        let m = fixture::module_handoff(seed, target.device_target());
        let (_, b) = wire(&cap, &m);
        let result = InertSemanticCompilerModuleHandoffV5::decode_owned(b);
        if target_error {
            assert!(matches!(
                result,
                Err(InertSemanticCompilerModuleHandoffErrorV5::Framing(
                    InertSemanticCompilerModuleHandoffErrorV3::TargetMismatch
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(InertSemanticCompilerModuleHandoffErrorV5::Framing(
                    InertSemanticCompilerModuleHandoffErrorV3::FinalCommitmentMismatch
                ))
            ));
        }
    }
}

#[test]
fn conditional_outer_v5_rejects_old_versions_ranges_truncation_and_unsealed_mutation() {
    let cap = capsule(Profile::Gfx942, 3);
    let m = fixture::module_handoff(3, Profile::Gfx942.device_target());
    let (_, b) = wire(&cap, &m);
    assert!(
        InertSemanticCompilerModuleHandoffV3::decode_owned(b.clone().into_boxed_slice()).is_err()
    );
    assert!(native_v4::InertSemanticCompilerModuleHandoffV4::decode_owned(b.clone()).is_err());
    assert!(
        InertSemanticCompilerModuleHandoffV5::decode_owned(
            fixture::outer(3).canonical_bytes().to_vec()
        )
        .is_err()
    );
    for end in [0, 39, 40, 205, b.len() - 1] {
        assert!(InertSemanticCompilerModuleHandoffV5::decode_owned(b[..end].to_vec()).is_err());
    }
    for i in [0, 8, 10, 12, 20, 24, 32, 40, b.len() - 1] {
        let mut bad = b.clone();
        bad[i] ^= 1;
        assert!(InertSemanticCompilerModuleHandoffV5::decode_owned(bad).is_err());
    }
    let shared = Arc::new(b);
    for r in [2..1, 0..shared.len() + 1, usize::MAX..usize::MAX] {
        assert!(matches!(
            InertSemanticCompilerModuleHandoffV5::decode_shared_vec(shared.clone(), r),
            Err(InertSemanticCompilerModuleHandoffErrorV5::Range)
        ));
    }
}

#[test]
fn module_v2_backing_capacity_counts_full_shared_and_box_payloads() {
    let module = fixture::module_handoff(3, Profile::Gfx942.device_target());
    let n = module.canonical_bytes().len();
    assert!(module.backing_capacity() >= n);
    let boxed = CompilerModuleHandoffV2::decode_owned(module.canonical_bytes().into()).unwrap();
    assert_eq!(boxed.backing_capacity(), n);
    let mut b = Vec::with_capacity(n + 4096);
    b.extend_from_slice(&[1; 13]);
    b.extend_from_slice(module.canonical_bytes());
    b.extend_from_slice(&[2; 17]);
    let cap = b.capacity();
    let b = Arc::new(b);
    let v = CompilerModuleHandoffV2::decode_shared_vec_range(b.clone(), 13, n).unwrap();
    assert_eq!(v.backing_capacity(), cap);
    assert!(v.backing_capacity() > v.canonical_bytes().len());
    let slice: Arc<[u8]> = b.as_slice().into();
    let s = CompilerModuleHandoffV2::decode_shared_range(slice.clone(), 13, n).unwrap();
    assert_eq!(s.backing_capacity(), slice.len());
    assert_eq!(s.backing_capacity(), n + 30);
    assert_eq!(s.canonical_bytes(), module.canonical_bytes());
}

#[test]
fn conditional_outer_v5_mixed_pair_schema_rejected_after_outer_rehash() {
    let cap = capsule(Profile::Gfx942, 3);
    let m = fixture::module_handoff(3, Profile::Gfx942.device_target());
    let (_, b) = wire(&cap, &m);
    let pair_start = b.len() - 32 - 132;
    for (at, value) in [
        (8, 4),
        (pair_start + 7, b'4'),
        (pair_start + 8, 4),
        (pair_start + 10, 1),
        (pair_start + 16, 1),
    ] {
        let mut bad = b.clone();
        assert_ne!(bad[at], value);
        bad[at] = value;
        let end = bad.len() - 32;
        let digest = derive_identity_sha256(
            b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V5\0",
            &bad[..end],
        )
        .unwrap();
        bad[end..].copy_from_slice(&digest);
        assert!(InertSemanticCompilerModuleHandoffV5::decode_owned(bad).is_err());
    }
}
