use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiRegularAttributesV1, SemanticAbiValueAttributesV1,
};

include!("production_semantic_kir_v1/semantic_ssa_prepared_input_transport_v1_tests.rs");

#[derive(Clone, Copy)]
enum EntryShape {
    Acyclic,
    EmptyCycle,
    ScalarCycle,
    UnreachableCycle,
    SparseCycle,
    TwoBackedges,
}

fn entry_owner(shape: EntryShape) -> ProductionSemanticSsaOwnerV1 {
    let original = noop_semantic_owner(&["invocation_entry"]);
    let old = &original.semantic().functions()[0];
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let source = SemanticSourceProvenanceV1::unavailable();
    let value = || SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], u32_ty).unwrap();
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let goto = |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let increment = || {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                value(),
                SemanticRvalueV1::new(
                    u32_ty,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: SemanticOperandV1::Copy(value()),
                        right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            u32_ty,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(1, 4).unwrap(),
                            ),
                        )),
                    },
                ),
            )),
        )
    };
    let switch = |backedge, exit| SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(value()),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, exit),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, backedge),
        )
        .unwrap(),
    };
    let entry = if matches!(shape, EntryShape::SparseCycle) {
        2
    } else {
        0
    };
    let blocks = match shape {
        EntryShape::Acyclic => vec![block(40, vec![], SemanticTerminatorKindV1::Return)],
        EntryShape::EmptyCycle => vec![block(40, vec![], goto(0))],
        EntryShape::ScalarCycle => vec![
            block(40, vec![increment()], switch(0, 1)),
            block(41, vec![], SemanticTerminatorKindV1::Return),
        ],
        EntryShape::UnreachableCycle => vec![
            block(40, vec![], SemanticTerminatorKindV1::Return),
            block(41, vec![], goto(0)),
        ],
        EntryShape::SparseCycle => vec![
            block(40, vec![], SemanticTerminatorKindV1::Unreachable),
            block(41, vec![], SemanticTerminatorKindV1::Unreachable),
            block(42, vec![increment()], switch(2, 3)),
            block(43, vec![], SemanticTerminatorKindV1::Return),
        ],
        EntryShape::TwoBackedges => vec![
            block(
                40,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(value()),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![
                            SemanticSwitchTargetV1::new(
                                0,
                                edge(SemanticEdgeRoleV1::SwitchValue, 3),
                            ),
                            SemanticSwitchTargetV1::new(
                                1,
                                edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            ),
                        ],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(41, vec![increment()], goto(0)),
            block(42, vec![increment()], goto(0)),
            block(43, vec![], SemanticTerminatorKindV1::Return),
        ],
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([30; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            u32_ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        ))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([20; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([21; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([22; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([23; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([24; 32]),
        source,
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([31; 32]),
                unit,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([32; 32]),
                u32_ty,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(entry),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(old.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        vec![
            unit_type(),
            plain_bit_scalar_type(
                10,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
        ],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn original_entry(
    owner: &ProductionSemanticSsaOwnerV1,
) -> (
    &SemanticFunctionDeclV1,
    &ProductionSemanticSsaFunctionPlanV1,
) {
    (
        &owner.source_semantic().functions()[0],
        owner
            .plan_for_function(SemanticFunctionIdV1::from_index(0))
            .unwrap(),
    )
}

#[test]
fn invocation_layout_preserves_source_trap_and_sparse_coordinates() {
    for (shape, blocks, entry, predecessors, arguments) in [
        (EntryShape::Acyclic, 1, 0, 0, 0),
        (EntryShape::EmptyCycle, 1, 0, 1, 0),
        (EntryShape::ScalarCycle, 2, 0, 1, 1),
        (EntryShape::UnreachableCycle, 2, 0, 0, 0),
        (EntryShape::SparseCycle, 4, 2, 1, 1),
        (EntryShape::TwoBackedges, 4, 0, 2, 1),
    ] {
        let owner = entry_owner(shape);
        let (source, ssa) = original_entry(&owner);
        for trap in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(10000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 10000);
            with_invocation_entry_plan_v1(
                source,
                ssa,
                SemanticEmissionPlacementV1 {
                    first_block: 17,
                    first_value: 100,
                },
                trap,
                &mut budget,
                |plan, budget| {
                    assert_eq!(INVOCATION_ENTRY_RELATION_VERSION_V1, 1);
                    assert_eq!(plan.layout.source_entry, BlockId(17 + entry));
                    assert_eq!(plan.layout.entry_predecessors, predecessors);
                    assert_eq!(plan.layout.trap, trap.then_some(BlockId(17 + blocks)));
                    let expected =
                        (predecessors != 0).then_some(BlockId(17 + blocks + u32::from(trap)));
                    assert_eq!(plan.layout.preheader, expected);
                    assert_eq!(
                        plan.invocation_block(),
                        expected.unwrap_or(BlockId(17 + entry))
                    );
                    assert_eq!(
                        plan.layout.next_block,
                        17 + blocks + u32::from(trap) + u32::from(predecessors != 0)
                    );
                    assert_eq!(plan.entry_arguments().len(), arguments);
                    assert!(std::ptr::eq(
                        plan.entry_arguments(),
                        ssa.plan().entry_arguments()
                    ));
                    plan.check_arguments(ssa.plan().entry_arguments(), budget)
                },
            )
            .unwrap();
            assert_eq!(budget.storage(), 0);
        }
    }
}

#[test]
fn original_entry_relation_rejects_foreign_source_plan_ledger_and_argument_roster() {
    let owner = entry_owner(EntryShape::ScalarCycle);
    let foreign = entry_owner(EntryShape::ScalarCycle);
    let (source, ssa) = original_entry(&owner);
    let (other_source, other_ssa) = original_entry(&foreign);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10000);
    with_invocation_entry_plan_v1(
        source,
        ssa,
        SemanticEmissionPlacementV1::default(),
        false,
        &mut budget,
        |plan, budget| {
            plan.check_source(source, ssa, budget)?;
            assert!(plan.check_source(other_source, ssa, budget).is_err());
            assert!(plan.check_source(source, other_ssa, budget).is_err());
            assert!(plan.check_arguments(&[], budget).is_err());
            let original = plan.entry_arguments()[0];
            assert!(plan.check_arguments(&[original, original], budget).is_err());
            let wrong_value = SsaArgumentV1::new(
                original.variable(),
                SsaValueV1::BlockArgument {
                    block: SsaBlockIdV1::new(source.entry().index()),
                    variable: original.variable(),
                },
            );
            assert!(plan.check_arguments(&[wrong_value], budget).is_err());
            let mut other_work = CanonicalKernelIrWorkBudgetV1::new(1000);
            let mut other = ArgumentBudgetV1::new(&mut other_work, 1000);
            assert!(
                plan.check_arguments(plan.entry_arguments(), &mut other)
                    .is_err()
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), 0);
}

fn independent_entry_bytes() -> usize {
    use std::mem::{align_of, size_of};
    let align = align_of::<usize>().max(align_of::<Option<BlockId>>());
    let layout_fields =
        3 * size_of::<u32>() + 2 * size_of::<Option<BlockId>>() + size_of::<usize>();
    let layout = layout_fields.div_ceil(align) * align;
    2 * size_of::<&()>() + size_of::<usize>() + layout
}

// Count from the independently admitted inputs and the named scan schedule,
// not a successful run's observed work or peak.
fn independent_entry_work(shape: EntryShape, ssa: &ProductionSemanticSsaFunctionPlanV1) -> usize {
    let (blocks, reachable_edges) = match shape {
        EntryShape::Acyclic => (1, 0),
        EntryShape::EmptyCycle => (1, 1),
        EntryShape::ScalarCycle => (2, 2),
        EntryShape::UnreachableCycle => (2, 0),
        EntryShape::SparseCycle => (4, 2),
        EntryShape::TwoBackedges => (4, 5),
    };
    let definitions = ssa.plan().entry_definitions().len();
    let arguments = ssa.plan().entry_arguments().len();
    let lookup = definitions.checked_ilog2().unwrap_or(0) as usize + 2;
    4 + blocks + 2 * reachable_edges + definitions + arguments * (lookup + 2)
}

#[test]
fn entry_plan_has_independent_exact_and_one_short_work_storage_limits() {
    let bytes = independent_entry_bytes();
    assert_eq!(bytes, invocation_entry_plan_bytes_v1());
    for shape in [
        EntryShape::EmptyCycle,
        EntryShape::ScalarCycle,
        EntryShape::SparseCycle,
        EntryShape::TwoBackedges,
    ] {
        let owner = entry_owner(shape);
        let (source, ssa) = original_entry(&owner);
        let required = independent_entry_work(shape, ssa);
        for floor in [0, 73] {
            let prior_work = 11;
            for (work_short, storage_short) in [(false, false), (true, false), (false, true)] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(
                    prior_work + required - usize::from(work_short),
                );
                let mut budget =
                    ArgumentBudgetV1::new(&mut work, floor + bytes - usize::from(storage_short));
                budget.charge_work(prior_work).unwrap();
                budget.reserve_storage(floor).unwrap();
                let result = with_invocation_entry_plan_v1(
                    source,
                    ssa,
                    SemanticEmissionPlacementV1::default(),
                    true,
                    &mut budget,
                    |_, _| Ok(()),
                );
                assert_eq!(result.is_ok(), !work_short && !storage_short);
                assert_eq!(budget.storage(), floor);
                if storage_short {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Storage(_)
                            )
                        )
                    ));
                    assert_eq!(budget.work(), prior_work);
                    assert_eq!(budget.peak_storage(), floor);
                    assert_eq!(budget.failed_storage(), Some(floor + bytes));
                } else {
                    assert_eq!(budget.peak_storage(), floor + bytes);
                    assert_eq!(budget.failed_storage(), None);
                    if work_short {
                        assert!(matches!(
                            result,
                            Err(
                                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                    ArgumentResourceV1::Work(_)
                                )
                            )
                        ));
                    } else {
                        assert_eq!(budget.work(), prior_work + required);
                    }
                }
                drop(budget);
                assert_eq!(
                    work.failed_work(),
                    work_short.then_some(prior_work + required)
                );
            }
        }
    }
}

#[test]
fn entry_scope_preserves_caller_floor_history_and_successful_output_charges() {
    let owner = entry_owner(EntryShape::ScalarCycle);
    let (source, ssa) = original_entry(&owner);
    let bytes = independent_entry_bytes();
    for behavior in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 73 + bytes + 13);
        budget.reserve_storage(73).unwrap();
        assert!(budget.reserve_storage(10000).is_err());
        assert!(budget.charge_work(20000).is_err());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_invocation_entry_plan_v1(
                source,
                ssa,
                SemanticEmissionPlacementV1::default(),
                false,
                &mut budget,
                |_, budget| {
                    budget.reserve_storage(13)?;
                    match behavior {
                        0 => Ok([0_u8; 13]),
                        1 => Err(invocation_entry_error_v1()),
                        _ => panic!("entry callback unwind"),
                    }
                },
            )
        }));
        match behavior {
            0 => {
                assert_eq!(result.unwrap().unwrap(), [0; 13]);
                assert_eq!(budget.storage(), 86);
                budget.release_storage(13).unwrap();
            }
            1 => assert!(result.unwrap().is_err()),
            _ => assert!(result.is_err()),
        }
        assert_eq!(budget.storage(), 73);
        assert_eq!(budget.peak_storage(), 73 + bytes + 13);
        assert_eq!(budget.failed_storage(), Some(10073));
        drop(budget);
        assert_eq!(work.failed_work(), Some(20000));
    }
}

