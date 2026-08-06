#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod decode;
mod encode;
mod error;
mod model;

pub use decode::decode_frontend_unit_v1;
pub use encode::{FRONTEND_UNIT_MAGIC_V1, FRONTEND_UNIT_VERSION_V1, encode_frontend_unit_v1};
pub use error::{DecodeError, ValidationError};
pub use model::{
    BasicBlockV1, BlockIdV1, FrontendUnitV1, FunctionIdentityV1, FunctionRoleV1,
    MAX_BLOCKS_PER_FUNCTION_V1, MAX_FUNCTION_NAME_BYTES_V1, MAX_FUNCTIONS_V1,
    MAX_PARAMETERS_PER_FUNCTION_V1, MAX_SUCCESSORS_PER_BLOCK_V1, MAX_TOTAL_BLOCKS_V1,
    MAX_UNIT_BYTES_V1, MonomorphizedFunctionV1, SourceFileIdentityV1, SourceLocationV1,
    StableTypeIdentityV1, TypedSignatureV1,
};
