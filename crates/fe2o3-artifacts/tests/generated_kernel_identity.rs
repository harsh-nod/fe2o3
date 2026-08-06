use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestBytes, Dimensions, LaunchContract,
    Mutability, Name, PointerWidth, TypeIdentity, derive_generated_kernel_identity_v2,
};

fn digest(byte: u8) -> DigestBytes {
    DigestBytes::from_bytes([byte; 32])
}

fn identity(type_byte: u8, layout_byte: u8) -> TypeIdentity {
    TypeIdentity::new(
        DeclaredRustTypeIdentity::from_untrusted_bytes(digest(type_byte)),
        DeclaredRustLayoutIdentity::from_untrusted_bytes(digest(layout_byte)),
    )
}

fn abi(output_access: Access, output_identity: TypeIdentity) -> AbiLayout {
    let kind = AbiKind::Slice {
        element_size: 4,
        element_alignment: 4,
    };
    let shared = identity(0x10, 0x11);
    AbiLayout::new(
        48,
        8,
        PointerWidth::Bits64,
        vec![
            AbiField::new(
                Name::new("arg0").unwrap(),
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
                Name::new("arg1").unwrap(),
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
                Name::new("arg2").unwrap(),
                32,
                16,
                8,
                kind,
                Mutability::Mutable,
                output_access,
                AddressSpace::Global,
                output_identity,
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn launch(block_x: u32) -> LaunchContract {
    LaunchContract::new(
        1,
        BlockSize::Exact(Dimensions::new(block_x, 1, 1).unwrap()),
        Dimensions::new(u32::MAX, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap()
}

fn derive(
    binding: [u8; 32],
    profile: &str,
    abi: &AbiLayout,
    launch: &LaunchContract,
) -> DigestBytes {
    derive_generated_kernel_identity_v2(
        profile,
        binding,
        "vecadd",
        "vecadd",
        digest(0x20),
        digest(0x21),
        abi,
        launch,
    )
}

fn hex(digest: DigestBytes) -> String {
    digest
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn generated_identity_is_deterministic_and_contract_complete() {
    let base_abi = abi(Access::WriteOnly, identity(0x12, 0x13));
    let base_launch = launch(256);
    let base = derive(
        [0x30; 32],
        "typed-vecadd-layout-v2",
        &base_abi,
        &base_launch,
    );
    assert_eq!(
        base,
        derive(
            [0x30; 32],
            "typed-vecadd-layout-v2",
            &base_abi,
            &base_launch
        )
    );
    assert_eq!(
        hex(base),
        "9103fdd4eb558db62b5b26476a02051fe8a2302ae6ae8623c93c103e15ed7612"
    );

    assert_ne!(
        base,
        derive(
            [0x31; 32],
            "typed-vecadd-layout-v2",
            &base_abi,
            &base_launch
        )
    );
    assert_ne!(
        base,
        derive([0x30; 32], "different-profile", &base_abi, &base_launch)
    );
    assert_ne!(
        base,
        derive(
            [0x30; 32],
            "typed-vecadd-layout-v2",
            &abi(Access::ReadWrite, identity(0x12, 0x13)),
            &base_launch,
        )
    );
    assert_ne!(
        base,
        derive(
            [0x30; 32],
            "typed-vecadd-layout-v2",
            &abi(Access::WriteOnly, identity(0x12, 0x14)),
            &base_launch,
        )
    );
    assert_ne!(
        base,
        derive(
            [0x30; 32],
            "typed-vecadd-layout-v2",
            &base_abi,
            &launch(128),
        )
    );
}
