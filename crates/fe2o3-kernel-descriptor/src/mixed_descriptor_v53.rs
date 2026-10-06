//! Exact nominal ABI plus mandatory, independently decoded CFG-only contracts.

use crate::mixed_conditional_v26::{
    MAX_MIXED_CONTRACT_BYTES_V26, MIXED_CONTRACT_CODEC_STORAGE_V26, MixedContractErrorV26,
    MixedContractV26, decode_mixed_contract_v26,
};
use crate::mixed_descriptor_family::mixed_descriptor_family;

mixed_descriptor_family!(
    b"FE2O3D53",
    53,
    "distinct V53 header",
    COMPILER_MIXED_DESCRIPTOR_SECTION_V53,
    ".fe2o3.kd.v53",
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V53,
    MIXED_DESCRIPTOR_READER_STORAGE_V53,
    MixedDescriptorTableV53,
    MixedDescriptorErrorV53,
    MixedContractV26,
    MixedContractErrorV26,
    MAX_MIXED_CONTRACT_BYTES_V26,
    MIXED_CONTRACT_CODEC_STORAGE_V26,
    decode_mixed_contract_v26,
    decode_mixed_descriptor_v53,
    encoded_mixed_descriptor_v53_len,
    encode_mixed_descriptor_v53
);
