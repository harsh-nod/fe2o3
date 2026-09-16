use super::*;
use crate::optimization_v1::{BranchOp, CondBranchOp, IndexAttr, IndexType, ReturnOp};
use pliron::{
    attribute::AttrObj,
    basic_block::BasicBlock,
    builtin::{
        attributes::FPSingleAttr,
        op_interfaces::OneRegionInterface,
        ops::FuncOp,
        types::{FP32Type, FunctionType},
    },
    identifier::Identifier,
    irbuild::observer::RewriteEvent,
    op::verify_op,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Denied {
    storage: bool,
    prior: usize,
    request: usize,
}

struct Ledger {
    work: usize,
    live: usize,
    limit: usize,
    storage_limit: usize,
    failure: Option<Denied>,
}

impl Ledger {
    fn new() -> Self {
        Self {
            work: 0,
            live: 0,
            limit: 1_000_000,
            storage_limit: 1_000_000,
            failure: None,
        }
    }
}

impl IntegerIdentityBudgetV1 for Ledger {
    type Error = Denied;
    fn charge_work(&mut self, work: usize) -> Result<(), Denied> {
        if self
            .work
            .checked_add(work)
            .is_none_or(|next| next > self.limit)
        {
            let error = Denied {
                storage: false,
                prior: self.work,
                request: work,
            };
            self.failure.get_or_insert(error.clone());
            return Err(error);
        }
        self.work += work;
        Ok(())
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Denied> {
        if self
            .live
            .checked_add(bytes)
            .is_none_or(|next| next > self.storage_limit)
        {
            let error = Denied {
                storage: true,
                prior: self.live,
                request: bytes,
            };
            self.failure.get_or_insert(error.clone());
            return Err(error);
        }
        self.live += bytes;
        Ok(())
    }
    fn release_storage(&mut self, bytes: usize) {
        self.live = self.live.checked_sub(bytes).unwrap();
    }
}

fn append(context: &mut Context, block: Ptr<BasicBlock>, op: impl Op) -> Ptr<Operation> {
    let operation = op.get_operation();
    operation.insert_at_back(block, context);
    operation
}

fn integer(context: &Context, width: u32, sign: Signedness, bits: u64) -> AttrObj {
    Box::new(IntegerAttr::new(
        IntegerType::get(context, width, sign),
        APInt::from_u64(bits, bw(width as usize)),
    ))
}

struct Fixture {
    function: FuncOp,
    binary: Ptr<Operation>,
    returned: Ptr<Operation>,
    input: Value,
}

fn make_fixture(
    context: &mut Context,
    kind: BinaryKindAttr,
    ty: TypeHandle,
    attribute: AttrObj,
    reverse: bool,
) -> Fixture {
    let checked = matches!(
        kind,
        BinaryKindAttr::CheckedAdd
            | BinaryKindAttr::CheckedSubtract
            | BinaryKindAttr::CheckedMultiply
    );
    let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
    let results = if checked {
        vec![ty, ty, boolean, boolean]
    } else {
        vec![ty, ty]
    };
    let signature = FunctionType::get(context, vec![ty], results);
    let function = FuncOp::new(
        context,
        Identifier::try_from("identity").unwrap(),
        signature,
    );
    let block = function.get_entry_block(context);
    let input = block.deref(context).get_argument(0);
    let constant = ConstantOp::new(context, attribute);
    let constant = append(context, block, constant);
    let literal = constant.deref(context).get_result(0);
    let (lhs, rhs) = if reverse {
        (literal, input)
    } else {
        (input, literal)
    };
    let binary = BinaryOp::new(context, kind, lhs, rhs);
    let binary = append(context, block, binary);
    let value = binary.deref(context).get_result(0);
    let values = if checked {
        let overflow = binary.deref(context).get_result(1);
        vec![value, value, overflow, overflow]
    } else {
        vec![value, value]
    };
    let returned = ReturnOp::new(context, values);
    let returned = append(context, block, returned);
    Fixture {
        function,
        binary,
        returned,
        input,
    }
}

fn run(context: &mut Context, fixture: &Fixture) -> IRStatus {
    verify_op(&fixture.function, context).unwrap();
    let mut ledger = Ledger::new();
    let changed = integer_identity_canonicalization_v1(
        fixture.function.get_operation(),
        context,
        &mut ledger,
    )
    .unwrap();
    assert_eq!(ledger.live, 0);
    verify_op(&fixture.function, context).unwrap();
    changed
}

fn check_replacement(context: &Context, fixture: &Fixture, checked: bool) {
    assert!(fixture.binary.try_deref(context).is_err());
    for slot in [0, 1] {
        assert_eq!(
            fixture.returned.deref(context).get_operand(slot),
            fixture.input
        );
    }
    if checked {
        let overflow = fixture.returned.deref(context).get_operand(2);
        assert_eq!(overflow, fixture.returned.deref(context).get_operand(3));
        let constant =
            Operation::get_op::<ConstantOp>(overflow.defining_op().unwrap(), context).unwrap();
        let attribute = constant.get_attr_gpu_constant_value(context).unwrap();
        let integer = attribute.downcast_ref::<IntegerAttr>().unwrap();
        assert_eq!(integer.get_type().deref(context).width(), 1);
        assert_eq!(
            integer.get_type().deref(context).signedness(),
            Signedness::Signless
        );
        assert!(integer.value().is_zero());
    }
}

#[test]
fn every_fixed_signed_and_unsigned_identity_preserves_every_result_use() {
    for sign in [Signedness::Signed, Signedness::Unsigned] {
        for width in [8, 16, 32, 64] {
            for kind in [
                BinaryKindAttr::Add,
                BinaryKindAttr::Subtract,
                BinaryKindAttr::Multiply,
                BinaryKindAttr::BitAnd,
                BinaryKindAttr::BitOr,
                BinaryKindAttr::BitXor,
                BinaryKindAttr::CheckedAdd,
                BinaryKindAttr::CheckedSubtract,
                BinaryKindAttr::CheckedMultiply,
            ] {
                for reverse in [false, true] {
                    let subtract = matches!(
                        kind,
                        BinaryKindAttr::Subtract | BinaryKindAttr::CheckedSubtract
                    );
                    if reverse && subtract {
                        continue;
                    }
                    let context = &mut Context::new();
                    let neutral = match kind {
                        BinaryKindAttr::Multiply | BinaryKindAttr::CheckedMultiply => 1,
                        BinaryKindAttr::BitAnd => u64::MAX >> (64 - width),
                        _ => 0,
                    };
                    let ty = IntegerType::get(context, width, sign).into();
                    let fixture = make_fixture(
                        context,
                        kind,
                        ty,
                        integer(context, width, sign, neutral),
                        reverse,
                    );
                    let checked = fixture.binary.deref(context).get_num_results() == 2;
                    assert_eq!(run(context, &fixture), IRStatus::Changed);
                    check_replacement(context, &fixture, checked);
                    assert_eq!(run(context, &fixture), IRStatus::Unchanged);
                }
            }
        }
    }
}

#[test]
fn near_miss_operands_and_trapping_operators_remain_unchanged() {
    for (kind, bits, reverse) in [
        (BinaryKindAttr::Add, 1, false),
        (BinaryKindAttr::Subtract, 0, true),
        (BinaryKindAttr::Multiply, 0, false),
        (BinaryKindAttr::BitAnd, 0xffff_fffe, false),
        (BinaryKindAttr::BitOr, 1, true),
        (BinaryKindAttr::BitXor, 1, false),
        (BinaryKindAttr::CheckedAdd, 1, false),
        (BinaryKindAttr::CheckedSubtract, 0, true),
        (BinaryKindAttr::CheckedMultiply, 0, false),
        (BinaryKindAttr::Divide, 1, false),
        (BinaryKindAttr::Divide, 0, false),
        (BinaryKindAttr::Remainder, 1, false),
        (BinaryKindAttr::ShiftLeft, 0, false),
        (BinaryKindAttr::ShiftRight, 0, false),
        (BinaryKindAttr::ShiftLeft, 32, false),
    ] {
        let context = &mut Context::new();
        let ty = IntegerType::get(context, 32, Signedness::Unsigned).into();
        let fixture = make_fixture(
            context,
            kind,
            ty,
            integer(context, 32, Signedness::Unsigned, bits),
            reverse,
        );
        assert_eq!(run(context, &fixture), IRStatus::Unchanged);
        assert!(fixture.binary.try_deref(context).is_ok());
    }
}

#[test]
fn floating_bool_index_signless_and_extra_attributes_do_not_enter_the_family() {
    for bits in [0.0f32.to_bits(), (-0.0f32).to_bits(), f32::NAN.to_bits()] {
        let context = &mut Context::new();
        let ty = FP32Type::get(context).into();
        let fixture = make_fixture(
            context,
            BinaryKindAttr::Add,
            ty,
            Box::new(FPSingleAttr::from(f32::from_bits(bits))),
            false,
        );
        assert_eq!(run(context, &fixture), IRStatus::Unchanged);
    }
    for width in [1, 32] {
        let context = &mut Context::new();
        let ty = IntegerType::get(context, width, Signedness::Signless).into();
        let fixture = make_fixture(
            context,
            BinaryKindAttr::BitOr,
            ty,
            integer(context, width, Signedness::Signless, 0),
            false,
        );
        assert_eq!(run(context, &fixture), IRStatus::Unchanged);
    }
    let context = &mut Context::new();
    let ty = IndexType::get(context).into();
    let fixture = make_fixture(
        context,
        BinaryKindAttr::Add,
        ty,
        Box::new(IndexAttr(0)),
        false,
    );
    assert_eq!(run(context, &fixture), IRStatus::Unchanged);
    let context = &mut Context::new();
    let ty = IntegerType::get(context, 32, Signedness::Unsigned).into();
    let fixture = make_fixture(
        context,
        BinaryKindAttr::Add,
        ty,
        integer(context, 32, Signedness::Unsigned, 0),
        false,
    );
    fixture.binary.deref_mut(context).attributes.0.insert(
        Identifier::try_from("extra").unwrap(),
        integer(context, 32, Signedness::Unsigned, 1),
    );
    assert_eq!(run(context, &fixture), IRStatus::Unchanged);
}

#[test]
fn malformed_payload_width_and_result_shape_are_refused_before_rewrite() {
    for malformed_overflow in [false, true] {
        let context = &mut Context::new();
        let ty = IntegerType::get(context, 32, Signedness::Unsigned).into();
        let fixture = make_fixture(
            context,
            BinaryKindAttr::Add,
            ty,
            integer(context, 32, Signedness::Unsigned, 0),
            false,
        );
        if malformed_overflow {
            let binary = Operation::get_op::<BinaryOp>(fixture.binary, context).unwrap();
            binary.set_attr_gpu_binary_kind(context, BinaryKindAttr::CheckedAdd);
        } else {
            let operand = fixture.binary.deref(context).get_operand(1);
            let constant =
                Operation::get_op::<ConstantOp>(operand.defining_op().unwrap(), context).unwrap();
            constant.set_attr_gpu_constant_value(
                context,
                Box::new(IntegerAttr::new(
                    IntegerType::get(context, 32, Signedness::Unsigned),
                    APInt::zero(bw(256)),
                )),
            );
        }
        assert!(verify_op(&fixture.function, context).is_err());
        let mut ledger = Ledger::new();
        assert_eq!(
            integer_identity_canonicalization_v1(
                fixture.function.get_operation(),
                context,
                &mut ledger
            )
            .unwrap(),
            IRStatus::Unchanged
        );
        assert_eq!(ledger.live, 0);
        assert!(fixture.binary.try_deref(context).is_ok());
        assert!(verify_op(&fixture.function, context).is_err());
    }
}

#[test]
fn loop_body_and_header_identities_do_not_change_control_or_move_definitions() {
    let context = &mut Context::new();
    let scalar = IntegerType::get(context, 32, Signedness::Unsigned).into();
    let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
    let function = FuncOp::new(
        context,
        Identifier::try_from("loop_identity").unwrap(),
        FunctionType::get(context, vec![scalar, boolean], vec![scalar]),
    );
    let entry = function.get_entry_block(context);
    let input = entry.deref(context).get_argument(0);
    let condition = entry.deref(context).get_argument(1);
    let region = function.get_region(context);
    let header = BasicBlock::new(context, None, vec![]);
    let body = BasicBlock::new(context, None, vec![]);
    let exit = BasicBlock::new(context, None, vec![]);
    for block in [header, body, exit] {
        block.insert_at_back(region, context);
    }
    let zero = ConstantOp::new(context, integer(context, 32, Signedness::Unsigned, 0));
    let zero = append(context, entry, zero).deref(context).get_result(0);
    let branch = BranchOp::new(context, header, vec![]);
    let entry_branch = append(context, entry, branch);
    let add = BinaryOp::new(context, BinaryKindAttr::Add, input, zero);
    let add = append(context, header, add);
    let branch = CondBranchOp::new(context, condition, body, vec![], exit, vec![]);
    let header_branch = append(context, header, branch);
    let add_result = add.deref(context).get_result(0);
    let second = BinaryOp::new(context, BinaryKindAttr::Add, add_result, zero);
    let second = append(context, body, second);
    let branch = BranchOp::new(context, header, vec![]);
    let backedge = append(context, body, branch);
    let returned = ReturnOp::new(context, vec![add_result]);
    let returned = append(context, exit, returned);
    verify_op(&function, context).unwrap();
    let mut ledger = Ledger::new();
    assert_eq!(
        integer_identity_canonicalization_v1(function.get_operation(), context, &mut ledger)
            .unwrap(),
        IRStatus::Changed
    );
    assert_eq!(ledger.live, 0);
    for removed in [add, second] {
        assert!(removed.try_deref(context).is_err());
    }
    for (op, block) in [
        (entry_branch, entry),
        (header_branch, header),
        (backedge, body),
    ] {
        assert_eq!(op.deref(context).get_parent_block(), Some(block));
    }
    assert_eq!(returned.deref(context).get_operand(0), input);
    verify_op(&function, context).unwrap();
}

#[test]
fn independently_derived_work_boundaries_preserve_history_floor_and_late_failure() {
    // Root/growth/container/region/block = 6; three scans = 3; matcher kind,
    // verify, dynamic operand, constant lookup/verify/clone, decision = 7.
    // Ordinary replacement: results2 + uses2 + Vec growth2 => 22 total.
    // Checked: results3 + uses4 + Vec growth2 + false creation1 => 26 total.
    for (kind, work) in [(BinaryKindAttr::Add, 22), (BinaryKindAttr::CheckedAdd, 26)] {
        for allowed in [work, work - 1] {
            let context = &mut Context::new();
            let ty = IntegerType::get(context, 32, Signedness::Unsigned).into();
            let fixture = make_fixture(
                context,
                kind,
                ty,
                integer(context, 32, Signedness::Unsigned, 0),
                false,
            );
            verify_op(&fixture.function, context).unwrap();
            let mut ledger = Ledger::new();
            ledger.work = 5;
            ledger.live = 83;
            ledger.limit = 5 + allowed;
            let result = integer_identity_canonicalization_v1(
                fixture.function.get_operation(),
                context,
                &mut ledger,
            );
            if allowed == work {
                assert_eq!(result.unwrap(), IRStatus::Changed);
                assert_eq!(ledger.failure, None);
            } else {
                let error = Denied {
                    storage: false,
                    prior: 5 + allowed,
                    request: 1,
                };
                assert!(
                    matches!(result, Err(IntegerIdentityErrorV1::Budget(ref actual)) if actual == &error)
                );
                assert_eq!(ledger.failure, Some(error));
            }
            assert_eq!(ledger.work, 5 + allowed);
            assert_eq!(ledger.live, 83);
            assert!(fixture.binary.try_deref(context).is_err());
            verify_op(&fixture.function, context).unwrap();
            // A late-denied candidate is discarded, never retried/adopted.
        }
    }
}

struct Observer(Arc<Mutex<Vec<(Value, Value)>>>);
impl RewriteObserver for Observer {
    fn observe(&mut self, _context: &Context, event: RewriteEvent) {
        if let RewriteEvent::ValueReplaced { old, new } = event {
            self.0.lock().unwrap().push((old, new));
        }
    }
}

#[test]
fn checked_observer_records_both_ordered_original_result_identities() {
    let context = &mut Context::new();
    let ty = IntegerType::get(context, 32, Signedness::Unsigned).into();
    let fixture = make_fixture(
        context,
        BinaryKindAttr::CheckedAdd,
        ty,
        integer(context, 32, Signedness::Unsigned, 0),
        false,
    );
    let old = [
        fixture.binary.deref(context).get_result(0),
        fixture.binary.deref(context).get_result(1),
    ];
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut ledger = Ledger::new();
    assert_eq!(
        integer_identity_canonicalization_with_observer_v1(
            fixture.function.get_operation(),
            context,
            &mut ledger,
            Box::new(Observer(events.clone()))
        )
        .unwrap(),
        IRStatus::Changed
    );
    assert_eq!(ledger.live, 0);
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], (old[0], fixture.input));
    assert_eq!(
        events[1],
        (old[1], fixture.returned.deref(context).get_operand(2))
    );
    check_replacement(context, &fixture, true);
    verify_op(&fixture.function, context).unwrap();
}

#[test]
fn initial_storage_denial_preserves_existing_history_and_candidate() {
    let context = &mut Context::new();
    let ty = IntegerType::get(context, 32, Signedness::Unsigned).into();
    let fixture = make_fixture(
        context,
        BinaryKindAttr::Add,
        ty,
        integer(context, 32, Signedness::Unsigned, 0),
        false,
    );
    let mut ledger = Ledger::new();
    ledger.work = 5;
    ledger.live = 83;
    let headers = size_of::<Vec<Ptr<Operation>>>() + size_of::<Vec<Value>>();
    ledger.storage_limit = 83 + headers - 1;
    let error = Denied {
        storage: true,
        prior: 83,
        request: headers,
    };
    let result = integer_identity_canonicalization_v1(
        fixture.function.get_operation(),
        context,
        &mut ledger,
    );
    assert!(matches!(result, Err(IntegerIdentityErrorV1::Budget(ref actual)) if actual == &error));
    assert_eq!(ledger.failure, Some(error));
    assert_eq!(ledger.work, 6);
    assert_eq!(ledger.live, 83);
    assert!(fixture.binary.try_deref(context).is_ok());
    verify_op(&fixture.function, context).unwrap();
}
