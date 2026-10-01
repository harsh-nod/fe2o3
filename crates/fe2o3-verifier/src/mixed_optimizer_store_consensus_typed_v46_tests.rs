use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirPrivateByteAnalysisV38 as Physical, CanonicalKirPrivateByteLimitsV38,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    FormalIndexWidth, Function as KirFunction, MemoryAccess, Module, Operation as KirOperation,
    ScalarType, Signature, StorageLayoutLimitsV1, Terminator, Type, ValueDef, ValueId,
};

const LIMIT: usize = 256 * 1024 * 1024;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 0,
    edges: 0,
    containment_depth: 0,
    object_bytes: 0,
};

#[path = "mixed_optimizer_storage_consensus_v53_tests.rs"]
mod storage_v53;

fn layouts(module: &Module) -> StorageLayoutLimitsV1 {
    if module.storage_layouts.is_empty() {
        LAYOUTS
    } else {
        StorageLayoutLimitsV1 {
            rows: 8,
            edges: 8,
            containment_depth: 8,
            object_bytes: 16,
        }
    }
}

fn block(id: u32, operations: Vec<KirOperation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}

fn diamond(scalar: ScalarType) -> Module {
    let ty = Type::Scalar(scalar);
    let width = u32::from(scalar.bit_width().unwrap() / 8);
    let access = MemoryAccess::new(AddressSpace::Private, width);
    let store = KirOperation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(2),
            value: ValueId(0),
            access,
        },
    );
    let jump = || Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    };
    let mut module = Module::new("typed-store-consensus");
    module.functions.push(KirFunction::internal_helper(
        "typed_consensus",
        Signature::new(vec![ty.clone(), Type::BOOL], vec![ty.clone()]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(
                0,
                vec![KirOperation::effect_free(
                    ValueDef::new(
                        ValueId(2),
                        Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                    ),
                    OperationKind::Alloca {
                        element: ty.clone(),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: width,
                    },
                )],
                Terminator::ConditionalBranch {
                    condition: ValueId(1),
                    then_target: BlockId(1),
                    then_arguments: vec![],
                    else_target: BlockId(2),
                    else_arguments: vec![],
                },
            ),
            block(1, vec![store.clone()], jump()),
            block(2, vec![store], jump()),
            block(
                3,
                vec![KirOperation::effect_free(
                    ValueDef::new(ValueId(3), ty),
                    OperationKind::Load {
                        pointer: ValueId(2),
                        access,
                    },
                )],
                Terminator::Return {
                    values: vec![ValueId(3)],
                },
            ),
        ],
    ));
    module
}

fn assert_load_not_erased(module: Module) {
    let layouts = layouts(&module);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (input, receipt) =
        Owner::from_module_ref_with_verification_budget_v18(&module, layouts, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let tail = fe2o3_kernel_opt::prepare_owned_cross_block_forwarding_v18(
        &input,
        Default::default(),
        layouts,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(tail.retained_storage()).unwrap();
    assert!(tail.origins().iter().all(|row| row.store.is_none()));
    assert_eq!(input.identity(), tail.output().identity());
    let (checked, credit) = tail.replay_against(&input, &mut budget).unwrap();
    budget.reserve_storage(credit.retained_storage()).unwrap();
    assert!(checked.origins().iter().all(|row| row.store.is_none()));
}

#[test]
fn typed_forwarding_preserves_a_load_when_one_store_arm_did_not_execute() {
    let mut module = diamond(ScalarType::U32);
    module.functions[0].body.as_mut().unwrap().blocks[2]
        .operations
        .clear();
    assert_load_not_erased(module);
}

#[test]
fn typed_forwarding_does_not_reuse_a_store_across_a_reentered_allocation() {
    let mut module = diamond(ScalarType::U32);
    let function = module.functions[0].body.as_mut().unwrap();
    let alloca = function.blocks[0].operations.remove(0);
    let first_store = function.blocks[1].operations.remove(0);
    function.blocks[0].operations = vec![alloca, first_store];
    function.blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(0),
        arguments: vec![],
    });
    function.blocks[2].operations.clear();
    // Each visit to block zero creates fresh undefined bytes. Moving its Store
    // onto the backedge leaves the initial path without a reaching definition.
    let store = function.blocks[0].operations.pop().unwrap();
    function.blocks[1].operations.push(store);
    assert_load_not_erased(module);
}

// A test-only retained canonical owner, not an original-source frame receipt.
// Production must supply its owner-authenticated allocation-origin resolver.
struct FixtureAllocations<'a, 'owner>(&'a Inventory<'owner>);

impl ByteAllocationResolverV30 for FixtureAllocations<'_, '_> {
    fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()> {
        if !std::ptr::eq(owner, self.0.owner()) {
            return Err(mismatch());
        }
        out.budget.charge_work(1)?;
        Ok(())
    }

