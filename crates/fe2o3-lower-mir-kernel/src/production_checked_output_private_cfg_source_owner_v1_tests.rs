use super::*;
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, ScalarBitsV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationLimitsV1, SimulationRaceAssessmentV1, SimulationRequestV1, SimulationTargetV1,
};

fn private_store(local: u32, stored: u128) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(local, U32),
            constant(U32, stored, 4),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    )
}

fn root_switch() -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: value(4, BOOL),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                edge(SemanticEdgeRoleV1::SwitchValue, 1),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
        )
        .unwrap(),
    }
}

fn helper_call(callee: u32, target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new(
            SemanticFunctionIdV1::from_index(callee),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(0, UNIT),
                edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn cross_source(shared: bool) -> ProductionPreRankedKirOwnerV1 {
    let (seed, _) = fixture_with_blocks_and_symbol(
        Fixture::Literal(true),
        shared,
        |_, blocks| blocks,
        |ordinal| {
            if shared {
                format!("private_cfg_{ordinal}")
            } else {
                "private_array_relation".to_owned()
            }
        },
        &[U32, U32],
    );
    let semantic = seed.source_semantic();
    let functions = semantic
        .functions()
        .iter()
        .enumerate()
        .map(|(ordinal, function)| {
            let root = function.role() == SemanticFunctionRoleV1::KernelRoot;
            let tag = 30 + ordinal as u8 * 10;
            let mut locals = function.locals().to_vec();
            let abi = if root {
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 13; 32]),
                    U32,
                    SemanticLocalRoleV1::Argument(0),
                    function.source(),
                ));
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 14; 32]),
                    BOOL,
                    SemanticLocalRoleV1::Temporary,
                    function.source(),
                ));
                SemanticFunctionAbiV1::from_rustc(
                    function.abi().identity(),
                    semantic.target().identity(),
                    SemanticCanonAbiV1::GpuKernel,
                    SemanticExternAbiV1::GpuKernel,
                    false,
                    false,
                    1,
                    vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        U32,
                        SemanticAbiPassModeV1::Direct(
                            SemanticAbiValueAttributesV1::new(
                                SemanticAbiRegularAttributesV1::new(
                                    false, None, false, false, false, true,
                                ),
                                SemanticAbiExtensionV1::None,
                                0,
                                None,
                            )
                            .unwrap(),
                        ),
                    ))],
                    SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
                )
                .unwrap()
                .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
                .unwrap()
            } else {
                function.abi().clone()
            };
            let read = assignment(2, U32, SemanticRvalueKindV1::Use(value(1, U32)));
            let blocks = if root {
                // Only the input selects whether the read is executed. Calls and
                // literal assertions may disappear without merging Store/Load.
                let after_read = if !shared {
                    literal_terminator(true, 2)
                } else {
                    helper_call(0, if ordinal == 1 { 3 } else { 2 })
                };
                let mut blocks = vec![
                    block(
                        tag + 1,
                        vec![
                            private_store(1, 99),
                            assignment(
                                4,
                                BOOL,
                                SemanticRvalueKindV1::Binary {
                                    operation: SemanticBinaryOpV1::Equal,
                                    left: value(3, U32),
                                    right: constant(U32, 0, 4),
                                },
                            ),
                        ],
                        root_switch(),
                    ),
                    block(tag + 2, vec![read], after_read),
                    block(tag + 3, vec![], SemanticTerminatorKindV1::Return),
                ];
                if shared && ordinal == 1 {
                    blocks.push(block(tag + 4, vec![], helper_call(2, 2)));
                }
                blocks
            } else {
                // Both direct helpers need real private effects so their calls
                // and definitions survive import into the deletion candidate.
                vec![
                    block(
                        tag + 1,
                        vec![private_store(1, 77), read],
                        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
                    ),
                    block(tag + 2, vec![], SemanticTerminatorKindV1::Return),
                ]
            };
            let rebuilt = SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                abi,
                locals,
                function.entry(),
                blocks,
            )
            .unwrap();
            if root {
                rebuilt.with_kernel_entry(function.kernel_entry().unwrap().clone())
            } else {
                rebuilt
            }
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let launches: Vec<_> = ssa
        .source_semantic()
        .roots()
        .iter()
        .map(|root| {
            let tag = 30 + root.index() as u8 * 10;
            crate::ProductionSourceLaunchRootInputV1::new(
                match root.index() {
                    0 => "logical_0",
                    1 => "logical_1",
                    3 => "logical_3",
                    _ => unreachable!(),
                },
                [tag; 32],
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch =
        crate::ProductionSourceLaunchRosterV1::try_new(ssa.source_semantic(), &launches).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn cross_memory(module: &Module, expected: usize, expected_helpers: usize) {
    let mut pairs = 0;
    let mut helper_pairs = 0;
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        let root = module
            .kernels
            .iter()
            .any(|kernel| kernel.entry == function.id);
        let mut store = None;
        let mut read = None;
        let mut ordinal = 0;
        for block in &body.blocks {
            for operation in &block.operations {
                match operation.kind {
                    OperationKind::Store {
                        pointer,
                        value,
                        access,
                    } if access.address_space == AddressSpace::Private => {
                        assert!(store.replace((block.id, pointer, ordinal, value)).is_none());
                    }
                    OperationKind::Load { pointer, access }
                        if access.address_space == AddressSpace::Private =>
                    {
                        assert!(read.replace((block.id, pointer, ordinal)).is_none());
                    }
                    _ => {}
                }
                ordinal += 1;
            }
        }
        if let Some((read_block, pointer, read_ordinal)) = read {
            let (store_block, stored_pointer, store_ordinal, value) = store.unwrap();
            if root {
                assert_ne!(read_block, store_block);
            } else {
                assert_eq!(read_block, store_block);
            }
            assert_eq!(pointer, stored_pointer);
            assert!(store_ordinal < read_ordinal);
            let definitions: Vec<_> = body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.results.iter().any(|result| result.id == value))
                .collect();
            assert_eq!(definitions.len(), 1);
            assert_eq!(
                definitions[0].kind,
                OperationKind::Constant(Constant::U32(if root { 99 } else { 77 }))
            );
            assert_eq!(
                definitions[0].results[0].ty,
                Type::Scalar(fe2o3_kernel_ir::ScalarType::U32)
            );
            // There is exactly one Store in this function, so its ordinal is the
            // only possible latest anchor, not merely an equal-valued Store.
            let operations: Vec<_> = body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .collect();
            assert!(
                matches!(operations[store_ordinal].kind, OperationKind::Store { value: stored, .. } if stored == value)
            );
            assert!(
                matches!(operations[read_ordinal].kind, OperationKind::Load { pointer: loaded, .. } if loaded == pointer)
            );
            if root {
                pairs += 1;
            } else {
                helper_pairs += 1;
            }
        } else {
            assert!(store.is_none());
        }
    }
    assert_eq!(pairs, expected);
    assert_eq!(helper_pairs, expected_helpers);
}

fn source_anchor_spans(source: &ProductionPreRankedKirOwnerV1) {
    for root in source.source_launch().roots() {
        let root = root.selected_root();
        let declaration =
            &source.semantic_ssa().source_semantic().functions()[root.index() as usize];
        assert_eq!(declaration.abi().arguments().len(), 1);
        assert_eq!(declaration.locals()[3].ty(), U32);
        assert_eq!(
            declaration.locals()[3].role(),
            SemanticLocalRoleV1::Argument(0)
        );
        assert_eq!(declaration.locals()[4].ty(), BOOL);
        assert_eq!(
            declaration.locals()[4].role(),
            SemanticLocalRoleV1::Temporary
        );
        assert_eq!(
            declaration.blocks()[0].statements()[1],
            assignment(
                4,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Equal,
                    left: value(3, U32),
                    right: constant(U32, 0, 4),
                },
            )
        );
        assert_eq!(declaration.blocks()[0].terminator().kind(), &root_switch());
        let SemanticStatementKindV1::Store(store) = declaration.blocks()[0].statements()[0].kind()
        else {
            panic!("exact source Store")
        };
        assert_eq!(store.destination(), &place(1, U32));
        assert_eq!(store.value(), &constant(U32, 99, 4));
        let SemanticStatementKindV1::Assign(read) = declaration.blocks()[1].statements()[0].kind()
        else {
            panic!("exact source read")
        };
        assert_eq!(
            read.value().kind(),
            &SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32)))
        );
        let functions: Vec<_> = source
            .correspondence
            .lowered_functions()
            .iter()
            .filter(|row| row.correspondence_owner() == root && row.semantic_function() == root)
            .collect();
        assert_eq!(functions.len(), 1);
        let function = source
            .executable()
            .module()
            .functions
            .iter()
            .find(|function| &function.id == functions[0].kernel_ir_function())
            .unwrap();
        for block in [0_u32, 1] {
            let spans: Vec<_> = source
                .correspondence
                .statement_operation_spans()
                .iter()
                .filter(|span| {
                    span.correspondence_owner() == root
                        && span.semantic_function() == root
                        && span.semantic_block().index() == block
                        && span.statement_ordinal() == 0
                })
                .collect();
            assert_eq!(spans.len(), 1);
            let span = spans[0];
            let physical = function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|body| body.id == span.kernel_ir_block())
                .unwrap();
            let first = span.first_operation_ordinal() as usize;
            let end = first + span.operation_count() as usize;
            let matched = physical.operations[first..end]
                .iter()
                .filter(|operation| {
                    matches!(
                (block, &operation.kind),
                (0, OperationKind::Store { access, .. }) | (1, OperationKind::Load { access, .. })
                    if access.address_space == AddressSpace::Private)
                })
                .count();
            assert_eq!(matched, 1);
        }
    }
}

