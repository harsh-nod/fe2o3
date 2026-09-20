use super::*;
use crate::commutative_bitwise_cse_v1::{
    CommutativeBitwiseDominanceCsePassV1, commutative_bitwise_dominance_cse_v1,
    commutative_bitwise_dominance_cse_with_observer_v1,
};
use crate::optimization_v1::IndexType;
use pliron::builtin::types::FP32Type;

#[pliron::derive::pliron_attr(
    name = "gpu.commutative_bitwise_collision_test_v1",
    format = "$0",
    verifier = "succ"
)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct CollidingAttr(u32);

impl Hash for CollidingAttr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0u8.hash(state);
    }
}

fn binary(
    context: &mut Context,
    block: Ptr<BasicBlock>,
    kind: BinaryKindAttr,
    lhs: Value,
    rhs: Value,
) -> Ptr<Operation> {
    let operation = BinaryOp::new(context, kind, lhs, rhs);
    append(context, block, operation)
}

struct Pair {
    function: FuncOp,
    anchor: Ptr<Operation>,
    duplicate: Ptr<Operation>,
    returned: Ptr<Operation>,
}

fn pair(context: &mut Context, scalar: TypeHandle, kind: BinaryKindAttr) -> Pair {
    let function = function(context, "swapped", vec![scalar, scalar], vec![scalar]);
    let entry = function.get_entry_block(context);
    let next = block(context, &function);
    let lhs = entry.deref(context).get_argument(0);
    let rhs = entry.deref(context).get_argument(1);
    let anchor = binary(context, entry, kind, lhs, rhs);
    let branch = BranchOp::new(context, next, vec![]);
    append(context, entry, branch);
    let duplicate = binary(context, next, kind, rhs, lhs);
    let value = duplicate.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![value]);
    let returned = append(context, next, returned);
    Pair {
        function,
        anchor,
        duplicate,
        returned,
    }
}

fn run_commutative(context: &mut Context, function: &FuncOp) -> IRStatus {
    verify_op(function, context).unwrap();
    let cfg = |context: &Context| {
        function
            .get_region(context)
            .deref(context)
            .iter(context)
            .map(|block| {
                let terminator = block.deref(context).get_tail().unwrap();
                let successors = terminator.deref(context).successors().collect::<Vec<_>>();
                (block, terminator, successors)
            })
            .collect::<Vec<_>>()
    };
    let before = cfg(context);
    let mut ledger = Ledger::generous();
    let status = commutative_bitwise_dominance_cse_v1(
        function.get_operation(),
        context,
        &mut DomInfo::default(),
        &mut ledger,
    )
    .unwrap();
    assert_eq!(ledger.live, 0);
    verify_op(function, context).unwrap();
    assert_eq!(cfg(context), before);
    status
}

#[test]
fn all_fixed_widths_and_bitwise_kinds_are_new_positive_but_old_positional_negative() {
    for signedness in [Signedness::Signed, Signedness::Unsigned] {
        for width in [8, 16, 32, 64] {
            for kind in [
                BinaryKindAttr::BitAnd,
                BinaryKindAttr::BitOr,
                BinaryKindAttr::BitXor,
            ] {
                let context = &mut Context::new();
                let scalar = IntegerType::get(context, width, signedness).into();
                let fixture = pair(context, scalar, kind);
                let original_operands =
                    fixture.anchor.deref(context).operands().collect::<Vec<_>>();
                let original_block = fixture.anchor.deref(context).get_parent_block();
                assert_eq!(run(context, &fixture.function), IRStatus::Unchanged);
                assert!(fixture.duplicate.try_deref(context).is_ok());
                assert_eq!(
                    run_commutative(context, &fixture.function),
                    IRStatus::Changed
                );
                assert!(fixture.duplicate.try_deref(context).is_err());
                assert_eq!(
                    fixture.anchor.deref(context).get_parent_block(),
                    original_block
                );
                assert_eq!(
                    fixture.anchor.deref(context).operands().collect::<Vec<_>>(),
                    original_operands
                );
                assert_eq!(
                    fixture.returned.deref(context).get_operand(0),
                    fixture.anchor.deref(context).get_result(0)
                );
                assert_eq!(
                    run_commutative(context, &fixture.function),
                    IRStatus::Unchanged
                );
            }
        }
    }
}

