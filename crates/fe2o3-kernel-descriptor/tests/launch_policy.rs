use fe2o3_kernel_descriptor::{
    BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, DeviceTargetV1, DimensionsV1,
    EvidenceDigest, EvidenceIdentity, Gfx942KernelFamilyBundleV1, Gfx942LaunchBoundsV1,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelFamilyIdentityV1, KernelFamilyPolicyErrorV1,
    KernelFamilyVariantDescriptorV1, KernelId, KernelInterfaceIdentityV1, LaunchConstraintsV1,
    TypedKernelFamilyVariantExpectationV1, ValidName,
};

struct SaxpyFamily;
struct SaxpyInterface;
struct Workgroup64;
struct Workgroup256;

fn evidence(byte: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([byte; 32]),
        EvidenceDigest::from_sha256_bytes([byte.wrapping_add(1); 32]),
    )
}

fn kernel(id: u8, entry: &str, workgroup: u32, evidence_byte: u8) -> KernelDescriptorV1 {
    KernelDescriptorV1::new(
        KernelId::from_bytes([id; 32]),
        ValidName::new("saxpy").unwrap(),
        ValidName::new(entry).unwrap(),
        ValidName::new(format!("{entry}.kd")).unwrap(),
        evidence(evidence_byte),
        evidence(evidence_byte.wrapping_add(2)),
        vec![],
        KernelAbiLayoutV1::new(0, 0, 8).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(workgroup, 1, 1).unwrap()),
            DimensionsV1::new(65_535, 1, 1).unwrap(),
            workgroup,
            0,
            0,
        )
        .unwrap(),
        vec![],
    )
    .unwrap()
}

fn family(byte: u8) -> KernelFamilyIdentityV1 {
    KernelFamilyIdentityV1::from_opaque_bytes([byte; 32])
}

fn interface(byte: u8) -> KernelInterfaceIdentityV1 {
    KernelInterfaceIdentityV1::from_opaque_bytes([byte; 32])
}

#[allow(clippy::too_many_arguments)]
fn variant(
    family_byte: u8,
    interface_byte: u8,
    variant_name: &str,
    kernel_id: u8,
    entry: &str,
    workgroup: u32,
    evidence_byte: u8,
    artifact_byte: u8,
    bounds: Gfx942LaunchBoundsV1,
) -> KernelFamilyVariantDescriptorV1 {
    KernelFamilyVariantDescriptorV1::new(
        family(family_byte),
        interface(interface_byte),
        ValidName::new(variant_name).unwrap(),
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        &kernel(kernel_id, entry, workgroup, evidence_byte),
        CanonicalCodeObjectDigest::from_bytes([artifact_byte; 32]),
        bounds,
    )
    .unwrap()
}

fn wg64() -> KernelFamilyVariantDescriptorV1 {
    variant(
        1,
        2,
        "wg64",
        3,
        "saxpy_wg64",
        64,
        4,
        5,
        Gfx942LaunchBoundsV1::new(64, 64, 4, 8).unwrap(),
    )
}

fn wg256() -> KernelFamilyVariantDescriptorV1 {
    variant(
        1,
        2,
        "wg256",
        6,
        "saxpy_wg256",
        256,
        7,
        8,
        Gfx942LaunchBoundsV1::new(256, 256, 2, 4).unwrap(),
    )
}

#[test]
fn launch_bounds_enforce_gfx942_limits() {
    assert!(Gfx942LaunchBoundsV1::new(1, 1_024, 1, 8).is_ok());
    assert_eq!(
        Gfx942LaunchBoundsV1::new(0, 64, 1, 8),
        Err(KernelFamilyPolicyErrorV1::InvalidFlatWorkgroupRange)
    );
    assert_eq!(
        Gfx942LaunchBoundsV1::new(65, 64, 1, 8),
        Err(KernelFamilyPolicyErrorV1::InvalidFlatWorkgroupRange)
    );
    assert!(matches!(
        Gfx942LaunchBoundsV1::new(1, 1_025, 1, 8),
        Err(KernelFamilyPolicyErrorV1::FlatWorkgroupLimitExceeded { .. })
    ));
    assert_eq!(
        Gfx942LaunchBoundsV1::new(1, 64, 0, 8),
        Err(KernelFamilyPolicyErrorV1::InvalidWavesPerExecutionUnitRange)
    );
    assert!(matches!(
        Gfx942LaunchBoundsV1::new(1, 64, 1, 9),
        Err(KernelFamilyPolicyErrorV1::WavesPerExecutionUnitLimitExceeded { .. })
    ));
}

