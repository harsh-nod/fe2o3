use super::{native_v4::*, tests_wire_adversarial as fixture, *};
use fe2o3_compiler_lineage::*;

const TARGET: &str = "gfx942:xnack-";
const LIMIT: usize = MAX_NATIVE_REFINED_FORWARDING_CARRIER_STORAGE_V1;
type Failure = InertSemanticCompilerModuleHandoffErrorV4;
type Common = InertSemanticCompilerModuleHandoffErrorV3;

fn capsule(seed: u8, payload_seed: u8) -> InertProductionSemanticCapsuleV4 {
    let base = fixture::capsule(seed, TARGET, &fixture::llvm_module(seed));
    let pair = NativeRefinedForwardingCarrierLayoutV1::new::<()>(19, 64).unwrap();
    let layout = InertProductionSemanticCapsuleLayoutV4::new::<()>(
        base.canonical_bytes().len(),
        pair.encoded_len(),
    )
    .unwrap();
    let mut bytes = vec![payload_seed; layout.encoded_len()];
    bytes[layout.base_range()].copy_from_slice(base.canonical_bytes());
    seal_native_refined_forwarding_carrier_v1(
        pair,
        &mut bytes[layout.carrier_range()],
        LIMIT,
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    seal_inert_production_semantic_capsule_v4(layout, &mut bytes, LIMIT, |_| Ok::<_, ()>(()))
        .unwrap();
    InertProductionSemanticCapsuleV4::decode_owned(bytes).unwrap()
}
fn wire(
    capsule: &InertProductionSemanticCapsuleV4,
    module: &CompilerModuleHandoffV2,
) -> (InertSemanticCompilerModuleHandoffLayoutV4, Vec<u8>) {
    let layout = InertSemanticCompilerModuleHandoffLayoutV4::new(
        capsule.canonical_bytes().len(),
        module.canonical_bytes().len(),
    )
    .unwrap();
    let mut bytes = vec![0; layout.encoded_len()];
    bytes[layout.capsule_range()].copy_from_slice(capsule.canonical_bytes());
    bytes[layout.module_handoff_range()].copy_from_slice(module.canonical_bytes());
    seal_inert_semantic_compiler_module_handoff_v4(
        layout,
        &mut bytes,
        capsule.identity(),
        module.identity(),
        |_| Ok::<_, ()>(()),
    )
    .unwrap();
    (layout, bytes)
}
fn rehash_outer(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let digest = derive_identity_sha256(
        b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V4\0",
        &bytes[..end],
    )
    .unwrap();
    bytes[end..].copy_from_slice(&digest);
}

#[test]
fn native_v4_handoff_shares_every_payload_and_releases_backing() {
    let cap = capsule(3, 17);
    let module = fixture::module_handoff(3, TARGET);
    let (layout, bytes) = wire(&cap, &module);
    assert_eq!(
        preflight_inert_semantic_compiler_module_handoff_v4(&cap, &module).unwrap(),
        layout
    );
    let pointer = bytes.as_ptr();
    let owned = InertSemanticCompilerModuleHandoffV4::decode_owned(bytes).unwrap();
    assert_eq!(owned.canonical_bytes().as_ptr(), pointer);
    assert_eq!(owned.capsule().identity(), cap.identity());
    assert_eq!(owned.module_handoff(), &module);
    assert!(!owned.grants_authority());

    let mut bytes = vec![29; 23];
    bytes.extend_from_slice(owned.canonical_bytes());
    let end = bytes.len();
    bytes.extend_from_slice(&[41; 17]);
    let backing = Arc::new(bytes);
    let weak = Arc::downgrade(&backing);
    let decoded =
        InertSemanticCompilerModuleHandoffV4::decode_shared_vec(backing.clone(), 23..end).unwrap();
    assert_eq!(decoded.canonical_bytes().as_ptr(), backing[23..].as_ptr());
    assert_eq!(
        decoded.capsule().canonical_bytes().as_ptr(),
        backing[23 + layout.capsule_range().start..].as_ptr()
    );
    assert_eq!(
        decoded.module_handoff().canonical_bytes().as_ptr(),
        backing[23 + layout.module_handoff_range().start..].as_ptr()
    );
    let start = backing.as_ptr() as usize;
    for part in [
        decoded.capsule().base().canonical_bytes(),
        decoded
            .capsule()
            .base()
            .receipts()
            .semantic_mir()
            .canonical_preimage(),
        decoded.capsule().carrier_bytes(),
        decoded.module_handoff().module_bytes(),
        decoded.pair_binding_bytes(),
    ] {
        assert!((part.as_ptr() as usize) >= start + 23);
        assert!((part.as_ptr() as usize) + part.len() <= start + end);
    }
    drop(backing);
    assert_eq!(decoded.canonical_bytes(), owned.canonical_bytes());
    assert_eq!(decoded.identity(), owned.identity());
    assert_eq!(
        decoded.pair_binding_identity(),
        owned.pair_binding_identity()
    );
    assert_eq!(decoded.pair_binding_identity().byte_len(), 132);
    drop(decoded);
    assert!(weak.upgrade().is_none());
}

#[test]
fn native_v4_pair_binds_carrier_not_just_legacy_base() {
    let first = capsule(3, 17);
    let second = capsule(3, 19);
    assert_eq!(first.base(), second.base());
    let module = fixture::module_handoff(3, TARGET);
    let (_, first_wire) = wire(&first, &module);
    let (layout, second_wire) = wire(&second, &module);
    let a = InertSemanticCompilerModuleHandoffV4::decode_owned(first_wire.clone()).unwrap();
    let b = InertSemanticCompilerModuleHandoffV4::decode_owned(second_wire.clone()).unwrap();
    assert_ne!(a.identity(), b.identity());
    assert_ne!(a.pair_binding_identity(), b.pair_binding_identity());
    let mut splice = first_wire;
    splice[layout.capsule_range()].copy_from_slice(&second_wire[layout.capsule_range()]);
    rehash_outer(&mut splice);
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::decode_owned(splice),
        Err(Failure::Framing(Common::CapsuleIdentityMismatch))
    ));
}

