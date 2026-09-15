use super::*;
use crate::optimize_checked_canonical_kernel_ir_v1;
use fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirOperationOriginV1,
    CanonicalKirTransitionCandidateV1, Constant, Function, Module, Operation, OperationKind,
    ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
use std::mem::size_of_val;

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 1_000_000_000;
const FLOOR: usize = 23;
const PRIOR: usize = 7;
type ReceiptError = KernelIrCheckedOptimizationReceiptErrorV1;

fn source(literal: u32) -> Module {
    let mut block = BasicBlock::new(BlockId(40));
    for (id, value) in [(17, literal), (93, 99)] {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(value)),
        ));
    }
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(17)],
    });
    let mut module = Module::new("receipt-adapter");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
        vec![],
        vec![block],
    ));
    module
}

fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, storage.retained_storage())
}

// Component boundaries below start with explicitly transferred real owners and
// receipts. These setup ledgers are not a whole optimizer-plus-wire budget test.
fn prepare(input: &(Owner, usize)) -> CheckedNeutralKernelIrOwnerV1 {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input.1).unwrap();
    let checked = optimize_checked_canonical_kernel_ir_v1(&input.0, &mut budget).unwrap();
    assert_eq!(budget.storage(), input.1);
    checked
}
fn encode(
    input: &(Owner, usize),
    checked: &CheckedNeutralKernelIrOwnerV1,
) -> (InertCanonicalKirTransitionReceiptV1, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = input.1 + checked.storage().retained_storage();
    budget.reserve_storage(floor).unwrap();
    let (receipt, storage) =
        encode_checked_canonical_optimization_receipt_v1(&input.0, checked, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    (receipt, storage.retained_storage())
}

#[test]
fn actual_changed_and_noop_outputs_roundtrip_with_exact_endpoint_borrows() {
    for module in [source(7), Module::new("m")] {
        let input = admit(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR).unwrap();
        budget.reserve_storage(FLOOR + input.1).unwrap();
        let checked = optimize_checked_canonical_kernel_ir_v1(&input.0, &mut budget).unwrap();
        assert_eq!(
            checked.report().passes().iter().any(|pass| pass.changed()),
            !module.functions.is_empty()
        );
        let checked_storage = checked.storage().retained_storage();
        budget.reserve_storage(checked_storage).unwrap();
        let (wire, wire_storage) =
            encode_checked_canonical_optimization_receipt_v1(&input.0, &checked, &mut budget)
                .unwrap();
        budget
            .reserve_storage(wire_storage.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let receipt = decode_and_check_canonical_optimization_receipt_v1(
            &input.0,
            checked.owner(),
            wire.canonical_bytes(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        let retained = receipt.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        assert!(std::ptr::eq(receipt.input(), &input.0));
        assert!(std::ptr::eq(receipt.output(), checked.owner()));
        assert_eq!(receipt.receipt().canonical_bytes(), wire.canonical_bytes());
        assert_eq!(
            receipt.receipt().candidate().operations,
            checked.occurrences().candidate().operations
        );
        assert!(!receipt.grants_authority());
        assert!(!receipt.receipt().grants_authority());
        drop(receipt);
        budget.release_storage(retained).unwrap();
        drop(wire);
        budget
            .release_storage(wire_storage.retained_storage())
            .unwrap();
        drop(checked);
        budget.release_storage(checked_storage).unwrap();
        let input_storage = input.1;
        drop(input);
        budget.release_storage(input_storage).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn encoder_requires_complete_history_but_not_historical_pointer_identity() {
    let input = admit(&source(7));
    let checked = prepare(&input);
    for literal in [7, 8] {
        let other = admit(&source(literal));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR + input.1 + other.1 + checked.storage().retained_storage();
        budget.reserve_storage(floor).unwrap();
        let result =
            encode_checked_canonical_optimization_receipt_v1(&other.0, &checked, &mut budget);
        assert_eq!(budget.storage(), floor);
        if literal == 7 {
            let (wire, _) = result.unwrap();
            assert!(
                wire.input_identity()
                    .matches_verified(other.0.canonical().identity())
            );
            assert!(!std::ptr::eq(&input.0, &other.0));
            drop(wire);
        } else {
            assert!(matches!(result, Err(ReceiptError::InputHistory)));
            assert_eq!(
                budget.work(),
                2 + other.0.canonical().canonical_bytes().len()
                    + checked.native_input_audit_bytes().len()
            );
            assert_eq!(budget.peak_storage(), floor);
        }
    }
}

#[test]
fn inert_endpoint_edits_do_not_choose_actual_input_or_output() {
    let input = admit(&source(7));
    let checked = prepare(&input);
    let (wire, wire_storage) = encode(&input, &checked);
    // Two digest coordinates and two canonical-length coordinates. All remain
    // valid wire framing, but neither actual owner is selected by these bytes.
    for offset in [16, 48, 56, 88] {
        let mut bytes = wire.canonical_bytes().to_vec();
        bytes[offset] ^= 1;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR
            + input.1
            + checked.storage().retained_storage()
            + wire_storage
            + size_of::<Vec<u8>>()
            + bytes.capacity();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            decode_and_check_canonical_optimization_receipt_v1(
                &input.0,
                checked.owner(),
                &bytes,
                &mut budget,
            ),
            Err(ReceiptError::Transition(
                CanonicalKirTransitionErrorV1::Rule("transition receipt endpoint or policy")
            ))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn independently_admitted_other_output_cannot_reuse_a_receipt() {
    let input = admit(&source(7));
    let checked = prepare(&input);
    let (wire, wire_storage) = encode(&input, &checked);
    let other = admit(&source(8));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + input.1 + checked.storage().retained_storage() + wire_storage + other.1;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        decode_and_check_canonical_optimization_receipt_v1(
            &input.0,
            &other.0,
            wire.canonical_bytes(),
            &mut budget,
        ),
        Err(ReceiptError::Transition(
            CanonicalKirTransitionErrorV1::Rule("transition receipt endpoint or policy")
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn reframed_rows_still_require_complete_coverage_and_matching_operation_payload() {
    let input = admit(&source(7));
    let checked = prepare(&input);
    let rows = checked.occurrences().candidate();
    assert_eq!(rows.operations.len(), 1);
    let mut operations = rows.operations.to_vec();
    let CanonicalKirOperationOriginV1::Retained(mut original) = operations[0].origin else {
        panic!("expected the actual surviving constant's retained origin");
    };
    assert_eq!(original.operation, 0);
    original.operation = 1;
    operations[0].origin = CanonicalKirOperationOriginV1::Retained(original);
    for missing in [false, true] {
        let candidate = CanonicalKirTransitionCandidateV1 {
            operations: if missing { &[] } else { &operations },
            ..rows
        };
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR
            + input.1
            + checked.storage().retained_storage()
            + size_of_val(&operations)
            + operations.capacity() * size_of_val(&operations[0]);
        budget.reserve_storage(floor).unwrap();
        // The public inert codec deliberately permits these hostile rows.
        let (wire, storage) = InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
            input.0.canonical().identity(),
            checked.owner().canonical().identity(),
            candidate,
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let result = decode_and_check_canonical_optimization_receipt_v1(
            &input.0,
            checked.owner(),
            wire.canonical_bytes(),
            &mut budget,
        );
        if missing {
            assert!(matches!(
                result,
                Err(ReceiptError::Transition(
                    CanonicalKirTransitionErrorV1::IncompleteRows
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ReceiptError::Transition(
                    CanonicalKirTransitionErrorV1::Rule("retained operation payload")
                ))
            ));
        }
        assert_eq!(budget.storage(), floor + storage.retained_storage());
        drop(wire);
        budget.release_storage(storage.retained_storage()).unwrap();
    }
}

#[test]
fn even_reframed_exact_endpoints_do_not_bypass_module_metadata_rules() {
    let input = admit(&Module::new("m"));
    let checked = prepare(&input);
    let other = admit(&Module::new("n"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + input.1 + checked.storage().retained_storage() + other.1;
    budget.reserve_storage(floor).unwrap();
    let (wire, storage) = InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
        input.0.canonical().identity(),
        other.0.canonical().identity(),
        checked.occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(matches!(
        decode_and_check_canonical_optimization_receipt_v1(
            &input.0,
            &other.0,
            wire.canonical_bytes(),
            &mut budget,
        ),
        Err(ReceiptError::Transition(
            CanonicalKirTransitionErrorV1::Rule("module metadata")
        ))
    ));
    assert_eq!(budget.storage(), floor + storage.retained_storage());
    drop(wire);
    budget.release_storage(storage.retained_storage()).unwrap();
}

#[test]
fn policy_and_trailing_payload_are_rejected_without_checker_fallback() {
    let input = admit(&Module::new("m"));
    let checked = prepare(&input);
    let (wire, wire_storage) = encode(&input, &checked);
    for policy in [false, true] {
        let mut bytes = wire.canonical_bytes().to_vec();
        if policy {
            bytes[10] = 2;
        } else {
            bytes.push(0);
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = FLOOR
            + input.1
            + checked.storage().retained_storage()
            + wire_storage
            + size_of::<Vec<u8>>()
            + bytes.capacity();
        budget.reserve_storage(floor).unwrap();
        let result = decode_and_check_canonical_optimization_receipt_v1(
            &input.0,
            checked.owner(),
            &bytes,
            &mut budget,
        );
        let expected = if policy {
            "header policy"
        } else {
            "frame length"
        };
        assert!(matches!(result,
            Err(ReceiptError::Codec(CanonicalKirTransitionReceiptErrorV1::Malformed(field)))
                if field == expected
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
    }
}

#[test]
fn empty_encoding_exact_and_one_under_work_are_source_derived() {
    // Historical comparison2+37+37, codec entry1, empty frame copy/hash
    // 2*132+39-byte domain+8-byte length =311. Total388, prefix77.
    const TOTAL: usize = 388;
    const BEFORE_COPY: usize = 77;
    let input = admit(&Module::new("m"));
    let checked = prepare(&input);
    assert_eq!(input.0.canonical().canonical_bytes().len(), 37);
    for short in [false, true] {
        let mut work = Work::new(PRIOR + TOTAL - usize::from(short));
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR).unwrap();
        let floor = FLOOR + input.1 + checked.storage().retained_storage();
        budget.reserve_storage(floor).unwrap();
        let result =
            encode_checked_canonical_optimization_receipt_v1(&input.0, &checked, &mut budget);
        assert_eq!(budget.storage(), floor);
        if short {
            assert!(matches!(
                result,
                Err(ReceiptError::Codec(
                    CanonicalKirTransitionReceiptErrorV1::Resource(Resource::Work(_))
                ))
            ));
            assert_eq!(budget.work(), PRIOR + BEFORE_COPY);
            assert!(budget.charge_work(usize::MAX).is_err());
            assert_eq!(work.failed_work(), Some(PRIOR + TOTAL));
        } else {
            let (receipt, _) = result.unwrap();
            assert_eq!(receipt.canonical_bytes().len(), 132);
            assert_eq!(budget.work(), PRIOR + TOTAL);
            assert_eq!(budget.failed_storage(), None);
            drop(receipt);
        }
    }
}

#[test]
fn empty_decode_check_exact_and_one_under_work_are_source_derived() {
    // Entry1 + codec(1+132+0+0+311)=444 + inventories4 + endpoint80
    // + independent checker7 + wrapper arithmetic/construct3 =539.
    const TOTAL: usize = 539;
    const BEFORE_WRAPPER: usize = 536;
    let input = admit(&Module::new("m"));
    let checked = prepare(&input);
    let (wire, wire_storage) = encode(&input, &checked);
    for short in [false, true] {
        let mut work = Work::new(PRIOR + TOTAL - usize::from(short));
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR).unwrap();
        let floor = FLOOR + input.1 + checked.storage().retained_storage() + wire_storage;
        budget.reserve_storage(floor).unwrap();
        let result = decode_and_check_canonical_optimization_receipt_v1(
            &input.0,
            checked.owner(),
            wire.canonical_bytes(),
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        if short {
            assert!(matches!(
                result,
                Err(ReceiptError::Resource(Resource::Work(_)))
            ));
            assert_eq!(budget.work(), PRIOR + BEFORE_WRAPPER);
            assert!(budget.charge_work(usize::MAX).is_err());
            assert_eq!(work.failed_work(), Some(PRIOR + TOTAL));
        } else {
            let receipt = result.unwrap();
            assert_eq!(budget.work(), PRIOR + TOTAL);
            let retained = receipt.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            drop(receipt);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn empty_decode_check_exact_and_one_under_storage_bound_coexisting_headers() {
    // Empty rows allocate no vector payload. The peak is receipt header+132
    // bytes, two inventory headers, checked-view header, and checker State.
    // State is two inventory references + candidate +18 Vec headers; all its
    // fields have pointer alignment on the supported host, with no extra pad.
    let receipt_storage = size_of::<InertCanonicalKirTransitionReceiptV1>() + 132;
    let inventories = 2 * size_of::<CanonicalKirInventoryV1<'_>>();
    let view = size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>();
    let state = 2 * size_of::<&CanonicalKirInventoryV1<'_>>()
        + size_of::<CanonicalKirTransitionCandidateV1<'_>>()
        + 18 * size_of::<Vec<usize>>();
    let peak = receipt_storage + inventories + view + state;
    let input = admit(&Module::new("m"));
    let checked = prepare(&input);
    let (wire, wire_storage) = encode(&input, &checked);
    assert_eq!(wire_storage, receipt_storage);
    for short in [false, true] {
        let floor = FLOOR + input.1 + checked.storage().retained_storage() + wire_storage;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, floor + peak - usize::from(short));
        budget.charge_work(PRIOR).unwrap();
        budget.reserve_storage(floor).unwrap();
        let result = decode_and_check_canonical_optimization_receipt_v1(
            &input.0,
            checked.owner(),
            wire.canonical_bytes(),
            &mut budget,
        );
        assert_eq!(budget.storage(), floor);
        if short {
            assert!(matches!(
                result,
                Err(ReceiptError::Transition(
                    CanonicalKirTransitionErrorV1::Resource(Resource::Storage(_))
                ))
            ));
            assert_eq!(budget.work(), PRIOR + 531);
            assert_eq!(
                budget.peak_storage(),
                floor + receipt_storage + inventories + view
            );
            assert_eq!(budget.failed_storage(), Some(floor + peak));
            assert!(budget.reserve_storage(usize::MAX - floor).is_err());
            assert_eq!(budget.failed_storage(), Some(floor + peak));
        } else {
            let receipt = result.unwrap();
            assert_eq!(budget.work(), PRIOR + 539);
            assert_eq!(budget.peak_storage(), floor + peak);
            assert_eq!(
                receipt.storage().retained_storage(),
                size_of::<CheckedCanonicalOptimizationReceiptV1<'_, '_>>() + 132
            );
            let retained = receipt.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            drop(receipt);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
}
