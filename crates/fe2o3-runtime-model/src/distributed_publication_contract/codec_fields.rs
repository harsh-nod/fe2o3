//! Sequential fields built from the private fixed-storage byte primitives.

use super::codec_primitives::{fixed, put, take, u16_from_le, u16_le, u64_from_le, u64_le};
use super::{DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1, DistributedPublicationContractErrorV1};

macro_rules! distributed_codec_fields_expr_v1 {
    ($($tokens:tt)*) => { $($tokens)* };
}
include!("codec_fields_body.rs");

pub(super) fn bytes_equal(left: &[u8], right: &[u8]) -> bool {
    distributed_codec_bytes_equal_body_v1!(distributed_codec_fields_expr_v1, left, right, index, [])
}

pub(super) fn read_header(
    bytes: &[u8],
    offset: &mut usize,
    domain: &[u8],
) -> Result<(), DistributedPublicationContractErrorV1> {
    distributed_codec_read_header_body_v1!(distributed_codec_fields_expr_v1, bytes, offset, domain)
}

pub(super) fn write_header(bytes: &mut [u8], offset: &mut usize, domain: &[u8]) {
    distributed_codec_write_header_body_v1!(distributed_codec_fields_expr_v1, bytes, offset, domain)
}

pub(super) fn read_u64(
    bytes: &[u8],
    offset: &mut usize,
) -> Result<u64, DistributedPublicationContractErrorV1> {
    distributed_codec_read_u64_body_v1!(distributed_codec_fields_expr_v1, bytes, offset)
}

pub(super) fn write_u64(bytes: &mut [u8], offset: &mut usize, value: u64) {
    distributed_codec_write_u64_body_v1!(distributed_codec_fields_expr_v1, bytes, offset, value)
}
