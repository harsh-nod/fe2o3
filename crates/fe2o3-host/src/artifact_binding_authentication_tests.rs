use super::*;
use crate::loaded_kernel::{LoadedKernelLoadError, validate_issuance};
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
    ArtifactContainerV1, BlockSize, CodeObjectFormat, CodeObjectPayload, CompilerIdentity,
    ContainerValidationError, DigestAlgorithm, Dimensions, IdentityText, KernelEntry, ManifestV1,
    Mutability, Name, ScalarType, ToolIdentity,
};
use std::sync::OnceLock;

const LOGICAL_NAME: &str = "vector_add";
const EXPORT_NAME: &str = "vector_add.kd";
const GENERAL_BINDING: [u8; 32] = [0x73; 32];

fn marker_function() {}

macro_rules! marker {
    ($marker:ident, $bytes:expr) => {
        struct $marker;

        unsafe impl KernelMarkerV1 for $marker {
            type Function = fn();
            type Registration = ();

            const LOGICAL_NAME: &'static str = LOGICAL_NAME;
            const EXPORT_NAME: &'static str = EXPORT_NAME;
            const FUNCTION: Self::Function = marker_function;
            const REGISTRATION: &'static Self::Registration = &();
        }

        // SAFETY: Each fixture deliberately models backend-issued bytes for
        // authentication-only tests. Invalid fixtures exercise rejection and
        // are never loaded; the valid fixture stops at the inert test binding.
        unsafe impl CompilerGeneratedKernelContractV1 for $marker {
            const PROFILE: CompilerGeneratedKernelProfileV1 =
                CompilerGeneratedKernelProfileV1::TypedVecAddF32V1;
            const KERNEL_BINDING_ID_V1: [u8; 32] = [0x42; 32];

            fn artifact_container_bytes() -> &'static [u8] {
                static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
                BYTES.get_or_init(|| $bytes).as_slice()
            }
        }
    };
}

macro_rules! marker_v2 {
    ($marker:ident, $binding:expr, $bytes:expr) => {
        struct $marker;

        unsafe impl KernelMarkerV1 for $marker {
            type Function = fn();
            type Registration = ();

            const LOGICAL_NAME: &'static str = LOGICAL_NAME;
            const EXPORT_NAME: &'static str = EXPORT_NAME;
            const FUNCTION: Self::Function = marker_function;
            const REGISTRATION: &'static Self::Registration = &();
        }

        // SAFETY: These data-only fixtures exercise V2 authentication and are
        // never loaded into HIP.
        unsafe impl CompilerGeneratedKernelContractV1 for $marker {
            const PROFILE: CompilerGeneratedKernelProfileV1 =
                CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2;
            const KERNEL_BINDING_ID_V1: [u8; 32] = $binding;

            fn artifact_container_bytes() -> &'static [u8] {
                static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
                BYTES.get_or_init(|| $bytes).as_slice()
            }
        }
    };
}

