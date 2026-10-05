//! Exact nominal ABI plus mandatory, independently decoded CFG or explicitly predicated contracts.

use crate::mixed_conditional_v86::{
    MAX_MIXED_CONTRACT_BYTES_V86, MIXED_CONTRACT_CODEC_STORAGE_V86, MixedContractErrorV86,
    MixedContractV86, decode_mixed_contract_v86,
};
use crate::mixed_descriptor_family::mixed_descriptor_family;

mixed_descriptor_family!(
    b"FE2O3D89",
    89,
    "distinct V89 header",
    COMPILER_MIXED_DESCRIPTOR_SECTION_V89,
    ".fe2o3.kd.v89",
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V89,
    MIXED_DESCRIPTOR_READER_STORAGE_V89,
    MixedDescriptorTableV89,
    MixedDescriptorErrorV89,
    MixedContractV86,
    MixedContractErrorV86,
    MAX_MIXED_CONTRACT_BYTES_V86,
    MIXED_CONTRACT_CODEC_STORAGE_V86,
    decode_mixed_contract_v86,
    decode_mixed_descriptor_v89,
    encoded_mixed_descriptor_v89_len,
    encode_mixed_descriptor_v89
);