#[test]
fn actual_replacements_enable_a_later_swapped_chain_without_reassociation() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let function = function(context, "chain", vec![scalar, scalar, scalar], vec![scalar]);
    let entry = function.get_entry_block(context);
    let next = block(context, &function);
    let a = entry.deref(context).get_argument(0);
    let b = entry.deref(context).get_argument(1);
    let c = entry.deref(context).get_argument(2);
    let first = binary(context, entry, BinaryKindAttr::BitAnd, a, b);
    let first_value = first.deref(context).get_result(0);
    let second = binary(context, entry, BinaryKindAttr::BitOr, first_value, c);
    let branch = BranchOp::new(context, next, vec![]);
    append(context, entry, branch);
    let duplicate_first = binary(context, next, BinaryKindAttr::BitAnd, b, a);
    let duplicate_value = duplicate_first.deref(context).get_result(0);
    let duplicate_second = binary(context, next, BinaryKindAttr::BitOr, c, duplicate_value);
    let value = duplicate_second.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![value]);
    let returned = append(context, next, returned);
    assert_eq!(run(context, &function), IRStatus::Unchanged);
    assert_eq!(run_commutative(context, &function), IRStatus::Changed);
    for operation in [duplicate_first, duplicate_second] {
        assert!(operation.try_deref(context).is_err());
    }
    assert_eq!(
        returned.deref(context).get_operand(0),
        second.deref(context).get_result(0)
    );
    assert_eq!(run_commutative(context, &function), IRStatus::Unchanged);
}

#[test]
fn reassociation_and_unrelated_value_numbers_are_not_the_relation() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let function = function(
        context,
        "associative_negative",
        vec![scalar, scalar, scalar],
        vec![scalar, scalar],
    );
    let entry = function.get_entry_block(context);
    let a = entry.deref(context).get_argument(0);
    let b = entry.deref(context).get_argument(1);
    let c = entry.deref(context).get_argument(2);
    let bc = binary(context, entry, BinaryKindAttr::BitAnd, b, c);
    let bc_value = bc.deref(context).get_result(0);
    let abc = binary(context, entry, BinaryKindAttr::BitAnd, a, bc_value);
    let ab = binary(context, entry, BinaryKindAttr::BitAnd, a, b);
    let ab_value = ab.deref(context).get_result(0);
    let abc_other = binary(context, entry, BinaryKindAttr::BitAnd, ab_value, c);
    let values = vec![
        abc.deref(context).get_result(0),
        abc_other.deref(context).get_result(0),
    ];
    let returned = ReturnOp::new(context, values);
    append(context, entry, returned);
    assert_eq!(run_commutative(context, &function), IRStatus::Unchanged);
    for operation in [bc, abc, ab, abc_other] {
        assert!(operation.try_deref(context).is_ok());
    }
}

#[test]
fn siblings_join_and_backedge_are_not_dominance_evidence() {
    let context = &mut Context::new();
    let fixture = diamond(context, false);
    assert_eq!(
        run_commutative(context, &fixture.function),
        IRStatus::Unchanged
    );
    for operation in [fixture.left, fixture.right, fixture.join] {
        assert!(operation.try_deref(context).is_ok());
    }
    for seed_at_entry in [false, true] {
        let context = &mut Context::new();
        let scalar = u32_type(context);
        let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
        let function = function(context, "loop", vec![boolean, scalar, scalar], vec![scalar]);
        let entry = function.get_entry_block(context);
        let header = block(context, &function);
        let body = block(context, &function);
        let exit = block(context, &function);
        let condition = entry.deref(context).get_argument(0);
        let a = entry.deref(context).get_argument(1);
        let b = entry.deref(context).get_argument(2);
        let seed = seed_at_entry.then(|| binary(context, entry, BinaryKindAttr::BitXor, a, b));
        let branch = BranchOp::new(context, header, vec![]);
        append(context, entry, branch);
        let header_value = binary(context, header, BinaryKindAttr::BitXor, b, a);
        let branch = CondBranchOp::new(context, condition, body, vec![], exit, vec![]);
        append(context, header, branch);
        let body_value = binary(context, body, BinaryKindAttr::BitXor, a, b);
        let branch = BranchOp::new(context, header, vec![]);
        append(context, body, branch);
        let value = header_value.deref(context).get_result(0);
        let returned = ReturnOp::new(context, vec![value]);
        let returned = append(context, exit, returned);
        assert_eq!(run_commutative(context, &function), IRStatus::Changed);
        assert_eq!(header_value.try_deref(context).is_ok(), !seed_at_entry);
        assert!(body_value.try_deref(context).is_err());
        assert_eq!(
            returned.deref(context).get_operand(0),
            seed.unwrap_or(header_value).deref(context).get_result(0)
        );
    }
}