marker!(
    ValidMarker,
    container_bytes(&[(0x11, LOGICAL_NAME, EXPORT_NAME)], "gfx942")
);
marker!(MalformedMarker, vec![0, 1, 2, 3]);
marker!(
    ZeroMatchMarker,
    container_bytes(&[(0x11, "other", "other.kd")], "gfx942")
);
marker!(
    PartialNameMismatchMarker,
    container_bytes(&[(0x11, LOGICAL_NAME, "other.kd")], "gfx942")
);
marker!(DuplicateMatchMarker, {
    let mut bytes = container_bytes(
        &[
            (0x11, LOGICAL_NAME, EXPORT_NAME),
            (0x12, "wector_add", "wector_add.kd"),
        ],
        "gfx942",
    );
    mutate_all(&mut bytes, b"wector_add", b"vector_add");
    bytes
});
marker!(PayloadMutationMarker, {
    let mut bytes = container_bytes(&[(0x11, LOGICAL_NAME, EXPORT_NAME)], "gfx942");
    let last = bytes.last_mut().expect("fixture has a payload");
    *last ^= 0xff;
    bytes
});
marker!(ManifestMutationMarker, {
    let mut bytes = container_bytes(&[(0x11, LOGICAL_NAME, EXPORT_NAME)], "gfx942");
    mutate_first(&mut bytes, LOGICAL_NAME.as_bytes(), b"wector_add");
    bytes
});
marker!(WrongEffectProfileMarker, {
    container_bytes_with_abi(
        &[(0x11, LOGICAL_NAME, EXPORT_NAME)],
        "gfx942",
        typed_vecadd_abi(Access::ReadWrite, false),
    )
});
marker!(WrongTypeIdentityProfileMarker, {
    container_bytes_with_abi(
        &[(0x11, LOGICAL_NAME, EXPORT_NAME)],
        "gfx942",
        typed_vecadd_abi(Access::WriteOnly, true),
    )
});
marker!(
    WrongTargetMarker,
    container_bytes(&[(0x11, LOGICAL_NAME, EXPORT_NAME)], "gfx1100")
);
marker_v2!(
    ValidRustcLayoutMarker,
    [0x42; 32],
    rustc_layout_container_bytes([0x42; 32], true, true)
);
marker_v2!(
    WrongRustcLayoutBindingMarker,
    [0x43; 32],
    rustc_layout_container_bytes([0x42; 32], true, true)
);
marker_v2!(
    LegacyIdentityUnderRustcLayoutMarker,
    [0x42; 32],
    rustc_layout_container_bytes([0x42; 32], false, true)
);
marker_v2!(
    ForgedRustcLayoutKernelIdMarker,
    [0x42; 32],
    rustc_layout_container_bytes([0x42; 32], true, false)
);

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

fn digest(byte: u8) -> DigestBytes {
    DigestBytes::from_bytes([byte; 32])
}

fn container_bytes(kernels: &[(u8, &str, &str)], architecture: &str) -> Vec<u8> {
    container_bytes_with_abi(
        kernels,
        architecture,
        typed_vecadd_abi(Access::WriteOnly, false),
    )
}

fn container_bytes_with_abi(
    kernels: &[(u8, &str, &str)],
    architecture: &str,
    abi: AbiLayout,
) -> Vec<u8> {
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"inert-test-hsaco".to_vec())
            .unwrap();
    let object_digest = payload.digest().bytes();
    let code_object = CodeObjectIdentity::new(
        object_digest,
        CodeObjectFormat::NativeExecutable,
        payload.bytes().len() as u64,
    )
    .unwrap();
    let target = TargetIdentity::new(
        text(AMDGPU_TRIPLE),
        text(architecture),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![],
    )
    .unwrap();
    let entries = kernels
        .iter()
        .map(|&(id, logical, export)| {
            KernelEntry::new(
                digest(id),
                name(logical),
                name(export),
                digest(id.wrapping_add(0x20)),
                digest(id.wrapping_add(0x40)),
                object_digest,
                vec![],
                LaunchContract::new(
                    1,
                    BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
                    Dimensions::new(65_535, 1, 1).unwrap(),
                    0,
                    0,
                )
                .unwrap(),
                abi.clone(),
            )
            .unwrap()
        })
        .collect();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("test")),
        ToolIdentity::new(text("fe2o3"), text("test")),
        target,
        vec![code_object],
        entries,
    )
    .unwrap();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload])
        .unwrap()
        .to_bytes()
}

fn rustc_layout_container_bytes(
    binding: [u8; 32],
    canonical_layout: bool,
    canonical_kernel_id: bool,
) -> Vec<u8> {
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"inert-test-hsaco".to_vec())
            .unwrap();
    let object_digest = payload.digest().bytes();
    let code_object = CodeObjectIdentity::new(
        object_digest,
        CodeObjectFormat::NativeExecutable,
        payload.bytes().len() as u64,
    )
    .unwrap();
    let target = TargetIdentity::new(
        text(AMDGPU_TRIPLE),
        text("gfx942"),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![],
    )
    .unwrap();
    let abi = if canonical_layout {
        typed_vecadd_rustc_layout_abi()
    } else {
        typed_vecadd_abi(Access::WriteOnly, false)
    };
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(65_535, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    let source_digest = digest(0x31);
    let executable_digest = digest(0x51);
    let kernel_id = if canonical_kernel_id {
        derive_generated_kernel_identity_v2(
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            binding,
            LOGICAL_NAME,
            EXPORT_NAME,
            source_digest,
            executable_digest,
            &abi,
            &launch,
        )
    } else {
        digest(0x11)
    };
    let kernel = KernelEntry::new(
        kernel_id,
        name(LOGICAL_NAME),
        name(EXPORT_NAME),
        source_digest,
        executable_digest,
        object_digest,
        vec![],
        launch,
        abi,
    )
    .unwrap();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("test")),
        ToolIdentity::new(text("fe2o3"), text("test")),
        target,
        vec![code_object],
        vec![kernel],
    )
    .unwrap();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload])
        .unwrap()
        .to_bytes()
}

