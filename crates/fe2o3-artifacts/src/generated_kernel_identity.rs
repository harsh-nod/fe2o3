use sha2::{Digest, Sha256};

use crate::{
    AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    DigestBytes, LaunchContract, Mutability, PointerWidth, ScalarType,
};

/// Domain for compiler-generated kernel contract identities.
pub const GENERATED_KERNEL_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/GENERATED-KERNEL-IDENTITY/V2\0";

/// Domain for pre-executable compiler-generated host contract identities.
pub const GENERATED_HOST_CONTRACT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GENERATED-HOST-CONTRACT-IDENTITY/V1\0";

/// Derives the pre-executable identity of a generated host contract.
///
/// This commits to the complete canonical host ABI, including Rust type and
/// layout identities and memory effects, plus launch geometry and marker
/// binding. It intentionally excludes source and executable digests so a
/// procedural macro can emit it before device compilation and linking.
#[allow(clippy::too_many_arguments)]
pub fn derive_generated_host_contract_identity_v1(
    profile_tag: &str,
    kernel_binding: [u8; 32],
    logical_name: &str,
    export_name: &str,
    abi: &AbiLayout,
    launch: &LaunchContract,
) -> DigestBytes {
    derive_identity(
        GENERATED_HOST_CONTRACT_IDENTITY_DOMAIN_V1,
        profile_tag,
        kernel_binding,
        logical_name,
        export_name,
        None,
        abi,
        launch,
    )
}

/// Derives an identity over the complete generated kernel contract.
#[allow(clippy::too_many_arguments)]
pub fn derive_generated_kernel_identity_v2(
    profile_tag: &str,
    kernel_binding: [u8; 32],
    logical_name: &str,
    export_name: &str,
    source_digest: DigestBytes,
    executable_digest: DigestBytes,
    abi: &AbiLayout,
    launch: &LaunchContract,
) -> DigestBytes {
    derive_identity(
        GENERATED_KERNEL_IDENTITY_DOMAIN_V2,
        profile_tag,
        kernel_binding,
        logical_name,
        export_name,
        Some((source_digest, executable_digest)),
        abi,
        launch,
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_identity(
    domain: &[u8],
    profile_tag: &str,
    kernel_binding: [u8; 32],
    logical_name: &str,
    export_name: &str,
    source_and_executable: Option<(DigestBytes, DigestBytes)>,
    abi: &AbiLayout,
    launch: &LaunchContract,
) -> DigestBytes {
    let mut bytes = Vec::new();
    frame(&mut bytes, domain);
    frame(&mut bytes, profile_tag.as_bytes());
    frame(&mut bytes, &kernel_binding);
    frame(&mut bytes, logical_name.as_bytes());
    frame(&mut bytes, export_name.as_bytes());
    if let Some((source_digest, executable_digest)) = source_and_executable {
        frame(&mut bytes, source_digest.as_bytes());
        frame(&mut bytes, executable_digest.as_bytes());
    }
    encode_abi(&mut bytes, abi);
    encode_launch(&mut bytes, launch);
    DigestBytes::from_bytes(Sha256::digest(bytes).into())
}

fn encode_abi(bytes: &mut Vec<u8>, abi: &AbiLayout) {
    bytes.extend_from_slice(&abi.size().to_le_bytes());
    bytes.extend_from_slice(&abi.alignment().to_le_bytes());
    bytes.push(pointer_width_tag(abi.pointer_width()));
    bytes.extend_from_slice(&(abi.fields().len() as u32).to_le_bytes());
    for field in abi.fields() {
        frame(bytes, field.name().as_str().as_bytes());
        bytes.extend_from_slice(&field.offset().to_le_bytes());
        bytes.extend_from_slice(&field.size().to_le_bytes());
        bytes.extend_from_slice(&field.alignment().to_le_bytes());
        match field.kind() {
            AbiKind::Scalar(scalar) => {
                bytes.push(1);
                bytes.push(scalar_tag(scalar));
            }
            AbiKind::Pointer {
                pointee_size,
                pointee_alignment,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&pointee_size.to_le_bytes());
                bytes.extend_from_slice(&pointee_alignment.to_le_bytes());
            }
            AbiKind::Slice {
                element_size,
                element_alignment,
            } => {
                bytes.push(3);
                bytes.extend_from_slice(&element_size.to_le_bytes());
                bytes.extend_from_slice(&element_alignment.to_le_bytes());
            }
        }
        bytes.push(mutability_tag(field.mutability()));
        bytes.push(access_tag(field.access()));
        bytes.push(address_space_tag(field.address_space()));
        bytes.push(ownership_tag(field.ownership()));
        bytes.push(alias_tag(field.alias_class()));
        frame(bytes, field.type_identity().rust_type().bytes().as_bytes());
        frame(bytes, field.type_identity().layout().bytes().as_bytes());
    }
}

fn encode_launch(bytes: &mut Vec<u8>, launch: &LaunchContract) {
    bytes.push(launch.rank());
    match launch.block_size() {
        BlockSize::Any => bytes.push(1),
        BlockSize::Exact(dimensions) => {
            bytes.push(2);
            encode_dimensions(bytes, dimensions);
        }
        BlockSize::AtMost(dimensions) => {
            bytes.push(3);
            encode_dimensions(bytes, dimensions);
        }
    }
    encode_dimensions(bytes, launch.max_grid());
    bytes.extend_from_slice(&launch.static_shared_memory_bytes().to_le_bytes());
    bytes.extend_from_slice(&launch.max_dynamic_shared_memory_bytes().to_le_bytes());
}

fn encode_dimensions(bytes: &mut Vec<u8>, dimensions: crate::Dimensions) {
    bytes.extend_from_slice(&dimensions.x().to_le_bytes());
    bytes.extend_from_slice(&dimensions.y().to_le_bytes());
    bytes.extend_from_slice(&dimensions.z().to_le_bytes());
}

fn frame(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&(field.len() as u64).to_le_bytes());
    bytes.extend_from_slice(field);
}