#[test]
fn unreachable_and_non_ssa_regions_keep_their_operations() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let function = function(context, "dead", vec![scalar, scalar], vec![scalar]);
    let entry = function.get_entry_block(context);
    let a = entry.deref(context).get_argument(0);
    let b = entry.deref(context).get_argument(1);
    let anchor = binary(context, entry, BinaryKindAttr::BitAnd, a, b);
    let value = anchor.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![value]);
    append(context, entry, returned);
    let dead = BasicBlock::new(context, None, vec![scalar, scalar]);
    dead.insert_at_back(function.get_region(context), context);
    let dead_a = dead.deref(context).get_argument(0);
    let dead_b = dead.deref(context).get_argument(1);
    let first = binary(context, dead, BinaryKindAttr::BitAnd, dead_a, dead_b);
    let second = binary(context, dead, BinaryKindAttr::BitAnd, dead_b, dead_a);
    let value = second.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![value]);
    append(context, dead, returned);
    assert_eq!(run_commutative(context, &function), IRStatus::Unchanged);
    for operation in [first, second] {
        assert!(operation.try_deref(context).is_ok());
    }

    let context = &mut Context::new();
    let module = ModuleOp::new(context, Identifier::try_from("graph").unwrap());
    let a = ConstantOp::new(context, integer(context, 5));
    let a_value = a.get_operation().deref(context).get_result(0);
    module.append_operation(context, a.get_operation(), 0);
    let b = ConstantOp::new(context, integer(context, 7));
    let b_value = b.get_operation().deref(context).get_result(0);
    module.append_operation(context, b.get_operation(), 0);
    let first = BinaryOp::new(context, BinaryKindAttr::BitAnd, a_value, b_value).get_operation();
    module.append_operation(context, first, 0);
    let second = BinaryOp::new(context, BinaryKindAttr::BitAnd, b_value, a_value).get_operation();
    module.append_operation(context, second, 0);
    verify_op(&module, context).unwrap();
    let mut ledger = Ledger::generous();
    assert_eq!(
        commutative_bitwise_dominance_cse_v1(
            module.get_operation(),
            context,
            &mut DomInfo::default(),
            &mut ledger
        )
        .unwrap(),
        IRStatus::Unchanged
    );
    assert_eq!(ledger.live, 0);
    for operation in [first, second] {
        assert!(operation.try_deref(context).is_ok());
    }
    verify_op(&module, context).unwrap();
}

