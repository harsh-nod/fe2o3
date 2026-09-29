use super::*;
use crate::integer_identity_v2::{
    integer_identity_canonicalization_v2, integer_identity_canonicalization_with_observer_v2,
};

struct Measured {
    inner: Ledger,
    peak: usize,
}
impl Measured {
    fn new(work: usize, storage: usize) -> Self {
        let mut inner = Ledger::new();
        inner.limit = work;
        inner.storage_limit = storage;
        Self { inner, peak: 0 }
    }
}
impl IntegerIdentityBudgetV1 for Measured {
    type Error = Denied;
    fn charge_work(&mut self, value: usize) -> Result<(), Denied> {
        self.inner.charge_work(value)
    }
    fn reserve_storage(&mut self, value: usize) -> Result<(), Denied> {
        self.inner.reserve_storage(value)?;
        self.peak = self.peak.max(self.inner.live);
        Ok(())
    }
    fn release_storage(&mut self, value: usize) {
        self.inner.release_storage(value);
    }
}

struct Chain {
    fixture: Fixture,
    aliases: Vec<Ptr<Operation>>,
}
fn chain(context: &mut Context, depth: usize) -> Chain {
    assert!(depth > 0);
    let ty = IntegerType::get(context, 64, Signedness::Unsigned).into();
    let function = FuncOp::new(
        context,
        Identifier::try_from("reverse_block_layout").unwrap(),
        FunctionType::get(context, vec![ty], vec![ty, ty]),
    );
    let entry = function.get_entry_block(context);
    let input = entry.deref(context).get_argument(0);
    let region = function.get_region(context);
    let blocks: Vec<_> = (0..=depth)
        .map(|_| {
            let block = BasicBlock::new(context, None, vec![]);
            block.insert_at_back(region, context);
            block
        })
        .collect();
    let zero = ConstantOp::new(context, integer(context, 64, Signedness::Unsigned, 0));
    let mut value = append(context, entry, zero).deref(context).get_result(0);
    let branch = BranchOp::new(context, blocks[depth], vec![]);
    append(context, entry, branch);
    let mut aliases = Vec::new();
    for position in (1..=depth).rev() {
        let operation = BinaryOp::new(context, BinaryKindAttr::Add, value, value);
        let operation = append(context, blocks[position], operation);
        value = operation.deref(context).get_result(0);
        aliases.push(operation);
        let branch = BranchOp::new(context, blocks[position - 1], vec![]);
        append(context, blocks[position], branch);
    }
    let binary = BinaryOp::new(context, BinaryKindAttr::Add, input, value);
    let binary = append(context, blocks[0], binary);
    let value = binary.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![value, value]);
    let returned = append(context, blocks[0], returned);
    Chain {
        fixture: Fixture {
            function,
            binary,
            returned,
            input,
        },
        aliases,
    }
}

#[test]
fn worklist_revisits_a_user_exposed_by_a_later_layout_producer() {
    let context = &mut Context::new();
    let fixture = chain(context, 1);
    verify_op(&fixture.fixture.function, context).unwrap();
    let mut old = Ledger::new();
    assert_eq!(
        integer_identity_canonicalization_v1(
            fixture.fixture.function.get_operation(),
            context,
            &mut old
        )
        .unwrap(),
        IRStatus::Changed
    );
    assert!(fixture.fixture.binary.try_deref(context).is_ok());
    assert_eq!(old.live, 0);
    let mut new = Ledger::new();
    assert_eq!(
        integer_identity_canonicalization_v2(
            fixture.fixture.function.get_operation(),
            context,
            &mut new
        )
        .unwrap(),
        IRStatus::Changed
    );
    check_replacement(context, &fixture.fixture, false);
    verify_op(&fixture.fixture.function, context).unwrap();
    assert_eq!(new.live, 0);
}

#[test]
fn worklist_saturates_cascading_aliases_and_is_idempotent_without_rescans() {
    for depth in [1usize, 16, 64, 256] {
        let context = &mut Context::new();
        let fixture = chain(context, depth);
        verify_op(&fixture.fixture.function, context).unwrap();
        let mut meter = Measured::new(2_000_000, 2_000_000);
        assert_eq!(
            integer_identity_canonicalization_v2(
                fixture.fixture.function.get_operation(),
                context,
                &mut meter
            )
            .unwrap(),
            IRStatus::Changed
        );
        for operation in &fixture.aliases {
            assert!(operation.try_deref(context).is_err());
        }
        check_replacement(context, &fixture.fixture, false);
        verify_op(&fixture.fixture.function, context).unwrap();
        // Each original binary has two def-use edges, one removal and at most
        // two queue visits. Paid heap/index traversal is logarithmic in depth.
        let log = (usize::BITS - (depth + 1).leading_zeros()) as usize;
        assert!(meter.inner.work <= 300 * (depth + 2) * (log + 1));
        assert_eq!(meter.inner.live, 0);
        let mut again = Ledger::new();
        assert_eq!(
            integer_identity_canonicalization_v2(
                fixture.fixture.function.get_operation(),
                context,
                &mut again
            )
            .unwrap(),
            IRStatus::Unchanged
        );
        assert_eq!(again.live, 0);
    }
}

