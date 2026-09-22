use super::*;
use fe2o3_compiler_lineage::*;
use std::sync::Arc;

// Reuse only a test fixture and its framing regressions. These repeated tests
// remain explicit in the handoff roster; no authenticated source is fabricated.
#[path = "../../fe2o3-compiler-lineage/src/expanded_publication_v4_tests.rs"]
pub(crate) mod lineage_tests;
use lineage_tests::{Fixture, free, identity};

pub(crate) fn module(target: &str, count: usize, seed: u8) -> CompilerModuleHandoffV2 {
    let target = DeviceTargetV1::parse(target).unwrap();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap();
    let names = ["maple", "zebra", "amber"];
    let descriptors = ["maple.kd", "zebra.kd", "amber.kd"];
    let order = if count == 1 { &[0][..] } else { &[2, 0, 1][..] };
    let rows = [
        CompilerModuleSymbolRoleV1::KernelEntry,
        CompilerModuleSymbolRoleV1::KernelDescriptor,
    ]
    .into_iter()
    .flat_map(|role| {
        order.iter().map(move |&i| {
            (
                role,
                if role == CompilerModuleSymbolRoleV1::KernelEntry {
                    names[i]
                } else {
                    descriptors[i]
                },
            )
        })
    });
    let manifest = CompilerModuleSymbolManifestV1::new(rows).unwrap();
    let mut text = format!("; inert expanded framing fixture {seed}\n");
    for name in names.iter().take(count) {
        text.push_str(&format!(
            "define amdgpu_kernel void @{name}() {{ ret void }}\n"
        ));
    }
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        manifest,
        text.as_bytes(),
    )
    .unwrap()
}
pub(crate) fn fixture(
    target: &str,
    kind: ExpandedSourceKindV1,
    count: usize,
) -> (Fixture, CompilerModuleHandoffV2) {
    let module = module(target, count, 1);
    let mut fixture = Fixture::new(target, kind, count, 0);
    fixture.receipts[11] = module.symbol_manifest().canonical_bytes().to_vec();
    fixture.receipts[14] = InertFinalCompilerModuleCommitmentV3::from_handoff(&module)
        .unwrap()
        .canonical_bytes()
        .to_vec();
    fixture.native_axes = [
        identity(COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V3, &fixture.receipts[10]),
        ExpandedContentIdentityV4::from_declared(
            *module.module_identity().sha256(),
            module.module_identity().byte_len(),
        )
        .unwrap(),
        ExpandedContentIdentityV4::from_declared(
            *module.symbol_manifest().identity().sha256(),
            module.symbol_manifest().identity().byte_len(),
        )
        .unwrap(),
        ExpandedContentIdentityV4::from_declared(
            module.envelope().identity().as_bytes(),
            module.envelope().canonical_bytes().len() as u64,
        )
        .unwrap(),
    ];
    fixture.refresh();
    (fixture, module)
}
pub(crate) fn outer(
    target: &str,
    kind: ExpandedSourceKindV1,
    count: usize,
) -> InertSemanticCompilerModuleHandoffV4 {
    let (f, m) = fixture(target, kind, count);
    InertSemanticCompilerModuleHandoffV4::new(&f.capsule(), &m, usize::MAX, &mut free).unwrap()
}