fn general_profile_container_bytes(
    abi: AbiLayout,
    launch: LaunchContract,
    kernel_id: DigestBytes,
    source_digest: DigestBytes,
    executable_digest: DigestBytes,
) -> Vec<u8> {
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"general-profile-hsaco".to_vec())
            .unwrap();
    let object_digest = payload.digest().bytes();
    let code_object = CodeObjectIdentity::new(
        object_digest,
        CodeObjectFormat::NativeExecutable,
        payload.bytes().len() as u64,
    )
    .unwrap();
    let target = TargetIdentity::new(
        text(AMDGPU_TRIPLE),
        text("gfx942"),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![],
    )
    .unwrap();
    let kernel = KernelEntry::new(
        kernel_id,
        name(LOGICAL_NAME),
        name(EXPORT_NAME),
        source_digest,
        executable_digest,
        object_digest,
        vec![],
        launch,
        abi,
    )
    .unwrap();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("test")),
        ToolIdentity::new(text("fe2o3"), text("test")),
        target,
        vec![code_object],
        vec![kernel],
    )
    .unwrap();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload])
        .unwrap()
        .to_bytes()
}

fn general_profile_abi(scalar: ScalarType, output_access: Access) -> AbiLayout {
    AbiLayout::new(
        40,
        8,
        PointerWidth::Bits64,
        vec![
            AbiField::new(
                name("count"),
                0,
                4,
                4,
                AbiKind::Scalar(scalar),
                Mutability::Immutable,
                Access::ByValue,
                AddressSpace::Value,
                generated_type_identity("u32", "u32-size4-align4"),
                ArgumentOwnership::ByValue,
                AliasClass::Value,
            )
            .unwrap(),
            AbiField::new(
                name("input"),
                8,
                16,
                8,
                AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                Mutability::Immutable,
                Access::ReadOnly,
                AddressSpace::Global,
                generated_type_identity("&[f32]", "slice-f32-ptr64-size16-align8"),
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            )
            .unwrap(),
            AbiField::new(
                name("output"),
                24,
                16,
                8,
                AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                Mutability::Mutable,
                output_access,
                AddressSpace::Global,
                generated_type_identity(
                    "fe2o3_device::DisjointSlice<f32>",
                    "disjoint-slice-f32-ptr64-size16-align8",
                ),
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn general_profile_launch(block_x: u32) -> LaunchContract {
    LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(block_x, 1, 1).unwrap()),
        Dimensions::new(65_535, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap()
}

fn general_profile_host_contract_identity(
    binding: [u8; 32],
    abi: &AbiLayout,
    launch: &LaunchContract,
) -> DigestBytes {
    derive_generated_host_contract_identity_v1(
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        binding,
        LOGICAL_NAME,
        EXPORT_NAME,
        abi,
        launch,
    )
}

fn general_profile_final_kernel_identity(
    binding: [u8; 32],
    abi: &AbiLayout,
    launch: &LaunchContract,
    source_digest: DigestBytes,
    executable_digest: DigestBytes,
) -> DigestBytes {
    derive_generated_kernel_identity_v2(
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        binding,
        LOGICAL_NAME,
        EXPORT_NAME,
        source_digest,
        executable_digest,
        abi,
        launch,
    )
}

fn validated_general_profile_identity(
    abi: AbiLayout,
    launch: LaunchContract,
    kernel_id: DigestBytes,
) -> ArtifactKernelIdentityV1 {
    validated_general_profile_identity_with_digests(
        abi,
        launch,
        kernel_id,
        digest(0x61),
        digest(0x81),
    )
}

fn validated_general_profile_identity_with_digests(
    abi: AbiLayout,
    launch: LaunchContract,
    kernel_id: DigestBytes,
    source_digest: DigestBytes,
    executable_digest: DigestBytes,
) -> ArtifactKernelIdentityV1 {
    let bytes =
        general_profile_container_bytes(abi, launch, kernel_id, source_digest, executable_digest);
    let container = ArtifactContainerV1::from_bytes(&bytes).unwrap();
    let selected = container.select_native_kernel(kernel_id).unwrap();
    ValidatedArtifactSelectionV1::validate(selected, &context(7, "gfx942"))
        .unwrap()
        .identity()
        .clone()
}

fn typed_vecadd_abi(output_access: Access, wrong_type_identity: bool) -> AbiLayout {
    let kind = AbiKind::Slice {
        element_size: 4,
        element_alignment: 4,
    };
    let shared = if wrong_type_identity {
        TypeIdentity::new(
            DeclaredRustTypeIdentity::from_untrusted_bytes(digest(0xd1)),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(digest(0xd2)),
        )
    } else {
        generated_type_identity("&[f32]", "slice-f32-ptr64-size16-align8")
    };
    let output = generated_type_identity(
        "fe2o3_device::DisjointSlice<f32>",
        "disjoint-slice-f32-ptr64-size16-align8",
    );
    AbiLayout::new(
        48,
        8,
        PointerWidth::Bits64,
        vec![
            AbiField::new(
                name("arg0"),
                0,
                16,
                8,
                kind,
                Mutability::Immutable,
                Access::ReadOnly,
                AddressSpace::Global,
                shared,
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            )
            .unwrap(),
            AbiField::new(
                name("arg1"),
                16,
                16,
                8,
                kind,
                Mutability::Immutable,
                Access::ReadOnly,
                AddressSpace::Global,
                shared,
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            )
            .unwrap(),
            AbiField::new(
                name("arg2"),
                32,
                16,
                8,
                kind,
                Mutability::Mutable,
                output_access,
                AddressSpace::Global,
                output,
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn typed_vecadd_rustc_layout_abi() -> AbiLayout {
    let [shared0, shared1, output] = host_typed_vecadd_type_identities().unwrap();
    let kind = AbiKind::Slice {
        element_size: 4,
        element_alignment: 4,
    };
    AbiLayout::new(
        48,
        8,
        PointerWidth::Bits64,
        vec![
            AbiField::new(
                name("arg0"),
                0,
                16,
                8,
                kind,
                Mutability::Immutable,
                Access::ReadOnly,
                AddressSpace::Global,
                shared0,
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            )
            .unwrap(),
            AbiField::new(
                name("arg1"),
                16,
                16,
                8,
                kind,
                Mutability::Immutable,
                Access::ReadOnly,
                AddressSpace::Global,
                shared1,
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            )
            .unwrap(),
            AbiField::new(
                name("arg2"),
                32,
                16,
                8,
                kind,
                Mutability::Mutable,
                Access::WriteOnly,
                AddressSpace::Global,
                output,
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn mutate_first(bytes: &mut [u8], from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let offset = bytes
        .windows(from.len())
        .position(|window| window == from)
        .expect("fixture contains manifest name");
    bytes[offset..offset + to.len()].copy_from_slice(to);
}

fn mutate_all(bytes: &mut [u8], from: &[u8], to: &[u8]) {
    assert_eq!(from.len(), to.len());
    let mut offset = 0;
    while let Some(relative) = bytes[offset..]
        .windows(from.len())
        .position(|window| window == from)
    {
        let start = offset + relative;
        bytes[start..start + to.len()].copy_from_slice(to);
        offset = start + to.len();
    }
}

fn context(identity: usize, target: &str) -> ObservedContext {
    ObservedContext::for_test(identity, 0, target, 1_024, 65_536)
}

#[test]
fn authenticates_exact_embedded_bytes_and_mints_only_an_inert_test_binding() {
    let observed = context(7, "gfx942");
    let authenticated = AuthenticatedKernelArtifactV1::<ValidMarker>::authenticate(&observed)
        .expect("exact embedded fixture must authenticate");
    assert_eq!(authenticated.identity().name().as_str(), LOGICAL_NAME);
    assert_eq!(authenticated.identity().symbol().as_str(), EXPORT_NAME);

    let AuthenticatedKernelArtifactV1 { validated, binding } = authenticated;
    let loaded = LoadedKernel::from_test_binding(binding.into_inner());
    assert_eq!(loaded.identity(), validated.identity());
    assert_eq!(loaded.device(), observed.device());
}

#[test]
fn rejects_malformed_embedded_bytes() {
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<MalformedMarker>::authenticate(&context(7, "gfx942")),
        Err(GeneratedArtifactAuthenticationError::Decode(_))
    ));
}

#[test]
fn rejects_zero_and_duplicate_exact_name_matches() {
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<ZeroMatchMarker>::authenticate(&context(7, "gfx942")),
        Err(GeneratedArtifactAuthenticationError::MatchingKernelNotFound)
    ));
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<DuplicateMatchMarker>::authenticate(&context(7, "gfx942")),
        Err(GeneratedArtifactAuthenticationError::Decode(_))
    ));
}

#[test]
fn rejects_partial_marker_name_mismatch() {
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<PartialNameMismatchMarker>::authenticate(&context(
            7, "gfx942"
        )),
        Err(GeneratedArtifactAuthenticationError::MatchingKernelNotFound)
    ));
}

#[test]
fn rejects_payload_and_manifest_mutation() {
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<PayloadMutationMarker>::authenticate(&context(7, "gfx942")),
        Err(GeneratedArtifactAuthenticationError::Decode(
            ContainerDecodeError::Validation(ContainerValidationError::DigestMismatch(_))
        ))
    ));
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<ManifestMutationMarker>::authenticate(&context(
            7, "gfx942"
        )),
        Err(GeneratedArtifactAuthenticationError::MatchingKernelNotFound)
    ));
}