#[test]
fn complete_attributes_kinds_and_types_remain_distinct() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let fixture = pair(context, scalar, BinaryKindAttr::BitAnd);
    fixture
        .duplicate
        .deref_mut(context)
        .attributes
        .0
        .insert(Identifier::try_from("extra").unwrap(), integer(context, 1));
    assert_eq!(
        run_commutative(context, &fixture.function),
        IRStatus::Unchanged
    );
    assert!(fixture.duplicate.try_deref(context).is_ok());

    let context = &mut Context::new();
    let scalar = u32_type(context);
    let other = IntegerType::get(context, 32, Signedness::Signed).into();
    let function = function(
        context,
        "typed",
        vec![scalar, scalar, other, other],
        vec![scalar],
    );
    let entry = function.get_entry_block(context);
    let a = entry.deref(context).get_argument(0);
    let b = entry.deref(context).get_argument(1);
    let c = entry.deref(context).get_argument(2);
    let d = entry.deref(context).get_argument(3);
    let and = binary(context, entry, BinaryKindAttr::BitAnd, a, b);
    let or = binary(context, entry, BinaryKindAttr::BitOr, b, a);
    let signed = binary(context, entry, BinaryKindAttr::BitAnd, c, d);
    let returned = ReturnOp::new(context, vec![a]);
    append(context, entry, returned);
    assert_eq!(run_commutative(context, &function), IRStatus::Unchanged);
    for operation in [and, or, signed] {
        assert!(operation.try_deref(context).is_ok());
    }
}

#[test]
fn full_equal_attribute_maps_hash_equally_without_ordered_operands() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let fixture = pair(context, scalar, BinaryKindAttr::BitOr);
    for (operation, names) in [
        (fixture.anchor, ["a", "b"]),
        (fixture.duplicate, ["b", "a"]),
    ] {
        for name in names {
            operation
                .deref_mut(context)
                .attributes
                .0
                .insert(Identifier::try_from(name).unwrap(), integer(context, 7));
        }
    }
    let a = crate::commutative_bitwise_cse_v1::key(fixture.anchor, context).unwrap();
    let b = crate::commutative_bitwise_cse_v1::key(fixture.duplicate, context).unwrap();
    assert!(!a.exactly_equal(b, context));
    assert!(crate::commutative_bitwise_cse_v1::equal(a, b, context));
    assert_eq!(
        crate::commutative_bitwise_cse_v1::fingerprint(a, context),
        crate::commutative_bitwise_cse_v1::fingerprint(b, context)
    );
    assert_eq!(
        run_commutative(context, &fixture.function),
        IRStatus::Changed
    );
}

#[test]
fn forced_collision_checks_full_keys_with_exact_and_one_short_postmutation_work() {
    // The unchanged positional collision fixture costs 101 units. Four keys
    // inspected add 16, three hashes add 6, three full comparisons add 6.
    const WORK: usize = 129;
    for allowed in [WORK, WORK - 1] {
        let context = &mut Context::new();
        let scalar = u32_type(context);
        let function = function(
            context,
            "collision",
            vec![scalar, scalar],
            vec![scalar, scalar],
        );
        let entry = function.get_entry_block(context);
        let a = entry.deref(context).get_argument(0);
        let b = entry.deref(context).get_argument(1);
        let first = binary(context, entry, BinaryKindAttr::BitAnd, a, b);
        let unequal = binary(context, entry, BinaryKindAttr::BitAnd, a, b);
        let duplicate = binary(context, entry, BinaryKindAttr::BitAnd, b, a);
        for (operation, payload) in [(first, 10), (unequal, 20), (duplicate, 10)] {
            operation.deref_mut(context).attributes.0.insert(
                Identifier::try_from("collision_payload").unwrap(),
                Box::new(CollidingAttr(payload)),
            );
        }
        let first_key = crate::commutative_bitwise_cse_v1::key(first, context).unwrap();
        let unequal_key = crate::commutative_bitwise_cse_v1::key(unequal, context).unwrap();
        let duplicate_key = crate::commutative_bitwise_cse_v1::key(duplicate, context).unwrap();
        assert!(!crate::commutative_bitwise_cse_v1::equal(
            first_key,
            unequal_key,
            context
        ));
        assert!(crate::commutative_bitwise_cse_v1::equal(
            first_key,
            duplicate_key,
            context
        ));
        assert_eq!(
            crate::commutative_bitwise_cse_v1::fingerprint(first_key, context),
            crate::commutative_bitwise_cse_v1::fingerprint(unequal_key, context)
        );
        assert_eq!(
            crate::commutative_bitwise_cse_v1::fingerprint(first_key, context),
            crate::commutative_bitwise_cse_v1::fingerprint(duplicate_key, context)
        );
        let values = vec![
            duplicate.deref(context).get_result(0),
            unequal.deref(context).get_result(0),
        ];
        let returned = ReturnOp::new(context, values);
        let returned = append(context, entry, returned);
        verify_op(&function, context).unwrap();
        let mut ledger = Ledger::generous();
        ledger.live = 97;
        ledger.work = 7;
        ledger.work_limit = 7 + allowed;
        let result = commutative_bitwise_dominance_cse_v1(
            function.get_operation(),
            context,
            &mut DomInfo::default(),
            &mut ledger,
        );
        if allowed == WORK {
            assert_eq!(result.unwrap(), IRStatus::Changed);
            assert_eq!(ledger.work, 7 + WORK);
        } else {
            let error = Denied {
                storage: false,
                prior: 7 + WORK - 1,
                request: 1,
            };
            assert!(
                matches!(result, Err(DominanceCseErrorV1::Budget(ref actual)) if actual == &error)
            );
            assert_eq!(ledger.failure, Some(error));
        }
        assert_eq!(ledger.live, 97);
        assert!(first.try_deref(context).is_ok());
        assert!(unequal.try_deref(context).is_ok());
        assert!(duplicate.try_deref(context).is_err());
        assert_eq!(
            returned.deref(context).get_operand(0),
            first.deref(context).get_result(0)
        );
        assert_eq!(
            returned.deref(context).get_operand(1),
            unequal.deref(context).get_result(0)
        );
        verify_op(&function, context).unwrap();
        // The one-short result is a refused, already-mutated raw candidate.
    }
}

