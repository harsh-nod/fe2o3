use super::*;
use crate::{
    CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1, CanonicalPolicy3ExecutionReceiptErrorV1,
    InertCanonicalPolicy4ExecutionReceiptV1,
    checked_load_forwarding_v1::tests::{STORAGE, WORK, evaluate, fixture},
    encode_checked_canonical_policy4_execution_receipt_v1,
    optimize_checked_canonical_kernel_ir_policy5_v1,
};
use fe2o3_kernel_ir::{BinaryOp, CanonicalKernelIrWorkBudgetV1 as Work, Module, OperationKind};
use std::mem::size_of_val;

const PREFIX: usize = 31;

struct Prepared {
    input: Owner,
    input_storage: usize,
    checked: CheckedOwner,
    wire: InertCanonicalPolicy4ExecutionReceiptV1,
}
impl Prepared {
    fn inputs(&self) -> CanonicalPolicy5SemanticInputsV1<'_> {
        let prefix = self.checked.intermediate_policy4();
        CanonicalPolicy5SemanticInputsV1 {
            input: &self.input,
            intermediate: prefix.intermediate_policy3().owner(),
            stored: prefix.owner(),
            output: self.checked.owner(),
            policy4_wire: self.wire.canonical_bytes(),
            policy5_record: self.checked.execution().canonical_bytes(),
            load_rows: self.checked.load_forwarding_rows(),
        }
    }
    fn floor(&self) -> usize {
        PREFIX
            + self.input_storage
            + self.checked.retained_storage()
            + self.wire.storage().retained_storage()
    }
}

fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, storage.retained_storage())
}

// Explicit transfer from a preparation ledger to the receipt-checking ledger.
fn prepared(module: &Module) -> Prepared {
    let (input, input_storage) = admit(module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input_storage).unwrap();
    let checked = optimize_checked_canonical_kernel_ir_policy5_v1(&input, &mut budget).unwrap();
    budget.reserve_storage(checked.retained_storage()).unwrap();
    let wire = encode_checked_canonical_policy4_execution_receipt_v1(
        &input,
        checked.intermediate_policy4(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), input_storage + checked.retained_storage());
    Prepared {
        input,
        input_storage,
        checked,
        wire,
    }
}

fn record(inputs: CanonicalPolicy5SemanticInputsV1<'_>, rows: usize) -> [u8; 200] {
    let store_count = u64::from_le_bytes(inputs.policy4_wire[32..40].try_into().unwrap());
    policy5_record_v1(
        inputs.input,
        inputs.intermediate,
        inputs.stored,
        inputs.output,
        usize::try_from(store_count).unwrap(),
        rows,
    )
    .unwrap()
}

fn discard_semantic(receipt: ReplayedPolicy5SemanticRelationV1<'_>, budget: &mut Budget<'_>) {
    let size = receipt.storage().retained_storage();
    budget.reserve_storage(size).unwrap();
    drop(receipt);
    budget.release_storage(size).unwrap();
}

