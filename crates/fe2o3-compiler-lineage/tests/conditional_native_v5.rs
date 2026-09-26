//! Inert framing components only: no source/proof/nominal or execution custody.
#![cfg(test)]
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_lineage::*;
use fe2o3_rustc_invocation::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
const LIMIT: usize = MAX_NATIVE_CONDITIONAL_STORAGE_V1;

fn output() -> (NativeConditionalOutputLayoutV1, Vec<u8>) {
    let layout = NativeConditionalOutputLayoutV1::new::<()>(1, 2, 3).unwrap();
    let mut b = vec![0; layout.encoded_len()];
    b[layout.history_range()].copy_from_slice(b"h");
    b[layout.catalog_range()].copy_from_slice(b"cc");
    b[layout.descriptor_range()].copy_from_slice(b"ddd");
    seal_native_conditional_output_v1(layout, &mut b, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    (layout, b)
}
fn carrier(output: &[u8]) -> Vec<u8> {
    let l = NativeConditionalCarrierLayoutV1::new::<()>(output.len(), 4).unwrap();
    let mut b = vec![0; l.encoded_len()];
    b[l.output_range()].copy_from_slice(output);
    b[l.source_range()].copy_from_slice(b"src2");
    seal_native_conditional_carrier_v1(l, &mut b, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    b
}
fn invocation(profile: Profile) -> Vec<u8> {
    let closure =
        CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap();
    let unit = RustcUnitV2::new(
        "/workspace",
        vec![
            "/opt/rustc".into(),
            "--crate-name".into(),
            "conditional_fixture".into(),
            "src/lib.rs".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Zcodegen-backend=/opt/backend.so".into(),
        ],
    )
    .unwrap();
    let env = CompileEnvironmentV2::from_child_environment(
        [
            ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
            ("FE2O3_HSACO_DIR", "/workspace/out"),
            ("FE2O3_TARGET", profile.device_target()),
            ("FE2O3_VERIFY_KERNEL_IR", "1"),
        ]
        .into_iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v))),
    )
    .unwrap();
    let v2 = RustcInvocationDescriptorV2::new([4; 32], [6; 32], unit, env).unwrap();
    encode_descriptor_v3(&RustcInvocationDescriptorV3::new(v2, closure).unwrap()).unwrap()
}
fn metadata(profile: Profile) -> (NativeConditionalMetadataLayoutV1, Vec<u8>) {
    let inv = invocation(profile);
    let layout = canonical_semantic_target_layout_transcript_v1(
        "amdgcn-amd-amdhsa",
        "layout",
        64,
        profile.cpu(),
        "features",
    )
    .unwrap();
    let axis = |n| TargetLineageIdentityV3::new([n; 32], 1).unwrap();
    let lowering = InertNativeLoweringAssociationV1::new(NativeLoweringAssociationInputsV1 {
        final_native: InertNativeNeutralSubjectV1::new([1; 32], 1, [2; 32], 1).unwrap(),
        carrier: axis(3),
        descriptor: axis(4),
        pre_descriptor_llvm: axis(5),
        final_llvm: axis(6),
        module_handoff: axis(7),
        profile,
    });
    let input = NativeConditionalMetadataInputV1 {
        invocation: &inv,
        rustc_inventory: b"inventory",
        rustc_preflight: b"preflight",
        semantic_target_layout: &layout,
        native_lowering: lowering.canonical_bytes(),
        final_module_commitment: b"FFI alone admits this",
    };
    let l = NativeConditionalMetadataLayoutV1::new::<()>(input).unwrap();
    let mut b = vec![0; l.encoded_len()];
    for (r, p) in [
        (l.invocation_range(), input.invocation),
        (l.rustc_inventory_range(), input.rustc_inventory),
        (l.rustc_preflight_range(), input.rustc_preflight),
        (
            l.semantic_target_layout_range(),
            input.semantic_target_layout,
        ),
        (l.native_lowering_range(), input.native_lowering),
        (
            l.final_module_commitment_range(),
            input.final_module_commitment,
        ),
    ] {
        b[r].copy_from_slice(p);
    }
    seal_native_conditional_metadata_v1(l, &mut b, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    (l, b)
}
fn capsule(meta: &[u8]) -> (InertProductionSemanticCapsuleLayoutV5, Vec<u8>) {
    let pair = carrier(&output().1);
    let l = InertProductionSemanticCapsuleLayoutV5::new::<()>(meta.len(), pair.len()).unwrap();
    let mut b = vec![0; l.encoded_len()];
    b[l.metadata_range()].copy_from_slice(meta);
    b[l.carrier_range()].copy_from_slice(&pair);
    seal_inert_production_semantic_capsule_v5(l, &mut b, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    (l, b)
}
fn hash(domain: &[u8], b: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(domain);
    h.update((b.len() as u64).to_le_bytes());
    h.update(b);
    h.finalize().into()
}

#[test]
fn conditional_output_v1_fixed_header_ranges_and_domains() {
    let (l, b) = output();
    assert_eq!(b.len(), 166);
    assert_eq!(&b[..16], b"F2NCO1\0\0\x01\0\x01\0\x30\0\0\0");
    assert_eq!(l.history_range(), 48..49);
    assert_eq!(l.catalog_range(), 97..99);
    assert_eq!(l.descriptor_range(), 99..102);
    assert_eq!(&b[49..65], b"F2NCA1\0\0\x01\0\x01\0\x30\0\0\0");
    assert_eq!(
        &b[102..134],
        hash(b"FE2O3/NATIVE-CONDITIONAL-AUXILIARY/V1\0", &b[49..102])
    );
    assert_eq!(
        &b[134..],
        hash(b"FE2O3/NATIVE-CONDITIONAL-OUTPUT/V1\0", &b[..134])
    );
    let r = read_native_conditional_output_v1(&b, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    assert_eq!(
        (r.history(), r.catalog(), r.descriptor_bytes()),
        (&b"h"[..], &b"cc"[..], &b"ddd"[..])
    );
    assert!(!r.grants_authority());
    assert_eq!(b, output().1);
}

#[test]
fn conditional_output_v1_exact_prepaid_composite_work_and_refusal() {
    let (l, original) = output();
    let aux = 53 + b"FE2O3/NATIVE-CONDITIONAL-AUXILIARY/V1\0".len() + 8 + 128 + 96 + 32;
    let outer = 134 + b"FE2O3/NATIVE-CONDITIONAL-OUTPUT/V1\0".len() + 8 + 128 + 96 + 32;
    for limit in [aux + outer - 1, aux + outer] {
        let mut b = original.clone();
        b[..48].fill(7);
        let before = b.clone();
        let mut work = 0;
        let mut calls = 0;
        let result = seal_native_conditional_output_v1(l, &mut b, LIMIT, |n| {
            calls += 1;
            if n > limit {
                return Err("work");
            }
            work += n;
            Ok(())
        });
        assert_eq!(calls, 1);
        if limit < aux + outer {
            assert!(matches!(
                result,
                Err(NativeConditionalOutputErrorV1::Charge("work"))
            ));
            assert_eq!(b, before);
            assert_eq!(work, 0);
        } else {
            assert!(result.is_ok());
            assert_eq!(work, aux + outer);
            assert_eq!(b, original);
        }
    }
    let mut charges = Vec::new();
    read_native_conditional_output_v1(&original, LIMIT, |n| {
        charges.push(n);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(
        charges,
        vec![
            48,
            134 + b"FE2O3/NATIVE-CONDITIONAL-OUTPUT/V1\0".len() + 8 + 128 + 32,
            48,
            53 + b"FE2O3/NATIVE-CONDITIONAL-AUXILIARY/V1\0".len() + 8 + 128 + 32
        ]
    );
}

#[test]
fn conditional_output_v1_callback_and_destructor_unwind_before_all_mutation() {
    let (l, mut b) = output();
    let before = b.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ =
                seal_native_conditional_output_v1::<()>(l, &mut b, LIMIT, |_| panic!("callback"));
        }))
        .is_err()
    );
    assert_eq!(b, before);
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("closure destruction");
        }
    }
    let bomb = Bomb;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = seal_native_conditional_output_v1(l, &mut b, LIMIT, move |_| {
                let _ = &bomb;
                Ok::<_, ()>(())
            });
        }))
        .is_err()
    );
    assert_eq!(b, before);
}

