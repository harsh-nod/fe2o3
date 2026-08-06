use sha2::{Digest, Sha256};

use crate::{
    AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    DigestBytes, LaunchContract, Mutability, PointerWidth, ScalarType,
};

/// Domain for compiler-generated kernel contract identities.
pub const GENERATED_KERNEL_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/GENERATED-KERNEL-IDENTITY/V2\0";

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
    let mut bytes = Vec::new();
    frame(&mut bytes, GENERATED_KERNEL_IDENTITY_DOMAIN_V2);
    frame(&mut bytes, profile_tag.as_bytes());
    frame(&mut bytes, &kernel_binding);
    frame(&mut bytes, logical_name.as_bytes());
    frame(&mut bytes, export_name.as_bytes());
    frame(&mut bytes, source_digest.as_bytes());
    frame(&mut bytes, executable_digest.as_bytes());
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
