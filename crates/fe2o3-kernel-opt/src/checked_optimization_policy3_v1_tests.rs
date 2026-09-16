use super::*;
use crate::{
    optimize_checked_canonical_kernel_ir_policy3_v1, optimize_checked_canonical_kernel_ir_v1,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Function, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature, Terminator,
    Type, ValueDef, ValueId,
};

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 1_000_000_000;
const PREFIX: usize = 29;
const PRIOR: usize = 7;

fn source() -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let value = |id| ValueDef::new(ValueId(id), u32_ty.clone());
    let expression = |id| {
        Operation::effect_free(
            value(id),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(8),
                rhs: ValueId(9),
            },
        )
    };
    let store = |id| {
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(99),
                value: ValueId(id),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        )
    };
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![expression(1), store(1)];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(33),
        then_target: BlockId(30),
        then_arguments: vec![],
        else_target: BlockId(70),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(30));
    left.operations = vec![expression(2), store(2)];
    left.terminator = Some(Terminator::Branch {
        target: BlockId(90),
        arguments: vec![],
    });
    let mut right = BasicBlock::new(BlockId(70));
    right.operations = vec![expression(3), store(3)];
    right.terminator = Some(Terminator::Branch {
        target: BlockId(90),
        arguments: vec![],
    });
    let mut join = BasicBlock::new(BlockId(90));
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    let mut module = Module::new("policy3-diamond");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![pointer.clone(), Type::BOOL, u32_ty.clone(), u32_ty.clone()],
            vec![u32_ty.clone()],
        ),
        vec![ValueId(99), ValueId(33), ValueId(8), ValueId(9)],
        vec![entry, left, right, join],
    ));
    module
}

fn admit(source: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(source, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}

fn census(owner: &Owner) -> (usize, usize) {
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let binary = body
        .blocks
        .iter()
        .flat_map(|b| &b.operations)
        .filter(|op| {
            matches!(
                op.kind,
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    ..
                }
            )
        })
        .count();
    let stores = body
        .blocks
        .iter()
        .flat_map(|b| &b.operations)
        .filter(|op| matches!(op.kind, OperationKind::Store { .. }))
        .count();
    (binary, stores)
}

#[test]
fn actual_dominating_diamond_executes_eight_passes_and_checks_exact_output_and_receipt() {
    let input = admit(&source());
    let bytes = input.0.canonical().canonical_bytes().to_vec();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(PRIOR).unwrap();
    budget.reserve_storage(PREFIX + input.1).unwrap();
    let historical = optimize_checked_canonical_kernel_ir_v1(&input.0, &mut budget).unwrap();
    assert_eq!(historical.report().passes().len(), 7);
    assert_eq!(census(historical.owner()), (3, 3));
    drop(historical);
    let checked = optimize_checked_canonical_kernel_ir_policy3_v1(&input.0, &mut budget).unwrap();
    assert_eq!(budget.storage(), PREFIX + input.1);
    assert_eq!(input.0.canonical().canonical_bytes(), bytes);
    assert_eq!(checked.native_input_audit_bytes(), bytes);
    assert_ne!(checked.owner().canonical().canonical_bytes(), bytes);
    assert_eq!(census(checked.owner()), (1, 3));
    let names = checked
        .report()
        .passes()
        .iter()
        .map(|pass| pass.pass().name())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "sparse-conditional-constant-propagation",
            "simplify-control-flow",
            "select-same-value-canonicalization",
            "dead-code-elimination",
            "local-pure-common-subexpression-elimination",
            "dominance-pure-common-subexpression-elimination",
            "dead-code-elimination",
            "simplify-control-flow"
        ]
    );
    assert!(checked.report().passes()[5].changed());
    assert!(checked.map().matches_execution(checked.report()));
    assert_eq!(checked.execution().canonical_bytes().len(), 776);
    let retained = checked.storage().retained_storage();
    budget.reserve_storage(retained).unwrap();
    let receipt =
        encode_checked_canonical_policy3_execution_receipt_v1(&input.0, &checked, &mut budget)
            .unwrap();
    let wire_storage = receipt.storage().retained_storage();
    budget.reserve_storage(wire_storage).unwrap();
    assert_eq!(
        receipt.canonical_bytes().len(),
        CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1
            + u64::from_le_bytes(receipt.canonical_bytes()[24..32].try_into().unwrap()) as usize
    );
    let decoded = decode_and_check_canonical_policy3_execution_receipt_v1(
        &input.0,
        &checked,
        receipt.canonical_bytes(),
        &mut budget,
    )
    .unwrap();
    let decoded_storage = decoded.storage().retained_storage();
    budget.reserve_storage(decoded_storage).unwrap();
    assert!(std::ptr::eq(decoded.output(), checked.owner()));
    assert!(std::ptr::eq(decoded.execution_owner(), &checked));
    assert_eq!(
        decoded.receipt().canonical_bytes(),
        receipt.canonical_bytes()
    );
    assert_eq!(decoded.receipt().digest(), receipt.digest());
    assert!(!decoded.grants_authority());
    drop(decoded);
    budget.release_storage(decoded_storage).unwrap();
    drop(receipt);
    budget.release_storage(wire_storage).unwrap();
    drop(checked);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), PREFIX + input.1);
    assert_eq!(budget.failed_storage(), None);
}

