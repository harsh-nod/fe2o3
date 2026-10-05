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

fn run_fixture(
    layout: Layout,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &SourceSlots<'_, '_>,
        &TileExpansion<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_prepared(work, storage, prepared, |plan, out| {
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
                    examine(&slots, &tile, &mut writer)
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
            Err(error) => panic!("original tile fixture preparation: {error:?}"),
        }
    })
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
        assert!(
            run_fixture(layout, baseline.1 - 1, baseline.3, check_actual_tile_slots)
                .0
                .is_err()
        );
        assert!(
            run_fixture(layout, baseline.1, baseline.3 - 1, check_actual_tile_slots)
                .0
                .is_err()
        );
    }
}
