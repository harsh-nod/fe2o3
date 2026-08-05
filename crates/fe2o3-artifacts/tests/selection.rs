#![allow(dead_code)]

mod common;

use common::{digest, kernel_with_object_digest, name, object_identity, target, text};
use fe2o3_artifacts::{
    ArtifactContainerV1, Capability, CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload,
    CompilerIdentity, DeclaredTargetMismatch, DigestAlgorithm, Endianness, KernelSelectionError,
    ManifestV1, PointerWidth, TargetIdentity, ToolIdentity,
};

fn payload(bytes: &[u8]) -> CodeObjectPayload {
    CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, bytes.to_vec()).unwrap()
}

#[test]
fn selection_borrows_the_exact_kernel_target_identity_and_payload() {
    let first = payload(b"first-native-code-object");
    let second = payload(b"second-native-code-object");
    let first_digest = first.digest().bytes();
    let second_digest = second.digest().bytes();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![Capability::AmdWave]),
        vec![
            object_identity(first_digest, first.bytes().len() as u64),
            object_identity(second_digest, second.bytes().len() as u64),
        ],
        vec![
            kernel_with_object_digest(
                0x11,
                "first",
                "first.kd",
                first_digest,
                vec![Capability::AmdWave],
            ),
            kernel_with_object_digest(
                0x12,
                "second",
                "second.kd",
                second_digest,
                vec![Capability::AmdWave],
            ),
        ],
    )
    .unwrap();
    let container =
        ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![second, first]).unwrap();

    let selected = container.select_native_kernel(digest(0x12)).unwrap();
    assert_eq!(selected.manifest(), container.manifest());
    assert_eq!(selected.target(), container.manifest().target());
    assert_eq!(selected.kernel().name(), &name("second"));
    assert_eq!(selected.code_object().digest(), second_digest);
    assert_eq!(selected.payload(), b"second-native-code-object");
    assert_eq!(
        container.select_native_kernel(digest(0xff)),
        Err(KernelSelectionError::UnknownKernel(digest(0xff)))
    );
}

#[test]
fn selection_rejects_payloads_that_require_a_finalizer() {
    let payload = payload(b"relocatable-object");
    let object_digest = payload.digest().bytes();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        vec![
            CodeObjectIdentity::new(
                object_digest,
                CodeObjectFormat::RelocatableObject,
                payload.bytes().len() as u64,
            )
            .unwrap(),
        ],
        vec![kernel_with_object_digest(
            0x11,
            "kernel",
            "kernel.kd",
            object_digest,
            vec![],
        )],
    )
    .unwrap();
    let container =
        ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap();

    assert_eq!(
        container.select_native_kernel(digest(0x11)),
        Err(KernelSelectionError::UnsupportedFormat(
            CodeObjectFormat::RelocatableObject
        ))
    );
}

fn native_container() -> ArtifactContainerV1 {
    let payload = payload(b"native-code-object");
    let object_digest = payload.digest().bytes();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![Capability::AmdWave]),
        vec![object_identity(object_digest, payload.bytes().len() as u64)],
        vec![kernel_with_object_digest(
            0x11,
            "kernel",
            "kernel.kd",
            object_digest,
            vec![Capability::AmdWave],
        )],
    )
    .unwrap();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap()
}

fn runtime_target(
    triple: &str,
    architecture: &str,
    pointer_width: PointerWidth,
    endianness: Endianness,
    capabilities: Vec<Capability>,
) -> TargetIdentity {
    TargetIdentity::new(
        text(triple),
        text(architecture),
        pointer_width,
        endianness,
        capabilities,
    )
    .unwrap()
}

#[test]
fn declared_target_match_is_exact_except_for_capability_supersets() {
    let container = native_container();
    let selected = container.select_native_kernel(digest(0x11)).unwrap();
    let runtime = runtime_target(
        "amdgcn-amd-amdhsa",
        "gfx1100",
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::Atomics, Capability::AmdWave],
    );

    selected.check_declared_target(&runtime).unwrap();
}

#[test]
fn every_declared_target_mismatch_fails_closed() {
    let container = native_container();
    let selected = container.select_native_kernel(digest(0x11)).unwrap();
    let cases = [
        (
            DeclaredTargetMismatch::Triple,
            runtime_target(
                "other-vendor-target",
                "gfx1100",
                PointerWidth::Bits64,
                Endianness::Little,
                vec![Capability::AmdWave],
            ),
        ),
        (
            DeclaredTargetMismatch::Architecture,
            runtime_target(
                "amdgcn-amd-amdhsa",
                "gfx942",
                PointerWidth::Bits64,
                Endianness::Little,
                vec![Capability::AmdWave],
            ),
        ),
        (
            DeclaredTargetMismatch::PointerWidth {
                artifact: PointerWidth::Bits64,
                candidate: PointerWidth::Bits32,
            },
            runtime_target(
                "amdgcn-amd-amdhsa",
                "gfx1100",
                PointerWidth::Bits32,
                Endianness::Little,
                vec![Capability::AmdWave],
            ),
        ),
        (
            DeclaredTargetMismatch::Endianness {
                artifact: Endianness::Little,
                candidate: Endianness::Big,
            },
            runtime_target(
                "amdgcn-amd-amdhsa",
                "gfx1100",
                PointerWidth::Bits64,
                Endianness::Big,
                vec![Capability::AmdWave],
            ),
        ),
        (
            DeclaredTargetMismatch::MissingCapability(Capability::AmdWave),
            runtime_target(
                "amdgcn-amd-amdhsa",
                "gfx1100",
                PointerWidth::Bits64,
                Endianness::Little,
                vec![],
            ),
        ),
    ];

    for (expected, runtime) in cases {
        assert_eq!(selected.check_declared_target(&runtime), Err(expected));
    }
}