// Partial receipt-boundary tests explicitly transfer this genuine checked owner
// out of a preparation ledger; they do not claim whole-pipeline resource caps.
fn prepared() -> ((Owner, usize), CheckedOwner) {
    let input = admit(&Module::new("m"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input.1).unwrap();
    let checked = optimize_checked_canonical_kernel_ir_policy3_v1(&input.0, &mut budget).unwrap();
    assert_eq!(
        checked.owner().canonical().canonical_bytes(),
        input.0.canonical().canonical_bytes()
    );
    (input, checked)
}

#[test]
fn unchanged_endpoints_cannot_substitute_policy_count_order_profile_or_execution_identity() {
    let (input, checked) = prepared();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = PREFIX + input.1 + checked.storage().retained_storage();
    budget.reserve_storage(floor).unwrap();
    let receipt =
        encode_checked_canonical_policy3_execution_receipt_v1(&input.0, &checked, &mut budget)
            .unwrap();
    budget
        .reserve_storage(receipt.storage().retained_storage())
        .unwrap();
    // Record offsets: policy0/profile2/count4, B8, O48, profile88,
    // map264, first pass296, second pass356. All offsets are fixed wire bytes.
    for offset in [32, 34, 36, 40, 80, 120, 296, 328, 388] {
        let mut bad = receipt.canonical_bytes().to_vec();
        bad[offset] ^= 1;
        let before = budget.storage();
        assert!(matches!(
            decode_and_check_canonical_policy3_execution_receipt_v1(
                &input.0,
                &checked,
                &bad,
                &mut budget
            ),
            Err(E::ExecutionWitness)
        ));
        assert_eq!(budget.storage(), before);
    }
    // A plain historical F2NTR1 body does not satisfy this typed composition.
    assert!(matches!(
        decode_and_check_canonical_policy3_execution_receipt_v1(
            &input.0,
            &checked,
            &receipt.canonical_bytes()[CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1..],
            &mut budget
        ),
        Err(E::Header)
    ));
    let mut reordered = receipt.canonical_bytes().to_vec();
    let a = reordered[328..388].to_vec();
    let b = reordered[388..448].to_vec();
    reordered[328..388].copy_from_slice(&b);
    reordered[388..448].copy_from_slice(&a);
    assert!(matches!(
        decode_and_check_canonical_policy3_execution_receipt_v1(
            &input.0,
            &checked,
            &reordered,
            &mut budget
        ),
        Err(E::ExecutionWitness)
    ));
}

#[test]
fn framing_caps_trailing_bytes_and_lengths_are_canonical() {
    assert_eq!(CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1, 808);
    assert_eq!(
        MAX_CANONICAL_POLICY3_EXECUTION_RECEIPT_BYTES_V1,
        4 * 1024 * 1024
    );
    assert_eq!(
        checked_length(4 * 1024 * 1024 - 808).unwrap(),
        4 * 1024 * 1024
    );
    assert!(matches!(
        checked_length(4 * 1024 * 1024 - 807),
        Err(E::Limit)
    ));
    assert!(matches!(checked_length(usize::MAX), Err(E::Limit)));
    let (input, checked) = prepared();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(PREFIX + input.1 + checked.storage().retained_storage())
        .unwrap();
    let receipt =
        encode_checked_canonical_policy3_execution_receipt_v1(&input.0, &checked, &mut budget)
            .unwrap();
    budget
        .reserve_storage(receipt.storage().retained_storage())
        .unwrap();
    for offset in [0, 8, 10, 12, 16, 24] {
        let mut bad = receipt.canonical_bytes().to_vec();
        bad[offset] ^= 1;
        assert!(matches!(
            validate_header(&bad, &checked, &mut budget),
            Err(E::Header) | Err(E::Limit)
        ));
    }
    let mut trailing = receipt.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(matches!(
        validate_header(&trailing, &checked, &mut budget),
        Err(E::Header)
    ));
    trailing[24..32].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        validate_header(&trailing, &checked, &mut budget),
        Err(E::Limit)
    ));
    let mut semantic = receipt.canonical_bytes().to_vec();
    semantic[808] ^= 1;
    assert!(matches!(
        decode_and_check_canonical_policy3_execution_receipt_v1(
            &input.0,
            &checked,
            &semantic,
            &mut budget
        ),
        Err(E::Semantic(_))
    ));
}