#[test]
fn mutation_replays_actual_four_owners_and_only_sealed_overload_authenticates() {
    let prepared = prepared(&fixture());
    let inputs = prepared.inputs();
    assert_eq!(inputs.load_rows.len(), 2);
    assert_ne!(
        inputs.stored.canonical().canonical_bytes(),
        inputs.output.canonical().canonical_bytes()
    );
    for value in [0, 1, 17, 0x8000_0000, u32::MAX] {
        assert_eq!(evaluate(inputs.stored, value), (value, value, 2, 3));
        assert_eq!(evaluate(inputs.output, value), (value, value, 2, 1));
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(prepared.floor()).unwrap();
    budget.charge_work(7).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let replay = check_published_policy5_semantic_relation_v1(inputs, &mut budget).unwrap();
    assert!(std::ptr::eq(
        replay.policy4_relation().input(),
        inputs.input
    ));
    assert!(std::ptr::eq(
        replay.policy4_relation().intermediate(),
        inputs.intermediate
    ));
    assert!(std::ptr::eq(
        replay.policy4_relation().output(),
        inputs.stored
    ));
    assert!(std::ptr::eq(replay.output(), inputs.output));
    assert!(std::ptr::eq(
        replay.load_forwarding_rows(),
        inputs.load_rows
    ));
    assert!(std::ptr::eq(
        replay.unauthenticated_execution_record(),
        inputs.policy5_record
    ));
    assert!(!replay.authenticates_execution());
    assert!(!replay.grants_authority());
    discard_semantic(replay, &mut budget);
    let before = budget.work();
    let authenticated = check_canonical_policy5_execution_relation_v1(
        inputs.input,
        &prepared.checked,
        inputs.policy4_wire,
        inputs.policy5_record,
        inputs.load_rows,
        &mut budget,
    )
    .unwrap();
    assert!(std::ptr::eq(
        authenticated.execution_owner(),
        &prepared.checked
    ));
    assert!(std::ptr::eq(
        authenticated.policy4_execution().execution_owner(),
        prepared.checked.intermediate_policy4()
    ));
    assert!(std::ptr::eq(authenticated.output(), inputs.output));
    assert!(authenticated.authenticates_execution());
    assert!(!authenticated.grants_authority());
    let size = authenticated.storage().retained_storage();
    budget.reserve_storage(size).unwrap();
    drop(authenticated);
    budget.release_storage(size).unwrap();
    assert_eq!(budget.storage(), prepared.floor());
    assert!(budget.work() > before && before > 7);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn unchanged_record_builder_matches_literal_layout_and_existing_seal() {
    for module in [fixture(), Module::new("no-op")] {
        let prepared = prepared(&module);
        let inputs = prepared.inputs();
        let bytes = record(inputs, inputs.load_rows.len());
        assert_eq!(&bytes, prepared.checked.execution().canonical_bytes());
        assert_eq!(&bytes[..8], b"F2P5EX1\0");
        assert_eq!(bytes[8..16], [1, 0, 5, 0, 4, 0, 1, 0]);
        assert_eq!(&bytes[16..24], &1u64.to_le_bytes());
        assert_eq!(&bytes[24..32], &inputs.policy4_wire[32..40]);
        assert_eq!(
            &bytes[32..40],
            &(inputs.load_rows.len() as u64).to_le_bytes()
        );
        for (offset, owner) in [
            (40, inputs.input),
            (80, inputs.intermediate),
            (120, inputs.stored),
            (160, inputs.output),
        ] {
            assert_eq!(
                &bytes[offset..offset + 32],
                owner.canonical().identity().digest()
            );
            assert_eq!(
                &bytes[offset + 32..offset + 40],
                &owner
                    .canonical()
                    .identity()
                    .canonical_length()
                    .to_le_bytes()
            );
        }
    }
}

#[test]
fn equal_byte_independent_owners_are_semantic_inputs_not_execution_witnesses() {
    let prepared = prepared(&Module::new("no-op"));
    let inputs = prepared.inputs();
    let (other, storage) = admit(inputs.output.module());
    assert!(!std::ptr::eq(&other, inputs.output));
    assert_eq!(
        other.canonical().canonical_bytes(),
        inputs.output.canonical().canonical_bytes()
    );
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = prepared.floor() + storage;
    budget.reserve_storage(floor).unwrap();
    let replay = check_published_policy5_semantic_relation_v1(
        CanonicalPolicy5SemanticInputsV1 {
            input: &other,
            intermediate: &other,
            stored: &other,
            output: &other,
            ..inputs
        },
        &mut budget,
    )
    .unwrap();
    assert!(std::ptr::eq(replay.policy4_relation().input(), &other));
    assert!(std::ptr::eq(
        replay.policy4_relation().intermediate(),
        &other
    ));
    assert!(std::ptr::eq(replay.policy4_relation().output(), &other));
    assert!(std::ptr::eq(replay.output(), &other));
    assert!(!replay.authenticates_execution());
    discard_semantic(replay, &mut budget);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn admissible_published_nested_work_claim_is_not_sealed_execution() {
    let prepared = prepared(&Module::new("no-op"));
    let inputs = prepared.inputs();
    let mut wire = inputs.policy4_wire.to_vec();
    wire[CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1 + 32 + 96] ^= 1;
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = prepared.floor() + wire.capacity();
    budget.reserve_storage(floor).unwrap();
    let replay = check_published_policy5_semantic_relation_v1(
        CanonicalPolicy5SemanticInputsV1 {
            policy4_wire: &wire,
            ..inputs
        },
        &mut budget,
    )
    .unwrap();
    discard_semantic(replay, &mut budget);
    assert!(matches!(
        check_canonical_policy5_execution_relation_v1(
            inputs.input,
            &prepared.checked,
            &wire,
            inputs.policy5_record,
            inputs.load_rows,
            &mut budget
        ),
        Err(Error::Policy4(
            CanonicalPolicy4ExecutionReceiptErrorV1::Policy3(
                CanonicalPolicy3ExecutionReceiptErrorV1::ExecutionWitness
            )
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn exact_record_length_and_every_fixed_record_byte_are_checked() {
    let prepared = prepared(&Module::new("no-op"));
    let inputs = prepared.inputs();
    for length in [0, 199, 201] {
        let mut record = inputs.policy5_record.to_vec();
        record.resize(length, 0);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = prepared.floor() + record.capacity();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            check_published_policy5_semantic_relation_v1(
                CanonicalPolicy5SemanticInputsV1 {
                    policy5_record: &record,
                    ..inputs
                },
                &mut budget
            ),
            Err(Error::Header)
        ));
        assert_eq!(budget.work(), 1);
        assert_eq!(budget.storage(), floor);
    }
    for offset in 0..POLICY5_EXECUTION_RECORD_BYTES_V1 {
        let mut changed: [u8; 200] = inputs.policy5_record.try_into().unwrap();
        changed[offset] ^= 1;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = prepared.floor() + size_of_val(&changed);
        budget.reserve_storage(floor).unwrap();
        assert!(
            matches!(
                check_published_policy5_semantic_relation_v1(
                    CanonicalPolicy5SemanticInputsV1 {
                        policy5_record: &changed,
                        ..inputs
                    },
                    &mut budget
                ),
                Err(Error::ExecutionClaim)
            ),
            "record byte {offset}"
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn foreign_swapped_and_stale_graph_claims_cannot_hide_wrong_load_relation() {
    let prepared = prepared(&fixture());
    let inputs = prepared.inputs();
    let (foreign, foreign_storage) = admit(&Module::new("foreign"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = prepared.floor() + foreign_storage;
    budget.reserve_storage(floor).unwrap();
    for axis in 0..4 {
        let mut changed = inputs;
        match axis {
            0 => changed.input = &foreign,
            1 => changed.intermediate = &foreign,
            2 => changed.stored = &foreign,
            _ => changed.output = &foreign,
        }
        assert!(check_published_policy5_semantic_relation_v1(changed, &mut budget).is_err());
        assert_eq!(budget.storage(), floor);
    }
    assert!(
        check_published_policy5_semantic_relation_v1(
            CanonicalPolicy5SemanticInputsV1 {
                stored: inputs.output,
                output: inputs.stored,
                ..inputs
            },
            &mut budget
        )
        .is_err()
    );
    let stale = CanonicalPolicy5SemanticInputsV1 {
        output: inputs.stored,
        ..inputs
    };
    let stale_record = record(stale, inputs.load_rows.len());
    budget.reserve_storage(size_of_val(&stale_record)).unwrap();
    assert!(matches!(
        check_published_policy5_semantic_relation_v1(
            CanonicalPolicy5SemanticInputsV1 {
                policy5_record: &stale_record,
                ..stale
            },
            &mut budget
        ),
        Err(Error::Forwarding(_))
    ));
    // A freshly admitted malicious O has correct new identity bytes but the
    // actual claimed replacement computes XOR rather than the checked copy.
    let mut module = inputs.output.module().clone();
    let coordinate = inputs.load_rows[0].load;
    let operation = &mut module.functions[coordinate.block.function.0 as usize]
        .body
        .as_mut()
        .unwrap()
        .blocks[coordinate.block.block as usize]
        .operations[coordinate.operation as usize];
    let (lhs, rhs) = match operation.kind {
        OperationKind::Binary { lhs, rhs, .. } => Some((lhs, rhs)),
        _ => None,
    }
    .expect("the real load-forwarded copy");
    operation.kind = OperationKind::Binary {
        op: BinaryOp::BitXor,
        lhs,
        rhs,
    };
    let (wrong, wrong_storage) = admit(&module);
    budget.reserve_storage(wrong_storage).unwrap();
    let wrong_inputs = CanonicalPolicy5SemanticInputsV1 {
        output: &wrong,
        ..inputs
    };
    let wrong_record = record(wrong_inputs, inputs.load_rows.len());
    budget.reserve_storage(size_of_val(&wrong_record)).unwrap();
    assert!(matches!(
        check_published_policy5_semantic_relation_v1(
            CanonicalPolicy5SemanticInputsV1 {
                policy5_record: &wrong_record,
                ..wrong_inputs
            },
            &mut budget
        ),
        Err(Error::Forwarding(_))
    ));
    assert_eq!(budget.storage(), floor + wrong_storage + 400);
}

#[test]
fn missing_extra_reordered_duplicate_and_foreign_rows_fail_after_exact_reframing() {
    let prepared = prepared(&fixture());
    let inputs = prepared.inputs();
    assert_eq!(inputs.load_rows.len(), 2);
    for mode in 0..5 {
        let mut rows = inputs.load_rows.to_vec();
        match mode {
            0 => {
                rows.pop();
            }
            1 => rows.push(rows[0]),
            2 => rows.swap(0, 1),
            3 => rows[1] = rows[0],
            _ => rows[0].first.block.function.0 = 99,
        }
        let record = record(inputs, rows.len());
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = prepared.floor() + rows.capacity() * size_of::<Row>() + size_of_val(&record);
        budget.reserve_storage(floor).unwrap();
        assert!(
            matches!(
                check_published_policy5_semantic_relation_v1(
                    CanonicalPolicy5SemanticInputsV1 {
                        policy5_record: &record,
                        load_rows: &rows,
                        ..inputs
                    },
                    &mut budget
                ),
                Err(Error::Forwarding(_))
            ),
            "row mode {mode}"
        );
        assert!(
            check_canonical_policy5_execution_relation_v1(
                inputs.input,
                &prepared.checked,
                inputs.policy4_wire,
                &record,
                &rows,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn malformed_nested_p4_wire_is_not_treated_as_an_opaque_identity() {
    let prepared = prepared(&Module::new("no-op"));
    let inputs = prepared.inputs();
    for offset in [
        0,
        8,
        12,
        16,
        24,
        32,
        CANONICAL_POLICY4_EXECUTION_RECEIPT_HEADER_V1,
    ] {
        let mut wire = inputs.policy4_wire.to_vec();
        wire[offset] ^= 1;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = prepared.floor() + wire.capacity();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            check_published_policy5_semantic_relation_v1(
                CanonicalPolicy5SemanticInputsV1 {
                    policy4_wire: &wire,
                    ..inputs
                },
                &mut budget
            ),
            Err(Error::Policy4(_))
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[path = "checked_optimization_policy5_semantic_resource_v1_tests.rs"]
mod resources;
