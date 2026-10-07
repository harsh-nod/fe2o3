//! Re-admitted inert source fixture, never rustc producer or proof authority.
use super::*;
use fe2o3_kernel_descriptor::{
    DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1, ScalarTypeV1,
    SourceTypeDescriptorV1, SourceTypeDescriptorV3, SourceTypeRecordV1, ValidName,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKirDefinitionCoordinateV1 as Definition, ExecutionOperationV15,
    ExecutionTileLayoutV1 as Layout, OperationKind, ScalarType,
};
use fe2o3_lower_mir_kernel::{
    ProductionContextCallBoundaryV29, ProductionContextRootInputV29,
    ProductionExecutionSourceInputV29, ProductionKernelArgumentAbiArgumentV18,
    ProductionKernelArgumentAbiInputV18, ProductionKernelArgumentAbiKindV18,
    ProductionKernelArgumentAbiRootV18, ProductionPendingScopedSourceOwnerV29,
    ProductionPreparedSourceV18, ProductionScopeCallKindV29, ProductionScopeCallableCandidateV29,
    ProductionScopeEventCandidateV29, ProductionScopeEventKindV29, ProductionSemanticKirLimitsV1,
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    ProductionSourceLaunchRosterV1, ProductionSourceTileExpansionV159 as TileExpansion,
    ProductionSourceTileLeafV162 as Leaf,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

#[path = "original_semantic_mir_expanded_component_models_v210_tests.rs"]
mod component_models;
#[path = "original_semantic_mir_execution_call_transfer_v286_tests.rs"]
mod execution_transfer_tests;

fn owner() -> ProductionSemanticSsaOwnerV1 {
    let bytes = include_bytes!("fixtures/original-tile-descriptor-v163.bin");
    let semantic = AdmittedInertSemanticMirV1::decode_exact_v29_canonical(
        bytes,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn boundary(block: usize, declaration: &SemanticBasicBlockV1) -> ProductionContextCallBoundaryV29 {
    let SemanticTerminatorKindV1::Call(call) = declaration.terminator().kind() else {
        panic!("fixture call boundary")
    };
    let destination = call.destination().unwrap();
    ProductionContextCallBoundaryV29 {
        block: SemanticBlockIdV1::from_index(block.try_into().unwrap()),
        statement_count: declaration.statements().len(),
        destination: destination.place().local(),
        destination_type: destination.place().ty(),
        target: destination.edge().target(),
        unwind: call.unwind(),
    }
}

fn prepared(budget: &mut Budget<'_>) -> Result<ProductionPreparedSourceV18> {
    prepared_with_owner(budget, owner)
}

fn prepared_with_owner(
    budget: &mut Budget<'_>,
    owner: impl Fn() -> ProductionSemanticSsaOwnerV1,
) -> Result<ProductionPreparedSourceV18> {
    let projection = owner();
    let semantic = projection.source_semantic();
    assert_eq!(semantic.roots(), &[SemanticFunctionIdV1::from_index(0)]);
    let root = &semantic.functions()[0];
    let helper = &semantic.functions()[1];
    let entry = root.kernel_entry().unwrap();
    let export = std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        semantic,
        &[ProductionSourceLaunchRootInputV1::new(
            export,
            *entry.kernel_binding_identity().as_bytes(),
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap();
    let SemanticTerminatorKindV1::Call(issue) = root.blocks()[0].terminator().kind() else {
        panic!("fixture issuer")
    };
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding: issuer,
        operation:
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::ContextIssue { context },
            ),
        ..
    } = &semantic.callables()[issue.callee().index() as usize]
    else {
        panic!("fixture nominal context issuer")
    };
    let SemanticTerminatorKindV1::Call(helper_call) = root.blocks()[1].terminator().kind() else {
        panic!("fixture logical helper")
    };
    let SemanticTerminatorKindV1::Call(derive) = helper.blocks()[0].terminator().kind() else {
        panic!("fixture workgroup derivation")
    };
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding: derive_binding,
        operation_identity,
        operation:
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::WorkgroupDerive {
                    context: derive_context,
                    workgroup,
                },
            ),
    } = &semantic.callables()[derive.callee().index() as usize]
    else {
        panic!("fixture original derive terminal")
    };
    assert_eq!(context, derive_context);
    let roots = [ProductionContextRootInputV29 {
        semantic_sha256: projection.source_semantic_sha256(),
        root: SemanticFunctionIdV1::from_index(0),
        root_identity: root.identity(),
        helper: SemanticFunctionIdV1::from_index(1),
        helper_identity: helper.identity(),
        issuer: issue.callee(),
        issuer_identity: issuer.identity(),
        context_type: *context,
        context_identity: semantic.types()[context.index() as usize].identity(),
        issuance: boundary(0, &root.blocks()[0]),
        helper_call: boundary(1, &root.blocks()[1]),
        helper_context_local: issue.destination().unwrap().place().local(),
        helper_arguments: helper_call.arguments(),
    }];
    let mut classes =
        vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    classes[helper_call.callee().index() as usize] =
        ProductionScopeCallableCandidateV29::Provider {
            function: SemanticFunctionIdV1::from_index(1),
            identity: helper.identity(),
        };
    classes[derive.callee().index() as usize] = ProductionScopeCallableCandidateV29::Derive {
        binding: derive_binding.identity(),
        operation: *operation_identity,
        context: *context,
        workgroup: *workgroup,
    };
    let mut events = vec![ProductionScopeEventCandidateV29 {
        function: SemanticFunctionIdV1::from_index(0),
        block: SemanticBlockIdV1::from_index(1),
        statement_count: root.blocks()[1].statements().len(),
        kind: ProductionScopeEventKindV29::Call {
            callee: helper_call.callee(),
            kind: ProductionScopeCallKindV29::Provider,
        },
    }];
    for (block, declaration) in helper.blocks().iter().enumerate() {
        let kind = match declaration.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => ProductionScopeEventKindV29::Call {
                callee: call.callee(),
                kind: if call.callee() == derive.callee() {
                    ProductionScopeCallKindV29::Derive
                } else {
                    ProductionScopeCallKindV29::Ordinary
                },
            },
            SemanticTerminatorKindV1::Return => ProductionScopeEventKindV29::Return,
            _ => panic!("fixture provider control"),
        };
        events.push(ProductionScopeEventCandidateV29 {
            function: SemanticFunctionIdV1::from_index(1),
            block: SemanticBlockIdV1::from_index(block.try_into().unwrap()),
            statement_count: declaration.statements().len(),
            kind,
        });
    }
    let mut offset = 0u64;
    let mut alignment = 1u64;
    let mut arguments = Vec::new();
    for (ordinal, ty) in root.abi().source_input_types().iter().enumerate() {
        let declaration = &semantic.types()[ty.index() as usize];
        let layout = declaration.layout();
        let align = layout.alignment_bytes();
        offset = offset.checked_add(align - 1).unwrap() & !(align - 1);
        alignment = alignment.max(align);
        let name = ValidName::new(format!("arg{ordinal}")).unwrap();
        let ordinal = ordinal.try_into().unwrap();
        let position = offset.try_into().unwrap();
        let kind = match declaration.shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }) => {
                let source =
                    SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U32));
                let layout =
                    DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U32));
                ProductionKernelArgumentAbiKindV18::Descriptor {
                    source: SourceTypeDescriptorV3::Scalar(ScalarTypeV1::U32),
                    argument: LogicalArgumentV1::scalar(ordinal, name, &source, &layout, position)
                        .unwrap(),
                }
            }
            SemanticTypeShapeV1::Pointer(pointer) => {
                assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
                assert_eq!(pointer.mutability(), SemanticMutabilityV1::Immutable);
                let SemanticTypeShapeV1::Slice { element } =
                    semantic.types()[pointer.pointee().index() as usize].shape()
                else {
                    panic!("fixture shared slice")
                };
                assert_eq!(
                    semantic.types()[element.index() as usize].shape(),
                    &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32
                    })
                );
                let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(
                    ScalarTypeV1::U32,
                ));
                let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(
                    ScalarTypeV1::U32,
                ));
                ProductionKernelArgumentAbiKindV18::Descriptor {
                    source: SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::U32),
                    argument: LogicalArgumentV1::shared_slice(
                        ordinal, name, &source, &layout, position,
                    )
                    .unwrap(),
                }
            }
            _ => panic!("unexpected fixture kernel ABI input"),
        };
        arguments.push(ProductionKernelArgumentAbiArgumentV18 {
            semantic_type_identity: declaration.identity(),
            kind,
        });
        offset = offset.checked_add(layout.size_bytes().unwrap()).unwrap();
    }
    let binding = entry.kernel_binding_identity();
    let abi_roots = [ProductionKernelArgumentAbiRootV18 {
        kernel_binding: binding.as_bytes(),
        export,
        arguments: &arguments,
        explicit_argument_bytes: offset.try_into().unwrap(),
        kernarg_alignment_bytes: alignment.try_into().unwrap(),
    }];
    Ok(
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
            owner(),
            launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: projection.source_semantic_sha256(),
                roots: &roots,
                classes: &classes,
                events: &events,
            },
            ProductionKernelArgumentAbiInputV18 { roots: &abi_roots },
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )?,
    )
}

