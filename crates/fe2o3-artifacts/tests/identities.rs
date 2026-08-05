use fe2o3_artifacts::{
    BlockSize, Capability, CodeObjectFormat, CodeObjectIdentity, CompilerIdentity, DigestBytes,
    Dimensions, Endianness, IdentityText, LaunchContract, MAX_IDENTITY_TEXT_BYTES, MAX_NAME_BYTES,
    Name, PointerWidth, TargetIdentity, ToolIdentity, ValidationError,
};

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

#[test]
fn names_and_tool_identities_are_bounded_and_restricted() {
    assert_eq!(
        Name::new("vector_add.kd").unwrap().as_str(),
        "vector_add.kd"
    );
    assert!(Name::new("").is_err());
    assert!(Name::new("not a symbol").is_err());
    assert!(Name::new("x".repeat(MAX_NAME_BYTES + 1)).is_err());
    assert!(IdentityText::new(" compiler").is_err());
    assert!(IdentityText::new("compiler\nversion").is_err());
    assert!(IdentityText::new("x".repeat(MAX_IDENTITY_TEXT_BYTES + 1)).is_err());

    let compiler = CompilerIdentity::new(text("rustc"), text("1.94.0"));
    let producer = ToolIdentity::new(text("fe2o3"), text("0.1.0"));
    assert_eq!(compiler.name().as_str(), "rustc");
    assert_eq!(compiler.version().as_str(), "1.94.0");
    assert_eq!(producer.name().as_str(), "fe2o3");
    assert_eq!(producer.version().as_str(), "0.1.0");
}

#[test]
fn digest_bytes_are_opaque_and_code_objects_require_content() {
    let identity = DigestBytes::from_bytes([0xa5; 32]);
    assert_eq!(identity.as_bytes(), &[0xa5; 32]);
    assert!(CodeObjectIdentity::new(identity, CodeObjectFormat::SpirV, 0).is_err());

    let object = CodeObjectIdentity::new(identity, CodeObjectFormat::SpirV, 7).unwrap();
    assert_eq!(object.digest(), identity);
    assert_eq!(object.format(), CodeObjectFormat::SpirV);
    assert_eq!(object.byte_len(), 7);
}

#[test]
fn target_capabilities_are_unique_and_canonical() {
    let target = TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text("gfx1100"),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![Capability::AmdWave, Capability::Atomics],
    )
    .unwrap();
    assert_eq!(target.triple().as_str(), "amdgcn-amd-amdhsa");
    assert_eq!(target.architecture().as_str(), "gfx1100");
    assert_eq!(target.pointer_width(), PointerWidth::Bits64);
    assert_eq!(target.endianness(), Endianness::Little);
    assert_eq!(
        target.capabilities(),
        &[Capability::Atomics, Capability::AmdWave]
    );

    assert!(matches!(
        TargetIdentity::new(
            text("amdgcn-amd-amdhsa"),
            text("gfx1100"),
            PointerWidth::Bits64,
            Endianness::Little,
            vec![Capability::AmdWave, Capability::AmdWave],
        ),
        Err(ValidationError::Duplicate {
            field: "target capability"
        })
    ));
}

#[test]
fn launch_contract_rejects_invalid_rank_dimensions_and_overflow() {
    let contract = LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
        Dimensions::new(65_535, 1, 1).unwrap(),
        0,
        4096,
    )
    .unwrap();
    assert_eq!(contract.rank(), 1);
    assert_eq!(contract.static_shared_memory_bytes(), 0);
    assert_eq!(contract.max_dynamic_shared_memory_bytes(), 4096);

    assert!(matches!(
        LaunchContract::new(0, BlockSize::Any, Dimensions::new(1, 1, 1).unwrap(), 0, 0,),
        Err(ValidationError::InvalidRank(0))
    ));
    assert!(matches!(
        LaunchContract::new(1, BlockSize::Any, Dimensions::new(1, 2, 1).unwrap(), 0, 0,),
        Err(ValidationError::InvalidDimension { field: "grid" })
    ));
    assert!(matches!(
        LaunchContract::new(
            3,
            BlockSize::Any,
            Dimensions::new(u32::MAX, u32::MAX, u32::MAX).unwrap(),
            0,
            0,
        ),
        Err(ValidationError::Overflow("grid"))
    ));
    assert!(matches!(
        LaunchContract::new(
            1,
            BlockSize::Any,
            Dimensions::new(1, 1, 1).unwrap(),
            1,
            u32::MAX,
        ),
        Err(ValidationError::Overflow("shared memory"))
    ));
}
