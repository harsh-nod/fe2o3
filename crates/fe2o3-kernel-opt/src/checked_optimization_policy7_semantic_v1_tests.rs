//! Generic semantics use real consumed owners, but an inert test transcript.
//! Actual backend producer bytes are checked separately against its real stage.
#[path = "checked_optimization_policy8_composition_v1_tests.rs"]
mod policy8_composition;
use super::{STORAGE, WORK, fixture};
use crate::*;
use fe2o3_kernel_analysis::{
    CanonicalKirRedundantStoreRetainedOperationV1 as Retained,
    CanonicalKirRedundantStoreRowV1 as Row,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirOperationCoordinateV1 as Coordinate,
    InertCanonicalKirTransitionReceiptV1 as Wire, Module, OperationKind,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
type Error = CanonicalPolicy7SemanticErrorV1;

#[test]
fn owned_decoded_rows_borrow_into_actual_semantics_without_self_reference() {
    for duplicate in [false, true] {
        let p = prepared(duplicate);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(p.floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let decoded = decode_canonical_policy7_rows_v1(&p.record, &mut budget).unwrap();
        let storage = decoded.storage().retained_storage();
        budget.reserve_storage(storage).unwrap();
        {
            let mut inputs = p.inputs();
            inputs.continuation = decoded.claims();
            assert_eq!(inputs.continuation.deletion_rows, p.continuation.rows());
            assert_eq!(
                inputs.continuation.retained_operations,
                p.continuation.retained_operations()
            );
            let receipt =
                check_published_policy7_semantic_relation_v1(inputs, &mut budget).unwrap();
            assert_eq!(
                receipt.continuation().relation().rows().as_ptr(),
                decoded.claims().deletion_rows.as_ptr()
            );
            assert!(!receipt.authenticates_execution());
            let retained = receipt.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            drop(receipt);
            budget.release_storage(retained).unwrap();
        }
        assert_eq!(budget.storage(), p.floor + storage);
        drop(decoded);
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), p.floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn inert_identity_payloads_and_ordered_foreign_rows_still_need_semantic_admission() {
    let p = prepared(true);
    let mut foreign = p.continuation.rows().to_vec();
    for row in &mut foreign {
        row.anchor.block.function.0 = u32::MAX;
        row.removed.block.function.0 = u32::MAX;
    }
    let foreign_record = encode(
        p.checked.execution().canonical_bytes(),
        p.checked.owner(),
        p.continuation.output(),
        &foreign,
        p.continuation.retained_operations(),
    );
    let mut bad_prefix = p.record.clone();
    bad_prefix[16] ^= 1;
    let mut bad_input = p.record.clone();
    bad_input[272] ^= 1;
    let mut bad_output = p.record.clone();
    bad_output[312] ^= 1;
    // These fixture buffers are external caller backing, not decoder copies.
    for record in [&foreign_record, &bad_prefix, &bad_input, &bad_output] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(p.floor).unwrap();
        let decoded = decode_canonical_policy7_rows_v1(record, &mut budget).unwrap();
        assert!(!decoded.grants_authority());
        let storage = decoded.storage().retained_storage();
        budget.reserve_storage(storage).unwrap();
        {
            let mut inputs = p.inputs();
            inputs.continuation = decoded.claims();
            assert!(check_published_policy7_semantic_relation_v1(inputs, &mut budget).is_err());
        }
        assert_eq!(budget.storage(), p.floor + storage);
        drop(decoded);
        budget.release_storage(storage).unwrap();
        assert_eq!(budget.storage(), p.floor);
    }
}

#[test]
fn long_store_run_has_complete_rows_and_stale_or_swapped_graphs_are_refused() {
    let mut module = fixture();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let store = block.operations[1].clone();
    for _ in 0..31 {
        block.operations.insert(4, store.clone());
    }
    // The real P6 execution inherits the Policy3 logical execution envelope;
    // this larger fixture exhausts WORK during preparation, not P7 semantics.
    {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let (input, input_storage, p5) = prepared_policy5(&module, &mut budget);
        let ledger = budget.work_ledger_identity_v1();
        match continue_checked_canonical_kernel_ir_policy6_v1(&input, p5, &mut budget) {
            Err(CanonicalPolicy6OptimizationErrorV1::Observation(
                fe2o3_pliron::KirNeutralOptimizationErrorV1::Execution(
                    fe2o3_pliron::PlironOptimizationErrorV12::Resources(Resource::Work(error)),
                ),
            )) => {
                assert_eq!(error.limit(), WORK);
                assert!(error.actual() > WORK);
            }
            Err(error) => panic!("unexpected long-fixture preparation error: {error:?}"),
            Ok(_) => panic!("long-fixture preparation unexpectedly fits WORK"),
        }
        assert_eq!(budget.storage(), input_storage);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(budget.work() > 0);
        drop(input);
        budget.release_storage(input_storage).unwrap();
        assert_eq!(budget.storage(), 0);
    }
    const LONG_PREPARATION_WORK: usize = 1_000_000_000;
    let (p, preparation_work) = prepared_module_with_work(&module, LONG_PREPARATION_WORK);
    assert!(preparation_work > WORK);
    assert_eq!(p.continuation.rows().len(), 33);
    run(p.inputs(), p.floor, WORK, STORAGE).0.unwrap();
    for stale in [&p.input, p.checked.owner()] {
        let record = encode(
            p.checked.execution().canonical_bytes(),
            p.checked.owner(),
            stale,
            p.continuation.rows(),
            p.continuation.retained_operations(),
        );
        let mut inputs = p.inputs();
        inputs.output = stale;
        inputs.continuation.execution_record = &record;
        assert!(run(inputs, p.floor, WORK, STORAGE).0.is_err());
    }
    let mut inputs = p.inputs();
    inputs.prefix.output = p.continuation.output();
    inputs.output = p.checked.owner();
    assert!(run(inputs, p.floor, WORK, STORAGE).0.is_err());
}

struct Prepared {
    input: Owner,
    checked: CheckedCanonicalKernelIrOwnerPolicy6V1,
    continuation: OwnedRedundantStoreContinuationV1,
    p4: InertCanonicalPolicy4ExecutionReceiptV1,
    p6_wire: Wire,
    record: Vec<u8>,
    floor: usize,
}
fn encode(
    prefix: &[u8; 256],
    input: &Owner,
    output: &Owner,
    rows: &[Row],
    origins: &[Retained],
) -> Vec<u8> {
    let extent = 384 + (rows.len() + origins.len()) * 24;
    let mut bytes = vec![0; 384];
    bytes[..16].copy_from_slice(b"F2P7EX1\0\x01\0\x07\0\0\x01\0\0");
    bytes[16..272].copy_from_slice(prefix);
    for (offset, graph) in [(272, input), (312, output)] {
        let id = graph.canonical().identity();
        bytes[offset..offset + 32].copy_from_slice(id.digest());
        bytes[offset + 32..offset + 40].copy_from_slice(&id.canonical_length().to_le_bytes());
    }
    for (offset, count) in [
        (352, rows.len()),
        (360, origins.len()),
        (368, 1),
        (376, extent),
    ] {
        bytes[offset..offset + 8].copy_from_slice(&(count as u64).to_le_bytes());
    }
    let mut coordinate = |c: Coordinate| {
        for value in [c.block.function.0, c.block.block, c.operation] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    };
    for row in rows {
        coordinate(row.anchor);
        coordinate(row.removed);
    }
    for row in origins {
        coordinate(row.input);
        coordinate(row.output);
    }
    assert_eq!(bytes.len(), extent);
    bytes
}
fn prepared(duplicate: bool) -> Prepared {
    let mut module = fixture();
    if !duplicate {
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[0].operations.remove(4);
        body.blocks[0].operations.remove(3);
    }
    let result = prepared_module(&module);
    assert_eq!(
        result.continuation.rows().len(),
        if duplicate { 2 } else { 0 }
    );
    result
}
fn prepared_module(module: &Module) -> Prepared {
    prepared_module_with_work(module, WORK).0
}
fn prepared_policy5(
    module: &Module,
    budget: &mut Budget<'_>,
) -> (Owner, usize, CheckedCanonicalKernelIrOwnerPolicy5V1) {
    let (input, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let p5 = optimize_checked_canonical_kernel_ir_policy5_v1(&input, budget).unwrap();
    budget.reserve_storage(p5.retained_storage()).unwrap();
    (input, storage.retained_storage(), p5)
}
fn prepared_module_with_work(module: &Module, work_limit: usize) -> (Prepared, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (input, _, p5) = prepared_policy5(module, &mut budget);
    let checked = continue_checked_canonical_kernel_ir_policy6_v1(&input, p5, &mut budget).unwrap();
    budget.reserve_storage(checked.retained_storage()).unwrap();
    let p4 = encode_checked_canonical_policy4_execution_receipt_v1(
        &input,
        checked.intermediate_policy5().intermediate_policy4(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(p4.storage().retained_storage())
        .unwrap();
    let (p6_wire, storage) = Wire::from_candidate_with_budget(
        checked
            .intermediate_policy5()
            .owner()
            .canonical()
            .identity(),
        checked.owner().canonical().identity(),
        checked.continuation().occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let continuation =
        prepare_owned_redundant_store_continuation_v1(checked.owner(), &mut budget).unwrap();
    budget
        .reserve_storage(continuation.retained_storage())
        .unwrap();
    let record = encode(
        checked.execution().canonical_bytes(),
        checked.owner(),
        continuation.output(),
        continuation.rows(),
        continuation.retained_operations(),
    );
    let floor = 53 + budget.storage() + record.capacity();
    (
        Prepared {
            input,
            checked,
            continuation,
            p4,
            p6_wire,
            record,
            floor,
        },
        budget.work(),
    )
}
impl Prepared {
    fn inputs(&self) -> CanonicalPolicy7SemanticInputsV1<'_> {
        let p5 = self.checked.intermediate_policy5();
        let p4 = p5.intermediate_policy4();
        CanonicalPolicy7SemanticInputsV1 {
            prefix: CanonicalPolicy6SemanticInputsV1 {
                prefix: CanonicalPolicy5SemanticInputsV1 {
                    input: &self.input,
                    intermediate: p4.intermediate_policy3().owner(),
                    stored: p4.owner(),
                    output: p5.owner(),
                    policy4_wire: self.p4.canonical_bytes(),
                    policy5_record: p5.execution().canonical_bytes(),
                    load_rows: p5.load_forwarding_rows(),
                },
                output: self.checked.owner(),
                continuation: CanonicalPolicy6ContinuationClaimsV1 {
                    composition_record: self.checked.execution().canonical_bytes(),
                    integer_record: self.checked.continuation().execution().canonical_bytes(),
                    transition_wire: self.p6_wire.canonical_bytes(),
                },
            },
            output: self.continuation.output(),
            continuation: CanonicalPolicy7ContinuationClaimsV1 {
                execution_record: &self.record,
                deletion_rows: self.continuation.rows(),
                retained_operations: self.continuation.retained_operations(),
            },
        }
    }
}
fn run(
    inputs: CanonicalPolicy7SemanticInputsV1<'_>,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<usize, Error>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = check_published_policy7_semantic_relation_v1(inputs, &mut budget).map(|receipt| {
        assert!(std::ptr::eq(
            receipt.continuation().relation().input(),
            inputs.prefix.output
        ));
        assert!(std::ptr::eq(
            receipt.continuation().relation().output(),
            inputs.output
        ));
        assert_eq!(
            receipt.continuation().relation().rows(),
            inputs.continuation.deletion_rows
        );
        assert!(!receipt.authenticates_execution());
        assert!(!receipt.grants_authority());
        let retained = receipt.storage().retained_storage();
        assert_eq!(
            retained,
            receipt.policy6_relation().storage().retained_storage()
                + receipt.continuation().storage().retained_storage()
                + std::mem::size_of_val(&receipt)
                - std::mem::size_of_val(receipt.policy6_relation())
                - std::mem::size_of_val(receipt.continuation())
        );
        drop(receipt);
        retained
    });
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn actual_p6_then_dse_mutation_and_noop_have_exact_complete_semantics() {
    for duplicate in [false, true] {
        let p = prepared(duplicate);
        assert_eq!(
            p.checked.owner().canonical().canonical_bytes()
                != p.continuation.output().canonical().canonical_bytes(),
            duplicate
        );
        let a = run(p.inputs(), p.floor, WORK, STORAGE);
        a.0.unwrap();
        let b = run(p.inputs(), p.floor, WORK, STORAGE);
        assert_eq!(a.1, b.1);
        assert_eq!(a.2, b.2);
        assert_eq!(
            encode(
                p.checked.execution().canonical_bytes(),
                p.checked.owner(),
                p.continuation.output(),
                p.continuation.rows(),
                p.continuation.retained_operations()
            ),
            p.record
        );
        for row in p.continuation.retained_operations() {
            let old = &p.checked.owner().module().functions[row.input.block.function.0 as usize]
                .body
                .as_ref()
                .unwrap()
                .blocks[row.input.block.block as usize]
                .operations[row.input.operation as usize];
            let new = &p.continuation.output().module().functions
                [row.output.block.function.0 as usize]
                .body
                .as_ref()
                .unwrap()
                .blocks[row.output.block.block as usize]
                .operations[row.output.operation as usize];
            assert_eq!(old, new);
        }
    }
}

#[test]
fn every_transcript_byte_and_all_short_or_extra_extents_are_checked() {
    let p = prepared(true);
    for index in 0..p.record.len() {
        let mut changed = p.record.clone();
        changed[index] ^= 1;
        let mut inputs = p.inputs();
        inputs.continuation.execution_record = &changed;
        assert!(local(&p, inputs.continuation).is_err(), "byte {index}");
    }
    for len in [0, 383, p.record.len() - 1, p.record.len() + 1] {
        let mut changed = p.record.clone();
        changed.resize(len, 0);
        let mut inputs = p.inputs();
        inputs.continuation.execution_record = &changed;
        assert!(local(&p, inputs.continuation).is_err());
    }
}

fn local(p: &Prepared, claims: CanonicalPolicy7ContinuationClaimsV1<'_>) -> Result<(), Error> {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(p.floor).unwrap();
    let result = check_canonical_policy7_continuation_relation_v1(
        p.checked.execution().canonical_bytes(),
        p.checked.owner(),
        p.continuation.output(),
        claims,
        &mut budget,
    )
    .map(drop);
    assert_eq!(budget.storage(), p.floor);
    result
}

#[test]
fn independently_reframed_bad_rows_and_changed_actual_output_are_not_repaired() {
    let p = prepared(true);
    let rows = p.continuation.rows();
    let origins = p.continuation.retained_operations();
    let mut changed = rows.to_vec();
    changed.swap(0, 1);
    let mut duplicate = rows.to_vec();
    duplicate.push(rows[0]);
    let mut foreign = rows.to_vec();
    foreign[0].anchor.operation += 1;
    for bad in [changed, duplicate, foreign, rows[..1].to_vec()] {
        let record = encode(
            p.checked.execution().canonical_bytes(),
            p.checked.owner(),
            p.continuation.output(),
            &bad,
            origins,
        );
        let mut inputs = p.inputs();
        inputs.continuation.execution_record = &record;
        inputs.continuation.deletion_rows = &bad;
        assert!(run(inputs, p.floor, WORK, STORAGE).0.is_err());
    }
    let mut reordered = origins.to_vec();
    reordered.swap(0, 1);
    let mut duplicated = origins.to_vec();
    duplicated.push(origins[0]);
    let mut foreign = origins.to_vec();
    foreign[0].input.operation += 1;
    for bad in [
        reordered,
        duplicated,
        foreign,
        origins[..origins.len() - 1].to_vec(),
    ] {
        let record = encode(
            p.checked.execution().canonical_bytes(),
            p.checked.owner(),
            p.continuation.output(),
            rows,
            &bad,
        );
        let mut inputs = p.inputs();
        inputs.continuation.execution_record = &record;
        inputs.continuation.retained_operations = &bad;
        assert!(run(inputs, p.floor, WORK, STORAGE).0.is_err());
    }
    let mut module = p.continuation.output().module().clone();
    for operation in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
        if let OperationKind::Store { value, .. } = &mut operation.kind {
            *value = fe2o3_kernel_ir::ValueId(1);
            break;
        }
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (wrong, _) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    let record = encode(
        p.checked.execution().canonical_bytes(),
        p.checked.owner(),
        &wrong,
        rows,
        origins,
    );
    let mut inputs = p.inputs();
    inputs.output = &wrong;
    inputs.continuation.execution_record = &record;
    assert!(run(inputs, p.floor, WORK, STORAGE).0.is_err());
}

#[test]
fn equal_byte_owner_borrows_and_consistent_nested_claims_are_only_semantics() {
    let p = prepared(true);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (equal, _) = Owner::from_module_ref_with_verification_budget_v12(
        p.continuation.output().module(),
        &mut budget,
    )
    .unwrap();
    assert!(!std::ptr::eq(&equal, p.continuation.output()));
    let mut inputs = p.inputs();
    inputs.output = &equal;
    run(inputs, p.floor, WORK, STORAGE).0.unwrap();
    for map in [false, true] {
        let mut composition = *p.checked.execution().canonical_bytes();
        let mut integer = *p.checked.continuation().execution().canonical_bytes();
        if map {
            composition[216] ^= 1;
            integer[264] ^= 1;
        } else {
            integer[88] ^= 1;
        }
        let mut record = p.record.clone();
        record[16..272].copy_from_slice(&composition);
        let mut inputs = p.inputs();
        inputs.prefix.continuation.composition_record = &composition;
        inputs.prefix.continuation.integer_record = &integer;
        inputs.continuation.execution_record = &record;
        run(inputs, p.floor, WORK, STORAGE).0.unwrap();
    }
}

#[test]
fn complete_adapter_exact_work_and_storage_preserve_caller_floor() {
    let p = prepared(true);
    let measured = run(p.inputs(), p.floor, WORK, STORAGE);
    measured.0.unwrap();
    run(p.inputs(), p.floor, measured.1, measured.2).0.unwrap();
    assert!(
        run(p.inputs(), p.floor, measured.1 - 1, measured.2)
            .0
            .is_err()
    );
    assert!(
        run(p.inputs(), p.floor, measured.1, measured.2 - 1)
            .0
            .is_err()
    );
}