#[test]
fn native_v4_content_joins_reject_target_and_final_module_substitution() {
    let cap = capsule(3, 17);
    for (module, expected) in [
        (
            fixture::module_handoff(4, TARGET),
            Common::FinalCommitmentMismatch,
        ),
        (
            fixture::module_handoff(3, "gfx950:xnack-"),
            Common::TargetMismatch,
        ),
    ] {
        assert_eq!(
            preflight_inert_semantic_compiler_module_handoff_v4(&cap, &module),
            Err(Failure::Framing(expected.clone()))
        );
        let (_, bytes) = wire(&cap, &module);
        assert!(
            matches!(InertSemanticCompilerModuleHandoffV4::decode_owned(bytes), Err(Failure::Framing(error)) if error == expected)
        );
    }
}

#[test]
fn native_v4_handoff_rejects_versions_headers_ranges_and_nested_damage() {
    let cap = capsule(3, 17);
    let module = fixture::module_handoff(3, TARGET);
    let (layout, bytes) = wire(&cap, &module);
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV3::decode(&bytes),
        Err(Common::InvalidMagic)
    ));
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::decode_owned(
            fixture::outer(3).canonical_bytes().to_vec()
        ),
        Err(Failure::Framing(Common::InvalidMagic))
    ));
    for (offset, expected) in [
        (0, Common::InvalidMagic),
        (8, Common::UnsupportedVersion(5)),
        (10, Common::UnsupportedFlags(1)),
        (20, Common::NonzeroReserved),
    ] {
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        assert!(
            matches!(InertSemanticCompilerModuleHandoffV4::decode_owned(bad), Err(Failure::Framing(error)) if error == expected)
        );
    }
    for offset in [
        layout.capsule_range().start,
        layout.module_handoff_range().start,
        bytes.len() - 1,
    ] {
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        assert!(matches!(
            InertSemanticCompilerModuleHandoffV4::decode_owned(bad),
            Err(Failure::Framing(Common::OuterIdentityMismatch))
        ));
    }
    let pair_start = layout.module_handoff_range().end;
    let mut bad = bytes.clone();
    bad[pair_start..pair_start + 8].copy_from_slice(&INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3);
    rehash_outer(&mut bad);
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::decode_owned(bad),
        Err(Failure::Framing(Common::InvalidPairBindingMagic))
    ));
    let mut bad = bytes.clone();
    bad[layout.capsule_range().start] ^= 1;
    rehash_outer(&mut bad);
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::decode_owned(bad),
        Err(Failure::Capsule(
            InertProductionSemanticCapsuleErrorV4::Header
        ))
    ));
    for length in 0..bytes.len() {
        assert!(
            InertSemanticCompilerModuleHandoffV4::decode_owned(bytes[..length].to_vec()).is_err()
        );
    }
    let backing = Arc::new(bytes);
    for range in [2..1, 0..backing.len() + 1, usize::MAX..usize::MAX] {
        assert!(matches!(
            InertSemanticCompilerModuleHandoffV4::decode_shared_vec(backing.clone(), range),
            Err(Failure::Range)
        ));
    }
    let mut trailing = backing.as_ref().clone();
    trailing.push(0);
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::decode_owned(trailing),
        Err(Failure::Framing(Common::TrailingBytes))
    ));
}