#[test]
fn rejects_wrong_effects_and_opaque_type_identity() {
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<WrongEffectProfileMarker>::authenticate(&context(
            7, "gfx942",
        )),
        Err(GeneratedArtifactAuthenticationError::Profile(
            GeneratedKernelProfileError::AbiMismatch
        ))
    ));
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<WrongTypeIdentityProfileMarker>::authenticate(&context(
            7, "gfx942",
        )),
        Err(GeneratedArtifactAuthenticationError::Profile(
            GeneratedKernelProfileError::AbiMismatch
        ))
    ));
}

#[test]
fn authenticates_only_canonical_rustc_layout_evidence_and_bound_identity() {
    AuthenticatedKernelArtifactV1::<ValidRustcLayoutMarker>::authenticate(&context(7, "gfx942"))
        .expect("canonical V2 evidence and binding must authenticate");

    assert!(matches!(
        AuthenticatedKernelArtifactV1::<LegacyIdentityUnderRustcLayoutMarker>::authenticate(
            &context(7, "gfx942")
        ),
        Err(GeneratedArtifactAuthenticationError::Profile(
            GeneratedKernelProfileError::AbiMismatch
        ))
    ));
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<WrongRustcLayoutBindingMarker>::authenticate(&context(
            7, "gfx942"
        )),
        Err(GeneratedArtifactAuthenticationError::Profile(
            GeneratedKernelProfileError::KernelIdentityMismatch
        ))
    ));
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<ForgedRustcLayoutKernelIdMarker>::authenticate(&context(
            7, "gfx942"
        )),
        Err(GeneratedArtifactAuthenticationError::Profile(
            GeneratedKernelProfileError::KernelIdentityMismatch
        ))
    ));
}