#[test]
fn distinct_structured_keys_keep_linear_charged_work() {
    for count in [64, 256, 1024] {
        let context = &mut Context::new();
        let scalar = u32_type(context);
        let function = function(context, "scaling", vec![scalar, scalar], vec![scalar]);
        let entry = function.get_entry_block(context);
        let a = entry.deref(context).get_argument(0);
        let b = entry.deref(context).get_argument(1);
        for index in 0..count {
            let constant = ConstantOp::new(context, integer(context, index as u32));
            append(context, entry, constant);
            let operation = binary(context, entry, BinaryKindAttr::BitAnd, a, b);
            operation.deref_mut(context).attributes.0.insert(
                Identifier::try_from("discriminator").unwrap(),
                integer(context, index as u32),
            );
        }
        let returned = ReturnOp::new(context, vec![a]);
        append(context, entry, returned);
        verify_op(&function, context).unwrap();
        let mut ledger = Ledger::generous();
        ledger.work_limit = 96 + 96 * count;
        assert_eq!(
            commutative_bitwise_dominance_cse_v1(
                function.get_operation(),
                context,
                &mut DomInfo::default(),
                &mut ledger
            )
            .unwrap(),
            IRStatus::Unchanged
        );
        assert!(ledger.work <= 96 + 96 * count);
        assert_eq!(ledger.live, 0);
    }
}

#[test]
fn bool_index_signless_wide_float_traps_and_checked_arithmetic_are_excluded() {
    for (width, signedness) in [
        (1, Signedness::Signless),
        (32, Signedness::Signless),
        (128, Signedness::Signed),
        (128, Signedness::Unsigned),
    ] {
        let context = &mut Context::new();
        let ty = IntegerType::get(context, width, signedness).into();
        let fixture = pair(context, ty, BinaryKindAttr::BitAnd);
        assert_eq!(
            run_commutative(context, &fixture.function),
            IRStatus::Unchanged
        );
    }
    for kind in [
        BinaryKindAttr::Add,
        BinaryKindAttr::Subtract,
        BinaryKindAttr::Multiply,
        BinaryKindAttr::Divide,
        BinaryKindAttr::Remainder,
        BinaryKindAttr::ShiftLeft,
        BinaryKindAttr::ShiftRight,
        BinaryKindAttr::CheckedAdd,
        BinaryKindAttr::CheckedSubtract,
        BinaryKindAttr::CheckedMultiply,
    ] {
        let context = &mut Context::new();
        let ty = u32_type(context);
        let fixture = pair(context, ty, kind);
        assert_eq!(
            run_commutative(context, &fixture.function),
            IRStatus::Unchanged
        );
    }
    let context = &mut Context::new();
    let ty = IndexType::get(context).into();
    let fixture = pair(context, ty, BinaryKindAttr::BitXor);
    assert_eq!(
        run_commutative(context, &fixture.function),
        IRStatus::Unchanged
    );
    let context = &mut Context::new();
    let ty = FP32Type::get(context).into();
    let fixture = pair(context, ty, BinaryKindAttr::Add);
    assert_eq!(
        run_commutative(context, &fixture.function),
        IRStatus::Unchanged
    );
}

