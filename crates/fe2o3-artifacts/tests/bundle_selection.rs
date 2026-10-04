#[allow(dead_code)]
mod common;

use common::{digest, kernel_with_object_digest, name, object_identity, target, text};
use fe2o3_artifacts::{
    ArtifactContainerV1, BundleIndexV1, BundleKernelIndexEntryV1, BundlePayloadReferenceV1,
    BundleTargetAssociationV1, BundleValidationError, Capability, CodeObjectFormat,
    CodeObjectPayload, CompilerIdentity, DigestAlgorithm, Endianness, IdentityText,
    MAX_KERNEL_PAYLOAD_REFERENCES, ManifestV1, PointerWidth, TargetIdentity, ToolIdentity,
};

fn payload(id: u8, format: CodeObjectFormat, byte_len: u64) -> BundlePayloadReferenceV1 {
    BundlePayloadReferenceV1::new(digest(id), format, byte_len).unwrap()
}

fn index() -> BundleIndexV1 {
    BundleIndexV1::new(
        vec![BundleTargetAssociationV1::new(
            digest(0x10),
            target(PointerWidth::Bits64, vec![Capability::AmdWave]),
        )],
        vec![
            payload(0x70, CodeObjectFormat::NativeExecutable, 128),
            payload(0x40, CodeObjectFormat::LlvmBitcode, 96),
            payload(0x30, CodeObjectFormat::NativeExecutable, 48),
        ],
        vec![
            BundleKernelIndexEntryV1::new(
                digest(0x60),
                name("z_kernel.kd"),
                digest(0x10),
                vec![digest(0x70)],
            )
            .unwrap(),
            BundleKernelIndexEntryV1::new(
                digest(0x50),
                name("a_kernel.kd"),
                digest(0x10),
                vec![digest(0x40), digest(0x30)],
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn candidate_target(
    triple: &str,
    architecture: &str,
    pointer_width: PointerWidth,
    endianness: Endianness,
    capabilities: Vec<Capability>,
) -> TargetIdentity {
    TargetIdentity::new(
        IdentityText::new(triple).unwrap(),
        IdentityText::new(architecture).unwrap(),
        pointer_width,
        endianness,
        capabilities,
    )
    .unwrap()
}

fn compatible_target() -> TargetIdentity {
    candidate_target(
        "amdgcn-amd-amdhsa",
        "gfx1100",
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::AmdWave, Capability::Atomics],
    )
}

fn exact_profile() -> Vec<BundlePayloadReferenceV1> {
    vec![
        payload(0x40, CodeObjectFormat::LlvmBitcode, 96),
        payload(0x30, CodeObjectFormat::NativeExecutable, 48),
    ]
}

#[test]
fn selection_is_deterministic_and_set_order_independent() {
    let index = index();
    let selected = index
        .select_kernel_reference(
            digest(0x50),
            &name("a_kernel.kd"),
            &exact_profile(),
            &compatible_target(),
        )
        .unwrap();

    assert_eq!(selected.kernel_id(), digest(0x50));
    assert_eq!(selected.symbol(), &name("a_kernel.kd"));
    assert_eq!(selected.manifest_digest(), digest(0x10));
    assert_eq!(selected.payload_digests(), &[digest(0x30), digest(0x40)]);

    let decoded = BundleIndexV1::from_bytes(&index.to_bytes()).unwrap();
    assert_eq!(
        decoded
            .select_kernel_reference(
                digest(0x50),
                &name("a_kernel.kd"),
                &exact_profile(),
                &compatible_target(),
            )
            .unwrap(),
        selected
    );
}

#[test]
fn logical_identity_and_export_symbol_must_both_match() {
    let index = index();

    assert_eq!(
        index.select_kernel_reference(
            digest(0x51),
            &name("a_kernel.kd"),
            &exact_profile(),
            &compatible_target(),
        ),
        Err(BundleValidationError::UnknownKernel(digest(0x51)))
    );
    assert_eq!(
        index.select_kernel_reference(
            digest(0x50),
            &name("renamed.kd"),
            &exact_profile(),
            &compatible_target(),
        ),
        Err(BundleValidationError::ExportSymbolMismatch {
            kernel_id: digest(0x50)
        })
    );
}

#[test]
fn payload_membership_and_metadata_profile_are_exact() {
    let index = index();

    assert_eq!(
        index.select_kernel_reference(
            digest(0x50),
            &name("a_kernel.kd"),
            &[],
            &compatible_target(),
        ),
        Err(BundleValidationError::EmptyCollection {
            field: "selection payload profile"
        })
    );
    assert_eq!(
        index.select_kernel_reference(
            digest(0x50),
            &name("a_kernel.kd"),
            &[
                payload(0x30, CodeObjectFormat::NativeExecutable, 48),
                payload(0x30, CodeObjectFormat::NativeExecutable, 48),
            ],
            &compatible_target(),
        ),
        Err(BundleValidationError::Duplicate {
            field: "selection payload digest"
        })
    );
    assert_eq!(
        index.select_kernel_reference(
            digest(0x50),
            &name("a_kernel.kd"),
            &vec![
                payload(0x30, CodeObjectFormat::NativeExecutable, 48);
                MAX_KERNEL_PAYLOAD_REFERENCES + 1
            ],
            &compatible_target(),
        ),
        Err(BundleValidationError::TooMany {
            field: "selection payload profile",
            max: MAX_KERNEL_PAYLOAD_REFERENCES,
        })
    );

    for mismatched_membership in [
        vec![payload(0x30, CodeObjectFormat::NativeExecutable, 48)],
        vec![
            payload(0x30, CodeObjectFormat::NativeExecutable, 48),
            payload(0x70, CodeObjectFormat::NativeExecutable, 128),
        ],
    ] {
        assert_eq!(
            index.select_kernel_reference(
                digest(0x50),
                &name("a_kernel.kd"),
                &mismatched_membership,
                &compatible_target(),
            ),
            Err(BundleValidationError::PayloadMembershipMismatch {
                kernel_id: digest(0x50)
            })
        );
    }

    for mismatched_profile in [
        vec![
            payload(0x30, CodeObjectFormat::RelocatableObject, 48),
            payload(0x40, CodeObjectFormat::LlvmBitcode, 96),
        ],
        vec![
            payload(0x30, CodeObjectFormat::NativeExecutable, 49),
            payload(0x40, CodeObjectFormat::LlvmBitcode, 96),
        ],
    ] {
        assert_eq!(
            index.select_kernel_reference(
                digest(0x50),
                &name("a_kernel.kd"),
                &mismatched_profile,
                &compatible_target(),
            ),
            Err(BundleValidationError::PayloadProfileMismatch(digest(0x30)))
        );
    }
}

#[test]
fn target_profile_is_exact_except_for_capability_supersets() {
    let index = index();
    let cases = [
        (
            candidate_target(
                "amdgcn-unknown-amdhsa",
                "gfx1100",
                PointerWidth::Bits64,
                Endianness::Little,
                vec![Capability::AmdWave],
            ),
            "triple",
        ),
        (
            candidate_target(
                "amdgcn-amd-amdhsa",
                "gfx942",
                PointerWidth::Bits64,
                Endianness::Little,
                vec![Capability::AmdWave],
            ),
            "architecture",
        ),
        (
            candidate_target(
                "amdgcn-amd-amdhsa",
                "gfx1100",
                PointerWidth::Bits32,
                Endianness::Little,
                vec![Capability::AmdWave],
            ),
            "pointer width",
        ),
        (
            candidate_target(
                "amdgcn-amd-amdhsa",
                "gfx1100",
                PointerWidth::Bits64,
                Endianness::Big,
                vec![Capability::AmdWave],
            ),
            "endianness",
        ),
    ];

    for (candidate, field) in cases {
        assert_eq!(
            index.select_kernel_reference(
                digest(0x50),
                &name("a_kernel.kd"),
                &exact_profile(),
                &candidate,
            ),
            Err(BundleValidationError::TargetProfileMismatch {
                manifest_digest: digest(0x10),
                field,
            })
        );
    }

    let missing_capability = target(PointerWidth::Bits64, vec![]);
    assert_eq!(
        index.select_kernel_reference(
            digest(0x50),
            &name("a_kernel.kd"),
            &exact_profile(),
            &missing_capability,
        ),
        Err(BundleValidationError::MissingTargetCapability {
            manifest_digest: digest(0x10),
            capability: Capability::AmdWave,
        })
    );
}

fn one_kernel_container(
    id: u8,
    logical_name: &str,
    export_symbol: &str,
    bytes: &[u8],
) -> ArtifactContainerV1 {
    let payload = CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, bytes.to_vec()).unwrap();
    let payload_digest = payload.digest().bytes();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.95.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        vec![object_identity(
            payload_digest,
            payload.bytes().len() as u64,
        )],
        vec![kernel_with_object_digest(
            id,
            logical_name,
            export_symbol,
            payload_digest,
            vec![],
        )],
    )
    .unwrap();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap()
}

#[test]
fn cross_container_logical_and_export_ambiguity_is_rejected() {
    let first = one_kernel_container(0x50, "map", "map_u32.kd", b"first");
    let duplicate_logical = one_kernel_container(0x60, "map", "map_u64.kd", b"second");
    assert_eq!(
        BundleIndexV1::from_containers(&[first, duplicate_logical]),
        Err(BundleValidationError::Duplicate {
            field: "bundle kernel logical name"
        })
    );

    let first = one_kernel_container(0x50, "map", "map_u32.kd", b"first");
    let duplicate_export = one_kernel_container(0x60, "reduce", "map_u32.kd", b"second");
    assert_eq!(
        BundleIndexV1::from_containers(&[first, duplicate_export]),
        Err(BundleValidationError::Duplicate {
            field: "bundle kernel symbol"
        })
    );

    let first = one_kernel_container(0x50, "map", "map_u32.kd", b"first");
    let unique = one_kernel_container(0x60, "reduce", "reduce_u32.kd", b"second");
    let bundle = BundleIndexV1::from_containers(&[unique, first]).unwrap();
    assert!(bundle.kernels()[0].kernel_id() < bundle.kernels()[1].kernel_id());
}
