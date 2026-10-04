use super::*;

#[pliron::derive::pliron_attr(
    name = "gpu.dominance_cse_collision_test_v1",
    format = "$0",
    verifier = "succ"
)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct CollidingAttr(u32);

impl std::hash::Hash for CollidingAttr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Deliberately identical for distinct payloads; equality remains exact.
        std::hash::Hash::hash(&0u8, state);
    }
}

fn tag(context: &Context, operation: Ptr<Operation>, payload: u32) {
    operation.deref_mut(context).attributes.0.insert(
        Identifier::try_from("collision_payload").unwrap(),
        Box::new(CollidingAttr(payload)),
    );
}

fn assert_colliding_keys(context: &Context, left: Ptr<Operation>, right: Ptr<Operation>) {
    let left = BorrowedPureCseKeyV1::from_operation(left, context).unwrap();
    let right = BorrowedPureCseKeyV1::from_operation(right, context).unwrap();
    assert!(!left.exactly_equal(right, context));
    assert_eq!(fingerprint(left, context), fingerprint(right, context));
}

#[test]
fn forced_collision_uses_full_equality_and_charges_the_entire_chain() {
    // Empty Func/Return is 14; Return has 2 operands; three width-5 binaries
    // cost 3*(census 1 + scan 1 + width 5 + hash 6). First insertion costs
    // 1+2+2, second costs compare 11 + insert 1, and the duplicate costs two
    // comparisons 22 + result 2 + one use 1 + replacement Vec growth 2.
    // Two retained rows each cost one undo visit: 14+2+39+5+12+27+2 = 101.
    const WORK: usize = 101;
    const HISTORY: usize = 7;
    const FLOOR: usize = 97;
    for allowed in [WORK, WORK - 1] {
        let context = &mut Context::new();
        let scalar = u32_type(context);
        let function = function(
            context,
            "forced_collision",
            vec![scalar, scalar],
            vec![scalar, scalar],
        );
        let entry = function.get_entry_block(context);
        let lhs = entry.deref(context).get_argument(0);
        let rhs = entry.deref(context).get_argument(1);
        let first = bit_and(context, entry, lhs, rhs);
        let unequal = bit_and(context, entry, lhs, rhs);
        let duplicate = bit_and(context, entry, lhs, rhs);
        tag(context, first, 10);
        tag(context, unequal, 20);
        tag(context, duplicate, 10);
        assert_colliding_keys(context, first, unequal);
        assert_colliding_keys(context, duplicate, unequal);
        let first_key = BorrowedPureCseKeyV1::from_operation(first, context).unwrap();
        let duplicate_key = BorrowedPureCseKeyV1::from_operation(duplicate, context).unwrap();
        assert!(first_key.exactly_equal(duplicate_key, context));
        assert_eq!(
            fingerprint(first_key, context),
            fingerprint(duplicate_key, context)
        );
        let returned_values = vec![
            duplicate.deref(context).get_result(0),
            unequal.deref(context).get_result(0),
        ];
        let returned = ReturnOp::new(context, returned_values);
        let returned = append(context, entry, returned);
        verify_op(&function, context).unwrap();
        let mut ledger = Ledger::generous();
        ledger.work = HISTORY;
        ledger.live = FLOOR;
        ledger.work_limit = HISTORY + allowed;
        let result = dominance_pure_cse_v1(
            function.get_operation(),
            context,
            &mut DomInfo::default(),
            &mut ledger,
        );
        if allowed == WORK {
            assert_eq!(result.unwrap(), IRStatus::Changed);
            assert_eq!(ledger.work, HISTORY + WORK);
            assert_eq!(ledger.failure, None);
        } else {
            let error = Denied {
                storage: false,
                prior: HISTORY + WORK - 1,
                request: 1,
            };
            assert!(
                matches!(result, Err(DominanceCseErrorV1::Budget(ref actual)) if actual == &error)
            );
            assert_eq!(ledger.failure, Some(error));
            assert_eq!(ledger.work, HISTORY + WORK - 1);
        }
        assert_eq!(ledger.live, FLOOR);
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
        // The one-short denial occurs after mutation; its private candidate is
        // discarded here, not retried or treated as a successful transform.
    }
}

#[test]
fn forced_collision_scope_undo_retains_siblings_and_restores_the_ancestor() {
    let context = &mut Context::new();
    let fixture = diamond(context, true);
    let ancestor = fixture.earlier.unwrap();
    for (operation, payload) in [
        (ancestor, 10),
        (fixture.left, 20),
        (fixture.right, 20),
        (fixture.join, 10),
    ] {
        tag(context, operation, payload);
    }
    assert_colliding_keys(context, ancestor, fixture.left);
    assert_colliding_keys(context, ancestor, fixture.right);
    let left = BorrowedPureCseKeyV1::from_operation(fixture.left, context).unwrap();
    let right = BorrowedPureCseKeyV1::from_operation(fixture.right, context).unwrap();
    assert!(left.exactly_equal(right, context));
    assert_eq!(run(context, &fixture.function), IRStatus::Changed);
    for retained in [ancestor, fixture.left, fixture.right] {
        assert!(retained.try_deref(context).is_ok());
    }
    assert!(fixture.join.try_deref(context).is_err());
    assert_eq!(
        fixture.returned.deref(context).get_operand(0),
        ancestor.deref(context).get_result(0)
    );
    assert_eq!(run(context, &fixture.function), IRStatus::Unchanged);
}

