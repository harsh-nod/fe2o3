//! Complete fixed-size operation descriptions composed from the private fields.

use super::codec_fields::{read_header, read_u64, write_header, write_u64};
use super::codec_primitives::{finish, fixed, put};
use super::{
    DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1, DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1,
    DistributedExecutionPlanIdV1, DistributedMembershipIdV1, DistributedOperationIdV1,
    DistributedParticipantIdV1, DistributedPlacementPlanIdV1,
    DistributedPublicationContractErrorV1, DistributedRunIdV1, DistributedRuntimeInstanceIdV1,
    DistributedTargetDescriptionIdV1, IdentityDigestV1, ModelDistributedOperationBindingV1,
    RuntimeArtifactIdV1, RuntimeModelIdV1, UntrustedDistributedOperationCoordinatesV1,
};

macro_rules! distributed_codec_operation_expr_v1 {
    ($($tokens:tt)*) => { $($tokens)* };
}
include!("codec_operation_body.rs");

pub(super) fn write_digest(bytes: &mut [u8], offset: &mut usize, value: IdentityDigestV1) {
    distributed_codec_write_digest_body_v1!(
        distributed_codec_operation_expr_v1,
        bytes,
        offset,
        value
    )
}

pub(super) fn read_digest(
    bytes: &[u8],
    offset: &mut usize,
) -> Result<IdentityDigestV1, DistributedPublicationContractErrorV1> {
    distributed_codec_read_digest_body_v1!(distributed_codec_operation_expr_v1, bytes, offset)
}

pub(super) fn encode(
    binding: ModelDistributedOperationBindingV1,
) -> [u8; DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1] {
    distributed_codec_encode_operation_body_v1!(
        distributed_codec_operation_expr_v1,
        debug_assert_eq,
        binding
    )
}

pub(super) fn decode(
    bytes: &[u8],
) -> Result<ModelDistributedOperationBindingV1, DistributedPublicationContractErrorV1> {
    distributed_codec_decode_operation_body_v1!(distributed_codec_operation_expr_v1, bytes)
}
