use fe2o3_amdgcn_model::NativeV18TextDescriptorReplayErrorV60 as NativeError;
use fe2o3_compiler_lineage::{
    MixedMiddleEndInputV50 as MiddleEndInput, MixedMiddleEndLayoutV50 as MiddleEndLayout,
    encode_mixed_middle_end_v50 as encode_middle_end,
};
use fe2o3_verifier::{
    MixedNativeCorrespondenceErrorV60 as CorrespondenceError,
    MixedTargetSelectionSubjectV53 as TargetSubject,
    with_mixed_target_selection_v53 as with_target,
};
#[path = "../../fe2o3-amdgcn-model/src/native_v18_replay_fixture_v60.rs"]
mod native_fixture;
const SECTION_NAME: &str = ".fe2o3.kd.v53";
const RECEIPT_DOMAIN: &[u8] = b"FE2O3/ORIGINAL-MIR/POLICY11/LICM/STORE-CONSENSUS/TYPED/V50\0";
const RECEIPT_MAGIC: &[u8] = b"FE2O3/MIXED/TYPED-SOURCE-TAIL/V50\0";
const RECEIPT_VERSION: u16 = 50;
const FOREIGN_RECEIPT_DOMAIN: &[u8] =
    b"FE2O3/ORIGINAL-MIR/POLICY11/LICM/STORE-CONSENSUS/PREDICATED-TYPED/V90\0";
const FOREIGN_RECEIPT_MAGIC: &[u8] = b"FE2O3/MIXED/PREDICATED-TYPED-SOURCE-TAIL/V90\0";
const FOREIGN_RECEIPT_VERSION: u16 = 90;
macro_rules! receipt_refusal {
    ($error:expr) => {
        matches!($error, AdmissionError::MixedReceiptV53(_))
    };
}
macro_rules! binding_refusal {
    ($error:expr, $message:literal) => {
        matches!($error, AdmissionError::MixedV53($message))
    };
}

include!("mixed_worker_native_lineage_family_tests.rs");