#[derive(Clone, Copy, Debug)]
enum MalformedPayload {
    UntypedConstant,
    CheckedKindWithOneResult,
    NonBooleanOverflow,
}

#[test]
fn malformed_payloads_of_registered_pure_classes_never_enter_the_key_table() {
    use pliron::common_traits::Verify;

    for shape in [
        MalformedPayload::UntypedConstant,
        MalformedPayload::CheckedKindWithOneResult,
        MalformedPayload::NonBooleanOverflow,
    ] {
        let context = &mut Context::new();
        let scalar = u32_type(context);
        let function = function(
            context,
            "malformed_pure_payload",
            vec![scalar, scalar],
            vec![],
        );
        let entry = function.get_entry_block(context);
        let lhs = entry.deref(context).get_argument(0);
        let rhs = entry.deref(context).get_argument(1);
        let mut malformed = Vec::new();
        for _ in 0..2 {
            let operation = match shape {
                MalformedPayload::UntypedConstant => {
                    let constant = ConstantOp::new(context, integer(context, 1));
                    constant.set_attr_gpu_constant_value(context, Box::new(CollidingAttr(1)));
                    assert!(constant.verify(context).is_err());
                    constant.get_operation()
                }
                MalformedPayload::CheckedKindWithOneResult => {
                    let binary = BinaryOp::new(context, BinaryKindAttr::BitAnd, lhs, rhs);
                    binary.set_attr_gpu_binary_kind(context, BinaryKindAttr::CheckedAdd);
                    assert!(binary.verify(context).is_err());
                    binary.get_operation()
                }
                MalformedPayload::NonBooleanOverflow => {
                    let binary = BinaryOp::new(context, BinaryKindAttr::CheckedAdd, lhs, rhs);
                    binary.overflow(context).unwrap().set_type(context, scalar);
                    assert!(binary.verify(context).is_err());
                    binary.get_operation()
                }
            };
            operation.insert_at_back(entry, context);
            assert!(BorrowedPureCseKeyV1::from_operation(operation, context).is_none());
            malformed.push(operation);
        }
        let returned = ReturnOp::new(context, vec![]);
        append(context, entry, returned);
        assert!(verify_op(&function, context).is_err());
        // Deliberately bypass the normal verified-input prerequisite to probe
        // the concrete candidate verifier, not to admit this malformed graph.
        let mut ledger = Ledger::generous();
        assert_eq!(
            dominance_pure_cse_v1(
                function.get_operation(),
                context,
                &mut DomInfo::default(),
                &mut ledger,
            )
            .unwrap(),
            IRStatus::Unchanged
        );
        assert_eq!(ledger.live, 0);
        assert_eq!(ledger.failure, None);
        for operation in malformed {
            assert!(operation.try_deref(context).is_ok());
            assert!(BorrowedPureCseKeyV1::from_operation(operation, context).is_none());
        }
        assert!(verify_op(&function, context).is_err());
    }
}

#[test]
fn admitted_checked_integer_arithmetic_replaces_both_ordered_results() {
    for kind in [
        BinaryKindAttr::CheckedAdd,
        BinaryKindAttr::CheckedSubtract,
        BinaryKindAttr::CheckedMultiply,
    ] {
        let context = &mut Context::new();
        let scalar = u32_type(context);
        let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
        let function = function(
            context,
            "checked_results",
            vec![scalar, scalar],
            vec![scalar, boolean, boolean],
        );
        let entry = function.get_entry_block(context);
        let next = block(context, &function);
        let lhs = entry.deref(context).get_argument(0);
        let rhs = entry.deref(context).get_argument(1);
        let first = BinaryOp::new(context, kind, lhs, rhs);
        let first = append(context, entry, first);
        assert_eq!(first.deref(context).get_num_results(), 2);
        assert!(BorrowedPureCseKeyV1::from_operation(first, context).is_some());
        let branch = BranchOp::new(context, next, vec![]);
        append(context, entry, branch);
        let duplicate = BinaryOp::new(context, kind, lhs, rhs);
        let duplicate = append(context, next, duplicate);
        let returned_values = vec![
            duplicate.deref(context).get_result(0),
            duplicate.deref(context).get_result(1),
            duplicate.deref(context).get_result(1),
        ];
        let returned = ReturnOp::new(context, returned_values);
        let returned = append(context, next, returned);
        assert_eq!(run(context, &function), IRStatus::Changed);
        assert_eq!(first.deref(context).get_parent_block(), Some(entry));
        assert!(duplicate.try_deref(context).is_err());
        for (operand, result) in [(0, 0), (1, 1), (2, 1)] {
            assert_eq!(
                returned.deref(context).get_operand(operand),
                first.deref(context).get_result(result)
            );
        }
        assert_eq!(run(context, &function), IRStatus::Unchanged);
    }
}
