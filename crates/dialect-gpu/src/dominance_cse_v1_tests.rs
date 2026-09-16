use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use super::*;
use crate::{
    AddressSpaceAttr,
    optimization_v1::{
        AccessModeAttr, BinaryKindAttr, BinaryOp, BranchOp, CallOp, CondBranchOp, ConstantOp,
        LoadOp, PointerType, PreservedOperationKindAttr, PreservedOperationOp, ReturnOp, StoreOp,
    },
};
use pliron::{
    attribute::AttrObj,
    builtin::{
        attributes::IntegerAttr,
        op_interfaces::{OneRegionInterface, SingleBlockRegionInterface},
        ops::{FuncOp, ModuleOp},
        types::{FunctionType, IntegerType, Signedness},
    },
    identifier::Identifier,
    irbuild::observer::RewriteEvent,
    op::{Op, verify_op},
    r#type::TypeHandle,
    utils::apint::{APInt, bw},
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Denied {
    storage: bool,
    prior: usize,
    request: usize,
}

impl fmt::Display for Denied {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "denied {} after {} by {}",
            self.storage, self.prior, self.request
        )
    }
}

impl Error for Denied {}

struct Ledger {
    work: usize,
    live: usize,
    peak: usize,
    work_limit: usize,
    storage_limit: usize,
    failure: Option<Denied>,
}

impl Ledger {
    fn generous() -> Self {
        Self {
            work: 0,
            live: 0,
            peak: 0,
            work_limit: 1_000_000,
            storage_limit: 1_000_000,
            failure: None,
        }
    }
}

impl DominanceCseBudgetV1 for Ledger {
    type Error = Denied;

    fn charge_work(&mut self, work: usize) -> Result<(), Self::Error> {
        if self
            .work
            .checked_add(work)
            .is_none_or(|next| next > self.work_limit)
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

    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
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
        self.peak = self.peak.max(self.live);
        Ok(())
    }

    fn release_storage(&mut self, bytes: usize) {
        self.live = self.live.checked_sub(bytes).expect("no double release");
    }
}

fn u32_type(context: &Context) -> TypeHandle {
    IntegerType::get(context, 32, Signedness::Unsigned).into()
}

fn integer(context: &Context, value: u32) -> AttrObj {
    Box::new(IntegerAttr::new(
        IntegerType::get(context, 32, Signedness::Unsigned),
        APInt::from_u32(value, bw(32)),
    ))
}

fn function(
    context: &mut Context,
    name: &str,
    arguments: Vec<TypeHandle>,
    results: Vec<TypeHandle>,
) -> FuncOp {
    let ty = FunctionType::get(context, arguments, results);
    FuncOp::new(context, Identifier::try_from(name).unwrap(), ty)
}

fn block(context: &mut Context, function: &FuncOp) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(context, None, vec![]);
    block.insert_at_back(function.get_region(context), context);
    block
}

fn append(context: &mut Context, block: Ptr<BasicBlock>, operation: impl Op) -> Ptr<Operation> {
    let operation = operation.get_operation();
    operation.insert_at_back(block, context);
    operation
}

fn bit_and(
    context: &mut Context,
    block: Ptr<BasicBlock>,
    lhs: Value,
    rhs: Value,
) -> Ptr<Operation> {
    let op = BinaryOp::new(context, BinaryKindAttr::BitAnd, lhs, rhs);
    append(context, block, op)
}

fn run(context: &mut Context, function: &FuncOp) -> IRStatus {
    verify_op(function, context).expect("actual input Pliron verifies");
    let mut ledger = Ledger::generous();
    let changed = dominance_pure_cse_v1(
        function.get_operation(),
        context,
        &mut DomInfo::default(),
        &mut ledger,
    )
    .unwrap();
    assert_eq!(ledger.live, 0);
    verify_op(function, context).expect("actual output Pliron verifies");
    changed
}

struct Diamond {
    function: FuncOp,
    earlier: Option<Ptr<Operation>>,
    left: Ptr<Operation>,
    right: Ptr<Operation>,
    join: Ptr<Operation>,
    returned: Ptr<Operation>,
}

