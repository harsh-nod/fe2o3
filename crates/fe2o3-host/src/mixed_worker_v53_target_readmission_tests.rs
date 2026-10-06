use fe2o3_kernel_descriptor::{
    encode_mixed_descriptor_v53 as encode_descriptor,
    encoded_mixed_descriptor_v53_len as descriptor_len,
    mixed_conditional_v26::{
        MixedContractInputV26 as ContractInput, decode_mixed_contract_v26 as decode_contract,
        encode_mixed_contract_v26 as encode_contract,
        encoded_mixed_contract_v26_len as contract_len,
    },
};
use fe2o3_verifier::with_mixed_target_selection_v53 as with_target;
macro_rules! binding_refusal {
    ($error:expr, $message:literal) => {
        matches!($error, AdmissionError::MixedV53($message))
    };
}

include!("mixed_worker_target_readmission_family_tests.rs");
