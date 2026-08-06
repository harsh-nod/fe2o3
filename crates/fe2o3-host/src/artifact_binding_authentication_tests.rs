use super::*;
use crate::loaded_kernel::{LoadedKernelLoadError, validate_issuance};
use fe2o3_artifacts::{
    AbiLayout, ArtifactContainerV1, BlockSize, CodeObjectFormat, CodeObjectPayload,
    CompilerIdentity, ContainerValidationError, DigestAlgorithm, Dimensions, IdentityText,
    KernelEntry, ManifestV1, Name, ToolIdentity,
};
use std::sync::OnceLock;

const LOGICAL_NAME: &str = "vector_add";
const EXPORT_NAME: &str = "vector_add.kd";

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
marker!(
    WrongTargetMarker,
    container_bytes(&[(0x11, LOGICAL_NAME, EXPORT_NAME)], "gfx1100")
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
                    BlockSize::Exact(Dimensions::new(64, 1, 1).unwrap()),
                    Dimensions::new(65_535, 1, 1).unwrap(),
                    0,
                    0,
                )
                .unwrap(),
                AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).unwrap(),
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
