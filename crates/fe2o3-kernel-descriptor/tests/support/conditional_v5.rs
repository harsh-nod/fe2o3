//! Caller-authored inert V5 projections, never proof/source evidence.
#![allow(dead_code)]
use fe2o3_kernel_descriptor::*;
use sha2::{Digest, Sha256};
#[path = "conditional_v4.rs"]
pub mod v4;
pub use v4::{Fixture, mixed_read_alignments};

pub fn free(_: usize) -> Result<(), ()> {
    Ok(())
}

fn rows<T>(mut cursor: ConditionalInvocationCursorV1<'_, T>) -> Vec<T> {
    let mut result = Vec::new();
    while let Some(row) = cursor.next(&mut free).unwrap() {
        result.push(row);
    }
    result
}

fn contract_wire(view: &ConditionalInvocationContractV1<'_>, cpu: [u8; 32]) -> Vec<u8> {
    let old = view.theorem();
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V2/LE/SHARED-IEEE/CPU-SOURCE\0");
    for field in [
        view.subjects().aggregate_statement_identity,
        old.generated_source_identity,
        old.staging_receipt_identity,
        old.staging_obligation_identity,
        old.staging_signer_identity,
        old.staging_execution_identity,
        cpu,
    ] {
        hash.update(field);
    }
    let roots = rows(view.typed_roots());
    let arguments = rows(view.arguments());
    let reads = rows(view.reads());
    let premises = rows(view.premises());
    let input = ConditionalInvocationContractInputV2 {
        numerical_domain: view.numerical_domain(),
        subjects: *view.subjects(),
        theorem: ConditionalTheoremV2 {
            statement_identity: hash.finalize().into(),
            generated_source_identity: old.generated_source_identity,
            execution_identity: old.execution_identity,
            receipt_identity: old.receipt_identity,
            staging_receipt_identity: old.staging_receipt_identity,
            staging_obligation_identity: old.staging_obligation_identity,
            staging_signer_identity: old.staging_signer_identity,
            staging_execution_identity: old.staging_execution_identity,
            cpu_input_commitment: cpu,
        },
        typed_roots: &roots,
        arguments: &arguments,
        output: view.output(),
        reads: &reads,
        premises: &premises,
    };
    let mut bytes =
        vec![0; encoded_conditional_invocation_contract_v2_len(&input, &mut free).unwrap()];
    encode_conditional_invocation_contract_v2(&input, &mut bytes, &mut free).unwrap();
    bytes
}

pub fn with_input<R>(
    target: &str,
    entries: usize,
    inputs: usize,
    consume: impl FnOnce(DeviceDescriptorTableInputV5<'_>) -> R,
) -> R {
    with_custom_contracts(target, entries, inputs, |_| {}, consume)
}
pub fn with_custom_contracts<R>(
    target: &str,
    entries: usize,
    inputs: usize,
    customize: impl FnMut(&mut Fixture),
    consume: impl FnOnce(DeviceDescriptorTableInputV5<'_>) -> R,
) -> R {
    v4::with_custom_contracts(target, entries, inputs, customize, |old| {
        let bytes = old
            .contracts
            .iter()
            .map(|c| contract_wire(c, [17; 32]))
            .collect::<Vec<_>>();
        let contracts = bytes
            .iter()
            .map(|b| decode_conditional_invocation_contract_v2(b, &mut free).unwrap())
            .collect::<Vec<_>>();
        consume(DeviceDescriptorTableInputV5 {
            nominal: old.nominal,
            contracts: &contracts,
        })
    })
}
pub fn wire(target: &str, entries: usize, inputs: usize) -> Vec<u8> {
    wire_custom(target, entries, inputs, |_| {})
}
pub fn wire_custom(
    target: &str,
    entries: usize,
    inputs: usize,
    customize: impl FnMut(&mut Fixture),
) -> Vec<u8> {
    with_custom_contracts(target, entries, inputs, customize, |input| {
        let mut bytes = vec![0; encoded_device_descriptor_table_v5_len(&input, &mut free).unwrap()];
        encode_device_descriptor_table_v5(&input, &mut bytes, &mut free).unwrap();
        bytes
    })
}

pub fn contract_start(bytes: &[u8]) -> usize {
    bytes
        .windows(8)
        .position(|w| w == CONDITIONAL_INVOCATION_MAGIC_V2)
        .unwrap()
}
pub fn repair_identity(bytes: &mut [u8]) {
    let start = contract_start(bytes);
    let n = u32::from_le_bytes(bytes[start - 36..start - 32].try_into().unwrap()) as usize;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/CONDITIONAL-INVOCATION/V2\0");
    hash.update((n as u64).to_le_bytes());
    hash.update(&bytes[start..start + n]);
    bytes[start - 32..start].copy_from_slice(&hash.finalize());
}