#[test]
fn typed_family_variants_share_an_interface_and_admit_exact_policies() {
    let wg64 = wg64();
    let wg256 = wg256();
    assert_ne!(wg64.policy_identity(), wg256.policy_identity());
    let expected64 = TypedKernelFamilyVariantExpectationV1::<
        SaxpyFamily,
        SaxpyInterface,
        Workgroup64,
    >::from_descriptor(&wg64);
    let expected256 = TypedKernelFamilyVariantExpectationV1::<
        SaxpyFamily,
        SaxpyInterface,
        Workgroup256,
    >::from_descriptor(&wg256);
    let bundle = Gfx942KernelFamilyBundleV1::new(vec![wg64, wg256]).unwrap();

    assert_eq!(bundle.logical_name().as_str(), "saxpy");
    let admitted64 = bundle.admit_variant(&expected64).unwrap();
    let admitted256 = bundle.admit_variant(&expected256).unwrap();
    assert_eq!(admitted64.descriptor().entry_name().as_str(), "saxpy_wg64");
    assert_eq!(
        admitted256.descriptor().entry_name().as_str(),
        "saxpy_wg256"
    );
    assert!(!admitted64.grants_load_authority());
    assert!(!admitted64.grants_launch_authority());
}

#[test]
fn target_family_metadata_descriptor_and_artifact_substitution_fail_closed() {
    let original = wg64();
    let expected =
        TypedKernelFamilyVariantExpectationV1::<SaxpyFamily, SaxpyInterface, Workgroup64>::from_descriptor(&original);

    let wrong_target = KernelFamilyVariantDescriptorV1::new(
        family(1),
        interface(2),
        ValidName::new("wg64").unwrap(),
        DeviceTargetV1::parse("gfx950:xnack-").unwrap(),
        &kernel(3, "saxpy_wg64", 64, 4),
        CanonicalCodeObjectDigest::from_bytes([5; 32]),
        Gfx942LaunchBoundsV1::new(64, 64, 4, 8).unwrap(),
    );
    assert_eq!(
        wrong_target,
        Err(KernelFamilyPolicyErrorV1::UnsupportedTarget)
    );

    let cases = [
        (
            variant(
                9,
                2,
                "wg64",
                3,
                "saxpy_wg64",
                64,
                4,
                5,
                Gfx942LaunchBoundsV1::new(64, 64, 4, 8).unwrap(),
            ),
            KernelFamilyPolicyErrorV1::FamilySubstitution,
        ),
        (
            variant(
                1,
                2,
                "wg64",
                3,
                "saxpy_wg64",
                64,
                4,
                5,
                Gfx942LaunchBoundsV1::new(32, 64, 4, 8).unwrap(),
            ),
            KernelFamilyPolicyErrorV1::LaunchMetadataSubstitution,
        ),
        (
            variant(
                1,
                2,
                "wg64",
                3,
                "saxpy_wg64",
                64,
                44,
                5,
                Gfx942LaunchBoundsV1::new(64, 64, 4, 8).unwrap(),
            ),
            KernelFamilyPolicyErrorV1::DescriptorSubstitution,
        ),
        (
            variant(
                1,
                2,
                "wg64",
                3,
                "saxpy_wg64",
                64,
                4,
                55,
                Gfx942LaunchBoundsV1::new(64, 64, 4, 8).unwrap(),
            ),
            KernelFamilyPolicyErrorV1::ArtifactSubstitution,
        ),
    ];

    for (substituted, expected_error) in cases {
        let bundle = Gfx942KernelFamilyBundleV1::new(vec![substituted]).unwrap();
        assert_eq!(bundle.admit_variant(&expected).unwrap_err(), expected_error);
    }
}