#[test]
fn execution_record_comparison_has_independent_exact_and_short_work_prefix() {
    let (input, checked) = prepared();
    let mut prep_work = Work::new(WORK);
    let mut prep_budget = Budget::new(&mut prep_work, STORAGE);
    prep_budget
        .reserve_storage(input.1 + checked.storage().retained_storage())
        .unwrap();
    let receipt =
        encode_checked_canonical_policy3_execution_receipt_v1(&input.0, &checked, &mut prep_budget)
            .unwrap();
    let mut bad = receipt.canonical_bytes().to_vec();
    bad[32] = 2; // Deliberate unchanged-B/O cross-policy substitution.
    for short in [false, true] {
        // Fixed header bytes32 + full execution record776; not a measured cap.
        let mut work = Work::new(PRIOR + 808 - usize::from(short));
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR).unwrap();
        let floor = PREFIX + input.1 + checked.storage().retained_storage() + bad.len();
        budget.reserve_storage(floor).unwrap();
        let result = decode_and_check_canonical_policy3_execution_receipt_v1(
            &input.0,
            &checked,
            &bad,
            &mut budget,
        );
        if short {
            assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
        } else {
            assert!(matches!(result, Err(E::ExecutionWitness)));
        }
        assert_eq!(budget.work(), PRIOR + if short { 32 } else { 808 });
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        if short {
            assert_eq!(work.failed_work(), Some(PRIOR + 808));
        }
    }
}

#[test]
fn policy3_initial_denial_does_not_reset_history_or_return_an_output() {
    let input = admit(&source());
    let mut work = Work::new(PRIOR);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(PRIOR).unwrap();
    budget.reserve_storage(PREFIX + input.1).unwrap();
    assert!(optimize_checked_canonical_kernel_ir_policy3_v1(&input.0, &mut budget).is_err());
    assert_eq!(budget.work(), PRIOR);
    assert_eq!(budget.storage(), PREFIX + input.1);
    assert_eq!(budget.peak_storage(), PREFIX + input.1);
}

#[test]
fn wire_allocation_component_has_exact_and_one_short_live_storage() {
    let required = size_of::<InertCanonicalPolicy3ExecutionReceiptV1>() + 19;
    for short in [false, true] {
        let mut work = Work::new(PRIOR + 2);
        let mut budget = Budget::new(&mut work, PREFIX + required - usize::from(short));
        budget.charge_work(PRIOR).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let result = scoped(&mut budget, |budget| {
            let bytes = allocate(19, budget)?;
            assert_eq!(bytes.capacity(), 19);
            drop(bytes);
            Ok(())
        });
        if short {
            assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
        } else {
            result.unwrap();
        }
        assert_eq!(budget.work(), PRIOR + 2);
        assert_eq!(budget.storage(), PREFIX);
        assert_eq!(
            budget.peak_storage(),
            PREFIX + if short { 0 } else { required }
        );
        assert_eq!(budget.failed_storage(), short.then_some(PREFIX + required));
    }
}