fn diamond(context: &mut Context, dominating: bool) -> Diamond {
    let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
    let scalar = u32_type(context);
    let function = function(
        context,
        "diamond",
        vec![boolean, scalar, scalar],
        vec![scalar],
    );
    let entry = function.get_entry_block(context);
    let condition = entry.deref(context).get_argument(0);
    let lhs = entry.deref(context).get_argument(1);
    let rhs = entry.deref(context).get_argument(2);
    let left_block = block(context, &function);
    let right_block = block(context, &function);
    let join_block = block(context, &function);
    let earlier = dominating.then(|| bit_and(context, entry, lhs, rhs));
    let branch = CondBranchOp::new(context, condition, left_block, vec![], right_block, vec![]);
    append(context, entry, branch);
    let left = bit_and(context, left_block, lhs, rhs);
    let branch = BranchOp::new(context, join_block, vec![]);
    append(context, left_block, branch);
    let right = bit_and(context, right_block, lhs, rhs);
    let branch = BranchOp::new(context, join_block, vec![]);
    append(context, right_block, branch);
    let join = bit_and(context, join_block, lhs, rhs);
    let returned_value = join.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![returned_value]);
    let returned = append(context, join_block, returned);
    Diamond {
        function,
        earlier,
        left,
        right,
        join,
        returned,
    }
}

#[test]
fn dominating_expression_replaces_both_arms_and_join_without_moving_it() {
    let context = &mut Context::new();
    let fixture = diamond(context, true);
    let earlier = fixture.earlier.unwrap();
    let defining_block = earlier.deref(context).get_parent_block();
    assert_eq!(run(context, &fixture.function), IRStatus::Changed);
    for erased in [fixture.left, fixture.right, fixture.join] {
        assert!(erased.try_deref(context).is_err());
    }
    assert_eq!(earlier.deref(context).get_parent_block(), defining_block);
    assert_eq!(
        fixture.returned.deref(context).get_operand(0),
        earlier.deref(context).get_result(0)
    );
    assert_eq!(run(context, &fixture.function), IRStatus::Unchanged);
}

#[test]
fn siblings_and_join_do_not_reuse_a_nondominating_expression() {
    let context = &mut Context::new();
    let fixture = diamond(context, false);
    assert_eq!(run(context, &fixture.function), IRStatus::Unchanged);
    for retained in [fixture.left, fixture.right, fixture.join] {
        assert!(retained.try_deref(context).is_ok());
    }
}

#[test]
fn loop_uses_dominator_scopes_without_hoisting_from_a_backedge() {
    for seed_at_entry in [false, true] {
        let context = &mut Context::new();
        let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
        let scalar = u32_type(context);
        let function = function(context, "loop", vec![boolean, scalar, scalar], vec![scalar]);
        let entry = function.get_entry_block(context);
        let header = block(context, &function);
        let body = block(context, &function);
        let exit = block(context, &function);
        let condition = entry.deref(context).get_argument(0);
        let lhs = entry.deref(context).get_argument(1);
        let rhs = entry.deref(context).get_argument(2);
        let seed = seed_at_entry.then(|| bit_and(context, entry, lhs, rhs));
        let branch = BranchOp::new(context, header, vec![]);
        append(context, entry, branch);
        let header_expression = bit_and(context, header, lhs, rhs);
        let branch = CondBranchOp::new(context, condition, body, vec![], exit, vec![]);
        append(context, header, branch);
        let body_expression = bit_and(context, body, lhs, rhs);
        let branch = BranchOp::new(context, header, vec![]);
        append(context, body, branch);
        let returned_value = header_expression.deref(context).get_result(0);
        let returned = ReturnOp::new(context, vec![returned_value]);
        let returned = append(context, exit, returned);
        assert_eq!(run(context, &function), IRStatus::Changed);
        assert!(body_expression.try_deref(context).is_err());
        let expected = seed.unwrap_or(header_expression);
        assert_eq!(
            returned.deref(context).get_operand(0),
            expected.deref(context).get_result(0)
        );
        assert_eq!(header_expression.try_deref(context).is_ok(), !seed_at_entry);
    }
}

