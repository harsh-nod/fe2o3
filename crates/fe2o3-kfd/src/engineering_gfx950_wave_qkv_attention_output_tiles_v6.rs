//! Closed resident prefix-tile geometry; no generic launch or reuse authority.

use super::*;

pub(super) const SYMBOL: &str =
    "ferric_qwen3_claimed_rmsnorm_qkv_attention_output_tiles_bf16_f32_v6";
pub(super) const EXTENTS: [usize; 15] = [
    8192, 8192, 25_165_824, 512, 512, 580, 16_777_216, 8192, 6144, 4096, 2_359_296, 2_359_296,
    4096, 16_384, 1136,
];
pub(super) const WORKGROUP: [u16; 3] = [64, 1, 1];
pub(super) const GRID: [u32; 3] = [4096, 1, 1];
pub(super) const KERNARG_BYTES: usize = 120 + 256;
pub(super) const INITIAL_STATE: [u32; 284] = {
    let mut words = [0; 284];
    words[0] = 1;
    words[2] = 1;
    words
};

pub(super) fn role_access(index: usize) -> BufferAccessV1 {
    if index < 7 {
        BufferAccessV1::Read
    } else {
        BufferAccessV1::ReadWrite
    }
}

pub(super) fn role_alignment(index: usize) -> u32 {
    if matches!(index, 4 | 5 | 13 | 14) {
        4
    } else {
        2
    }
}

pub(super) fn validate_metadata(
    metadata: &KernelMetadataV1,
    digest: [u8; 32],
    symbol: &str,
) -> Result<()> {
    if symbol != SYMBOL
        || metadata.object_sha256 != digest
        || metadata.symbol != SYMBOL
        || metadata.kernarg_bytes as usize != KERNARG_BYTES
        || metadata.kernarg_alignment != 8
        || metadata.wavefront_size != 64
        || metadata.private_segment_bytes != 0
        || metadata.group_segment_bytes != 512
        || metadata.implicit_argument_offset != Some(120)
        || metadata.implicit_argument_bytes != 256
        || metadata.explicit_arguments.len() != 15
    {
        return Err("prefix tiles V6 physical ABI/resources".into());
    }
    for (index, argument) in metadata.explicit_arguments.iter().enumerate() {
        if argument.offset != index as u32 * 8
            || argument.bytes != 8
            || !argument.global_buffer
            || argument
                .pointee_alignment
                .is_some_and(|value| value != role_alignment(index))
            || argument
                .access
                .is_some_and(|access| access != role_access(index))
        {
            return Err("prefix tiles V6 pointer role/offset/alignment".into());
        }
    }
    Ok(())
}

// All addresses come from this Group's retained records, never caller addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OwnedRegion {
    pub(super) buffer: u64,
    pub(super) base: u64,
    pub(super) requested: usize,
    pub(super) backing: usize,
}

pub(super) fn fixups(regions: &[OwnedRegion; 15]) -> [PointerFixupV1; 15] {
    core::array::from_fn(|index| PointerFixupV1 {
        kernarg_offset: index as u32 * 8,
        buffer: regions[index].buffer,
        buffer_offset: 0,
        extent_bytes: EXTENTS[index] as u64,
        access: role_access(index),
    })
}

pub(super) fn validate_regions(
    regions: &[OwnedRegion; 15],
    pointers: &[PointerFixupV1; 15],
) -> Result<()> {
    let expected = fixups(regions);
    for (index, region) in regions.iter().enumerate() {
        if region.buffer == 0
            || region.base == 0
            || !region.base.is_multiple_of(PAGE_BYTES as u64)
            || region.requested != EXTENTS[index]
            || region.backing < region.requested
            || !region.backing.is_multiple_of(PAGE_BYTES)
            || region.base.checked_add(region.backing as u64).is_none()
            || pointers[index] != expected[index]
        {
            return Err("prefix tiles V6 owned extent/fixup".into());
        }
        for prior in &regions[..index] {
            if prior.buffer == region.buffer
                || (prior.base < region.base + region.backing as u64
                    && region.base < prior.base + prior.backing as u64)
            {
                return Err("prefix tiles V6 allocation alias".into());
            }
        }
    }
    Ok(())
}

pub(super) fn validate_final_state(state: [u32; 284]) -> Result<()> {
    const COUNTS: [u32; 5] = [1, 48, 1, 16, 64];
    const MASKS: [u32; 5] = [u32::MAX, u32::MAX, u32::MAX, u32::MAX, 3];
    if state[..4] != [1, 0, 0, 31]
        || state[4..9] != COUNTS
        || state[9..14] != COUNTS
        || state[14..19] != MASKS
        || state[19..24] != MASKS
        || state[24..154].iter().any(|owner| !(1..=64).contains(owner))
        || state[154..].iter().any(|arrivals| *arrivals != 64)
    {
        return Err("prefix tiles V6 terminal state".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "engineering_gfx950_wave_qkv_attention_output_tiles_v6_tests.rs"]
mod tests;