#[test]
fn full_source_interval_and_synthetics_are_reserved_before_next_instance() {
    let owner = entry_owner(EntryShape::SparseCycle);
    let (source, ssa) = original_entry(&owner);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10000);
    let first = with_invocation_entry_plan_v1(
        source,
        ssa,
        SemanticEmissionPlacementV1 {
            first_block: 17,
            first_value: 100,
        },
        true,
        &mut budget,
        |plan, _| Ok(plan.layout),
    )
    .unwrap();
    let second = with_invocation_entry_plan_v1(
        source,
        ssa,
        SemanticEmissionPlacementV1 {
            first_block: first.next_block,
            first_value: 200,
        },
        true,
        &mut budget,
        |plan, _| Ok(plan.layout),
    )
    .unwrap();
    assert_eq!(first.next_block, 23);
    assert_eq!(first.preheader, Some(BlockId(22)));
    assert_eq!(second.first_block, 23);
    assert_eq!(second.source_entry, BlockId(25));
    assert_eq!(second.preheader, Some(BlockId(28)));
    assert_eq!(second.next_block, 29);
    assert!(
        with_invocation_entry_plan_v1(
            source,
            ssa,
            SemanticEmissionPlacementV1 {
                first_block: u32::MAX - 5,
                first_value: 0
            },
            true,
            &mut budget,
            |_, _| Ok(())
        )
        .is_err()
    );
    assert_eq!(budget.storage(), 0);
}