#[test]
fn unreachable_blocks_are_not_cross_block_cse_candidates() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let function = function(context, "unreachable", vec![], vec![scalar]);
    let entry = function.get_entry_block(context);
    let dead = block(context, &function);
    let first = ConstantOp::new(context, integer(context, 7));
    let first = append(context, entry, first);
    let returned_value = first.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![returned_value]);
    append(context, entry, returned);
    let duplicate = ConstantOp::new(context, integer(context, 7));
    let duplicate = append(context, dead, duplicate);
    let returned_value = duplicate.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![returned_value]);
    append(context, dead, returned);
    assert_eq!(run(context, &function), IRStatus::Unchanged);
    assert!(duplicate.try_deref(context).is_ok());
}

#[test]
fn graph_region_and_nested_functions_never_share_available_values() {
    let context = &mut Context::new();
    let module = ModuleOp::new(context, Identifier::try_from("nested").unwrap());
    let mut retained = Vec::new();
    for _ in 0..2 {
        let value = ConstantOp::new(context, integer(context, 3));
        retained.push(value.get_operation());
        module.append_operation(context, value.get_operation(), 0);
    }
    for name in ["first", "second"] {
        let scalar = u32_type(context);
        let function = function(context, name, vec![], vec![scalar]);
        let entry = function.get_entry_block(context);
        let value = ConstantOp::new(context, integer(context, 3));
        let value = append(context, entry, value);
        retained.push(value);
        let returned_value = value.deref(context).get_result(0);
        let returned = ReturnOp::new(context, vec![returned_value]);
        append(context, entry, returned);
        module.append_operation(context, function.get_operation(), 0);
    }
    verify_op(&module, context).unwrap();
    let mut ledger = Ledger::generous();
    assert_eq!(
        dominance_pure_cse_v1(
            module.get_operation(),
            context,
            &mut DomInfo::default(),
            &mut ledger
        )
        .unwrap(),
        IRStatus::Unchanged
    );
    assert_eq!(ledger.live, 0);
    for operation in retained {
        assert!(operation.try_deref(context).is_ok());
    }
    verify_op(&module, context).unwrap();
}

#[test]
fn full_attributes_types_and_ordered_operands_remain_part_of_identity() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let function = function(context, "identity", vec![scalar, scalar], vec![scalar]);
    let entry = function.get_entry_block(context);
    let next = block(context, &function);
    let lhs = entry.deref(context).get_argument(0);
    let rhs = entry.deref(context).get_argument(1);
    let first = bit_and(context, entry, lhs, rhs);
    let branch = BranchOp::new(context, next, vec![]);
    append(context, entry, branch);
    let reversed = bit_and(context, next, rhs, lhs);
    let attributed = bit_and(context, next, lhs, rhs);
    attributed
        .deref_mut(context)
        .attributes
        .0
        .insert(Identifier::try_from("extra").unwrap(), integer(context, 1));
    let different_value = ConstantOp::new(context, integer(context, 1));
    let different_value = append(context, next, different_value);
    let wide = ConstantOp::new(
        context,
        Box::new(IntegerAttr::new(
            IntegerType::get(context, 64, Signedness::Unsigned),
            APInt::from_u64(1, bw(64)),
        )),
    );
    let wide = append(context, next, wide);
    let returned_value = first.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![returned_value]);
    append(context, next, returned);
    assert_eq!(run(context, &function), IRStatus::Unchanged);
    for retained in [first, reversed, attributed, different_value, wide] {
        assert!(retained.try_deref(context).is_ok());
    }
}

#[test]
fn equal_attribute_maps_with_different_insertion_order_have_equal_fingerprints() {
    let context = &mut Context::new();
    let scalar = u32_type(context);
    let function = function(
        context,
        "attribute_order",
        vec![scalar, scalar],
        vec![scalar],
    );
    let entry = function.get_entry_block(context);
    let lhs = entry.deref(context).get_argument(0);
    let rhs = entry.deref(context).get_argument(1);
    let first = bit_and(context, entry, lhs, rhs);
    let second = bit_and(context, entry, lhs, rhs);
    for (operation, names) in [(first, ["a", "b"]), (second, ["b", "a"])] {
        for name in names {
            operation
                .deref_mut(context)
                .attributes
                .0
                .insert(Identifier::try_from(name).unwrap(), integer(context, 5));
        }
    }
    let left = BorrowedPureCseKeyV1::from_operation(first, context).unwrap();
    let right = BorrowedPureCseKeyV1::from_operation(second, context).unwrap();
    assert!(left.exactly_equal(right, context));
    assert_eq!(fingerprint(left, context), fingerprint(right, context));
    let returned_value = second.deref(context).get_result(0);
    let returned = ReturnOp::new(context, vec![returned_value]);
    append(context, entry, returned);
    assert_eq!(run(context, &function), IRStatus::Changed);
    assert!(second.try_deref(context).is_err());
}