    fn site(
        &self,
        operation: Operation,
        out: &mut Writer<'_, '_>,
    ) -> Result<ByteAllocationSiteV30> {
        self.check_owner(self.0.owner(), out)?;
        out.budget
            .charge_work(self.0.operations().len().checked_ilog2().unwrap_or(0) as usize + 2)?;
        let index = self
            .0
            .operations()
            .binary_search_by_key(&operation, |row| row.coordinate)
            .map_err(|_| mismatch())?;
        if !matches!(
            self.0.operations()[index].operation.kind,
            OperationKind::Alloca { .. }
        ) {
            return Err(mismatch());
        }
        Ok(ByteAllocationSiteV30 {
            original: operation,
            physical_root_owner: 0,
        })
    }
}

fn with_pair(
    module: &Module,
    use_pair: impl FnOnce(
        &Inventory<'_>,
        &Inventory<'_>,
        &Pair<'_>,
        &Physical<'_, '_>,
        &Physical<'_, '_>,
        usize,
    ),
) {
    let layouts = layouts(module);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (input, receipt) =
        Owner::from_module_ref_with_verification_budget_v18(module, layouts, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let tail = fe2o3_kernel_opt::prepare_owned_cross_block_forwarding_v18(
        &input,
        Default::default(),
        layouts,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(tail.retained_storage()).unwrap();
    assert_eq!(
        tail.origins()
            .iter()
            .filter(|row| row.store.is_some())
            .count(),
        1
    );
    let (pair, receipt) = tail.replay_against(&input, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (before, receipt) = Inventory::derive_v18(&input, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (after, receipt) = Inventory::derive_v18(tail.output(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let limits = CanonicalKirPrivateByteLimitsV38 {
        max_boundaries: 4096,
    };
    let (before_physical, receipt) =
        fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
            &before,
            limits,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (after_physical, receipt) =
        fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(&after, limits, &mut budget)
            .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    use_pair(
        &before,
        &after,
        &pair,
        &before_physical,
        &after_physical,
        budget.storage(),
    );
}

fn run(
    floor: usize,
    work: usize,
    storage: usize,
    generate: impl FnOnce(&mut Writer<'_, '_>) -> Result<()>,
) -> (Result<String>, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    let result = (|| {
        budget.reserve_storage(floor + crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)?;
        let mut out = Writer::new(&mut budget)?;
        generate(&mut out)?;
        out.finish()
    })();
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn typed_store_consensus_emits_both_actual_interpreters_and_nominal_entry() {
    for scalar in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
    ] {
        with_pair(&diamond(scalar), |before, after, pair, bp, ap, floor| {
            let allocations = FixtureAllocations(before);
            let text = run(floor, LIMIT, LIMIT, |out| {
                let contracts = target_view_contracts_v38::TargetByteViewContractsV38::derive(
                    before,
                    FormalIndexWidth::Bits64,
                    out,
                )?;
                let final_contracts =
                    target_view_contracts_v38::TargetByteViewContractsV38::derive(
                        after,
                        FormalIndexWidth::Bits64,
                        out,
                    )?;
                assert_eq!(
                    generate(
                        before,
                        after,
                        pair,
                        bp,
                        ap,
                        &contracts,
                        &final_contracts,
                        FormalIndexWidth::Bits64,
                        1,
                        &allocations,
                        out
                    )?,
                    1
                );
                Ok(())
            })
            .0
            .unwrap();
            for name in [
                "byte_operation_0_3_v30",
                "byte_operation_1_3_v30",
                "forwarding_operation_3_v46",
                "forwarding_initial_relation_0_v46",
                "forwarding_initial_trace_0_v46",
                "forwarding_step_refines_0_v46",
                "forwarding_finite_trace_0_v46",
                "forwarding_store_fact_v46",
                "forwarding_selected_slot_v46",
                "byte_native_view_inputs_v38",
                "private_allocation_v30",
                "generation - 1",
                "MemoryOperationEffectV30::Trap",
                "MemoryOperationEffectV30::Copy",
                "MemoryOperationEffectV30::TagRead",
            ] {
                assert!(text.contains(name), "missing {name}");
            }
            assert!(text.contains("forwarding_before_fact_v46(3, n, little_endian)"));
            assert!(text.contains("observation.operation == MemorySourceOperationV30 { function: 0, block: 3, operation: 0 }"));
            assert!(text.contains("forall|allocation: MemoryAllocationV30| external.live.contains_key(allocation) ==> matches!(allocation, MemoryAllocationV30::External"));
            assert!(!text.contains("assume("));
            assert!(!text.contains("ConsensusOtherV46"));
            assert!(!text.contains("Seq<int>, memory: int"));
        });
    }
}

#[test]
fn typed_store_consensus_has_exact_full_work_and_storage_boundaries() {
    with_pair(
        &diamond(ScalarType::U32),
        |before, after, pair, bp, ap, floor| {
            let allocations = FixtureAllocations(before);
            let emit = |out: &mut Writer<'_, '_>| {
                let contracts = target_view_contracts_v38::TargetByteViewContractsV38::derive(
                    before,
                    FormalIndexWidth::Bits64,
                    out,
                )?;
                let final_contracts =
                    target_view_contracts_v38::TargetByteViewContractsV38::derive(
                        after,
                        FormalIndexWidth::Bits64,
                        out,
                    )?;
                let paid = out.budget.storage();
                assert_eq!(
                    generate(
                        before,
                        after,
                        pair,
                        bp,
                        ap,
                        &contracts,
                        &final_contracts,
                        FormalIndexWidth::Bits64,
                        1,
                        &allocations,
                        out
                    )?,
                    1
                );
                assert_eq!(out.budget.storage(), paid);
                Ok(())
            };
            let measured = run(floor, LIMIT, LIMIT, emit);
            let text = measured.0.unwrap();
            let exact = run(floor, measured.1, measured.2, emit);
            assert_eq!(exact.0.unwrap(), text);
            assert_eq!((exact.1, exact.2), (measured.1, measured.2));
            assert!(matches!(run(floor, measured.1 - 1, measured.2, emit).0,
            Err(Error::Resource(Resource::Work(error))) if error.actual() == measured.1 && error.limit() == measured.1 - 1));
            assert!(matches!(run(floor, measured.1, measured.2 - 1, emit).0,
            Err(Error::Resource(Resource::Storage(error))) if error.actual() == measured.2 && error.limit() == measured.2 - 1));
        },
    );
}

#[test]
fn typed_store_consensus_allocation_custody_retains_only_exact_pair_owners() {
    with_pair(
        &diamond(ScalarType::U32),
        |before, after, pair, _, _, floor| {
            let source = FixtureAllocations(before);
            run(floor, LIMIT, LIMIT, |out| {
                let allocation = Allocations::derive(before, after, pair, &source, out)?;
                allocation.check_owner(before.owner(), out)?;
                allocation.check_owner(after.owner(), out)?;
                let coordinate = before.operations()[0].coordinate;
                assert_eq!(
                    allocation.site(coordinate, out)?,
                    source.site(coordinate, out)?
                );
                assert!(
                    allocation
                        .site(before.operations()[1].coordinate, out)
                        .is_err()
                );
                allocation.check(out)?;
                out.budget.release_storage(1)?;
                assert!(matches!(
                    allocation.check(out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                out.budget.reserve_storage(1)?;
                assert!(matches!(
                    allocation.check(out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                Ok(())
            })
            .0
            .unwrap();
        },
    );
}

#[test]
fn typed_store_consensus_allocation_custody_rejects_foreign_ledger_before_debit() {
    with_pair(
        &diamond(ScalarType::U32),
        |before, after, pair, _, _, floor| {
            let source = FixtureAllocations(before);
            run(floor, LIMIT, LIMIT, |out| {
                let allocation = Allocations::derive(before, after, pair, &source, out)?;
                let original = (out.budget.work(), out.budget.storage());
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(allocation.required)?;
                let mut foreign = Writer::new(&mut budget)?;
                let paid = (foreign.budget.work(), foreign.budget.storage());
                assert!(matches!(
                    allocation.check_owner(after.owner(), &mut foreign),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!((foreign.budget.work(), foreign.budget.storage()), paid);
                assert!(matches!(
                    allocation.check_owner(before.owner(), out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!((out.budget.work(), out.budget.storage()), original);
                Ok(())
            })
            .0
            .unwrap();
        },
    );
}

#[test]
fn typed_store_consensus_refusal_projection_does_not_erase_trap_or_nominal_state() {
    let model = include_str!("mixed_optimizer_store_consensus_typed_v46.vrs");
    assert!(model.contains("MemoryStateV30 { pc: -3, values: seq![], ..state }"));
    assert!(model.contains("if state.valid { state }"));
    assert!(
        model.contains(
            "forwarding_terminal_state_v46(input) == forwarding_terminal_state_v46(output)"
        )
    );
    assert!(model.contains("state.memory.live.contains_key(pointer.allocation)"));
    assert!(model.contains("!forwarding_selected_allocation_v46(view.guards[i].allocation)"));
    assert!(model.contains("MemoryByteV37::PointerFragment { pointer, .. }"));
    assert!(model.contains("forwarding_pointer_separate_v46(object.relocations[offset].pointer)"));
    assert!(model.contains("if forwarding_removed_read_v46(observation) { seq![] }"));
    assert!(model.contains("else { seq![observation] }"));
    assert!(model.contains("MemoryOperationEffectV30::Refused | MemoryOperationEffectV30::Trap"));
    assert!(model.contains("| MemoryOperationEffectV30::TagRead { .. } => seq![observation]"));
    assert!(!model.contains("assume("));
}