fn helper_anchor_spans(source: &ProductionPreRankedKirOwnerV1, helper: u32, owners: &[u32]) {
    let helper = SemanticFunctionIdV1::from_index(helper);
    let declaration = &source.semantic_ssa().source_semantic().functions()[helper.index() as usize];
    assert_eq!(declaration.role(), SemanticFunctionRoleV1::InternalHelper);
    let statements = declaration.blocks()[0].statements();
    assert_eq!(statements.len(), 2);
    let SemanticStatementKindV1::Store(store) = statements[0].kind() else {
        panic!("exact helper source Store")
    };
    assert_eq!(store.destination(), &place(1, U32));
    assert_eq!(store.value(), &constant(U32, 77, 4));
    assert_eq!(
        statements[1],
        assignment(2, U32, SemanticRvalueKindV1::Use(value(1, U32)))
    );
    assert_eq!(
        declaration.blocks()[0].terminator().kind(),
        &SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1))
    );
    let functions: Vec<_> = source
        .correspondence
        .lowered_functions()
        .iter()
        .filter(|row| row.semantic_function() == helper)
        .collect();
    assert_eq!(
        functions
            .iter()
            .map(|row| row.correspondence_owner().index())
            .collect::<Vec<_>>(),
        owners
    );
    for row in functions {
        let function = source
            .executable()
            .module()
            .function(row.kernel_ir_function())
            .unwrap();
        for statement in [0_u32, 1] {
            let spans: Vec<_> = source
                .correspondence
                .statement_operation_spans()
                .iter()
                .filter(|span| {
                    span.correspondence_owner() == row.correspondence_owner()
                        && span.semantic_function() == helper
                        && span.semantic_block().index() == 0
                        && span.statement_ordinal() == statement
                })
                .collect();
            assert_eq!(spans.len(), 1);
            let span = spans[0];
            let block = function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == span.kernel_ir_block())
                .unwrap();
            let first = span.first_operation_ordinal() as usize;
            let end = first + span.operation_count() as usize;
            assert_eq!(
                block.operations[first..end]
                    .iter()
                    .filter(|operation| matches!(
                        (statement, &operation.kind),
                        (0, OperationKind::Store { access, .. })
                            | (1, OperationKind::Load { access, .. })
                            if access.address_space == AddressSpace::Private
                    ))
                    .count(),
                1
            );
        }
    }
}