#[test]
fn loads_calls_trapping_arithmetic_and_every_preserved_effect_family_are_excluded() {
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
    let lhs = entry.deref(context).get_argument(0);
    let rhs = entry.deref(context).get_argument(1);
    let pointer = entry.deref(context).get_argument(2);
    let mut retained = Vec::new();
    for _ in 0..2 {
        let add = BinaryOp::new(context, BinaryKindAttr::Add, lhs, rhs);
        retained.push(append(context, entry, add));
        let call = CallOp::new(context, "external", vec![lhs], vec![scalar]);
        retained.push(append(context, entry, call));
        for volatile in [false, true] {
            let load = LoadOp::new(context, pointer, 4, volatile).unwrap();
            retained.push(append(context, entry, load));
            let store = StoreOp::new(context, pointer, lhs, 4, volatile).unwrap();
            retained.push(append(context, entry, store));
        }
        for kind in [
            PreservedOperationKindAttr::Intrinsic,
            PreservedOperationKindAttr::MemoryIntrinsic,
            PreservedOperationKindAttr::Alloca,
            PreservedOperationKindAttr::GuardedLoad,
            PreservedOperationKindAttr::GuardedStore,
            PreservedOperationKindAttr::Barrier,
            PreservedOperationKindAttr::Atomic,
            PreservedOperationKindAttr::Fence,
            PreservedOperationKindAttr::WorkgroupBarrier,
            PreservedOperationKindAttr::WorkgroupMemory,
            PreservedOperationKindAttr::Matrix,
            PreservedOperationKindAttr::Gfx950LdsTranspose,
            PreservedOperationKindAttr::Wave,
            PreservedOperationKindAttr::InlineAssembly,
            PreservedOperationKindAttr::VerificationContractV12,
            PreservedOperationKindAttr::VectorLoadV12,
            PreservedOperationKindAttr::VectorStoreV12,
            PreservedOperationKindAttr::VectorLayoutConvertV12,
        ] {
            let op = PreservedOperationOp::new(context, kind, vec![], vec![]);
            retained.push(append(context, entry, op));
        }
    }
    let returned = ReturnOp::new(context, vec![lhs]);
    append(context, entry, returned);
    assert_eq!(run(context, &function), IRStatus::Unchanged);
    for operation in retained {
        assert!(operation.try_deref(context).is_ok());
    }
}

#[test]
fn distinct_constants_and_attributes_have_linear_structured_work_growth() {
    for count in [64, 256, 1024] {
        let context = &mut Context::new();
        let scalar = u32_type(context);
        let function = function(context, "scaling", vec![scalar, scalar], vec![scalar]);
        let entry = function.get_entry_block(context);
        let lhs = entry.deref(context).get_argument(0);
        let rhs = entry.deref(context).get_argument(1);
        for index in 0..count {
            let constant = ConstantOp::new(context, integer(context, index as u32));
            append(context, entry, constant);
            let operation = bit_and(context, entry, lhs, rhs);
            operation.deref_mut(context).attributes.0.insert(
                Identifier::try_from("discriminator").unwrap(),
                integer(context, index as u32),
            );
        }
        let returned = ReturnOp::new(context, vec![lhs]);
        append(context, entry, returned);
        verify_op(&function, context).unwrap();
        let mut ledger = Ledger::generous();
        // Each pair has <= 24 structured visits; geometric Vec/table growth
        // contributes < 32 visits/pair. 80 leaves fixed entry/exit overhead.
        ledger.work_limit = 80 + 64 * count;
        assert_eq!(
            dominance_pure_cse_v1(
                function.get_operation(),
                context,
                &mut DomInfo::default(),
                &mut ledger
            )
            .unwrap(),
            IRStatus::Unchanged
        );
        assert!(ledger.work <= 80 + 64 * count);
        assert_eq!(ledger.live, 0);
    }
}

