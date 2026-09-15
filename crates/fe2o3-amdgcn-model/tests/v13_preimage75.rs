//! Frozen pre-Composite73 production outputs, not a comparison of new wrappers.
//! Capture provenance and hashes: fixtures/v13_preimage75/PROVENANCE.md.
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_amdgcn_model::{
    ProductionTargetLaunchEvidenceKirV1, lower_verified_canonical_kir_to_amd_llvm_ir_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVersionV1 as Version, VerifiedCanonicalKernelIrErrorV1,
    VerifiedCanonicalKernelIrV1,
};

struct Fixture {
    kir: &'static str,
    identities: &'static str,
    llvm: [&'static str; 2],
    closures: [&'static str; 2],
}

macro_rules! fixture {
    ($name:literal) => {
        Fixture {
            kir: include_str!(concat!("fixtures/v13_preimage75/", $name, ".kir.hex")),
            identities: include_str!(concat!("fixtures/v13_preimage75/", $name, ".identities")),
            llvm: [
                include_str!(concat!("fixtures/v13_preimage75/", $name, ".gfx942.ll")),
                include_str!(concat!("fixtures/v13_preimage75/", $name, ".gfx950.ll")),
            ],
            closures: [
                include_str!(concat!(
                    "fixtures/v13_preimage75/",
                    $name,
                    ".gfx942.closure.hex"
                )),
                include_str!(concat!(
                    "fixtures/v13_preimage75/",
                    $name,
                    ".gfx950.closure.hex"
                )),
            ],
        }
    };
}

fn bytes(text: &str) -> Vec<u8> {
    let text = text.strip_suffix('\n').unwrap();
    assert_eq!(text.len() % 2, 0);
    assert!(
        text.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
    (0..text.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&text[offset..offset + 2], 16).unwrap())
        .collect()
}

fn check(fixture: Fixture) {
    let canonical = bytes(fixture.kir);
    let identities: Vec<_> = fixture
        .identities
        .split_inclusive('\n')
        .map(bytes)
        .collect();
    assert_eq!(identities.len(), 4);
    assert!(identities.iter().all(|identity| identity.len() == 32));
    let (owner, module) = VerifiedCanonicalKernelIrV1::from_canonical_bytes_with_module(
        canonical.clone(),
        Version::V13,
    )
    .unwrap();
    owner.revalidate().unwrap();
    assert_eq!(owner.version(), Version::V13);
    assert_eq!(owner.canonical_bytes(), canonical);
    let recanonicalized = VerifiedCanonicalKernelIrV1::from_module(module, Version::V13).unwrap();
    assert_eq!(recanonicalized.canonical_bytes(), canonical);
    assert_eq!(owner.identity().digest().as_slice(), identities[0]);
    assert_eq!(owner.identity().canonical_length(), canonical.len() as u64);
    let launch = ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 3).unwrap();
    assert_eq!(launch.identity().as_slice(), identities[1]);
    assert!(!launch.grants_launch_authority());
    for (index, profile) in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ]
    .into_iter()
    .enumerate()
    {
        let lowered =
            lower_verified_canonical_kir_to_amd_llvm_ir_v1(&owner, 3, &launch, profile).unwrap();
        assert_eq!(lowered.llvm_ir().as_bytes(), fixture.llvm[index].as_bytes());
        assert_eq!(
            lowered.capability_closure().canonical_bytes(),
            bytes(fixture.closures[index]),
        );
        assert_eq!(
            lowered.capability_closure_identity().as_slice(),
            identities[index + 2]
        );
        assert!(lowered.declared_replay().is_none());
        assert!(!lowered.has_complete_operational_translation_derivation());
        assert!(!lowered.grants_load_authority());
        assert!(!lowered.grants_launch_authority());
        assert!(
            lower_verified_canonical_kir_to_amd_llvm_ir_v1(&owner, 4, &launch, profile).is_err()
        );
    }
    assert_ne!(fixture.llvm[0], fixture.llvm[1]);
    assert_ne!(fixture.closures[0], fixture.closures[1]);
}

#[test]
fn preimage75_v13_hierarchy_exact_both_profiles() {
    check(fixture!("hierarchy"));
}

#[test]
fn preimage75_v13_guarded_fill_exact_both_profiles() {
    check(fixture!("fill"));
}

#[test]
fn preimage75_v13_corpus_is_not_declared_v14() {
    for fixture in [fixture!("hierarchy"), fixture!("fill")] {
        let error =
            VerifiedCanonicalKernelIrV1::from_canonical_bytes(bytes(fixture.kir), Version::V14)
                .unwrap_err();
        assert_eq!(
            error,
            VerifiedCanonicalKernelIrErrorV1::NotDeclaredVersion {
                expected: Version::V14,
                actual: 13,
            },
        );
    }
}