fn pointer_width_tag(value: PointerWidth) -> u8 {
    match value {
        PointerWidth::Bits32 => 1,
        PointerWidth::Bits64 => 2,
    }
}

fn scalar_tag(value: ScalarType) -> u8 {
    match value {
        ScalarType::I8 => 1,
        ScalarType::U8 => 2,
        ScalarType::I16 => 3,
        ScalarType::U16 => 4,
        ScalarType::I32 => 5,
        ScalarType::U32 => 6,
        ScalarType::I64 => 7,
        ScalarType::U64 => 8,
        ScalarType::F16 => 9,
        ScalarType::F32 => 10,
        ScalarType::F64 => 11,
    }
}

fn mutability_tag(value: Mutability) -> u8 {
    match value {
        Mutability::Immutable => 1,
        Mutability::Mutable => 2,
    }
}

fn access_tag(value: Access) -> u8 {
    match value {
        Access::ByValue => 1,
        Access::ReadOnly => 2,
        Access::WriteOnly => 3,
        Access::ReadWrite => 4,
    }
}

fn address_space_tag(value: AddressSpace) -> u8 {
    match value {
        AddressSpace::Value => 1,
        AddressSpace::Global => 2,
        AddressSpace::Constant => 3,
        AddressSpace::Workgroup => 4,
        AddressSpace::Private => 5,
        AddressSpace::Generic => 6,
    }
}

fn ownership_tag(value: ArgumentOwnership) -> u8 {
    match value {
        ArgumentOwnership::ByValue => 1,
        ArgumentOwnership::SharedBorrow => 2,
        ArgumentOwnership::UniqueBorrow => 3,
        ArgumentOwnership::RawPointer => 4,
    }
}

