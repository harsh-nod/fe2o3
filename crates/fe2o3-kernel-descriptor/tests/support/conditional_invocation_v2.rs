//! Independent caller-authored projections, not source or proof receipts.
use fe2o3_kernel_descriptor::*;
use sha2::{Digest, Sha256};

#[path = "conditional_invocation.rs"]
pub mod v1;
pub use v1::{Fixture, free};

pub const FIXED_V1: usize = 624;
pub const FIXED_V2: usize = 656;
pub const THEOREM: usize = 320;
pub const CPU: usize = 576;
pub const V2_DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V2/LE/SHARED-IEEE/CPU-SOURCE\0";

pub fn statement(f: &Fixture, cpu: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(V2_DOMAIN);
    for field in [
        f.subjects.aggregate_statement_identity,
        f.theorem.generated_source_identity,
        f.theorem.staging_receipt_identity,
        f.theorem.staging_obligation_identity,
        f.theorem.staging_signer_identity,
        f.theorem.staging_execution_identity,
        cpu,
    ] {
        hash.update(field);
    }
    hash.finalize().into()
}

pub fn input(f: &Fixture, cpu: [u8; 32]) -> ConditionalInvocationContractInputV2<'_> {
    ConditionalInvocationContractInputV2 {
        numerical_domain: ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1,
        subjects: f.subjects,
        theorem: ConditionalTheoremV2 {
            statement_identity: statement(f, cpu),
            generated_source_identity: f.theorem.generated_source_identity,
            execution_identity: f.theorem.execution_identity,
            receipt_identity: f.theorem.receipt_identity,
            staging_receipt_identity: f.theorem.staging_receipt_identity,
            staging_obligation_identity: f.theorem.staging_obligation_identity,
            staging_signer_identity: f.theorem.staging_signer_identity,
            staging_execution_identity: f.theorem.staging_execution_identity,
            cpu_input_commitment: cpu,
        },
        typed_roots: &f.roots,
        arguments: &f.arguments,
        output: f.output,
        reads: &f.reads,
        premises: &f.premises,
    }
}

pub fn wire(f: &Fixture) -> Vec<u8> {
    let input = input(f, [17; 32]);
    let mut bytes =
        vec![0; encoded_conditional_invocation_contract_v2_len(&input, &mut free).unwrap()];
    encode_conditional_invocation_contract_v2(&input, &mut bytes, &mut free).unwrap();
    bytes
}

pub fn invalid(bytes: &[u8], field: &str) {
    match decode_conditional_invocation_contract_v2(bytes, &mut free) {
        Err(ConditionalInvocationWireErrorV1::Contract(
            ConditionalInvocationContractErrorV1::Invalid(actual),
        )) => assert_eq!(actual, field),
        _ => panic!("expected invalid {field}"),
    }
}

pub fn refused(f: &Fixture) {
    let input = input(f, [17; 32]);
    assert!(encoded_conditional_invocation_contract_v2_len(&input, &mut free).is_err());
    let mut bytes = vec![0xa5; 4096];
    assert!(encode_conditional_invocation_contract_v2(&input, &mut bytes, &mut free).is_err());
    assert!(bytes.iter().all(|b| *b == 0xa5));
}

pub fn set_len(bytes: &mut [u8]) {
    let n = bytes.len() as u32;
    bytes[12..16].copy_from_slice(&n.to_le_bytes());
}
