use super::{
    SemanticBackendPrimitiveV1, SemanticMirErrorV1, SemanticTargetArchitectureV1,
    SemanticTargetDataLayoutV1,
};

#[derive(Clone, Copy)]
pub(super) struct TargetPointerProfileV1 {
    pub(super) size_bytes: u64,
    alignment_bytes: u64,
    offset_bits: u16,
}

pub(super) fn validate_target_primitive(
    target: SemanticTargetDataLayoutV1,
    primitive: SemanticBackendPrimitiveV1,
) -> Result<(), SemanticMirErrorV1> {
    let expected = match target.architecture() {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => match primitive {
            SemanticBackendPrimitiveV1::Integer { bits, .. } => match bits {
                8 => Some((1, 1)),
                16 => Some((2, 2)),
                32 => Some((4, 4)),
                64 | 128 => Some(((bits / 8) as u64, 8)),
                _ => None,
            },
            SemanticBackendPrimitiveV1::Float { bits, .. } => match bits {
                16 => Some((2, 2)),
                32 => Some((4, 4)),
                64 => Some((8, 8)),
                128 => Some((16, 16)),
                _ => None,
            },
            SemanticBackendPrimitiveV1::Pointer { address_space, .. } => {
                gfx942_pointer_profile(address_space)
                    .map(|profile| (profile.size_bytes, profile.alignment_bytes))
            }
        },
    };
    if expected
        != primitive
            .size_bytes()
            .map(|size| (size, primitive.alignment_bytes()))
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok(())
}

pub(super) const fn target_pointer_profile(
    target: SemanticTargetDataLayoutV1,
    address_space: u32,
) -> Option<TargetPointerProfileV1> {
    match target.architecture() {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => gfx942_pointer_profile(address_space),
    }
}

pub(super) const fn target_object_size_bound_in(
    target: SemanticTargetDataLayoutV1,
    address_space: u32,
) -> Option<u64> {
    if matches!(address_space, 7..=9) {
        return None;
    }
    let profile = match target_pointer_profile(target, address_space) {
        Some(profile) => profile,
        None => return None,
    };
    match profile.offset_bits {
        16 => Some(1 << 15),
        32 => Some(1 << 31),
        64 => Some(1 << 61),
        // Rust source objects cannot inhabit descriptor address spaces whose
        // pointer-offset width is not one of rustc's object-size domains.
        _ => None,
    }
}

pub(super) fn target_vector_alignment(
    target: SemanticTargetDataLayoutV1,
    vector_size_bytes: u64,
) -> Result<u64, SemanticMirErrorV1> {
    match target.architecture() {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => gfx942_vector_alignment(vector_size_bytes),
    }
}

const fn gfx942_pointer_profile(address_space: u32) -> Option<TargetPointerProfileV1> {
    let (size_bytes, alignment_bytes, offset_bits) = match address_space {
        0 | 1 | 4 => (8, 8, 64),
        2 | 3 | 5 | 6 => (4, 4, 32),
        7 => (20, 32, 32),
        8 => (16, 16, 48),
        9 => (24, 32, 32),
        _ => return None,
    };
    Some(TargetPointerProfileV1 {
        size_bytes,
        alignment_bytes,
        offset_bits,
    })
}

fn gfx942_vector_alignment(vector_size_bytes: u64) -> Result<u64, SemanticMirErrorV1> {
    let alignment = match vector_size_bytes {
        2 => 2,
        3 | 4 => 4,
        6 | 8 => 8,
        12 | 16 => 16,
        24 | 32 => 32,
        64 => 64,
        128 => 128,
        256 => 256,
        size => size
            .checked_next_power_of_two()
            .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?,
    };
    Ok(alignment)
}