#[test]
fn effects_and_convergence_are_retained_while_total_bitwise_values_can_cross_them() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let pointer = PointerType::get(
        context,
        scalar,
        AddressSpaceAttr::Global,
        AccessModeAttr::ReadWrite,
    )
    .into();
    let function = function(
        context,
        "effects",
        vec![scalar, scalar, pointer],
        vec![scalar],
    );
    let entry = function.get_entry_block(context);
    let a = entry.deref(context).get_argument(0);
    let b = entry.deref(context).get_argument(1);
    let ptr = entry.deref(context).get_argument(2);
    let anchor = binary(context, entry, BinaryKindAttr::BitXor, a, b);
    let mut retained = Vec::new();
    for _ in 0..2 {
        let call = CallOp::new(context, "external", vec![a], vec![scalar]);
        retained.push(append(context, entry, call));
        for volatile in [false, true] {
            let load = LoadOp::new(context, ptr, 4, volatile).unwrap();
            retained.push(append(context, entry, load));
            let store = StoreOp::new(context, ptr, a, 4, volatile).unwrap();
            retained.push(append(context, entry, store));
        }
        for kind in [
            PreservedOperationKindAttr::Intrinsic,
            PreservedOperationKindAttr::MemoryIntrinsic,
            PreservedOperationKindAttr::Barrier,
            PreservedOperationKindAttr::Atomic,
            PreservedOperationKindAttr::Fence,
            PreservedOperationKindAttr::WorkgroupBarrier,
            PreservedOperationKindAttr::WorkgroupMemory,
            PreservedOperationKindAttr::Matrix,
            PreservedOperationKindAttr::Wave,
            PreservedOperationKindAttr::InlineAssembly,
        ] {
            let op = PreservedOperationOp::new(context, kind, vec![], vec![]);
            retained.push(append(context, entry, op));
        }
    }
    let duplicate = binary(context, entry, BinaryKindAttr::BitXor, b, a);
    let value = duplicate.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![value]);
    let returned = append(context, entry, returned);
    assert_eq!(run_commutative(context, &function), IRStatus::Changed);
    assert!(duplicate.try_deref(context).is_err());
    assert_eq!(
        returned.deref(context).get_operand(0),
        anchor.deref(context).get_result(0)
    );
    let actual = entry
        .deref(context)
        .iter(context)
        .filter(|op| retained.contains(op))
        .collect::<Vec<_>>();
    assert_eq!(actual, retained);
}

#[test]
fn empty_cfg_exact_and_one_short_budgets_preserve_unrelated_floor() {
    // The old empty case is exactly 14 work units. The closed key selector adds
    // four prepaid visits for the sole Return, with no eligible-key allocation.
    const WORK: usize = 18;
    let storage = size_of::<Vec<Ptr<Operation>>>()
        + 4 * size_of::<Ptr<Operation>>()
        + size_of::<HashMap<u64, usize>>()
        + size_of::<Vec<Available>>()
        + size_of::<Vec<Visit>>()
        + size_of::<Vec<Value>>()
        + 4 * size_of::<Visit>();
    for (work_limit, storage_limit, succeeds) in [
        (WORK, storage, true),
        (WORK - 1, storage, false),
        (WORK, storage - 1, false),
    ] {
        let context = &mut Context::new();
        let function = empty_function(context);
        let mut ledger = Ledger::generous();
        ledger.work = 11;
        ledger.live = 97;
        ledger.peak = 97;
        ledger.work_limit = 11 + work_limit;
        ledger.storage_limit = 97 + storage_limit;
        let result = commutative_bitwise_dominance_cse_v1(
            function.get_operation(),
            context,
            &mut DomInfo::default(),
            &mut ledger,
        );
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(ledger.live, 97);
        if succeeds {
            assert_eq!(ledger.work, 11 + WORK);
            assert_eq!(ledger.peak, 97 + storage);
        } else {
            let DominanceCseErrorV1::Budget(error) = result.unwrap_err() else {
                panic!("caller denial")
            };
            assert_eq!(Some(error), ledger.failure);
        }
    }
}