#[test]
fn authenticates_independent_bounded_scalar_slice_contract_identity() {
    let abi = general_profile_abi(ScalarType::U32, Access::WriteOnly);
    let launch = general_profile_launch(256);
    let generated_host_contract =
        general_profile_host_contract_identity(GENERAL_BINDING, &abi, &launch);
    let final_kernel_identity = general_profile_final_kernel_identity(
        GENERAL_BINDING,
        &abi,
        &launch,
        digest(0x61),
        digest(0x81),
    );
    assert_ne!(generated_host_contract, final_kernel_identity);
    let identity = validated_general_profile_identity(abi, launch, final_kernel_identity);

    validate_generated_profile(
        CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity: *generated_host_contract.as_bytes(),
        },
        GENERAL_BINDING,
        &identity,
    )
    .expect("independent generated expectation must match the artifact");
}

#[test]
fn rejects_general_profile_abi_effect_launch_binding_and_contract_identity_mismatches() {
    let expected_abi = general_profile_abi(ScalarType::U32, Access::WriteOnly);
    let expected_launch = general_profile_launch(256);
    let generated_host_contract =
        general_profile_host_contract_identity(GENERAL_BINDING, &expected_abi, &expected_launch);
    let final_kernel_identity = general_profile_final_kernel_identity(
        GENERAL_BINDING,
        &expected_abi,
        &expected_launch,
        digest(0x61),
        digest(0x81),
    );
    let profile = CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
        generated_host_contract_identity: *generated_host_contract.as_bytes(),
    };

    let cases = [
        validated_general_profile_identity(
            general_profile_abi(ScalarType::F32, Access::WriteOnly),
            expected_launch.clone(),
            final_kernel_identity,
        ),
        validated_general_profile_identity(
            general_profile_abi(ScalarType::U32, Access::ReadWrite),
            expected_launch.clone(),
            final_kernel_identity,
        ),
        validated_general_profile_identity(
            expected_abi.clone(),
            general_profile_launch(128),
            final_kernel_identity,
        ),
    ];
    for identity in cases {
        assert_eq!(
            validate_generated_profile(profile, GENERAL_BINDING, &identity),
            Err(GeneratedKernelProfileError::GeneratedContractIdentityMismatch)
        );
    }

    let identity =
        validated_general_profile_identity(expected_abi, expected_launch, final_kernel_identity);
    assert_eq!(
        validate_generated_profile(profile, [0x74; 32], &identity),
        Err(GeneratedKernelProfileError::GeneratedContractIdentityMismatch)
    );
    assert_eq!(
        validate_generated_profile(
            CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
                generated_host_contract_identity: [0xa5; 32],
            },
            GENERAL_BINDING,
            &identity,
        ),
        Err(GeneratedKernelProfileError::GeneratedContractIdentityMismatch)
    );
}

