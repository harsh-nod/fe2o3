//! Private fixed-storage byte operations for the descriptive wire codec.

use super::DistributedPublicationContractErrorV1;

macro_rules! distributed_codec_expr_v1 {
    ($($tokens:tt)*) => { $($tokens)* };
}
include!("codec_primitives_body.rs");

pub(super) fn take<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    count: usize,
) -> Result<&'a [u8], DistributedPublicationContractErrorV1> {
    distributed_codec_take_body_v1!(distributed_codec_expr_v1, bytes, offset, count)
}

pub(super) fn fixed<const N: usize>(
    bytes: &[u8],
    offset: &mut usize,
) -> Result<[u8; N], DistributedPublicationContractErrorV1> {
    distributed_codec_fixed_body_v1!(distributed_codec_expr_v1, bytes, offset, N)
}

// Callers must establish offset <= bytes.len() and
// value.len() <= bytes.len() - offset before writing. These are internal
// encoder obligations, not additional acceptance conditions for decoders.
pub(super) fn put(bytes: &mut [u8], offset: &mut usize, value: &[u8]) {
    distributed_codec_put_body_v1!(distributed_codec_expr_v1, bytes, offset, value)
}

pub(super) fn finish(
    bytes: &[u8],
    offset: usize,
) -> Result<(), DistributedPublicationContractErrorV1> {
    distributed_codec_finish_body_v1!(distributed_codec_expr_v1, bytes, offset)
}

pub(super) fn u16_le(value: u16) -> [u8; 2] {
    distributed_codec_u16_le_body_v1!(distributed_codec_expr_v1, value)
}

pub(super) fn u16_from_le(bytes: [u8; 2]) -> u16 {
    distributed_codec_u16_from_le_body_v1!(distributed_codec_expr_v1, bytes)
}

pub(super) fn u64_le(value: u64) -> [u8; 8] {
    distributed_codec_u64_le_body_v1!(distributed_codec_expr_v1, value)
}

pub(super) fn u64_from_le(bytes: [u8; 8]) -> u64 {
    distributed_codec_u64_from_le_body_v1!(distributed_codec_expr_v1, bytes)
}