fn alias_tag(value: AliasClass) -> u8 {
    match value {
        AliasClass::Value => 1,
        AliasClass::SharedReadOnly => 2,
        AliasClass::Exclusive => 3,
        AliasClass::SharedAtomic => 4,
        AliasClass::Unrestricted => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AbiField, DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, Dimensions, Name,
        TypeIdentity,
    };

    const PROFILE: &str = "test.scalar-slice.v1";
    const BINDING: [u8; 32] = [0x42; 32];

    fn digest(byte: u8) -> DigestBytes {
        DigestBytes::from_bytes([byte; 32])
    }

    fn type_identity(byte: u8) -> TypeIdentity {
        TypeIdentity::new(
            DeclaredRustTypeIdentity::from_untrusted_bytes(digest(byte)),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(digest(byte.wrapping_add(1))),
        )
    }

    fn hex(digest: DigestBytes) -> String {
        digest
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn abi(access: Access, identity: TypeIdentity) -> AbiLayout {
        AbiLayout::new(
            16,
            8,
            PointerWidth::Bits64,
            vec![
                AbiField::new(
                    Name::new("values").unwrap(),
                    0,
                    16,
                    8,
                    AbiKind::Slice {
                        element_size: 4,
                        element_alignment: 4,
                    },
                    Mutability::Mutable,
                    access,
                    AddressSpace::Global,
                    identity,
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
            Dimensions::new(65_535, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap()
    }

    fn host_identity(
        profile: &str,
        binding: [u8; 32],
        logical: &str,
        export: &str,
        abi: &AbiLayout,
        launch: &LaunchContract,
    ) -> DigestBytes {
        derive_generated_host_contract_identity_v1(profile, binding, logical, export, abi, launch)
    }

    #[test]
    fn host_contract_identity_is_domain_separated_from_final_kernel_identity() {
        let abi = abi(Access::WriteOnly, type_identity(0x10));
        let launch = launch(256);
        let host = host_identity(PROFILE, BINDING, "logical", "export.kd", &abi, &launch);
        let final_identity = derive_generated_kernel_identity_v2(
            PROFILE,
            BINDING,
            "logical",
            "export.kd",
            digest(0x20),
            digest(0x30),
            &abi,
            &launch,
        );

        assert_ne!(
            GENERATED_HOST_CONTRACT_IDENTITY_DOMAIN_V1,
            GENERATED_KERNEL_IDENTITY_DOMAIN_V2
        );
        assert_eq!(
            hex(host),
            "7b7be7d539eb847fe5c6f20bf8be2cf7730bfc2d3f3e01373db4e0e870a36669"
        );
        assert_ne!(host, final_identity);
    }

    #[test]
    fn framed_names_and_profile_components_cannot_alias() {
        let abi = abi(Access::WriteOnly, type_identity(0x10));
        let launch = launch(256);
        let first = host_identity("ab", BINDING, "c", "export.kd", &abi, &launch);
        let shifted = host_identity("a", BINDING, "bc", "export.kd", &abi, &launch);
        let export_shifted = host_identity("ab", BINDING, "c", "export.kd.x", &abi, &launch);

        assert_ne!(first, shifted);
        assert_ne!(first, export_shifted);
    }

    #[test]
    fn every_host_contract_component_changes_the_identity() {
        let canonical_abi = abi(Access::WriteOnly, type_identity(0x10));
        let canonical_launch = launch(256);
        let baseline = host_identity(
            PROFILE,
            BINDING,
            "logical",
            "export.kd",
            &canonical_abi,
            &canonical_launch,
        );
        let changed_binding = [0x43; 32];
        let changed_effect = abi(Access::ReadWrite, type_identity(0x10));
        let changed_type_layout = abi(Access::WriteOnly, type_identity(0x11));
        let changed_launch = launch(128);

        for changed in [
            host_identity(
                "test.scalar-slice.v2",
                BINDING,
                "logical",
                "export.kd",
                &canonical_abi,
                &canonical_launch,
            ),
            host_identity(
                PROFILE,
                changed_binding,
                "logical",
                "export.kd",
                &canonical_abi,
                &canonical_launch,
            ),
            host_identity(
                PROFILE,
                BINDING,
                "other",
                "export.kd",
                &canonical_abi,
                &canonical_launch,
            ),
            host_identity(
                PROFILE,
                BINDING,
                "logical",
                "other.kd",
                &canonical_abi,
                &canonical_launch,
            ),
            host_identity(
                PROFILE,
                BINDING,
                "logical",
                "export.kd",
                &changed_effect,
                &canonical_launch,
            ),
            host_identity(
                PROFILE,
                BINDING,
                "logical",
                "export.kd",
                &changed_type_layout,
                &canonical_launch,
            ),
            host_identity(
                PROFILE,
                BINDING,
                "logical",
                "export.kd",
                &canonical_abi,
                &changed_launch,
            ),
        ] {
            assert_ne!(baseline, changed);
        }
    }

    #[test]
    fn host_contract_identity_is_independent_of_later_executable_inputs() {
        let abi = abi(Access::WriteOnly, type_identity(0x10));
        let launch = launch(256);
        let host = host_identity(PROFILE, BINDING, "logical", "export.kd", &abi, &launch);
        let first_final = derive_generated_kernel_identity_v2(
            PROFILE,
            BINDING,
            "logical",
            "export.kd",
            digest(0x20),
            digest(0x30),
            &abi,
            &launch,
        );
        let changed_final = derive_generated_kernel_identity_v2(
            PROFILE,
            BINDING,
            "logical",
            "export.kd",
            digest(0x21),
            digest(0x31),
            &abi,
            &launch,
        );

        assert_eq!(
            host,
            host_identity(PROFILE, BINDING, "logical", "export.kd", &abi, &launch)
        );
        assert_ne!(first_final, changed_final);
        assert_ne!(host, first_final);
        assert_ne!(host, changed_final);
    }
}
