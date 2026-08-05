use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, BlockSize, Capability, CodeObjectFormat,
    CodeObjectIdentity, CompilerIdentity, DigestBytes, Dimensions, Endianness, IdentityText,
    KernelEntry, LaunchContract, MAX_CODE_OBJECTS, ManifestV1, Mutability, Name, PointerWidth,
    ScalarType, TargetIdentity, ToolIdentity, ValidationError,
};

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

fn digest(byte: u8) -> DigestBytes {
    DigestBytes::from_bytes([byte; 32])
}

fn object(id: u8) -> CodeObjectIdentity {
    CodeObjectIdentity::new(digest(id), CodeObjectFormat::NativeExecutable, 12_345).unwrap()
}

fn target(pointer_width: PointerWidth, capabilities: Vec<Capability>) -> TargetIdentity {
    TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text("gfx1100"),
        pointer_width,
        Endianness::Little,
        capabilities,
    )
    .unwrap()
}

fn abi(pointer_width: PointerWidth) -> AbiLayout {
    AbiLayout::new(
        4,
        4,
        pointer_width,
        vec![
            AbiField::new(
                name("n"),
                0,
                4,
                4,
                AbiKind::Scalar(ScalarType::U32),
                Mutability::Immutable,
                Access::ByValue,
                AddressSpace::Value,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn kernel(
    id: u8,
    logical_name: &str,
    symbol: &str,
    object_id: u8,
    capabilities: Vec<Capability>,
    pointer_width: PointerWidth,
) -> KernelEntry {
    KernelEntry::new(
        digest(id),
        name(logical_name),
        name(symbol),
        digest(0x22),
        digest(0x33),
        digest(object_id),
        capabilities,
        LaunchContract::new(
            1,
            BlockSize::Any,
            Dimensions::new(65_535, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap(),
        abi(pointer_width),
    )
    .unwrap()
}

fn manifest(
    target: TargetIdentity,
    objects: Vec<CodeObjectIdentity>,
    kernels: Vec<KernelEntry>,
) -> Result<ManifestV1, ValidationError> {
    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target,
        objects,
        kernels,
    )
}

#[test]
fn manifest_requires_closed_object_capability_and_pointer_references() {
    let valid = manifest(
        target(PointerWidth::Bits64, vec![Capability::AmdWave]),
        vec![object(0x44)],
        vec![kernel(
            0x11,
            "vector_add",
            "vector_add.kd",
            0x44,
            vec![Capability::AmdWave],
            PointerWidth::Bits64,
        )],
    )
    .unwrap();
    assert_eq!(valid.kernels()[0].code_object_digest(), digest(0x44));

    let missing_object = manifest(
        target(PointerWidth::Bits64, vec![Capability::AmdWave]),
        vec![object(0x43)],
        vec![kernel(
            0x11,
            "vector_add",
            "vector_add.kd",
            0x44,
            vec![Capability::AmdWave],
            PointerWidth::Bits64,
        )],
    );
    assert_eq!(missing_object, Err(ValidationError::MissingCodeObject));

    let missing_capability = manifest(
        target(PointerWidth::Bits64, vec![]),
        vec![object(0x44)],
        vec![kernel(
            0x11,
            "vector_add",
            "vector_add.kd",
            0x44,
            vec![Capability::AmdWave],
            PointerWidth::Bits64,
        )],
    );
    assert_eq!(
        missing_capability,
        Err(ValidationError::MissingCapability("amd-wave"))
    );

    let width_mismatch = manifest(
        target(PointerWidth::Bits32, vec![]),
        vec![object(0x44)],
        vec![kernel(
            0x11,
            "vector_add",
            "vector_add.kd",
            0x44,
            vec![],
            PointerWidth::Bits64,
        )],
    );
    assert_eq!(width_mismatch, Err(ValidationError::PointerWidthMismatch));
}

#[test]
fn manifest_canonicalizes_sets_and_rejects_duplicate_or_excess_records() {
    let validated = manifest(
        target(
            PointerWidth::Bits64,
            vec![Capability::AmdWave, Capability::Atomics],
        ),
        vec![object(0x44), object(0x43)],
        vec![
            kernel(
                0x12,
                "z_kernel",
                "z_kernel.kd",
                0x44,
                vec![Capability::AmdWave],
                PointerWidth::Bits64,
            ),
            kernel(
                0x11,
                "a_kernel",
                "a_kernel.kd",
                0x43,
                vec![Capability::Atomics],
                PointerWidth::Bits64,
            ),
        ],
    )
    .unwrap();
    assert_eq!(validated.code_objects()[0].digest(), digest(0x43));
    assert_eq!(validated.kernels()[0].kernel_id(), digest(0x11));
    assert_eq!(
        validated.target().capabilities(),
        &[Capability::Atomics, Capability::AmdWave]
    );

    let duplicate_name = manifest(
        target(PointerWidth::Bits64, vec![]),
        vec![object(0x43), object(0x44)],
        vec![
            kernel(0x11, "same", "first.kd", 0x43, vec![], PointerWidth::Bits64),
            kernel(
                0x12,
                "same",
                "second.kd",
                0x44,
                vec![],
                PointerWidth::Bits64,
            ),
        ],
    );
    assert!(matches!(
        duplicate_name,
        Err(ValidationError::Duplicate {
            field: "kernel name"
        })
    ));

    assert!(matches!(
        manifest(
            target(PointerWidth::Bits64, vec![]),
            vec![],
            vec![kernel(
                0x11,
                "kernel",
                "kernel.kd",
                0x44,
                vec![],
                PointerWidth::Bits64,
            )],
        ),
        Err(ValidationError::EmptyCollection {
            field: "code objects"
        })
    ));
    assert!(matches!(
        manifest(
            target(PointerWidth::Bits64, vec![]),
            vec![object(0x44); MAX_CODE_OBJECTS + 1],
            vec![kernel(
                0x11,
                "kernel",
                "kernel.kd",
                0x44,
                vec![],
                PointerWidth::Bits64,
            )],
        ),
        Err(ValidationError::TooMany {
            field: "code objects",
            ..
        })
    ));
}
