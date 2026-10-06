use fe2o3_kernel_descriptor::{
    encode_mixed_descriptor_v89 as encode_descriptor,
    encoded_mixed_descriptor_v89_len as descriptor_len,
    mixed_conditional_v86::{
        MixedContractInputV86 as ContractInput, decode_mixed_contract_v86 as decode_contract,
        encode_mixed_contract_v86 as encode_contract,
        encoded_mixed_contract_v86_len as contract_len,
    },
};
use fe2o3_verifier::with_mixed_target_selection_v89 as with_target;
macro_rules! binding_refusal {
    ($error:expr, $message:literal) => {
        matches!($error, AdmissionError::MixedV89($message))
    };
}

include!("mixed_worker_target_readmission_family_tests.rs");

#[test]
fn predicated_readmission_rejects_legacy_descriptor_before_native_callback() {
    let mut fixture = Fixture::new(ProductionAmdTargetProfileV1::Gfx942, 2);
    assert!(decode_mixed_descriptor_v89(&fixture.descriptor, &mut free).is_ok());
    fixture.descriptor[..8].copy_from_slice(b"FE2O3D53");
    fixture.descriptor[8..10].copy_from_slice(&53u16.to_le_bytes());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let entered = std::cell::Cell::new(false);
    assert!(
        fixture
            .readmit_with(&mut budget, |_, _| {
                entered.set(true);
                Ok(())
            })
            .is_err()
    );
    assert!(!entered.get());
    assert_eq!(budget.storage(), FLOOR);
}