#[test]
fn native_v4_outer_bounds_and_seal_denials_precede_writes() {
    let maximum = InertSemanticCompilerModuleHandoffLayoutV4::new(
        MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V4,
        MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
    )
    .unwrap();
    assert_eq!(
        maximum.encoded_len(),
        MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V4
    );
    assert_eq!(
        maximum.encoded_len(),
        MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3
    );
    for (capsule, module, expected) in [
        (0, 1, Common::CapsuleByteBoundExceeded),
        (1, 0, Common::ModuleHandoffByteBoundExceeded),
        (
            MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V4 + 1,
            1,
            Common::CapsuleByteBoundExceeded,
        ),
        (
            1,
            MAX_COMPILER_MODULE_HANDOFF_BYTES_V2 + 1,
            Common::ModuleHandoffByteBoundExceeded,
        ),
    ] {
        assert_eq!(
            InertSemanticCompilerModuleHandoffLayoutV4::new(capsule, module),
            Err(Failure::Framing(expected))
        );
    }
    let cap = capsule(3, 17);
    let module = fixture::module_handoff(3, TARGET);
    let (layout, _) = wire(&cap, &module);
    let mut bytes = vec![0x53; layout.encoded_len()];
    let before = bytes.clone();
    let exact_work = layout.encoded_len()
        + b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V4\0".len()
        + b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V4\0".len()
        + 848;
    for available in [exact_work - 1, exact_work] {
        let mut destination = before.clone();
        let mut calls = 0;
        let result = seal_inert_semantic_compiler_module_handoff_v4(
            layout,
            &mut destination,
            cap.identity(),
            module.identity(),
            |work| {
                calls += 1;
                assert_eq!(work, exact_work);
                if work > available { Err(work) } else { Ok(()) }
            },
        );
        assert_eq!(calls, 1);
        if available == exact_work {
            assert!(result.is_ok());
            assert_ne!(destination, before);
        } else {
            assert_eq!(
                result,
                Err(InertSemanticCompilerModuleHandoffErrorV4::Charge(
                    exact_work
                ))
            );
            assert_eq!(destination, before);
        }
    }
    assert_eq!(
        seal_inert_semantic_compiler_module_handoff_v4(
            layout,
            &mut bytes,
            cap.identity(),
            module.identity(),
            |_| Err(17)
        ),
        Err(InertSemanticCompilerModuleHandoffErrorV4::Charge(17))
    );
    assert_eq!(bytes, before);
    struct DropBomb;
    impl Drop for DropBomb {
        fn drop(&mut self) {
            panic!("callback destructor")
        }
    }
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let bomb = DropBomb;
            let _ = seal_inert_semantic_compiler_module_handoff_v4(
                layout,
                &mut bytes,
                cap.identity(),
                module.identity(),
                move |_| {
                    std::hint::black_box(&bomb);
                    Ok::<_, ()>(())
                },
            );
        }))
        .is_err()
    );
    assert_eq!(bytes, before);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = seal_inert_semantic_compiler_module_handoff_v4(
                layout,
                &mut bytes,
                cap.identity(),
                module.identity(),
                |_| -> Result<(), ()> { panic!("debit") },
            );
        }))
        .is_err()
    );
    assert_eq!(bytes, before);
    assert!(
        seal_inert_semantic_compiler_module_handoff_v4(
            layout,
            &mut bytes[1..],
            cap.identity(),
            module.identity(),
            |_| -> Result<(), ()> { panic!("must not charge") }
        )
        .is_err()
    );
    assert_eq!(bytes, before);
}

#[test]
fn native_v4_pair_golden_is_domain_separated_from_v3() {
    let schema = WireSchema {
        magic: INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V4,
        version: 4,
        pair_magic: INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V4,
        pair_version: 4,
        pair_domain: b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V4\0",
        outer_domain: b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V4\0",
    };
    let (bytes, digest) = encode_pair_binding(&schema, &[0x17; 32], 19, &[0x29; 32], 31).unwrap();
    // Independently computed with Node crypto from the specified fixed fields.
    assert_eq!(
        digest,
        [
            0x77, 0x33, 0xf1, 0x53, 0x44, 0x7d, 0x39, 0xcd, 0x7b, 0x3b, 0x11, 0x1c, 0xce, 0x5a,
            0xcb, 0xbc, 0x72, 0x32, 0x1c, 0x42, 0x32, 0x4f, 0xf4, 0x63, 0x51, 0x06, 0x46, 0x61,
            0xa7, 0x75, 0xd0, 0x3b,
        ]
    );
    assert!(ParsedPairBindingV3::decode(&bytes, &schema).is_ok());
    assert!(matches!(
        ParsedPairBindingV3::decode(&bytes, &WIRE_V3),
        Err(Common::InvalidPairBindingMagic)
    ));
    // Also pin the production schema, not only this independent reference.
    let cap = capsule(3, 17);
    let module = fixture::module_handoff(3, TARGET);
    let (_, bytes) = wire(&cap, &module);
    let handoff = InertSemanticCompilerModuleHandoffV4::decode_owned(bytes).unwrap();
    let (expected, _) = encode_pair_binding(
        &schema,
        cap.identity().sha256(),
        cap.identity().byte_len(),
        module.identity().sha256(),
        module.identity().byte_len(),
    )
    .unwrap();
    assert_eq!(handoff.pair_binding_bytes(), expected);
}