#[test]
fn actual_foreign_sealed_execution_and_full_input_bytes_cannot_substitute() {
    let (input, checked) = prepared();
    let foreign = admit(&Module::new("n"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(PREFIX + input.1 + foreign.1 + checked.storage().retained_storage())
        .unwrap();
    let other = optimize_checked_canonical_kernel_ir_policy3_v1(&foreign.0, &mut budget).unwrap();
    budget
        .reserve_storage(other.storage().retained_storage())
        .unwrap();
    let receipt =
        encode_checked_canonical_policy3_execution_receipt_v1(&input.0, &checked, &mut budget)
            .unwrap();
    budget
        .reserve_storage(receipt.storage().retained_storage())
        .unwrap();
    let floor = budget.storage();
    assert!(matches!(
        decode_and_check_canonical_policy3_execution_receipt_v1(
            &input.0,
            &other,
            receipt.canonical_bytes(),
            &mut budget
        ),
        Err(E::ExecutionWitness)
    ));
    assert!(matches!(
        decode_and_check_canonical_policy3_execution_receipt_v1(
            &foreign.0,
            &checked,
            receipt.canonical_bytes(),
            &mut budget
        ),
        Err(E::InputHistory)
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn matching_execution_record_does_not_admit_a_valid_foreign_semantic_body() {
    use fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1;

    // Admission is fixture setup. Both actual executions and all receipt checks
    // below share this ledger; no whole-admission exact resource cap is claimed.
    let input = admit(&Module::new("m"));
    let foreign = admit(&Module::new("n"));
    let (input_storage, foreign_storage) = (input.1, foreign.1);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(PRIOR).unwrap();
    budget
        .reserve_storage(PREFIX + input.1 + foreign.1)
        .unwrap();
    let checked = optimize_checked_canonical_kernel_ir_policy3_v1(&input.0, &mut budget).unwrap();
    let checked_storage = checked.storage().retained_storage();
    budget.reserve_storage(checked_storage).unwrap();
    let other = optimize_checked_canonical_kernel_ir_policy3_v1(&foreign.0, &mut budget).unwrap();
    let other_storage = other.storage().retained_storage();
    budget.reserve_storage(other_storage).unwrap();
    let wire =
        encode_checked_canonical_policy3_execution_receipt_v1(&input.0, &checked, &mut budget)
            .unwrap();
    let wire_storage = wire.storage().retained_storage();
    budget.reserve_storage(wire_storage).unwrap();
    let foreign_wire =
        encode_checked_canonical_policy3_execution_receipt_v1(&foreign.0, &other, &mut budget)
            .unwrap();
    let foreign_wire_storage = foreign_wire.storage().retained_storage();
    budget.reserve_storage(foreign_wire_storage).unwrap();

    // Prove the transplanted body comes from a valid independently checked
    // receipt, not a malformed semantic codec fixture.
    let genuine = decode_and_check_canonical_policy3_execution_receipt_v1(
        &foreign.0,
        &other,
        foreign_wire.canonical_bytes(),
        &mut budget,
    )
    .unwrap();
    let genuine_storage = genuine.storage().retained_storage();
    budget.reserve_storage(genuine_storage).unwrap();
    assert!(std::ptr::eq(genuine.execution_owner(), &other));
    drop(genuine);
    budget.release_storage(genuine_storage).unwrap();

    const HEADER: usize = CANONICAL_POLICY3_EXECUTION_RECEIPT_HEADER_V1;
    assert_eq!(
        wire.canonical_bytes().len(),
        foreign_wire.canonical_bytes().len()
    );
    assert_eq!(
        &wire.canonical_bytes()[..32],
        &foreign_wire.canonical_bytes()[..32]
    );
    assert_ne!(
        &wire.canonical_bytes()[HEADER..],
        &foreign_wire.canonical_bytes()[HEADER..]
    );
    let length = wire.canonical_bytes().len();
    budget
        .reserve_storage(size_of::<Vec<u8>>() + length)
        .unwrap();
    let mut hybrid = Vec::new();
    hybrid.try_reserve_exact(length).unwrap();
    budget.reserve_storage(hybrid.capacity() - length).unwrap();
    budget.charge_work(length).unwrap();
    hybrid.extend_from_slice(&wire.canonical_bytes()[..HEADER]);
    hybrid.extend_from_slice(&foreign_wire.canonical_bytes()[HEADER..]);
    let hybrid_storage = size_of::<Vec<u8>>() + hybrid.capacity();
    assert_eq!(&hybrid[..HEADER], &wire.canonical_bytes()[..HEADER]);
    assert_eq!(
        validate_header(&hybrid, &checked, &mut budget).unwrap(),
        &foreign_wire.canonical_bytes()[HEADER..]
    );
    let floor = budget.storage();
    let prefix = budget.work();
    assert!(matches!(
        decode_and_check_canonical_policy3_execution_receipt_v1(
            &input.0,
            &checked,
            &hybrid,
            &mut budget
        ),
        Err(E::Semantic(SemanticError::Transition(
            CanonicalKirTransitionErrorV1::Rule("transition receipt endpoint or policy")
        )))
    ));
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > prefix);
    assert_eq!(budget.failed_storage(), None);
    drop(hybrid);
    budget.release_storage(hybrid_storage).unwrap();
    drop(foreign_wire);
    budget.release_storage(foreign_wire_storage).unwrap();
    drop(wire);
    budget.release_storage(wire_storage).unwrap();
    drop(other);
    budget.release_storage(other_storage).unwrap();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(foreign);
    budget.release_storage(foreign_storage).unwrap();
    drop(input);
    budget.release_storage(input_storage).unwrap();
    assert_eq!(budget.storage(), PREFIX);
}