fn owner_with_live_context_reborrow_v170() -> ProductionSemanticSsaOwnerV1 {
    let original = owner();
    let semantic = original.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[1];
    let mut blocks = helper.blocks().to_vec();
    let initial_borrow = blocks[0].statements()[0].clone();
    assert!(
        matches!(initial_borrow.kind(), SemanticStatementKindV1::Assign(assignment)
        if matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable, ..
        }))
    );
    let successor = &blocks[1];
    let mut statements = vec![initial_borrow];
    statements.extend_from_slice(successor.statements());
    blocks[1] = SemanticBasicBlockV1::new(
        successor.identity(),
        successor.source(),
        statements,
        successor.terminator().clone(),
    )
    .unwrap();
    functions[1] = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        helper.source(),
        helper.abi().clone(),
        helper.locals().to_vec(),
        helper.entry(),
        blocks,
    )
    .unwrap();
    let request = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap();
    let semantic = request
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

pub(in super::super) fn run_fixture(
    layout: Layout,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &SourceSlots<'_, '_>,
        &TileExpansion<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_fixture_with_plan(layout, work, storage, |_, slots, tile, out| {
        examine(slots, tile, out)
    })
}

pub(in super::super) fn run_fixture_with_plan(
    layout: Layout,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &TileExpansion<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_fixture_with_preparation(layout, work, storage, prepared, examine)
}