fn empty_function(context: &mut Context) -> FuncOp {
    let function = function(context, "empty", vec![], vec![]);
    let entry = function.get_entry_block(context);
    let returned = ReturnOp::new(context, vec![]);
    append(context, entry, returned);
    function
}

#[test]
fn independent_empty_cfg_exact_and_one_short_work_and_storage_preserve_prefix() {
    // One Func region, one entry block, one zero-width Return: root 3, container
    // 1, region 1, census 2, DomInfo boundary 1, region entry 1, visit reserve 2,
    // Enter 1, Return 1, Exit 1. No keys/table/replacement storage is allocated.
    const WORK: usize = 14;
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
        let result = dominance_pure_cse_v1(
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
                panic!("original denial expected")
            };
            assert_eq!(Some(error.clone()), ledger.failure);
            if !error.storage {
                assert_eq!(ledger.work, 11 + WORK - 1);
            }
        }
    }
}

struct Observer {
    events: Arc<Mutex<Vec<(u8, bool)>>>,
    panic_after_erase: bool,
}

impl RewriteObserver for Observer {
    fn observe(&mut self, context: &Context, event: RewriteEvent) {
        match event {
            RewriteEvent::ValueReplaced { old, new } => {
                self.events.lock().unwrap().push((
                    0,
                    old.get_defining_block(context) != new.get_defining_block(context),
                ));
            }
            RewriteEvent::OperationErased(operation) => {
                self.events
                    .lock()
                    .unwrap()
                    .push((1, operation.try_deref(context).is_ok()));
                assert!(!self.panic_after_erase, "requested observer unwind");
            }
            _ => panic!("unexpected rewrite event"),
        }
    }
}

#[test]
fn cross_block_observer_uses_live_value_replacement_then_erasure_and_unwind_releases_storage() {
    for unwind in [false, true] {
        let mut context = Context::new();
        let fixture = diamond(&mut context, true);
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut ledger = Ledger::generous();
        ledger.live = 97;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dominance_pure_cse_with_observer_v1(
                fixture.function.get_operation(),
                &mut context,
                &mut DomInfo::default(),
                &mut ledger,
                Box::new(Observer {
                    events: events.clone(),
                    panic_after_erase: unwind,
                }),
            )
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(ledger.live, 97);
        let events = events.lock().unwrap();
        assert_eq!(events.len(), if unwind { 2 } else { 6 });
        for pair in events.chunks_exact(2) {
            assert_eq!(pair, &[(0, true), (1, true)]);
        }
        // An unwound raw candidate is discarded, never treated as committed O.
        if !unwind {
            verify_op(&fixture.function, &context).unwrap();
        }
        drop(context);
    }
}

#[test]
fn actual_vec_capacity_excess_is_accounted_before_later_work_or_denial() {
    for limit in [8, 7] {
        let mut ledger = Ledger::generous();
        ledger.live = 97;
        ledger.storage_limit = 97 + limit * size_of::<u64>();
        {
            let mut scope = StorageScope::new(&mut ledger);
            scope.reserve(4 * size_of::<u64>()).unwrap();
            // Inject an allocator outcome with a real eight-slot allocation;
            // do not assume this host allocator naturally overallocates Vec.
            let values = Vec::<u64>::with_capacity(8);
            let result = reconcile_vec_capacity::<u64, _>(4, values.capacity(), &mut scope);
            assert_eq!(result.is_ok(), limit == 8);
            if let Err(DominanceCseErrorV1::Budget(error)) = result {
                assert_eq!(
                    error,
                    Denied {
                        storage: true,
                        prior: 97 + 4 * size_of::<u64>(),
                        request: 4 * size_of::<u64>()
                    }
                );
            }
            drop(values);
        }
        assert_eq!(ledger.work, 0);
        assert_eq!(ledger.live, 97);
        if limit == 8 {
            assert_eq!(ledger.peak, 97 + 8 * size_of::<u64>());
        } else {
            assert!(ledger.failure.is_some());
        }
    }
}

struct ErasureGate(Arc<AtomicBool>);

impl RewriteObserver for ErasureGate {
    fn observe(&mut self, _: &Context, event: RewriteEvent) {
        if matches!(event, RewriteEvent::OperationErased(_)) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
}

struct AfterErasureBudget {
    ledger: Ledger,
    erased: Arc<AtomicBool>,
}

impl DominanceCseBudgetV1 for AfterErasureBudget {
    type Error = Denied;