#[test]
fn expanded_outer_round_trips_profiles_branches_and_ordered_roots_without_authority() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for kind in [ExpandedSourceKindV1::Direct, ExpandedSourceKindV1::Erased] {
            let (f, m) = fixture(target, kind, 3);
            let capsule = f.capsule();
            let owner =
                InertSemanticCompilerModuleHandoffV4::new(&capsule, &m, usize::MAX, &mut free)
                    .unwrap();
            assert!(!owner.grants_authority());
            let view = owner.view(usize::MAX, &mut free).unwrap();
            assert_eq!(view.module_handoff().canonical_bytes(), m.canonical_bytes());
            assert_eq!(view.capsule().canonical_bytes(), capsule.canonical_bytes());
            assert_eq!(
                view.identity(),
                identity(
                    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DOMAIN_V4,
                    owner.canonical_bytes()
                )
            );
            let pair = &owner.canonical_bytes()[owner.canonical_bytes().len() - 96..];
            assert_eq!(
                view.pair_identity(),
                identity(INERT_COMPILER_MODULE_PAIR_BINDING_DOMAIN_V4, pair)
            );
            let association = ExpandedOutputAssociationRefV1::read(
                view.capsule()
                    .receipt(ExpandedReceiptSlotV4::ProofBinding)
                    .canonical_preimage(),
                usize::MAX,
                &mut free,
            )
            .unwrap();
            assert_eq!(association.source_kind(), kind);
            for (i, semantic) in [7, 31, 88].into_iter().enumerate() {
                assert_eq!(
                    association.root(i, &mut free).unwrap().semantic_root(),
                    semantic
                );
            }
        }
    }
}
#[test]
fn transferred_vec_and_module_payload_keep_one_backing_until_views_drop() {
    let initial = outer("gfx942:xnack-", ExpandedSourceKindV1::Direct, 3);
    let mut bytes = Vec::with_capacity(initial.canonical_bytes().len() + 137);
    bytes.extend_from_slice(initial.canonical_bytes());
    let ptr = bytes.as_ptr();
    let capacity = bytes.capacity();
    let owner =
        InertSemanticCompilerModuleHandoffV4::decode_owned(bytes, usize::MAX, &mut free).unwrap();
    assert_eq!(owner.canonical_bytes().as_ptr(), ptr);
    assert_eq!(
        owner.retained_storage(),
        std::mem::size_of::<InertSemanticCompilerModuleHandoffV4>()
            + EXPANDED_HANDOFF_SHARED_VECTOR_STORAGE_V4
            + capacity
    );
    assert_eq!(Arc::strong_count(&owner.bytes), 1);
    let view = owner.view(usize::MAX, &mut free).unwrap();
    assert_eq!(Arc::strong_count(&owner.bytes), 2);
    let low = ptr as usize;
    let high = low + owner.canonical_bytes().len();
    for slice in [
        view.module_handoff().canonical_bytes(),
        view.module_handoff().module_bytes(),
        view.capsule().canonical_bytes(),
    ] {
        assert!((low..high).contains(&(slice.as_ptr() as usize)));
        assert!(slice.as_ptr() as usize + slice.len() <= high);
    }
    drop(view);
    assert_eq!(Arc::strong_count(&owner.bytes), 1);
}
#[test]
fn source_v3_identity_is_not_the_legacy_abi_receipt_domain() {
    let (f, m) = fixture("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1);
    let descriptor = CompilerDescriptorSourceV3::from_owned_canonical_bytes(
        f.receipts[10].clone(),
        usize::MAX,
        &mut free,
    )
    .unwrap();
    assert_eq!(descriptor.identity().sha256(), f.native_axes[0].sha256());
    assert_eq!(
        descriptor.identity().byte_len(),
        f.native_axes[0].byte_len()
    );
    assert_ne!(
        f.native_axes[0],
        identity(
            ExpandedReceiptSlotV4::NominalAbi.identity_domain(),
            &f.receipts[10]
        )
    );
    let mut bad = f;
    bad.native_axes[0] = identity(
        ExpandedReceiptSlotV4::NominalAbi.identity_domain(),
        &bad.receipts[10],
    );
    bad.refresh();
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::new(&bad.capsule(), &m, usize::MAX, &mut free),
        Err(ExpandedCompilerHandoffErrorV4::Lineage(
            ExpandedPublicationErrorV4::Format(
                "actual descriptor/native/manifest/envelope identities"
            )
        ))
    ));
}
#[test]
fn independently_rehashed_axis_and_module_donors_do_not_pass_structural_join() {
    for axis in 0..4 {
        let (mut f, m) = fixture("gfx942:xnack-", ExpandedSourceKindV1::Erased, 3);
        f.native_axes[axis] =
            ExpandedContentIdentityV4::from_declared([41 + axis as u8; 32], 123).unwrap();
        f.refresh();
        assert!(matches!(
            InertSemanticCompilerModuleHandoffV4::new(&f.capsule(), &m, usize::MAX, &mut free),
            Err(ExpandedCompilerHandoffErrorV4::Lineage(
                ExpandedPublicationErrorV4::Format(
                    "actual descriptor/native/manifest/envelope identities"
                )
            ))
        ));
    }
    let (f, _) = fixture("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1);
    let capsule = f.capsule();
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::new(
            &capsule,
            &module("gfx950:xnack-", 1, 1),
            usize::MAX,
            &mut free
        ),
        Err(ExpandedCompilerHandoffErrorV4::Lineage(
            ExpandedPublicationErrorV4::Format("expanded module target/kind/COV")
        ))
    ));
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::new(
            &capsule,
            &module("gfx942:xnack-", 1, 2),
            usize::MAX,
            &mut free
        ),
        Err(ExpandedCompilerHandoffErrorV4::Lineage(
            ExpandedPublicationErrorV4::Format("final module commitment")
        ))
    ));
}
#[test]
fn final_commitment_and_complete_manifest_remain_independent_obligations() {
    let (mut f, m) = fixture("gfx942:xnack-", ExpandedSourceKindV1::Direct, 1);
    f.receipts[11] = b"different-complete-export-manifest".to_vec();
    f.refresh();
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::new(&f.capsule(), &m, usize::MAX, &mut free),
        Err(ExpandedCompilerHandoffErrorV4::Lineage(
            ExpandedPublicationErrorV4::Format("complete final export manifest")
        ))
    ));
    f.receipts[14] = b"not-a-commitment".to_vec();
    f.refresh();
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::new(&f.capsule(), &m, usize::MAX, &mut free),
        Err(ExpandedCompilerHandoffErrorV4::Commitment(_))
    ));
}
#[test]
fn strict_outer_pair_versions_lengths_order_and_content_bindings() {
    let good = outer("gfx950:xnack-", ExpandedSourceKindV1::Erased, 3);
    let n = good.canonical_bytes().len();
    let pair = n - 96;
    for at in [
        0,
        8,
        10,
        12,
        20,
        28,
        36,
        40,
        pair,
        pair + 8,
        pair + 10,
        pair + 12,
        pair + 16,
        pair + 48,
        pair + 56,
        pair + 88,
    ] {
        let mut bytes = good.canonical_bytes().to_vec();
        bytes[at] ^= 1;
        assert!(
            InertSemanticCompilerModuleHandoffV4::decode_owned(bytes, usize::MAX, &mut free)
                .is_err(),
            "offset {at}"
        );
    }
    for keep in [0, 43, 44, n - 1] {
        assert!(
            InertSemanticCompilerModuleHandoffV4::decode_owned(
                good.canonical_bytes()[..keep].to_vec(),
                usize::MAX,
                &mut free
            )
            .is_err()
        );
    }
    let mut trailing = good.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(
        InertSemanticCompilerModuleHandoffV4::decode_owned(trailing, usize::MAX, &mut free)
            .is_err()
    );
    assert!(InertSemanticCompilerModuleHandoffV3::decode(good.canonical_bytes()).is_err());
    let mut old = good.canonical_bytes().to_vec();
    old[..8].copy_from_slice(b"F2O3IHV3");
    old[8..10].copy_from_slice(&3u16.to_le_bytes());
    assert!(matches!(
        InertSemanticCompilerModuleHandoffV4::decode_owned(old, usize::MAX, &mut free),
        Err(ExpandedCompilerHandoffErrorV4::Lineage(
            ExpandedPublicationErrorV4::Format("outer V4 header")
        ))
    ));
}