fn compare_sim(before: &[u8], after: &[u8], symbols: &[&str]) {
    use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12;
    let first = AdmittedSimulationModuleV1::admit_v12(
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(before.to_vec()).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    let last = AdmittedSimulationModuleV1::admit_v12(
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(after.to_vec()).unwrap(),
        SimulationLimitsV1::default(),
    )
    .unwrap();
    for (symbol, control) in symbols
        .iter()
        .flat_map(|symbol| [0_u32, 1, u32::MAX].map(|control| (symbol, control)))
    {
        let kernel = first
            .module()
            .kernels
            .iter()
            .find(|kernel| kernel.id.as_str() == *symbol)
            .unwrap();
        let final_kernel = last
            .module()
            .kernels
            .iter()
            .find(|candidate| candidate.id == kernel.id)
            .unwrap();
        assert_eq!(kernel.domain, final_kernel.domain);
        assert_eq!(kernel.workgroup_size, final_kernel.workgroup_size);
        assert_eq!(kernel.domain.rank(), 1);
        let mut grid = [1_u64; 3];
        for (axis, extent) in kernel.domain.extents().enumerate() {
            let fe2o3_kernel_ir::LaunchExtent::Static(extent) = extent else {
                panic!("expected the actual static source launch")
            };
            grid[axis] = u64::from(extent);
        }
        let workgroup = kernel.workgroup_size.unwrap();
        let workgroup = [workgroup.x, workgroup.y, workgroup.z];
        assert_eq!(grid, [64, 1, 1]);
        assert_eq!(workgroup, [64, 1, 1]);
        let invocations = grid.into_iter().try_fold(1_u64, u64::checked_mul).unwrap();
        assert_eq!(invocations, 64);
        let arguments = vec![SimulationArgumentV1::Scalar(ScalarBitsV1::u32(control))];
        let request = SimulationRequestV1::new(*symbol, grid, workgroup, arguments.clone());
        let run = |simulation: &AdmittedSimulationModuleV1| {
            simulation
                .simulate(
                    &request,
                    SimulationTargetV1::amdgpu_64(),
                    SimulationLimitsV1::default(),
                )
                .unwrap()
        };
        let original = run(&first);
        let output = run(&last);
        assert_eq!(output, run(&last));
        assert_eq!(original.arguments(), output.arguments());
        assert_eq!(original.shared_buffers(), output.shared_buffers());
        assert_eq!(original.invocations_executed(), invocations);
        assert_eq!(output.invocations_executed(), invocations);
        // The fixture has no external buffers; physical/source anchor assertions
        // above remain the nontrivial memory oracle, not empty copied-back data.
        assert_eq!(original.arguments(), arguments);
        for execution in [&original, &output] {
            assert!(matches!(
                execution.conflict_assessment(),
                SimulationConflictAssessmentV1::NoConflictsObserved
            ));
            assert!(matches!(
                execution.race_assessment(),
                SimulationRaceAssessmentV1::NoRacesObserved { .. }
            ));
        }
    }
}

#[test]
fn genuine_direct_scalar_cross_block_source_replays_on_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let source = cross_source(false);
        assert_eq!(source.semantic_ssa().source_semantic().functions().len(), 1);
        assert_eq!(source.source_launch().roots().len(), 1);
        assert_eq!(
            source.semantic_ssa().source_semantic().functions()[0].blocks()[1]
                .terminator()
                .kind(),
            &literal_terminator(true, 2)
        );
        source_anchor_spans(&source);
        cross_memory(source.executable().module(), 1, 0);
        let original = source.executable().canonical().canonical_bytes().to_vec();
        with_prepared(
            prepare(array_output_ranked_receipt_v1(source), profile, None),
            |input, budget| {
                let floor = budget.storage();
                cross_memory(input.bound.module(), 1, 0);
                let owner = AdmittedOutput::try_admit_general_v1(
                    input.receipt,
                    input.bound,
                    input.output,
                    budget,
                )
                .unwrap();
                cross_memory(owner.output().module(), 1, 0);
                assert_eq!(owner.kernels().len(), 1);
                assert!(owner.kernels()[0].accesses().is_empty());
                owner.verify_equivalence(budget).unwrap();
                assert_eq!(budget.storage(), floor);
                assert!(!owner.grants_artifact_or_launch_authority());
                compare_sim(
                    &original,
                    owner.output().canonical().canonical_bytes(),
                    &["private_array_relation"],
                );
            },
        );
    }
}