fn run_fixture_with_preparation(
    layout: Layout,
    work: usize,
    storage: usize,
    prepare: impl FnOnce(&mut Budget<'_>) -> Result<ProductionPreparedSourceV18>,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &TileExpansion<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_prepared(work, storage, prepare, |plan, out| {
        let source = plan.source(out)?;
        let result = source.with_checked_mixed_fixedpoint_optimization_v18(
            out.budget,
            |original, optimized, budget| {
                let floor = budget.storage();
                let tile = optimized.prepare_tile_expansion_v159(0, layout, budget)?;
                let tile_floor = budget.storage();
                let result = (|| {
                    let mut writer = Writer::new(budget)?;
                    let slots = SourceSlots::derive_tile_v162(plan, &tile, &mut writer)?;
                    slots.check_source(original, &mut writer)?;
                    examine(plan, &slots, &tile, &mut writer)
                })();
                if result.is_ok() {
                    budget.release_storage(budget.storage() - tile_floor)?;
                    tile.discard(budget)?;
                    assert_eq!(budget.storage(), floor);
                }
                result.map(|()| ((), 0))
            },
        );
        match result {
            Ok((owner, (), _receipt)) => {
                drop(owner);
                Ok(())
            }
            Err(fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18::Source(error)) => {
                Err(error.into())
            }
            Err(fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(error),
            )) => Err(error),
            Err(fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18::Adoption(
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Resource(error),
            )) => Err(error.into()),
            Err(fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18::Observation(
                fe2o3_pliron::KirNeutralOptimizationErrorV18::Execution(
                    fe2o3_pliron::PlironOptimizationErrorV12::Resources(error),
                ),
            )) => Err(error.into()),
            Err(fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18::Observation(
                fe2o3_pliron::KirNeutralOptimizationErrorV18::Mapping(
                    fe2o3_pliron::KirOptimizationMapErrorV12::Resources(error),
                ),
            )) => Err(error.into()),
            Err(error) => panic!("original tile fixture preparation: {error:?}"),
        }
    })
}

#[test]
fn original_context_reborrow_while_workgroup_live_is_refused_v170() {
    use fe2o3_lower_mir_kernel::{
        ProductionPendingScopedSourceErrorV29 as Pending, ProductionSemanticKirErrorV1 as Semantic,
        ProductionSourceOwnedViewErrorV18 as Source,
    };
    for layout in [Layout::Blocked, Layout::Striped] {
        let result = run_fixture_with_preparation(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            |budget| prepared_with_owner(budget, owner_with_live_context_reborrow_v170),
            generate_actual_tile_source_v168,
        );
        assert!(matches!(
            result.0,
            Err(Error::Source(Source::Source(Pending::Source(
                Semantic::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "nominal identity equations differ from their original source",
                }
            ))))
        ));
    }
}

fn check_actual_tile_slots(
    slots: &SourceSlots<'_, '_>,
    tile: &TileExpansion<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use super::super::slots::TileAllocationSlotsV164;
    use crate::mixed_optimizer_refinement_v26::semantics::byte_function_v30::ByteAllocationResolverV30;
    use fe2o3_kernel_analysis::CanonicalKirInventoryV18 as Inventory;
    let relation = slots.correspondence(out)?;
    let original = relation.inventory(out.budget)?;
    let loads = original
        .operations()
        .iter()
        .filter(|row| {
            matches!(
                row.operation.kind,
                OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 { .. }),
            )
        })
        .count();
    assert_eq!(
        loads, 2,
        "two actual source invocation instances, not shared model equations"
    );
    for row in original.operations() {
        let elements = match row.operation.kind {
            OperationKind::Execution(
                ExecutionOperationV15::MaskedTileLoadU32 { elements, .. }
                | ExecutionOperationV15::TileIntoFragmentU32 { elements, .. },
            ) => elements,
            _ => continue,
        };
        let definition = Definition::Result {
            operation: row.coordinate,
            result: 0,
        };
        for marker in [2, 3] {
            assert_eq!(
                slots.tile_leaf_v162(definition, &[marker], out)?,
                Leaf::Unit
            );
        }
        for field in 0..2 {
            for element in 0..u32::from(elements) {
                let leaf = slots.tile_leaf_v162(definition, &[field, element], out)?;
                assert_eq!(
                    leaf,
                    tile.aggregate_leaf_v162(definition, &[field, element], out.budget)?
                );
                let Leaf::Scalar { scalar, .. } = leaf else {
                    panic!("actual original tile component must retain its scalar type")
                };
                assert_eq!(
                    scalar,
                    if field == 0 {
                        ScalarType::U32
                    } else {
                        ScalarType::Bool
                    }
                );
            }
        }
    }
    let (inventory, receipt) = Inventory::derive_v18(tile.output(out.budget)?, out.budget)?;
    out.budget.reserve_storage(receipt.retained_storage())?;
    assert!(!inventory.operations().iter().any(|row| matches!(
        row.operation.kind,
        OperationKind::Execution(
            ExecutionOperationV15::MaskedTileLoadU32 { .. }
                | ExecutionOperationV15::TileIntoFragmentU32 { .. }
                | ExecutionOperationV15::FragmentIntoPartsU32 { .. }
        )
    )));
    let allocations = TileAllocationSlotsV164::derive(slots, &inventory, out)?;
    allocations.check_owner(inventory.owner(), out)?;
    Ok(())
}

