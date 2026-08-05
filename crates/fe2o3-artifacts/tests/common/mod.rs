use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, BlockSize, Capability, CodeObjectFormat,
    CodeObjectIdentity, CompilerIdentity, DigestBytes, Dimensions, Endianness, IdentityText,
    KernelEntry, LaunchContract, ManifestV1, Mutability, Name, PointerWidth, ScalarType,
    TargetIdentity, ToolIdentity,
};

pub fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

pub fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

pub fn digest(byte: u8) -> DigestBytes {
    DigestBytes::from_bytes([byte; 32])
}

fn launch() -> LaunchContract {
    LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(65_535, 1, 1).unwrap(),
        0,
        4096,
    )
    .unwrap()
}

fn abi(pointer_width: PointerWidth) -> AbiLayout {
    AbiLayout::new(
        32,
        8,
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
            AbiField::new(
                name("input"),
                8,
                8,
                8,
                AbiKind::Pointer {
                    pointee_size: 4,
                    pointee_alignment: 4,
                },
                Mutability::Immutable,
                Access::ReadOnly,
                AddressSpace::Global,
            )
            .unwrap(),
            AbiField::new(
                name("output"),
                16,
                16,
                8,
                AbiKind::Slice {
                    element_size: 4,
                    element_alignment: 4,
                },
                Mutability::Mutable,
                Access::ReadWrite,
                AddressSpace::Global,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

pub fn target(pointer_width: PointerWidth, capabilities: Vec<Capability>) -> TargetIdentity {
    TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text("gfx1100"),
        pointer_width,
        Endianness::Little,
        capabilities,
    )
    .unwrap()
}

pub fn kernel(
    id: u8,
    logical_name: &str,
    symbol: &str,
    object_id: u8,
    capabilities: Vec<Capability>,
) -> KernelEntry {
    KernelEntry::new(
        digest(id),
        name(logical_name),
        name(symbol),
        digest(0x22),
        digest(0x33),
        digest(object_id),
        capabilities,
        launch(),
        abi(PointerWidth::Bits64),
    )
    .unwrap()
}

pub fn object(id: u8) -> CodeObjectIdentity {
    CodeObjectIdentity::new(digest(id), CodeObjectFormat::NativeExecutable, 12_345).unwrap()
}

pub fn manifest() -> ManifestV1 {
    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(
            PointerWidth::Bits64,
            vec![Capability::AmdWave, Capability::Atomics],
        ),
        vec![object(0x44)],
        vec![kernel(
            0x11,
            "vector_add",
            "vector_add.kd",
            0x44,
            vec![Capability::AmdWave],
        )],
    )
    .unwrap()
}