#[test]
fn conditional_frames_independent_limits_without_large_allocations() {
    let l = NativeConditionalOutputLayoutV1::new::<()>(
        MAX_NATIVE_CONDITIONAL_HISTORY_BYTES_V1,
        MAX_NATIVE_CONDITIONAL_CATALOG_BYTES_V1,
        MAX_NATIVE_CONDITIONAL_DESCRIPTOR_BYTES_V1,
    )
    .unwrap();
    assert_eq!(l.encoded_len(), MAX_NATIVE_CONDITIONAL_OUTPUT_BYTES_V1);
    for args in [
        (0, 1, 1),
        (1, 0, 1),
        (1, 1, 0),
        (usize::MAX, 1, 1),
        (MAX_NATIVE_CONDITIONAL_HISTORY_BYTES_V1 + 1, 1, 1),
        (1, MAX_NATIVE_CONDITIONAL_CATALOG_BYTES_V1 + 1, 1),
        (1, 1, MAX_NATIVE_CONDITIONAL_DESCRIPTOR_BYTES_V1 + 1),
    ] {
        assert!(NativeConditionalOutputLayoutV1::new::<()>(args.0, args.1, args.2).is_err());
    }
    assert!(
        NativeConditionalCarrierLayoutV1::new::<()>(1, MAX_NATIVE_CONDITIONAL_SOURCE_BYTES_V1 + 1)
            .is_err()
    );
    assert_eq!(
        MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V5,
        MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V4
    );
    let (l, mut b) = output();
    let before = b.clone();
    let mut visits = 0;
    assert!(matches!(
        seal_native_conditional_output_v1(l, &mut b, LIMIT + 1, |_| {
            visits += 1;
            Ok::<_, ()>(())
        }),
        Err(NativeConditionalOutputErrorV1::StorageLimit)
    ));
    assert_eq!(visits, 0);
    assert_eq!(b, before);
}

