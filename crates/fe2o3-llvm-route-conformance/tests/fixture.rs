#![forbid(unsafe_code)]

//! Deterministic public gfx942 fixture conformance tests.

use fe2o3_llvm_handoff::{
    CallingConventionV1, CodeModelV1, CodeObjectVersionV1, Gfx942HandoffV1, KernelReturnTypeV1,
    KernelValueTypeV1, ModuleFlagV1, NamedMetadataV1, OptimizationLevelV1, ParameterAttributeV1,
    RelocationModelV1, TargetFeatureV1,
};
use fe2o3_llvm_route_conformance::{
    FixtureCollectionOrderV1, GFX942_FIXTURE_ADDRESS_SPACES_V1, GFX942_FIXTURE_ALIGNMENTS_V1,
    GFX942_FIXTURE_DEVICE_LIBRARIES_V1, GFX942_FIXTURE_OBLIGATIONS_V1, GFX942_FIXTURE_ORIGINS_V1,
    Gfx942FixtureBuilderV1, gfx942_fixture_v1,
};

#[test]
fn deterministic_fixture_round_trips_and_canonicalizes_unordered_inputs() {
    let declared = gfx942_fixture_v1().expect("declared fixture must remain valid");
    let repeated = Gfx942FixtureBuilderV1::new()
        .build()
        .expect("repeated fixture must remain valid");
    let reversed = Gfx942FixtureBuilderV1::new()
        .with_collection_order(FixtureCollectionOrderV1::Reversed)
        .build()
        .expect("reversed fixture must remain valid");

    assert_eq!(declared, repeated);
    assert_eq!(declared, reversed);
    assert_eq!(declared.identity(), repeated.identity());
    assert_eq!(declared.identity(), reversed.identity());
    assert_eq!(declared.encode_canonical(), repeated.encode_canonical());
    assert_eq!(declared.encode_canonical(), reversed.encode_canonical());
    assert_eq!(
        declared.identity().to_string(),
        "e9a4f4e16161f71d872cb1b8c3d599da834002e2c99b611e90d676e14d347b24"
    );

    let encoded = declared.encode_canonical();
    let decoded = Gfx942HandoffV1::decode_canonical(encoded.as_bytes())
        .expect("canonical fixture bytes must round trip");
    assert_eq!(decoded, declared);
    assert_eq!(decoded.encode_canonical(), encoded);
}

#[test]
fn fixture_covers_every_current_handoff_v1_family() {
    let fixture = gfx942_fixture_v1().expect("fixture must remain valid");
    let target = fixture.target();
    assert_eq!(target.cpu(), "gfx942");
    assert_eq!(target.code_object_version(), CodeObjectVersionV1::V6);
    assert_eq!(target.optimization_level(), OptimizationLevelV1::O2);
    assert_eq!(target.relocation_model(), RelocationModelV1::Pic);
    assert_eq!(target.code_model(), CodeModelV1::Small);
    assert_eq!(
        target
            .features()
            .iter()
            .map(|feature| (feature.feature(), feature.enabled()))
            .collect::<Vec<_>>(),
        vec![
            (TargetFeatureV1::WavefrontSize32, false),
            (TargetFeatureV1::WavefrontSize64, true),
            (TargetFeatureV1::Xnack, false),
        ]
    );

    let probe = fixture
        .kernels()
        .iter()
        .find(|kernel| kernel.symbol() == "address_space_probe")
        .expect("address-space probe kernel must exist");
    assert_eq!(
        probe.calling_convention(),
        CallingConventionV1::AmdGpuKernel
    );
    assert_eq!(probe.calling_convention().llvm_name(), "amdgpu_kernel");
    assert_eq!(probe.return_type(), KernelReturnTypeV1::Void);

    let mut address_spaces = Vec::new();
    let mut alignments = Vec::new();
    for parameter in probe.parameters() {
        if let KernelValueTypeV1::Pointer { address_space, .. } = parameter.value_type() {
            address_spaces.push(address_space);
            alignments.extend(parameter.attributes().iter().filter_map(|attribute| {
                if let ParameterAttributeV1::Align(value) = attribute {
                    Some(*value)
                } else {
                    None
                }
            }));
        }
    }
    address_spaces.sort_unstable();
    alignments.sort_unstable();
    assert_eq!(address_spaces, GFX942_FIXTURE_ADDRESS_SPACES_V1);
    assert_eq!(alignments, GFX942_FIXTURE_ALIGNMENTS_V1);

    assert_eq!(
        fixture.module().flags(),
        &[
            ModuleFlagV1::CodeObjectVersion6,
            ModuleFlagV1::PicLevel2,
            ModuleFlagV1::WcharSize4,
        ]
    );
    assert_eq!(
        fixture
            .module()
            .named_metadata()
            .iter()
            .map(|metadata| metadata.canonical_name())
            .collect::<Vec<_>>(),
        vec![
            NamedMetadataV1::OpenClVersion2_0.canonical_name(),
            NamedMetadataV1::OpenClSpirVersion2_0.canonical_name(),
            "llvm.ident.sha256",
        ]
    );
    assert_eq!(
        fixture
            .module()
            .device_libraries()
            .iter()
            .map(|library| library.kind())
            .collect::<Vec<_>>(),
        GFX942_FIXTURE_DEVICE_LIBRARIES_V1
    );

    let mut origin_kinds = fixture
        .origins()
        .iter()
        .map(|origin| origin.kind())
        .collect::<Vec<_>>();
    origin_kinds.sort_unstable();
    assert_eq!(origin_kinds, GFX942_FIXTURE_ORIGINS_V1);

    let mut obligation_kinds = fixture
        .obligations()
        .iter()
        .map(|obligation| obligation.kind())
        .collect::<Vec<_>>();
    obligation_kinds.sort_unstable();
    assert_eq!(obligation_kinds, GFX942_FIXTURE_OBLIGATIONS_V1);
    for origin in fixture.origins() {
        assert!(
            fixture
                .obligations()
                .iter()
                .any(|obligation| obligation.origin() == origin.identity()),
            "every fixture origin must be covered by an obligation"
        );
    }
}

#[test]
fn stage_and_nested_identities_survive_the_canonical_round_trip() {
    let fixture = gfx942_fixture_v1().expect("fixture must remain valid");
    let decoded = Gfx942HandoffV1::decode_canonical(fixture.encode_canonical().as_bytes())
        .expect("fixture must round trip");

    assert_eq!(decoded.stage_identities(), fixture.stage_identities());
    assert_eq!(
        decoded
            .origins()
            .iter()
            .map(|origin| origin.identity())
            .collect::<Vec<_>>(),
        fixture
            .origins()
            .iter()
            .map(|origin| origin.identity())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        decoded
            .obligations()
            .iter()
            .map(|obligation| obligation.identity())
            .collect::<Vec<_>>(),
        fixture
            .obligations()
            .iter()
            .map(|obligation| obligation.identity())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        decoded
            .module()
            .device_libraries()
            .iter()
            .map(|library| library.sha256())
            .collect::<Vec<_>>(),
        fixture
            .module()
            .device_libraries()
            .iter()
            .map(|library| library.sha256())
            .collect::<Vec<_>>()
    );
}