#[test]
fn denial_after_actual_swapped_erasure_and_observer_unwind_drop_local_storage() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let fixture = pair(context, scalar, BinaryKindAttr::BitAnd);
    let erased = Arc::new(AtomicBool::new(false));
    let mut budget = AfterErasureBudget {
        ledger: Ledger::generous(),
        erased: erased.clone(),
    };
    budget.ledger.live = 97;
    let error = commutative_bitwise_dominance_cse_with_observer_v1(
        fixture.function.get_operation(),
        context,
        &mut DomInfo::default(),
        &mut budget,
        Box::new(ErasureGate(erased)),
    )
    .unwrap_err();
    let DominanceCseErrorV1::Budget(error) = error else {
        panic!("caller denial")
    };
    assert_eq!(Some(error.clone()), budget.ledger.failure);
    assert_eq!(budget.ledger.work, error.prior);
    assert_eq!(budget.ledger.live, 97);
    assert!(fixture.duplicate.try_deref(context).is_err());
    // This raw candidate has mutated and is discarded, not promoted to an owner.
    let mut context = Context::new();
    let scalar = u32_type(&context);
    let fixture = pair(&mut context, scalar, BinaryKindAttr::BitAnd);
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut ledger = Ledger::generous();
    ledger.live = 97;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        commutative_bitwise_dominance_cse_with_observer_v1(
            fixture.function.get_operation(),
            &mut context,
            &mut DomInfo::default(),
            &mut ledger,
            Box::new(Observer {
                events: events.clone(),
                panic_after_erase: true,
            }),
        )
    }));
    assert!(result.is_err());
    assert_eq!(ledger.live, 97);
    assert_eq!(*events.lock().unwrap(), [(0, true), (1, true)]);
    drop(context);
}

#[test]
fn closed_pass_adapter_preserves_typed_denial_and_deterministic_representatives() {
    let context = &mut Context::new();
    let function = empty_function(context);
    let mut ledger = Ledger::generous();
    ledger.work_limit = 0;
    ledger.live = 97;
    let error = CommutativeBitwiseDominanceCsePassV1::new(&mut ledger)
        .run(
            function.get_operation(),
            context,
            &mut AnalysisManager::default(),
        )
        .err()
        .unwrap();
    let error = error
        .err
        .downcast_ref::<DominanceCseErrorV1<Denied>>()
        .unwrap();
    let DominanceCseErrorV1::Budget(error) = error else {
        panic!("caller denial")
    };
    assert_eq!(Some(error.clone()), ledger.failure);
    assert_eq!(ledger.live, 97);
    let mut expected = None;
    for _ in 0..8 {
        let context = &mut Context::new();
        let scalar = u32_type(context);
        let fixture = pair(context, scalar, BinaryKindAttr::BitXor);
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut ledger = Ledger::generous();
        commutative_bitwise_dominance_cse_with_observer_v1(
            fixture.function.get_operation(),
            context,
            &mut DomInfo::default(),
            &mut ledger,
            Box::new(Observer {
                events: events.clone(),
                panic_after_erase: false,
            }),
        )
        .unwrap();
        let actual = (events.lock().unwrap().clone(), ledger.work, ledger.peak);
        assert_eq!(actual.0, [(0, true), (1, true)]);
        if let Some(expected) = &expected {
            assert_eq!(&actual, expected);
        } else {
            expected = Some(actual);
        }
        assert_eq!(ledger.live, 0);
        verify_op(&fixture.function, context).unwrap();
    }
}