#[test]
fn general_profile_checks_final_source_executable_and_kernel_identity_separately() {
    let abi = general_profile_abi(ScalarType::U32, Access::WriteOnly);
    let launch = general_profile_launch(256);
    let generated_host_contract =
        general_profile_host_contract_identity(GENERAL_BINDING, &abi, &launch);
    let final_kernel_identity = general_profile_final_kernel_identity(
        GENERAL_BINDING,
        &abi,
        &launch,
        digest(0x61),
        digest(0x81),
    );
    let profile = CompilerGeneratedKernelProfileV1::ManifestDerivedScalarSliceV1 {
        generated_host_contract_identity: *generated_host_contract.as_bytes(),
    };

    let cases = [
        validated_general_profile_identity(abi.clone(), launch.clone(), digest(0xf1)),
        validated_general_profile_identity_with_digests(
            abi.clone(),
            launch.clone(),
            final_kernel_identity,
            digest(0x62),
            digest(0x81),
        ),
        validated_general_profile_identity_with_digests(
            abi,
            launch,
            final_kernel_identity,
            digest(0x61),
            digest(0x82),
        ),
    ];
    for identity in cases {
        assert_eq!(
            validate_generated_profile(profile, GENERAL_BINDING, &identity),
            Err(GeneratedKernelProfileError::KernelIdentityMismatch)
        );
    }
}

#[test]
fn rejects_wrong_target_and_rebinding_to_another_context() {
    assert!(matches!(
        AuthenticatedKernelArtifactV1::<WrongTargetMarker>::authenticate(&context(7, "gfx942")),
        Err(GeneratedArtifactAuthenticationError::Binding(
            ArtifactBindingError::IncompatibleAmdTarget { .. }
        ))
    ));

    let observed = context(7, "gfx942");
    let authenticated =
        AuthenticatedKernelArtifactV1::<ValidMarker>::authenticate(&observed).unwrap();
    let AuthenticatedKernelArtifactV1 { validated, binding } = authenticated;
    let binding = binding.into_inner();
    assert!(matches!(
        validate_issuance(&binding, &validated, &context(8, "gfx942")),
        Err(LoadedKernelLoadError::WrongContext)
    ));
}