#[test]
fn worklist_keeps_the_exact_signed_unsigned_and_checked_rule_family() {
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
                let context = &mut Context::new();
                let ty = IntegerType::get(context, width, sign).into();
                let neutral = match kind {
                    BinaryKindAttr::Multiply | BinaryKindAttr::CheckedMultiply => 1,
                    BinaryKindAttr::BitAnd => u64::MAX >> (64 - width),
                    _ => 0,
                };
                let fixture = make_fixture(
                    context,
                    kind,
                    ty,
                    integer(context, width, sign, neutral),
                    false,
                );
                let mut meter = Ledger::new();
                assert_eq!(
                    integer_identity_canonicalization_v2(
                        fixture.function.get_operation(),
                        context,
                        &mut meter
                    )
                    .unwrap(),
                    IRStatus::Changed
                );
                check_replacement(
                    context,
                    &fixture,
                    matches!(
                        kind,
                        BinaryKindAttr::CheckedAdd
                            | BinaryKindAttr::CheckedSubtract
                            | BinaryKindAttr::CheckedMultiply
                    ),
                );
                verify_op(&fixture.function, context).unwrap();
                assert_eq!(meter.live, 0);
            }
        }
    }
}

#[test]
fn worklist_does_not_fold_non_neutral_overflow_float_or_index_operations() {
    for kind in [
        BinaryKindAttr::Add,
        BinaryKindAttr::CheckedAdd,
        BinaryKindAttr::Subtract,
        BinaryKindAttr::CheckedSubtract,
        BinaryKindAttr::Multiply,
        BinaryKindAttr::CheckedMultiply,
    ] {
        let context = &mut Context::new();
        let ty = IntegerType::get(context, 8, Signedness::Unsigned).into();
        let fixture = make_fixture(
            context,
            kind,
            ty,
            integer(context, 8, Signedness::Unsigned, 2),
            false,
        );
        let mut meter = Ledger::new();
        assert_eq!(
            integer_identity_canonicalization_v2(
                fixture.function.get_operation(),
                context,
                &mut meter
            )
            .unwrap(),
            IRStatus::Unchanged
        );
        assert!(fixture.binary.try_deref(context).is_ok());
        verify_op(&fixture.function, context).unwrap();
        assert_eq!(meter.live, 0);
    }
    for index in [false, true] {
        let context = &mut Context::new();
        let ty = if index {
            IndexType::get(context).into()
        } else {
            FP32Type::get(context).into()
        };
        let attribute: AttrObj = if index {
            Box::new(IndexAttr(0))
        } else {
            Box::new(FPSingleAttr::from(0.0_f32))
        };
        let fixture = make_fixture(context, BinaryKindAttr::Add, ty, attribute, false);
        let mut meter = Ledger::new();
        assert_eq!(
            integer_identity_canonicalization_v2(
                fixture.function.get_operation(),
                context,
                &mut meter
            )
            .unwrap(),
            IRStatus::Unchanged
        );
        assert!(fixture.binary.try_deref(context).is_ok());
        assert_eq!(meter.live, 0);
    }
}

fn measured(
    work: usize,
    storage: usize,
) -> (
    Result<IRStatus, IntegerIdentityErrorV1<Denied>>,
    usize,
    usize,
    Option<Denied>,
) {
    let context = &mut Context::new();
    // Pointer-key sorting/probing is charged by actual work and can differ
    // across context identities. One real checked rewrite has a deterministic
    // cut while still exercising replacement allocation and both result uses.
    let ty = IntegerType::get(context, 32, Signedness::Unsigned).into();
    let fixture = make_fixture(
        context,
        BinaryKindAttr::CheckedAdd,
        ty,
        integer(context, 32, Signedness::Unsigned, 0),
        false,
    );
    verify_op(&fixture.function, context).unwrap();
    let mut meter = Measured::new(work, storage);
    meter.inner.live = 73;
    let result =
        integer_identity_canonicalization_v2(fixture.function.get_operation(), context, &mut meter);
    assert_eq!(meter.inner.live, 73);
    if result.is_ok() {
        check_replacement(context, &fixture, true);
        verify_op(&fixture.function, context).unwrap();
    }
    (result, meter.inner.work, meter.peak, meter.inner.failure)
}

#[test]
fn worklist_exact_and_one_short_work_and_storage_preserve_original_denials() {
    let (result, work, storage, failure) = measured(2_000_000, 2_000_000);
    assert_eq!(result.unwrap(), IRStatus::Changed);
    assert!(failure.is_none());
    let (result, exact_work, exact_storage, failure) = measured(work, storage);
    assert_eq!(result.unwrap(), IRStatus::Changed);
    assert_eq!((exact_work, exact_storage), (work, storage));
    assert!(failure.is_none());
    for storage_short in [false, true] {
        let (result, _, _, failure) = measured(
            work - usize::from(!storage_short),
            storage - usize::from(storage_short),
        );
        let failure = failure.unwrap();
        assert_eq!(failure.storage, storage_short);
        assert!(matches!(result, Err(IntegerIdentityErrorV1::Budget(actual)) if actual == failure));
    }
}

#[test]
fn worklist_observer_order_depends_on_source_ordinals_not_pointer_hashes() {
    let mut previous = None;
    for noise in [0, 7, 29] {
        let context = &mut Context::new();
        let retained: Vec<_> = (0..noise).map(|_| chain(context, 1)).collect();
        let fixture = chain(context, 17);
        let original: Vec<_> = fixture
            .aliases
            .iter()
            .copied()
            .chain([fixture.fixture.binary])
            .map(|op| op.deref(context).get_result(0))
            .collect();
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut meter = Ledger::new();
        assert_eq!(
            integer_identity_canonicalization_with_observer_v2(
                fixture.fixture.function.get_operation(),
                context,
                &mut meter,
                Box::new(Observer(events.clone()))
            )
            .unwrap(),
            IRStatus::Changed
        );
        let order: Vec<_> = events
            .lock()
            .unwrap()
            .iter()
            .map(|(old, _)| original.iter().position(|value| value == old).unwrap())
            .collect();
        assert_eq!(order, (0..18).collect::<Vec<_>>());
        if let Some(prior) = previous.replace(order.clone()) {
            assert_eq!(prior, order);
        }
        assert_eq!(meter.live, 0);
        assert_eq!(retained.len(), noise);
    }
}