fn unit_roots(
    source: &ProductionPreRankedKirOwnerV1,
) -> Vec<ProductionRankedSemanticProjectionRootV1> {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    source
        .source_launch()
        .roots()
        .iter()
        .map(|root| {
            let function = &source.semantic_ssa().source_semantic().functions()
                [root.selected_root().index() as usize];
            let name =
                std::str::from_utf8(function.kernel_entry().unwrap().export_symbol().as_bytes())
                    .unwrap();
            let layout = root.layout();
            let kernel = ProductionRankedKernelV1::new(
                name,
                0,
                vec![ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: layout.grid_identity(),
                        global_extents: layout.global_extents(),
                        workgroup_extents: layout.workgroup_extents(),
                        subgroup_size: layout.subgroup_size(),
                        full_physical_workgroups: layout.full_physical_workgroups(),
                    }],
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            let lowering = compile_ranked_kernel_for_lowering_v1(
                ProductionConstructionV1::ranked_kernel("private_cfg_unit_stage", kernel).unwrap(),
                ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            assert!(lowering.all_mandatory_reports_are_clean());
            ProductionRankedSemanticProjectionRootV1::new(
                root.selected_root(),
                root.source_rank(),
                lowering,
                "constructed source scalar slots; entry-only ranked graph".to_owned(),
                vec![],
                vec![],
            )
        })
        .collect()
}

#[test]
fn genuine_unit_local_keeps_root_cross_block_memory_and_original_source_sites() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let original = cross_source(true);
        assert_eq!(
            original.semantic_ssa().source_semantic().functions().len(),
            4
        );
        assert_eq!(original.source_launch().roots().len(), 2);
        assert_eq!(
            original.helper_source_policy_v1(),
            ProductionHelperSourcePolicyV1::UnitLocal
        );
        let semantic = original.semantic_ssa().source_semantic();
        let calls: Vec<_> = semantic
            .functions()
            .iter()
            .enumerate()
            .flat_map(|(caller, function)| {
                function.blocks().iter().filter_map(move |block| {
                    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                        return None;
                    };
                    let SemanticCallableDeclV1::Defined { function } =
                        &semantic.callables()[call.callee().index() as usize]
                    else {
                        panic!("expected a retained source helper definition")
                    };
                    Some((caller, function.index()))
                })
            })
            .collect();
        assert_eq!(calls, vec![(1, 0), (1, 2), (3, 0)]);
        source_anchor_spans(&original);
        helper_anchor_spans(&original, 0, &[1, 3]);
        helper_anchor_spans(&original, 2, &[1]);
        cross_memory(original.executable().module(), 2, 2);
        let physical_calls = original
            .executable()
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::Call { .. }))
            .count();
        assert_eq!(physical_calls, 3);
        let before = original.executable().canonical().canonical_bytes().to_vec();
        let roots = unit_roots(&original);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        let input = ProductionUnitLocalErasedSourceOwnerV1::input_storage_floor_v1(
            &original,
            &roots,
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(FLOOR + input).unwrap();
        let (source, extra) =
            ProductionUnitLocalErasedSourceOwnerV1::try_produce_v1(original, roots, &mut budget)
                .unwrap();
        budget.reserve_storage(extra.retained_storage()).unwrap();
        assert_eq!(source.deleted_call_count(), 3);
        assert_eq!(source.deleted_function_count(), 2);
        assert_eq!(
            source
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes(),
            before
        );
        cross_memory(source.erased().module(), 2, 0);
        source.verify_equivalence(&mut budget).unwrap();
        let binding =
            dialect_amdgcn::bind_production_target_v1(source.erased().module(), profile).unwrap();
        let (bound, storage) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                binding.module(),
                &mut budget,
            )
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, &mut budget)
                .unwrap();
        budget.reserve_storage(checked.retained_storage()).unwrap();
        let floor = budget.storage();
        let owner = crate::ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1::try_admit_v1(
            source,
            bound,
            checked,
            &mut budget,
        )
        .unwrap();
        cross_memory(owner.output().module(), 2, 0);
        assert_eq!(owner.kernels().len(), 2);
        assert!(
            owner
                .kernels()
                .iter()
                .all(|report| report.accesses().is_empty())
        );
        assert_eq!(
            owner
                .original_source()
                .executable()
                .canonical()
                .canonical_bytes(),
            before
        );
        owner.verify_equivalence(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(!owner.grants_artifact_or_launch_authority());
        compare_sim(
            &before,
            owner.output().canonical().canonical_bytes(),
            &["private_cfg_1", "private_cfg_3"],
        );
    }
}

#[test]
fn genuine_cross_block_source_still_refuses_verified_store_value_substitution() {
    fn mutate(module: &mut Module) {
        let mut found = 0;
        for function in &mut module.functions {
            let Some(body) = &mut function.body else {
                continue;
            };
            for block in &mut body.blocks {
                for operation in &mut block.operations {
                    if operation.kind == OperationKind::Constant(Constant::U32(99)) {
                        operation.kind = OperationKind::Constant(Constant::U32(100));
                        found += 1;
                    }
                }
            }
        }
        assert_eq!(found, 1);
    }
    let input = prepare(
        array_output_ranked_receipt_v1(cross_source(false)),
        Profile::Gfx942,
        Some(mutate),
    );
    with_prepared(input, |input, budget| {
        let floor = budget.storage();
        assert!(matches!(
            AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, budget),
            Err(AdmissionError::Coordinates(_))
        ));
        assert_eq!(budget.storage(), floor);
    });
}