#[test]
fn conditional_frames_reject_truncation_mutation_trailing_and_old_roles() {
    let (_, b) = output();
    for end in 0..b.len() {
        assert!(read_native_conditional_output_v1(&b[..end], LIMIT, |_| Ok::<_, ()>(())).is_err());
    }
    for i in 0..b.len() {
        let mut bad = b.clone();
        bad[i] ^= 1;
        assert!(
            read_native_conditional_output_v1(&bad, LIMIT, |_| Ok::<_, ()>(())).is_err(),
            "byte {i}"
        );
    }
    let mut trailing = b.clone();
    trailing.push(0);
    assert!(read_native_conditional_output_v1(&trailing, LIMIT, |_| Ok::<_, ()>(())).is_err());
    let new = carrier(&b);
    assert!(read_native_refined_forwarding_carrier_v1(&new, LIMIT, |_| Ok::<_, ()>(())).is_err());
    let old_l = NativeRefinedForwardingCarrierLayoutV1::new::<()>(b.len(), 4).unwrap();
    let mut old = vec![0; old_l.encoded_len()];
    old[old_l.output_range()].copy_from_slice(&b);
    seal_native_refined_forwarding_carrier_v1(old_l, &mut old, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    assert!(read_native_conditional_carrier_v1(&old, LIMIT, |_| Ok::<_, ()>(())).is_err());
    assert!(read_native_refined_forwarding_carrier_v1(&old, LIMIT, |_| Ok::<_, ()>(())).is_ok());
}

#[test]
fn conditional_metadata_v1_exact_fields_layout_and_unparsed_ffi_member() {
    let (l, b) = metadata(Profile::Gfx942);
    let r = read_native_conditional_metadata_v1(&b, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    assert_eq!(&b[..16], b"F2NCM1\0\0\x01\0\x01\0\x50\0\0\0");
    assert_eq!(&b[24..32], &[6, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(r.rustc_inventory(), b"inventory");
    assert_eq!(r.rustc_preflight(), b"preflight");
    assert_eq!(r.semantic_target_layout().default_pointer_width_bits, 64);
    assert_eq!(r.semantic_target_layout().target_cpu, "gfx942");
    assert_eq!(r.final_module_commitment(), b"FFI alone admits this");
    assert_eq!(r.invocation().as_ptr(), b[l.invocation_range()].as_ptr());
    assert!(!r.grants_authority());
    for at in [0, 8, 10, 12, 16, 24, 26, 32] {
        let mut bad = b.clone();
        bad[at] ^= 1;
        let end = bad.len() - 32;
        let digest = hash(b"FE2O3/NATIVE-CONDITIONAL-METADATA/V1\0", &bad[..end]);
        bad[end..].copy_from_slice(&digest);
        assert!(read_native_conditional_metadata_v1(&bad, LIMIT, |_| Ok::<_, ()>(())).is_err());
    }
    let mut bad = b.clone();
    bad[l.semantic_target_layout_range().start] = 255;
    seal_native_conditional_metadata_v1(l, &mut bad, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    assert!(matches!(
        read_native_conditional_metadata_v1(&bad, LIMIT, |_| Ok::<_, ()>(())),
        Err(NativeConditionalMetadataErrorV1::TargetLayout)
    ));
}

#[test]
fn conditional_capsule_v5_both_profiles_exact_shared_ranges_and_drop() {
    for p in [Profile::Gfx942, Profile::Gfx950] {
        let (ml, meta) = metadata(p);
        let (cl, b) = capsule(&meta);
        let mut backing = Vec::with_capacity(b.len() + 4096);
        backing.extend_from_slice(&[7; 13]);
        backing.extend_from_slice(&b);
        backing.extend_from_slice(&[9; 17]);
        let backing = Arc::new(backing);
        let weak = Arc::downgrade(&backing);
        let owner =
            InertProductionSemanticCapsuleV5::decode_shared_vec(backing.clone(), 13..13 + b.len())
                .unwrap();
        assert_eq!(owner.canonical_bytes().as_ptr(), backing[13..].as_ptr());
        assert_eq!(
            owner
                .rustc_identity_inventory()
                .canonical_preimage()
                .as_ptr(),
            backing[13 + cl.metadata_range().start + ml.rustc_inventory_range().start..].as_ptr()
        );
        assert_eq!(
            owner.carrier_bytes().as_ptr(),
            backing[13 + cl.carrier_range().start..].as_ptr()
        );
        assert_eq!(owner.history_bytes(), b"h");
        assert_eq!(owner.catalog_bytes(), b"cc");
        assert_eq!(owner.descriptor_bytes(), b"ddd");
        assert_eq!(owner.source_packet_bytes(), b"src2");
        assert_eq!(owner.invocation().amd_target(), p.device_target());
        assert_eq!(owner.native_lowering().inputs().profile, p);
        assert!(!owner.grants_authority());
        drop(backing);
        assert!(weak.upgrade().is_some());
        drop(owner);
        assert!(weak.upgrade().is_none());
        assert!(InertProductionSemanticCapsuleV4::decode_owned(b.clone()).is_err());
        assert!(InertProductionSemanticCapsuleV3::decode(&b).is_err());
    }
}

#[test]
fn conditional_capsule_v5_resealed_foreign_invocation_target_rejected() {
    let (l, mut meta) = metadata(Profile::Gfx942);
    let foreign = invocation(Profile::Gfx950);
    assert_eq!(foreign.len(), l.invocation_range().len());
    meta[l.invocation_range()].copy_from_slice(&foreign);
    seal_native_conditional_metadata_v1(l, &mut meta, LIMIT, |_| Ok::<_, ()>(())).unwrap();
    let (_, b) = capsule(&meta);
    assert!(matches!(
        InertProductionSemanticCapsuleV5::decode_owned(b),
        Err(InertProductionSemanticCapsuleErrorV5::Target)
    ));
}

#[test]
fn conditional_metadata_v1_work_refusal_and_truncation_preserve_input() {
    let (l, original) = metadata(Profile::Gfx942);
    let mut paid = 0;
    let mut b = original.clone();
    seal_native_conditional_metadata_v1(l, &mut b, LIMIT, |n| {
        paid += n;
        Ok::<_, ()>(())
    })
    .unwrap();
    for limit in [paid - 1, paid] {
        let mut candidate = original.clone();
        candidate[..80].fill(9);
        let before = candidate.clone();
        let r = seal_native_conditional_metadata_v1(l, &mut candidate, LIMIT, |n| {
            if n > limit { Err(()) } else { Ok(()) }
        });
        if limit < paid {
            assert!(r.is_err());
            assert_eq!(candidate, before);
        } else {
            assert!(r.is_ok());
            assert_eq!(candidate, original);
        }
    }
    for end in 0..original.len() {
        assert!(
            read_native_conditional_metadata_v1(&original[..end], LIMIT, |_| Ok::<_, ()>(()))
                .is_err()
        );
    }
    let mut total = 0;
    read_native_conditional_metadata_v1(&original, LIMIT, |n| {
        total += n;
        Ok::<_, ()>(())
    })
    .unwrap();
    for limit in [total - 1, total] {
        let mut remaining = limit;
        let r = read_native_conditional_metadata_v1(&original, LIMIT, |n| {
            remaining = remaining.checked_sub(n).ok_or(())?;
            Ok::<(), ()>(())
        });
        assert_eq!(r.is_ok(), limit == total);
    }
}

#[test]
fn conditional_capsule_v5_rejects_invalid_ranges_and_preserves_inert_leaf_boundary() {
    let (_, meta) = metadata(Profile::Gfx942);
    let (_, b) = capsule(&meta);
    let shared = Arc::new(b);
    for r in [2..1, 0..shared.len() + 1, usize::MAX..usize::MAX, 0..0] {
        assert!(InertProductionSemanticCapsuleV5::decode_shared_vec(shared.clone(), r).is_err());
    }
    // These intentionally non-leaf payloads exercise framing only, never genuine replay.
    let owner =
        InertProductionSemanticCapsuleV5::decode_shared_vec(shared.clone(), 0..shared.len())
            .unwrap();
    assert_eq!(owner.history_bytes(), b"h");
    assert!(!owner.grants_authority());
    for i in [0, 8, 10, 12, 16, 24, 32, 40] {
        let mut bad = shared.as_ref().clone();
        bad[i] ^= 1;
        let end = bad.len() - 32;
        let digest = hash(b"FE2O3/INERT-PRODUCTION-SEMANTIC-CAPSULE/V5\0", &bad[..end]);
        bad[end..].copy_from_slice(&digest);
        assert!(InertProductionSemanticCapsuleV5::decode_owned(bad).is_err());
    }
}