#[test]
fn original_tile_fixture_exercises_actual_loads_and_expanded_allocation_custody() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            |slots, tile, out| {
                assert_eq!(slots.tile_policy_v162(0, out)?, Some((layout, 64)));
                check_actual_tile_slots(slots, tile, out)
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_tile_fixture_has_exact_and_one_short_complete_resource_boundaries() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let baseline = run_fixture(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            check_actual_tile_slots,
        );
        baseline.0.unwrap();
        let exact = run_fixture(layout, baseline.1, baseline.3, check_actual_tile_slots);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        let work = run_fixture(layout, baseline.1 - 1, baseline.3, check_actual_tile_slots).0;
        let storage = run_fixture(layout, baseline.1, baseline.3 - 1, check_actual_tile_slots).0;
        use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
        assert!(matches!(work,
            Err(Error::Resource(Resource::Work(error)))
                | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
                if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
        assert!(matches!(storage,
            Err(Error::Resource(Resource::Storage(error)))
                | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
                if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
    }
}

fn generate_actual_tile_source_v168(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    _tile: &TileExpansion<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let mut program = SourceByteProgram::derive(plan, slots, out)?;
    let mut shared_entries = 0;
    for instance in 0..program.roots[0].0.len() {
        let row = plan.instance(0, instance, out)?;
        let semantic = slots
            .correspondence(out)?
            .source(out.budget)?
            .source_semantic(out.budget)?;
        let function = &semantic.functions()[row.function.index() as usize];
        for (local, declaration) in function.locals().iter().enumerate() {
            let SemanticLocalRoleV1::Argument(argument) = declaration.role() else {
                continue;
            };
            let Some(recipe) = super::super::source_bytes::execution_loans::entry_recipe(
                slots,
                plan,
                0,
                instance,
                SemanticLocalIdV1::from_index(local.try_into().unwrap()),
                out,
            )?
            else {
                continue;
            };
            let (parent, block) = row.incoming.unwrap();
            let operand = super::super::source_bytes::execution_loans::call_argument(
                slots,
                plan,
                0,
                parent,
                block.index() as usize,
                argument as usize,
                out,
            )?
            .unwrap();
            assert_eq!(
                recipe, operand.recipe,
                "helper entry must retain the caller's creation recipe"
            );
            assert!(!recipe.mutable);
            assert_ne!(recipe.instance, instance, "no callee-coordinate retagging");
            shared_entries += 1;
        }
    }
    assert_eq!(
        shared_entries, 2,
        "both real shared Workgroup helper instances"
    );
    let generated_start = out.text.len();
    program.emit(out)?;
    let generated = &out.text[generated_start..];
    for (operation, expected) in [
        ("ContextIssue", 1),
        ("WorkgroupDerive", 1),
        ("TileLoad", 2),
        ("TileTransport", 4),
    ] {
        assert_eq!(
            generated
                .matches(&format!(
                    "let event = InvocationSourceByteEventV36::{operation}("
                ))
                .count(),
            expected,
            "actual source call census for {operation}"
        );
    }
    assert!(generated.contains("InvocationSourceByteEventV36::ExecutionLoan("));
    assert!(generated.contains("InvocationSourceExecutionRoleV168::Context"));
    assert!(generated.contains("InvocationSourceExecutionRoleV168::Workgroup"));
    assert!(generated.contains("TileLoad(InvocationSourceExecutionTileLoadV168 { workgroup:"));
    assert_eq!(
        generated
            .matches("InvocationSourceOperandV36::Execution(")
            .count(),
        2 * shared_entries,
        "each argument appears in evaluation and its independent observation"
    );
    assert_eq!(
        generated.matches("invocation_source_value_evaluate_v42(source, InvocationSourceOperandV36::Execution(").count(),
        shared_entries,
    );
    assert_eq!(
        generated
            .matches("operand: InvocationSourceOperandV36::Execution(")
            .count(),
        shared_entries,
    );
    assert_eq!(
        generated
            .matches("=> invocation_source_execution_snapshot_install_v170(entered,")
            .count(),
        2
    );
    Ok(())
}

#[test]
fn original_execution_tile_source_dispatch_uses_actual_loans_and_selected_layouts() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture_with_plan(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            |plan, slots, tile, out| {
                generate_actual_tile_source_v168(plan, slots, tile, out)?;
                let (selected, absent) = match layout {
                    Layout::Blocked => ("Blocked", "Striped"),
                    Layout::Striped => ("Striped", "Blocked"),
                };
                assert_eq!(
                    out.text
                        .matches(&format!(
                            "layout: InvocationSourceTileLayoutV161::{selected}"
                        ))
                        .count(),
                    2
                );
                assert!(
                    !out.text
                        .contains(&format!("layout: InvocationSourceTileLayoutV161::{absent}"))
                );
                Ok(())
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_execution_tile_source_dispatch_has_exact_and_one_short_resources() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let baseline = run_fixture_with_plan(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            generate_actual_tile_source_v168,
        );
        baseline.0.unwrap();
        let exact = run_fixture_with_plan(
            layout,
            baseline.1,
            baseline.3,
            generate_actual_tile_source_v168,
        );
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        for (work, storage, is_work) in [
            (baseline.1 - 1, baseline.3, true),
            (baseline.1, baseline.3 - 1, false),
        ] {
            let result =
                run_fixture_with_plan(layout, work, storage, generate_actual_tile_source_v168).0;
            use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
            assert!(if is_work {
                matches!(result, Err(Error::Resource(Resource::Work(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
                    if error.actual() == baseline.1 && error.limit() == work)
            } else {
                matches!(result, Err(Error::Resource(Resource::Storage(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
                    if error.actual() == baseline.3 && error.limit() == storage)
            });
        }
    }
}

fn generate_actual_tile_target_v176(
    slots: &SourceSlots<'_, '_>,
    tile: &TileExpansion<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use super::super::tile_target::TileTargetV176;
    let target = TileTargetV176::derive(slots, out)?;
    let inventory = target.inventory(out)?;
    assert!(std::ptr::eq(inventory.owner(), tile.output(out.budget)?));
    assert!(!std::ptr::eq(
        inventory.owner(),
        slots.correspondence(out)?.inventory(out.budget)?.owner(),
    ));
    let actual_operations = inventory.operations().len();
    assert!(actual_operations > 0);
    let selected = tile.root_policy_v162(0, out.budget)?.unwrap().0;
    let mut retained_contexts = 0;
    let mut retained_workgroups = 0;
    for row in &inventory.definitions()[inventory.functions()[selected.0 as usize]
        .definitions
        .clone()]
    {
        use fe2o3_kernel_ir::ExecutionRoleV15 as Role;
        match row.ty {
            fe2o3_kernel_ir::Type::Execution(
                Role::MaskedTileU32 { .. } | Role::LaneFragmentU32 { .. },
            ) => panic!("tile role survived scalar expansion"),
            fe2o3_kernel_ir::Type::Execution(Role::Context) => retained_contexts += 1,
            fe2o3_kernel_ir::Type::Execution(Role::Workgroup) => retained_workgroups += 1,
            _ => {}
        }
    }
    assert!(retained_contexts > 0 && retained_workgroups > 0);
    let mut capability_steps = Vec::new();
    for operation in &inventory.operations()[inventory.functions()[selected.0 as usize]
        .operations
        .clone()]
    {
        let (code, destination, receiver) = match &operation.operation.kind {
            OperationKind::Execution(ExecutionOperationV15::ContextIssue) => {
                (0, operation.results.start as i64, -1)
            }
            OperationKind::Execution(ExecutionOperationV15::WorkgroupDerive { .. }) => (
                1,
                operation.results.start as i64,
                inventory.uses()[operation.operands.start].definition as i64,
            ),
            OperationKind::Execution(ExecutionOperationV15::ScopeEnd { discarded, .. }) => {
                assert!(discarded.is_empty());
                (
                    2,
                    -1,
                    inventory.uses()[operation.operands.start].definition as i64,
                )
            }
            _ => continue,
        };
        capability_steps.push(format!(
            "byte_execution_step_v178(s, operation, {code}, {destination}, {receiver})"
        ));
    }
    assert!(capability_steps.len() >= 3);
    assert_eq!(target.root_function(0, out)?, selected);
    target.emit(fe2o3_kernel_ir::FormalIndexWidth::Bits64, out)?;
    assert!(out.text.contains("spec fn byte_micro_step_0_v30("));
    for expression in capability_steps {
        assert!(
            out.text.contains(&expression),
            "missing actual capability step: {expression}"
        );
    }
    assert_eq!(target.inventory(out)?.operations().len(), actual_operations);
    Ok(())
}

fn generate_actual_tile_microcuts_v180(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    tile: &TileExpansion<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use super::super::tile_target::{TileMicroCutsV180, TileTargetV176};
    let target = TileTargetV176::derive(slots, out)?;
    let cuts = TileMicroCutsV180::derive(&target, plan, out)?;
    target.emit(fe2o3_kernel_ir::FormalIndexWidth::Bits64, out)?;
    cuts.emit(out)?;
    let inventory = target.inventory(out)?;
    let mut actual_candidates = 0usize;
    for instance in 0..plan.root(0, out)?.instances.len() {
        let row = plan.instance(0, instance, out)?;
        for block in 0..row.blocks.len() {
            let pc = row.blocks.start + block;
            assert_eq!(
                out.text.matches(&format!("source_pc == {pc} &&")).count(),
                1
            );
            let Some(gap) = tile.source_block_entry_gap_v177(
                0,
                instance,
                SemanticBlockIdV1::from_index(block.try_into().unwrap()),
                out.budget,
            )?
            else {
                continue;
            };
            if !matches!(
                gap.prefix_disposition(out.budget)?,
                fe2o3_lower_mir_kernel::ProductionOptimizedSourceGapV18::Reachable(_)
            ) {
                continue;
            }
            for candidate in 0..gap.candidate_count(out.budget)? {
                let point = gap.candidate(candidate, out.budget)?;
                let (physical, owner) = inventory
                    .blocks()
                    .iter()
                    .enumerate()
                    .find(|(_, row)| row.coordinate == point.block)
                    .unwrap();
                let operation = if point.first as usize == owner.operations.len() {
                    -1
                } else {
                    (owner.operations.start + point.first as usize) as i128
                };
                assert!(out.text.contains(&format!(
                    "m.state.pc == {physical} && m.next_operation == {operation} && m.observations.len() == {}", point.first)));
                actual_candidates += 1;
            }
        }
    }
    assert!(actual_candidates > 0);
    assert!(
        out.text
            .contains("proof fn invocation_tile_zero_edge_decreases_0_v180(")
    );
    for root in 0..slots
        .correspondence(out)?
        .source(out.budget)?
        .root_count(out.budget)?
    {
        assert!(out.text.contains(&format!(
            "invocation_tile_micro_operation_v181(byte_micro_step_{root}_v30(before, little_endian))"
        )));
        assert!(out.text.contains(&format!(
            "let finished = byte_micro_finish_{root}_v30(before);"
        )));
        assert!(
            out.text
                .contains(&format!("else {{ byte_micro_begin_{root}_v30(state) }};"))
        );
        assert!(out.text.contains(&format!(
            "if invocation_tile_cursor_{root}_v180(source_pc, start) {{"
        )));
        assert!(out.text.contains(&format!(
            "proof fn invocation_tile_micro_zero_is_exact_candidate_{root}_v181("
        )));
    }
    assert!(
        out.text
            .contains("finished.observations == before.observations")
    );
    assert!(
        out.text
            .contains("next, observations: seq![], returned: finished.returned, terminal,")
    );
    assert!(
        out.text
            .contains("source_pc, head.next, little_endian, (fuel - 1) as nat,")
    );
    assert!(!out.text.contains("assume("));
    Ok(())
}

fn generate_expanded_private_bindings_v188(
    slots: &SourceSlots<'_, '_>,
    _: &TileExpansion<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    assert_eq!(check_expanded_private_bindings_v190(slots, out)?, 0);
    Ok(())
}

fn check_expanded_private_bindings_v190(
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    use super::super::{
        byte_bindings::SourceByteBindings, slots::AllocationOrigin, tile_target::TileTargetV176,
    };
    let target = TileTargetV176::derive(slots, out)?;
    let inventory = target.inventory(out)?;
    let original = slots.correspondence(out)?.inventory(out.budget)?;
    assert!(!std::ptr::eq(inventory.owner(), original.owner()));
    let bindings = SourceByteBindings::derive_expanded_v188(&target, out)?;
    bindings.emit(out)?;
    assert!(out.text.contains(&format!(
        "target.values.len() == {}",
        inventory.definitions().len()
    )));
    let mut frames = 0usize;
    for row in inventory.operations() {
        if !matches!(row.operation.kind, OperationKind::Alloca { .. }) {
            continue;
        }
        let site = target.allocation_site(row.coordinate, out)?;
        let AllocationOrigin::OriginalFrame(frame) = slots.allocation_origin(site.original, out)?
        else {
            continue;
        };
        let (descriptor, original) = slots.descriptor_by_source(
            frame.root(),
            frame.instance(),
            frame.local(),
            frame.source_generation(),
            out,
        )?;
        assert_eq!(original.allocation(), site.original);
        assert_eq!(row.results.len(), 1);
        assert!(out.text.contains(&format!(
            "if source.slots.contains_key({descriptor}) && {} < target.values.len() {{ match target.values[{}]",
            row.results.start, row.results.start,
        )));
        assert!(out.text.contains(&format!(
            "owner == {} && invocation == 0 && site == slot.site",
            site.physical_root_owner,
        )));
        frames += 1;
    }
    assert_eq!(
        out.text.matches("private.insert(source.slots[").count(),
        frames
    );
    assert!(!out.text.contains("invocation_paired_"));
    Ok(frames)
}

#[test]
fn original_execution_tile_private_map_preserves_the_empty_original_frame_census() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            generate_expanded_private_bindings_v188,
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_execution_private_map_keeps_genuine_original_frames_and_actual_definitions() {
    for layout in [Layout::Blocked, Layout::Striped] {
        super::super::slots::tests::run_tile_slots_with_limits(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            |slots, out| {
                // This separately admitted source has two address-taken Rust
                // objects. The tile-only fixture above has no original frame.
                assert_eq!(check_expanded_private_bindings_v190(slots, out)?, 2);
                Ok(())
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_execution_private_map_genuine_frames_have_exact_and_short_resources() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let run = |work, storage| {
            super::super::slots::tests::run_tile_slots_with_limits(
                layout,
                work,
                storage,
                |slots, out| {
                    assert_eq!(check_expanded_private_bindings_v190(slots, out)?, 2);
                    Ok(())
                },
            )
        };
        let baseline = run(512 * 1024 * 1024, 512 * 1024 * 1024);
        baseline.0.unwrap();
        let exact = run(baseline.1, baseline.3);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as Source;
        for (work, storage, is_work) in [
            (baseline.1 - 1, baseline.3, true),
            (baseline.1, baseline.3 - 1, false),
        ] {
            let error = run(work, storage).0;
            assert!(if is_work {
                matches!(error, Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(Source::Resource(Resource::Work(error))))
                    if error.actual() == baseline.1 && error.limit() == work)
            } else {
                matches!(error, Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(Source::Resource(Resource::Storage(error))))
                    if error.actual() == baseline.3 && error.limit() == storage)
            });
        }
    }
}

#[test]
fn original_execution_tile_private_map_has_exact_and_one_short_resources() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let baseline = run_fixture(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            generate_expanded_private_bindings_v188,
        );
        baseline.0.unwrap();
        let exact = run_fixture(
            layout,
            baseline.1,
            baseline.3,
            generate_expanded_private_bindings_v188,
        );
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
        for (work, storage, is_work) in [
            (baseline.1 - 1, baseline.3, true),
            (baseline.1, baseline.3 - 1, false),
        ] {
            let error = run_fixture(
                layout,
                work,
                storage,
                generate_expanded_private_bindings_v188,
            )
            .0;
            assert!(if is_work {
                matches!(error, Err(Error::Resource(Resource::Work(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
                    if error.actual() == baseline.1 && error.limit() == work)
            } else {
                matches!(error, Err(Error::Resource(Resource::Storage(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
                    if error.actual() == baseline.3 && error.limit() == storage)
            });
        }
    }
}

#[test]
fn original_execution_tile_private_map_rejects_foreign_and_refunded_accounts() {
    for foreign in [false, true] {
        let mut reached = false;
        let result = run_fixture(
            Layout::Blocked,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            |slots, _, out| {
                let target = super::super::tile_target::TileTargetV176::derive(slots, out)?;
                let bindings =
                    super::super::byte_bindings::SourceByteBindings::derive_expanded_v188(
                        &target, out,
                    )?;
                reached = true;
                let error = if foreign {
                    let mut work =
                        fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(512 * 1024 * 1024);
                    let mut budget = Budget::new(&mut work, 512 * 1024 * 1024);
                    budget.reserve_storage(out.budget.storage())?;
                    let mut other = Writer::new(&mut budget)?;
                    bindings.emit(&mut other).unwrap_err()
                } else {
                    out.budget.release_storage(1)?;
                    bindings.emit(out).unwrap_err()
                };
                Err(error)
            },
        );
        assert!(reached);
        assert!(matches!(
            result.0,
            Err(Error::Resource(Resource::Accounting))
                | Err(Error::Source(
                    fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                        Resource::Accounting
                    )
                ))
        ));
    }
}

#[test]
fn original_execution_tile_microcuts_bind_actual_source_and_microstate_boundaries() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture_with_plan(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            generate_actual_tile_microcuts_v180,
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_execution_tile_microcuts_have_exact_and_one_short_resource_boundaries() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let baseline = run_fixture_with_plan(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            generate_actual_tile_microcuts_v180,
        );
        baseline.0.unwrap();
        run_fixture_with_plan(
            layout,
            baseline.1,
            baseline.3,
            generate_actual_tile_microcuts_v180,
        )
        .0
        .unwrap();
        use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
        assert!(
            matches!(run_fixture_with_plan(layout, baseline.1 - 1, baseline.3, generate_actual_tile_microcuts_v180).0,
            Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == baseline.1 && error.limit() == baseline.1 - 1)
        );
        assert!(
            matches!(run_fixture_with_plan(layout, baseline.1, baseline.3 - 1, generate_actual_tile_microcuts_v180).0,
            Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == baseline.3 && error.limit() == baseline.3 - 1)
        );
    }
}

#[test]
fn original_execution_tile_microcuts_reject_foreign_and_refunded_accounts() {
    use super::super::tile_target::{TileMicroCutsV180, TileTargetV176};
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let mut reached = false;
        let result = run_fixture_with_plan(
            Layout::Blocked,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            |plan, slots, _, out| {
                let target = TileTargetV176::derive(slots, out)?;
                let cuts = TileMicroCutsV180::derive(&target, plan, out)?;
                reached = true;
                let error = if foreign {
                    let mut work = Work::new(512 * 1024 * 1024);
                    let mut budget = Budget::new(&mut work, 512 * 1024 * 1024);
                    budget.reserve_storage(out.budget.storage())?;
                    let mut other = Writer::new(&mut budget)?;
                    cuts.emit(&mut other).unwrap_err()
                } else {
                    out.budget.release_storage(1)?;
                    cuts.emit(out).unwrap_err()
                };
                assert!(matches!(
                    error,
                    Error::Resource(Resource::Accounting)
                        | Error::Source(SourceError::Resource(Resource::Accounting))
                ));
                assert!(cuts.emit(out).is_err());
                Err(error)
            },
        );
        assert!(reached && result.0.is_err());
    }
}

#[test]
fn original_execution_tile_target_census_admits_actual_index_arithmetic() {
    use super::super::super::CanonicalByteScalarV30;
    use super::super::tile_target::TileTargetV176;
    use fe2o3_kernel_ir::{BinaryOp, FormalIndexWidth, Type as KirType};
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            |slots, _, out| {
                let target = TileTargetV176::derive(slots, out)?;
                let inventory = target.inventory(out)?;
                let (mut adds, mut multiplies) = (0, 0);
                for (index, row) in inventory.operations().iter().enumerate() {
                    let OperationKind::Binary { op, .. } = row.operation.kind else {
                        continue;
                    };
                    if !matches!(op, BinaryOp::Add | BinaryOp::Multiply) {
                        continue;
                    }
                    assert_eq!(row.results.len(), 1);
                    assert_eq!(
                        inventory.definitions()[row.results.start].ty,
                        &KirType::INDEX
                    );
                    CanonicalByteScalarV30::derive(
                        inventory,
                        index,
                        FormalIndexWidth::Bits64,
                        out,
                    )?;
                    match op {
                        BinaryOp::Add => adds += 1,
                        BinaryOp::Multiply => multiplies += 1,
                        _ => unreachable!(),
                    }
                }
                assert!(adds > 0 && multiplies > 0);
                Ok(())
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_execution_tile_target_emits_the_actual_expanded_graph() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            generate_actual_tile_target_v176,
        )
        .0
        .unwrap();
    }
}

fn generate_complete_actual_target_v187(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    tile: &TileExpansion<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use std::fmt::Write as _;
    super::super::emit_model_prelude_v187(out)?;
    generate_actual_tile_microcuts_v180(plan, slots, tile, out)?;
    // Use the production support-closure pass. No target operation or theorem
    // is replaced, and no source-to-target relation is added by this export.
    super::super::support_closure::retain_referenced(out)?;
    writeln!(out, "}}").map_err(|_| out.error())?;
    assert!(out.text.starts_with("use vstd::prelude::*;"));
    assert!(out.text.contains("spec fn byte_micro_step_0_v30("));
    assert!(out.text.contains("canonical_scalar_math_"));
    assert!(out.text.contains("byte_execution_step_v178(s, operation,"));
    assert!(
        out.text
            .contains("spec fn invocation_tile_micro_seek_0_v181(")
    );
    assert!(
        out.text
            .contains("proof fn invocation_tile_micro_zero_is_exact_candidate_0_v181(")
    );
    assert!(!out.text.contains("proof fn invocation_paired_"));
    Ok(())
}

#[test]
fn original_execution_tile_target_complete_model_uses_the_production_prelude() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture_with_plan(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            generate_complete_actual_target_v187,
        )
        .0
        .unwrap();
    }
}

#[test]
fn original_execution_tile_target_prelude_preserves_original_shared_bytes() {
    use std::fmt::Write as _;
    run_fixture(
        Layout::Blocked,
        512 * 1024 * 1024,
        512 * 1024 * 1024,
        |_, _, out| {
            let invocation = "use vstd::prelude::*;\nuse vstd::seq_lib::*;\nverus! {\n";
            write!(out, "{invocation}").map_err(|_| out.error())?;
            super::super::super::relation::emit_prelude(out)?;
            let mut original = out.text.clone();
            for shared in [
                super::super::super::super::structured_state_v30::STATE,
                super::super::super::control_generate::SOURCE_STATE,
                super::super::super::super::cfg_trace::PRELUDE,
                super::super::super::super::byte_memory_v30::BYTE_MEMORY_V30,
                super::super::bytes::INVOCATION_BYTES_V36,
                super::super::bytes::CLASSIFIED_VIEW_LAWS_V40,
                super::super::source_bytes::SOURCE_BYTES_V36,
                super::super::source_bytes::SOURCE_POINTERS_V36,
                super::super::source_frames::SOURCE_FRAMES_V36,
                super::SOURCE_FUNCTION_V36,
                super::super::effects::INVOCATION_EFFECTS_V36,
            ] {
                original.push_str(shared);
            }
            out.text.clear();
            super::super::emit_model_prelude_v187(out)?;
            assert_eq!(out.text, original);
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn original_execution_tile_target_complete_model_has_exact_resource_limits() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let baseline = run_fixture_with_plan(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            generate_complete_actual_target_v187,
        );
        baseline.0.unwrap();
        let exact = run_fixture_with_plan(
            layout,
            baseline.1,
            baseline.3,
            generate_complete_actual_target_v187,
        );
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        for (work, storage, is_work) in [
            (baseline.1 - 1, baseline.3, true),
            (baseline.1, baseline.3 - 1, false),
        ] {
            let error =
                run_fixture_with_plan(layout, work, storage, generate_complete_actual_target_v187)
                    .0;
            use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
            assert!(if is_work {
                matches!(error, Err(Error::Resource(Resource::Work(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
                    if error.actual() == baseline.1 && error.limit() == work)
            } else {
                matches!(error, Err(Error::Resource(Resource::Storage(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
                    if error.actual() == baseline.3 && error.limit() == storage)
            });
        }
    }
}

#[test]
#[ignore = "complete target-only models; no proof or original-source refinement authority"]
fn diagnostic_complete_expanded_tile_target_models_export_v187() {
    use sha2::{Digest, Sha256};
    use std::io::{BufWriter, Write as _};
    for (layout, label) in [(Layout::Blocked, "blocked"), (Layout::Striped, "striped")] {
        run_fixture_with_plan(layout, 512 * 1024 * 1024, 512 * 1024 * 1024, |plan, slots, tile, out| {
            generate_complete_actual_target_v187(plan, slots, tile, out)?;
            assert!(out.text.len() <= 16 * 1024 * 1024);
            let mut output = BufWriter::new(std::io::stdout().lock());
            write!(output, "{{\"kind\":\"fe2o3-expanded-tile-target-model-v187\",\"scope\":\"actual expanded target only\",\"layout\":\"{label}\",\"bytes\":{},\"sha256\":\"", out.text.len()).unwrap();
            for byte in Sha256::digest(out.text.as_bytes()) {
                write!(output, "{byte:02x}").unwrap();
            }
            write!(output, "\",\"model_hex\":\"").unwrap();
            for byte in out.text.as_bytes() {
                write!(output, "{byte:02x}").unwrap();
            }
            writeln!(output, "\"}}").unwrap();
            output.flush().unwrap();
            Ok(())
        }).0.unwrap();
    }
}

#[test]
fn original_execution_tile_target_has_exact_and_one_short_resources() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let baseline = run_fixture(
            layout,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            generate_actual_tile_target_v176,
        );
        baseline.0.unwrap();
        let exact = run_fixture(
            layout,
            baseline.1,
            baseline.3,
            generate_actual_tile_target_v176,
        );
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        for (work, storage, is_work) in [
            (baseline.1 - 1, baseline.3, true),
            (baseline.1, baseline.3 - 1, false),
        ] {
            let result = run_fixture(layout, work, storage, generate_actual_tile_target_v176).0;
            use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
            assert!(if is_work {
                matches!(result, Err(Error::Resource(Resource::Work(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
                    if error.actual() == baseline.1 && error.limit() == work)
            } else {
                matches!(result, Err(Error::Resource(Resource::Storage(error)))
                    | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
                    if error.actual() == baseline.3 && error.limit() == storage)
            });
        }
    }
}

#[test]
fn original_execution_tile_target_rejects_foreign_and_refunded_accounts() {
    use super::super::tile_target::TileTargetV176;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let mut reached = false;
        let result = run_fixture(
            Layout::Blocked,
            512 * 1024 * 1024,
            512 * 1024 * 1024,
            |slots, _, out| {
                let target = TileTargetV176::derive(slots, out)?;
                reached = true;
                let error = if foreign {
                    let mut work = Work::new(512 * 1024 * 1024);
                    let mut budget = Budget::new(&mut work, 512 * 1024 * 1024);
                    budget.reserve_storage(out.budget.storage())?;
                    let mut other = Writer::new(&mut budget)?;
                    target.root_function(0, &mut other).unwrap_err()
                } else {
                    out.budget.release_storage(1)?;
                    target.root_function(0, out).unwrap_err()
                };
                assert!(matches!(
                    error,
                    Error::Resource(Resource::Accounting)
                        | Error::Source(SourceError::Resource(Resource::Accounting))
                ));
                assert!(target.inventory(out).is_err());
                Err(error)
            },
        );
        assert!(reached);
        assert!(result.0.is_err());
    }
}
