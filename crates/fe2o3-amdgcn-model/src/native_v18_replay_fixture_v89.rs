//! Shared content-only fixtures. No test label is a trusted proof receipt.
use fe2o3_kernel_descriptor::{
    encode_mixed_descriptor_v89 as encode_descriptor,
    encoded_mixed_descriptor_v89_len as descriptor_len,
    mixed_conditional_v86::{
        MixedContractInputV86 as ContractInput, decode_mixed_contract_v86 as decode_contract,
        encode_mixed_contract_v86 as encode_contract,
        encoded_mixed_contract_v86_len as contract_len,
    },
};
const SECTION: &str =
    "\nmodule asm \".section .fe2o3.kd.v89,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";

include!("native_v18_replay_fixture_family.rs");