    fn charge_work(&mut self, work: usize) -> Result<(), Denied> {
        if self.erased.load(Ordering::SeqCst) && work != 0 {
            let error = Denied {
                storage: false,
                prior: self.ledger.work,
                request: work,
            };
            self.ledger.failure.get_or_insert(error.clone());
            Err(error)
        } else {
            self.ledger.charge_work(work)
        }
    }

    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Denied> {
        self.ledger.reserve_storage(bytes)
    }
    fn release_storage(&mut self, bytes: usize) {
        self.ledger.release_storage(bytes);
    }
}

#[test]
fn caller_denial_after_a_real_rewrite_preserves_error_and_releases_all_local_storage() {
    let mut context = Context::new();
    let fixture = diamond(&mut context, true);
    let erased = Arc::new(AtomicBool::new(false));
    let mut budget = AfterErasureBudget {
        ledger: Ledger::generous(),
        erased: erased.clone(),
    };
    budget.ledger.live = 97;
    let error = dominance_pure_cse_with_observer_v1(
        fixture.function.get_operation(),
        &mut context,
        &mut DomInfo::default(),
        &mut budget,
        Box::new(ErasureGate(erased)),
    )
    .unwrap_err();
    let DominanceCseErrorV1::Budget(error) = error else {
        panic!("original caller error")
    };
    assert_eq!(budget.ledger.failure, Some(error.clone()));
    assert_eq!(budget.ledger.work, error.prior);
    assert_eq!(budget.ledger.live, 97);
    assert_eq!(
        [fixture.left, fixture.right, fixture.join]
            .into_iter()
            .filter(|op| op.try_deref(&context).is_err())
            .count(),
        1
    );
    drop(context);
}

#[test]
fn pliron_pass_adapter_preserves_the_original_typed_caller_denial() {
    let context = &mut Context::new();
    let function = empty_function(context);
    let mut ledger = Ledger::generous();
    ledger.work_limit = 0;
    ledger.live = 97;
    let error = DominancePureCsePassV1::new(&mut ledger)
        .run(
            function.get_operation(),
            context,
            &mut AnalysisManager::default(),
        )
        .err()
        .expect("pass denied");
    let error = error
        .err
        .downcast_ref::<DominanceCseErrorV1<Denied>>()
        .expect("typed wrapper retained");
    let DominanceCseErrorV1::Budget(error) = error else {
        panic!("caller denial retained")
    };
    assert_eq!(Some(error.clone()), ledger.failure);
    assert_eq!(
        error,
        &Denied {
            storage: false,
            prior: 0,
            request: 1
        }
    );
    assert_eq!(ledger.live, 97);
    verify_op(&function, context).unwrap();
}

struct OrderObserver(Arc<Mutex<Vec<usize>>>);

impl RewriteObserver for OrderObserver {
    fn observe(&mut self, context: &Context, event: RewriteEvent) {
        if let RewriteEvent::OperationErased(operation) = event {
            let block = operation.deref(context).get_parent_block().unwrap();
            let region = block.deref(context).get_parent_region().unwrap();
            let ordinal = region
                .deref(context)
                .iter(context)
                .position(|candidate| candidate == block)
                .unwrap();
            self.0.lock().unwrap().push(ordinal);
        }
    }
}

#[test]
fn traversal_and_representatives_do_not_depend_on_randomized_hashmap_state() {
    let mut expected = None;
    for _ in 0..16 {
        let context = &mut Context::new();
        let fixture = diamond(context, true);
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut ledger = Ledger::generous();
        dominance_pure_cse_with_observer_v1(
            fixture.function.get_operation(),
            context,
            &mut DomInfo::default(),
            &mut ledger,
            Box::new(OrderObserver(order.clone())),
        )
        .unwrap();
        let observed = (order.lock().unwrap().clone(), ledger.work, ledger.peak);
        if let Some(expected) = &expected {
            assert_eq!(&observed, expected);
        } else {
            expected = Some(observed);
        }
        verify_op(&fixture.function, context).unwrap();
    }
}

mod adversarial {
    include!("dominance_cse_v1_adversarial_tests.rs");
}
