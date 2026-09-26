//! Assertions over events emitted by the actual source transaction and consumer.
use super::{Event, Observation, Outcome};

#[test]
fn v2_passive_cpu_crosscheck_does_not_change_frozen_receipt_json() {
    let receipt = super::Receipt {
        statement: [1; 32],
        generated_source: [2; 32],
        execution: [3; 32],
        receipt: [4; 32],
        reference_identity: [5; 32],
        reference_mir: [6; 32],
        kernel_identity: [7; 32],
        kernel_mir: [8; 32],
        cpu_input_commitment: [9; 32],
    };
    let value = serde_json::to_value(&receipt).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "statement": receipt.statement, "generated_source": receipt.generated_source,
            "execution": receipt.execution, "receipt": receipt.receipt,
            "reference_identity": receipt.reference_identity, "reference_mir": receipt.reference_mir,
            "kernel_identity": receipt.kernel_identity, "kernel_mir": receipt.kernel_mir,
        })
    );
}

pub(crate) fn check(observation: &Observation, formula: &serde_json::Value) {
    let [
        Event::Retained {
            root,
            receipt,
            reserved_receipt_bytes,
            work,
            storage,
        },
        Event::PhaseRetained(initial),
        Event::ReplayCallback {
            root: callback_root,
            receipt: callback_receipt,
            wire,
            verifying_key,
            aggregate,
            source,
            graph,
            epoch: _,
            work: callback_work,
            storage: callback_storage,
        },
        Event::ReplayAccepted {
            root: accepted_root,
            receipt: accepted_receipt,
            reserved_contract_bytes,
            contract,
            work: accepted_work,
            storage: accepted_storage,
        },
        Event::PhaseFinished {
            before,
            after,
            outcome: Outcome::Accepted,
        },
        Event::PhaseDropped {
            before: dropped,
            after: released,
            poisoned: false,
        },
    ] = observation.events.as_slice()
    else {
        panic!(
            "actual conditional retention/replay sequence: {:?}",
            observation.events
        )
    };
    assert_eq!(root, callback_root);
    assert_eq!(root, accepted_root);
    assert_eq!(receipt, callback_receipt);
    assert_eq!(receipt, accepted_receipt);
    for (name, identity) in [
        ("statement", receipt.statement),
        ("generated_source", receipt.generated_source),
        ("execution", receipt.execution),
        ("receipt", receipt.receipt),
    ] {
        assert_ne!(identity, [0; 32]);
        assert_eq!(formula[name], serde_json::json!(identity));
    }
    for identity in [
        &receipt.reference_identity,
        &receipt.reference_mir,
        &receipt.kernel_identity,
        &receipt.kernel_mir,
        &receipt.cpu_input_commitment,
        aggregate,
        source,
        graph,
        verifying_key,
    ] {
        assert_ne!(identity, &[0; 32]);
    }
    assert!(!wire.is_empty());
    assert!(*reserved_receipt_bytes > 0 && storage >= reserved_receipt_bytes);
    assert!(*work <= initial.work && *storage <= initial.storage);
    assert_eq!(initial, before);
    assert!(initial.work < *callback_work && callback_work <= accepted_work);
    assert!(*accepted_work <= after.work);
    assert!(*callback_storage >= initial.storage && *accepted_storage >= initial.storage);
    assert!(*reserved_contract_bytes > 0);
    assert!(
        fe2o3_kernel_descriptor::decode_conditional_invocation_contract_v1(contract, &mut |_| {
            Ok::<_, ()>(())
        })
        .is_err()
    );
    let contract =
        fe2o3_kernel_descriptor::decode_conditional_invocation_contract_v2(contract, &mut |_| {
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(contract.subjects().exact_graph_identity, *graph);
    assert_eq!(contract.subjects().aggregate_statement_identity, *aggregate);
    assert_eq!(contract.subjects().source_semantic_identity, *source);
    assert_eq!(
        contract.subjects().safe_reference_identity,
        receipt.reference_identity
    );
    assert_eq!(
        contract.subjects().safe_reference_mir_hash,
        receipt.reference_mir
    );
    assert_eq!(
        contract.subjects().kernel_subject_identity,
        receipt.kernel_identity
    );
    assert_eq!(contract.subjects().kernel_mir_hash, receipt.kernel_mir);
    assert_eq!(contract.theorem().statement_identity, receipt.statement);
    assert_eq!(
        contract.theorem().generated_source_identity,
        receipt.generated_source
    );
    assert_eq!(contract.theorem().execution_identity, receipt.execution);
    assert_eq!(contract.theorem().receipt_identity, receipt.receipt);
    assert_eq!(
        contract.theorem().cpu_input_commitment,
        receipt.cpu_input_commitment
    );
    assert!(*reserved_contract_bytes >= contract.canonical_bytes().len());
    assert_eq!(initial.storage + reserved_contract_bytes, after.storage);
    assert!(initial.failed_work.is_none() && initial.failed_storage.is_none());
    assert!(after.failed_work.is_none() && after.failed_storage.is_none());
    assert_eq!(after, dropped);
    assert_eq!(released.storage, 0);
    assert_eq!(released.work, after.work);
    assert_eq!(released.peak_storage, after.peak_storage);
    assert_eq!(released.failed_work, after.failed_work);
    assert_eq!(released.failed_storage, after.failed_storage);
}
