use super::*;
#[path = "tests/f32_arithmetic.rs"]
mod f32_arithmetic;
#[path = "tests/preflight.rs"]
mod preflight;
use dialect_gpu::ExecutionLayoutOp;
use dialect_kernel::{AllocationOriginAttr, SemanticConstantAttr};
use fe2o3_kernel_ir::{
    BasicBlock as KirBlock, GlobalCapabilityTypeV1, GlobalDisjointIndexContractV1,
    IntrinsicOperation, Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1, LaunchDomain,
    LaunchExtent, Module, Operation as KirOp, Signature, ValueDef,
};
use fe2o3_pliron_owner_core::ensure_context_identity;
use pliron::{
    builtin::{attributes::StringAttr, op_interfaces::SingleBlockRegionInterface},
    dialect::DialectName,
};

fn setup() -> Context {
    let mut context = Context::new();
    let dialect = DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap();
    dialect_kernel::register_dialect(&mut context, &dialect).unwrap();
    ensure_context_identity(&mut context).unwrap();
    context
}

// The same canonical shape as a typed disjoint fill, with arbitrary names and
// dynamic launch/length. The projector must not obtain geometry from its name.
fn fill_module(name: &str) -> Module {
    let context = KernelContextTypeV1::new(name, [1; 32], [2; 32], [3; 32]);
    let contract = GlobalDisjointIndexContractV1::new([8; 32], GlobalDisjointIndexSpaceV1::Index1d);
    let capability = GlobalCapabilityTypeV1::disjoint_write(Type::F32, context.clone(), contract);
    let physical = capability.physical_slice_type();
    let pointer = capability.physical_pointer_type();
    let mut block = KirBlock::new(BlockId(0));
    block.operations = vec![
        KirOp::kernel_context_issue(
            ValueId(3),
            context,
            KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]),
        ),
        KirOp::global_capability_bind(ValueId(4), capability, ValueId(3), ValueId(0)),
        KirOp::global_capability_index(ValueId(5), ValueId(4), ValueId(1), Some(contract)),
        KirOp::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(4) },
        ),
        KirOp::effect_free(
            ValueDef::new(ValueId(7), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(5),
                rhs: ValueId(6),
            },
        ),
        KirOp::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        KirOp::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Select {
                condition: ValueId(7),
                true_value: ValueId(5),
                false_value: ValueId(8),
            },
        ),
        KirOp::effect_free(
            ValueDef::new(ValueId(10), pointer.clone()),
            OperationKind::SliceData { slice: ValueId(4) },
        ),
        KirOp::effect_free(
            ValueDef::new(ValueId(11), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(10),
                offset: ValueId(9),
            },
        ),
        KirOp::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(11),
                predicate: ValueId(7),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("structural-canonical-ranked-test");
    module.functions.push(Function::kernel_entry(
        name,
        Signature::new(vec![physical, Type::INDEX, Type::F32], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        name,
        name,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn operations(module: &mut Module) -> &mut Vec<KirOp> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

// Plain slice inputs permit well-typed, unproved accesses. Those must remain
// unproved or be rejected by the projection, never repaired by fake bounds.
fn plain_module() -> Module {
    let mut module = fill_module("plain");
    let ops = operations(&mut module);
    ops.drain(..3);
    ops[0].kind = OperationKind::SliceLength { slice: ValueId(0) };
    let OperationKind::Compare { lhs, .. } = &mut ops[1].kind else {
        unreachable!()
    };
    *lhs = ValueId(1);
    let OperationKind::Select { true_value, .. } = &mut ops[3].kind else {
        unreachable!()
    };
    *true_value = ValueId(1);
    ops[4].kind = OperationKind::SliceData { slice: ValueId(0) };
    module
}

fn prepare(module: Module) -> (VerifiedCanonicalKernelIrV13, CanonicalRankedViewBlueprintV1) {
    let id = module.functions[0].id.clone();
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let blueprint = prepare_canonical_ranked_view_v1(&canonical, 17, &id).unwrap();
    (canonical, blueprint)
}

fn scalar_module(name: &str) -> Module {
    let mut module = fill_module(name);
    operations(&mut module).pop();
    module
}

// Internal structural planning is deliberately not admission. In particular,
// these plans with writes cannot be materialized into a checked receipt.
fn structural_plan(
    canonical: &VerifiedCanonicalKernelIrV13,
) -> Result<CanonicalRankedViewBlueprintV1, CanonicalRankedViewErrorV1> {
    let module = decode_module_v13(canonical.canonical_bytes()).unwrap();
    let function = &module.functions[0];
    Planner::new(*canonical.identity(), 17, function)?.finish(function)
}

fn plan_writes(module: Module) -> (VerifiedCanonicalKernelIrV13, CanonicalRankedViewBlueprintV1) {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let plan = structural_plan(&canonical).unwrap();
    (canonical, plan)
}

fn reject(module: Module, expected: &str) {
    let id = module.functions[0].id.clone();
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let error = prepare_canonical_ranked_view_v1(&canonical, 17, &id).unwrap_err();
    assert!(error.to_string().contains(expected), "{error}");
}

fn find<O: Op>(context: &Context, view: &CheckedCanonicalRankedViewV1) -> O {
    view.graph
        .live_operations
        .iter()
        .find_map(|p| Operation::get_op::<O>(*p, context))
        .unwrap()
}

fn rhs_constant(module: &mut Module, bits: u32) {
    let ops = operations(module);
    let n = ops.len() - 1;
    ops.insert(
        n,
        KirOp::effect_free(
            ValueDef::new(ValueId(12), Type::F32),
            OperationKind::Constant(Constant::F32Bits(bits)),
        ),
    );
    let OperationKind::GuardedStore { value, .. } = &mut ops[n + 1].kind else {
        unreachable!()
    };
    *value = ValueId(12);
}

#[test]
fn fill_blueprint_preserves_exact_write_but_cannot_publish_without_contracts() {
    let (canonical, plan) = plan_writes(fill_module("unselected_name"));
    assert_eq!(plan.covered_source_operations(), 10);
    assert_eq!(plan.block_count(), 3);
    assert_eq!(&plan.canonical, canonical.identity());
    let write = &plan.writes[0];
    assert_eq!(write.source_block(), BlockId(0));
    assert_eq!(write.source_operation(), 9);
    assert_eq!(write.parameter_ordinal(), 0);
    assert_eq!(write.pointer(), ValueId(11));
    assert_eq!(write.index(), ValueId(9));
    assert_eq!(write.predicate(), Some(ValueId(7)));
    assert_eq!(write.rhs(), ValueId(2));
    assert_eq!(write.element_stride_bytes(), 4);
    assert_eq!(write.access(), MemoryAccess::new(AddressSpace::Global, 4));
    assert!(plan.blocks[0].operations.iter().any(|op| matches!(op, PlannedOp::F32Parameter { result, parameter: 2 } if *result == write.rhs_node)));
    let Some(PlannedTerminator::LessThan {
        lhs, yes: 1, no: 2, ..
    }) = plan.blocks[0].terminator
    else {
        panic!("missing exact guard")
    };
    assert!(
        matches!(plan.blocks[1].operations[0], PlannedOp::Write { index, rhs, .. } if index == lhs && rhs == write.rhs_node)
    );
    let mut context = setup();
    let before = context.ir_mutation_attempt_epoch().unwrap();
    assert!(matches!(
        plan.materialize(&mut context),
        Err(CanonicalRankedViewErrorV1::MissingWriteContracts {
            block: BlockId(0),
            operation: 9
        })
    ));
    assert_eq!(context.ir_mutation_attempt_epoch().unwrap(), before);
    assert!(context.is_ir_empty());
    assert!(matches!(
        compile_canonical_ranked_view_v1(&mut context, &canonical, 17, &"unselected_name".into()),
        Err(CanonicalRankedViewErrorV1::MissingWriteContracts { .. })
    ));
    assert_eq!(context.ir_mutation_attempt_epoch().unwrap(), before);
}

#[test]
fn scalar_view_preserves_identity_parameter_origin_and_unknown_extent() {
    let (canonical, plan) = prepare(scalar_module("unselected_name"));
    let work = plan.tree_work();
    let mut context = setup();
    let view = plan.materialize(&mut context).unwrap();
    assert_eq!(view.function_id().as_str(), "unselected_name");
    assert_eq!(view.canonical_identity(), canonical.identity());
    assert_eq!(view.final_epoch(), 17);
    assert_eq!(view.covered_source_operations(), 9);
    view.revalidate(&context, &canonical, 17).unwrap();
    let ranked = find::<RankedViewOp>(&context, &view);
    assert_eq!(ranked.allocation_origin(&context), Some(1));
    assert_eq!(ranked.noalias_class(&context), Some(0));
    let ty = ranked.view_type(&context).unwrap();
    assert_eq!(ty.deref(&context).element_width(), 32);
    assert_eq!(ty.deref(&context).shape(), &[DYNAMIC_EXTENT]);
    assert_eq!(
        find::<SemanticTypedSymbolOp>(&context, &view).symbol(&context),
        Some(2)
    );
    assert!(view.writes().is_empty());
    assert!(view.write_rhs(0).is_none());
    assert!(
        view.graph
            .live_operations
            .iter()
            .all(|p| !Operation::is_op::<ExecutionLayoutOp>(*p, &context))
    );
    assert_eq!(
        work,
        3 + view.graph.live_blocks.len() + 2 * view.graph.live_operations.len()
    );
    let report = crate::run_pliron_ranked_bounds_check_v1(&context, view.pliron());
    assert!(report.is_clean(), "{:?}", report.findings());
}

#[test]
fn global_invocation_has_unknown_launch_and_float_bits_are_exact() {
    for bits in [0, 0x8000_0000, 0x3fa0_0000, 0x7fc0_1234] {
        let mut module = fill_module("global_index");
        let ops = operations(&mut module);
        ops.insert(
            2,
            KirOp::effect_free(
                ValueDef::new(ValueId(13), Type::INDEX),
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            ),
        );
        let OperationKind::GlobalCapabilityIndex(index) = &mut ops[3].kind else {
            unreachable!()
        };
        index.index = ValueId(13);
        rhs_constant(&mut module, bits);
        operations(&mut module).pop();
        let (canonical, plan) = prepare(module);
        let mut context = setup();
        let view = plan.materialize(&mut context).unwrap();
        assert_eq!(
            find::<InvocationIndexOp>(&context, &view).launch_extent(&context),
            Some(0)
        );
        assert_eq!(
            find::<SemanticTypedConstantOp>(&context, &view).bits(&context),
            Some(u64::from(bits))
        );
        view.revalidate(&context, &canonical, 17).unwrap();
    }
}

#[test]
fn different_select_guard_and_nonzero_false_arm_reject() {
    let mut module = plain_module();
    let ops = operations(&mut module);
    ops.insert(
        6,
        KirOp::effect_free(
            ValueDef::new(ValueId(12), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(1),
                rhs: ValueId(6),
            },
        ),
    );
    let OperationKind::GuardedStore { predicate, .. } = &mut ops[7].kind else {
        unreachable!()
    };
    *predicate = ValueId(12);
    reject(module, "exact predicate");

    let mut module = plain_module();
    operations(&mut module)[2].kind = OperationKind::Constant(Constant::Index(1));
    reject(module, "zero false arm");
}

#[test]
fn unguarded_selected_address_and_chained_gep_reject() {
    let mut module = plain_module();
    operations(&mut module)[6] = KirOp::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(11),
            value: ValueId(2),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    );
    reject(module, "exact predicate");

    let mut module = plain_module();
    let ops = operations(&mut module);
    let pointer_ty = ops[5].results[0].ty.clone();
    ops.insert(
        6,
        KirOp::effect_free(
            ValueDef::new(ValueId(12), pointer_ty),
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(8),
            },
        ),
    );
    reject(module, "chained offsets unsupported");
}

#[test]
fn direct_store_blueprint_does_not_invent_a_guard_or_bypass_contracts() {
    let mut module = plain_module();
    operations(&mut module)[5].kind = OperationKind::GetElementPointer {
        base: ValueId(10),
        offset: ValueId(1),
    };
    operations(&mut module)[6] = KirOp::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(11),
            value: ValueId(2),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    );
    let (_, plan) = plan_writes(module);
    assert_eq!(plan.block_count(), 1);
    assert_eq!(plan.writes[0].predicate(), None);
    assert!(matches!(
        plan.blocks[0].terminator,
        Some(PlannedTerminator::Return)
    ));
    assert!(matches!(
        plan.materialize(&mut setup()),
        Err(CanonicalRankedViewErrorV1::MissingWriteContracts { .. })
    ));
}

#[test]
fn unsupported_unused_operations_and_hierarchy_intrinsics_reject() {
    let mut module = plain_module();
    operations(&mut module).push(KirOp::effect_free(
        ValueDef::new(ValueId(12), Type::F64),
        OperationKind::Constant(Constant::F64Bits(0)),
    ));
    reject(module, "closed scalar/global-store projection");

    let mut module = plain_module();
    let mut intrinsic = IntrinsicOperation::global_id_1d();
    intrinsic.kind = IntrinsicKind::InvocationIndex {
        kind: IndexKind::Local,
        axis: Axis::X,
    };
    operations(&mut module).push(KirOp::effect_free(
        ValueDef::new(ValueId(12), Type::INDEX),
        OperationKind::Intrinsic(intrinsic),
    ));
    reject(module, "unavailable hierarchy/launch facts");
}

#[test]
fn unsupported_intrinsic_diagnostic_retains_exact_kind_and_source_site() {
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        let kinds = [
            IndexKind::Local,
            IndexKind::Workgroup,
            IndexKind::WorkgroupSize,
            IndexKind::WorkgroupCount,
        ]
        .map(|kind| IntrinsicKind::InvocationIndex { kind, axis });
        for intrinsic in kinds
            .into_iter()
            .chain([IntrinsicKind::LaunchExtent { axis }])
        {
            let mut module = scalar_module("intrinsic_diagnostic");
            module.kernels[0].domain = LaunchDomain::D3 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Dynamic,
                z: LaunchExtent::Dynamic,
            };
            let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
            block.id = BlockId(3);
            block.operations = vec![
                KirOp::effect_free(
                    ValueDef::new(ValueId(10), Type::INDEX),
                    OperationKind::Constant(Constant::Index(0)),
                ),
                KirOp::effect_free(
                    ValueDef::new(ValueId(11), Type::INDEX),
                    OperationKind::Constant(Constant::Index(1)),
                ),
                KirOp::effect_free(
                    ValueDef::new(ValueId(12), Type::INDEX),
                    OperationKind::Intrinsic(IntrinsicOperation::new(intrinsic, Type::INDEX)),
                ),
            ];
            let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
            let error =
                prepare_canonical_ranked_view_v1(&canonical, 17, &"intrinsic_diagnostic".into())
                    .unwrap_err();
            assert!(
                matches!(error, CanonicalRankedViewErrorV1::UnsupportedIntrinsic {
                block: BlockId(3), operation: 2, intrinsic: actual
            } if actual == intrinsic)
            );
            let diagnostic = error.to_string();
            assert!(
                diagnostic.contains(&format!("{intrinsic:?}")),
                "{diagnostic}"
            );
            assert!(
                diagnostic.contains("unavailable hierarchy/launch facts"),
                "{diagnostic}"
            );
        }
    }
}

fn hierarchy_module(kind: IndexKind, axis: Axis) -> Module {
    let mut module = scalar_module("canonical_hierarchy");
    module.kernels[0].domain = LaunchDomain::D3 {
        x: LaunchExtent::Static(130),
        y: LaunchExtent::Static(10),
        z: LaunchExtent::Static(7),
    };
    module.kernels[0].workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 4, 2));
    *operations(&mut module) = vec![KirOp::effect_free(
        ValueDef::new(ValueId(3), Type::INDEX),
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex { kind, axis },
            Type::INDEX,
        )),
    )];
    module
}

#[test]
fn canonical_local_and_workgroup_coordinates_use_exact_axis_geometry() {
    for (axis, dimension, extent, size) in [
        (Axis::X, 0, 130, 64),
        (Axis::Y, 1, 10, 4),
        (Axis::Z, 2, 7, 2),
    ] {
        for (kind, arithmetic) in [
            (IndexKind::Local, IndexBinaryKindAttr::Remainder),
            (IndexKind::Workgroup, IndexBinaryKindAttr::Divide),
        ] {
            let (canonical, plan) = prepare(hierarchy_module(kind, axis));
            let mut context = setup();
            let view = plan.materialize(&mut context).unwrap();
            view.revalidate(&context, &canonical, 17).unwrap();
            let binary = find::<IndexBinaryOp>(&context, &view);
            let global = find::<InvocationIndexOp>(&context, &view);
            let divisor = find::<IndexConstantOp>(&context, &view);
            assert_eq!(binary.kind(&context), Some(arithmetic));
            assert_eq!(binary.lhs(&context), global.result(&context));
            assert_eq!(binary.rhs(&context), divisor.result(&context));
            assert_eq!(global.dimension(&context), Some(dimension));
            assert_eq!(global.launch_extent(&context), Some(extent));
            assert_eq!(divisor.value(&context), Some(size));
        }
    }
}

#[test]
fn canonical_workgroup_size_is_not_a_target_default() {
    let mut module = hierarchy_module(IndexKind::WorkgroupSize, Axis::X);
    module.kernels[0].workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(17, 4, 2));
    let (_, plan) = prepare(module);
    let mut context = setup();
    let view = plan.materialize(&mut context).unwrap();
    assert_eq!(
        find::<IndexConstantOp>(&context, &view).value(&context),
        Some(17)
    );
}

#[test]
fn canonical_static_launch_extent_and_group_count_use_each_axis() {
    for (axis, extent, groups) in [(Axis::X, 130, 3), (Axis::Y, 10, 3), (Axis::Z, 7, 4)] {
        for (intrinsic, expected) in [
            (IntrinsicKind::LaunchExtent { axis }, extent),
            (
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::WorkgroupCount,
                    axis,
                },
                groups,
            ),
        ] {
            let mut module = hierarchy_module(IndexKind::WorkgroupCount, axis);
            operations(&mut module)[0].kind =
                OperationKind::Intrinsic(IntrinsicOperation::new(intrinsic, Type::INDEX));
            let (canonical, plan) = prepare(module);
            let mut context = setup();
            let view = plan.materialize(&mut context).unwrap();
            view.revalidate(&context, &canonical, 17).unwrap();
            assert_eq!(
                find::<IndexConstantOp>(&context, &view).value(&context),
                Some(expected)
            );
        }
    }
}

#[test]
fn canonical_launch_extent_needs_no_workgroup_size_but_requires_matching_entries() {
    let mut module = hierarchy_module(IndexKind::WorkgroupCount, Axis::X);
    module.kernels[0].workgroup_size = None;
    operations(&mut module)[0].kind = OperationKind::Intrinsic(IntrinsicOperation::new(
        IntrinsicKind::LaunchExtent { axis: Axis::X },
        Type::INDEX,
    ));
    let (_, plan) = prepare(module.clone());
    let mut context = setup();
    let view = plan.materialize(&mut context).unwrap();
    assert_eq!(
        find::<IndexConstantOp>(&context, &view).value(&context),
        Some(130)
    );
    let mut other = module.kernels[0].clone();
    other.id = "different_launch_extent".into();
    other.domain = LaunchDomain::D3 {
        x: LaunchExtent::Static(131),
        y: LaunchExtent::Static(10),
        z: LaunchExtent::Static(7),
    };
    module.kernels.push(other);
    reject(module, "unavailable hierarchy/launch facts");
}

#[test]
fn canonical_group_count_does_not_replace_dynamic_extent_with_one_group() {
    let mut module = hierarchy_module(IndexKind::WorkgroupCount, Axis::X);
    module.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Dynamic,
    };
    module.kernels[0].workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1));
    reject(module, "unavailable hierarchy/launch facts");
}

#[test]
fn canonical_hierarchy_rejects_missing_or_conflicting_workgroup_contracts() {
    for kind in [
        IndexKind::Local,
        IndexKind::Workgroup,
        IndexKind::WorkgroupSize,
        IndexKind::WorkgroupCount,
    ] {
        let mut missing = hierarchy_module(kind, Axis::X);
        missing.kernels[0].workgroup_size = None;
        reject(missing, "unavailable hierarchy/launch facts");

        let mut conflict = hierarchy_module(kind, Axis::X);
        let mut other = conflict.kernels[0].clone();
        other.id = "different_geometry".into();
        other.workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(32, 4, 2));
        conflict.kernels.push(other);
        reject(conflict, "unavailable hierarchy/launch facts");
    }
}

#[test]
fn canonical_hierarchy_keeps_dynamic_global_extents_dynamic() {
    let mut module = hierarchy_module(IndexKind::Local, Axis::X);
    module.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Dynamic,
    };
    module.kernels[0].workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1));
    let (_, plan) = prepare(module);
    let mut context = setup();
    let view = plan.materialize(&mut context).unwrap();
    assert_eq!(
        find::<InvocationIndexOp>(&context, &view).launch_extent(&context),
        Some(0)
    );
}

#[test]
fn nonidentity_capability_mapping_rejects_without_aliasing_it() {
    let mut module = fill_module("shifted");
    let ops = operations(&mut module);
    let Type::GlobalCapability(old) = &ops[1].results[0].ty else {
        unreachable!()
    };
    let contract = GlobalDisjointIndexContractV1::new(
        [8; 32],
        GlobalDisjointIndexSpaceV1::ShiftedIndex1d { offset: 1 },
    );
    let capability =
        GlobalCapabilityTypeV1::disjoint_write(Type::F32, old.context().clone(), contract);
    ops[1].results[0].ty = Type::GlobalCapability(capability);
    let OperationKind::GlobalCapabilityIndex(index) = &mut ops[2].kind else {
        unreachable!()
    };
    index.index_space = Some(contract);
    reject(module, "exact identity alias");
}

#[test]
fn alignment_rejects_and_volatile_effect_is_retained() {
    let mut module = plain_module();
    let OperationKind::GuardedStore { access, .. } = &mut operations(&mut module)[6].kind else {
        unreachable!()
    };
    access.alignment = 8;
    reject(module, "access/element stride");

    let mut module = plain_module();
    let OperationKind::GuardedStore { access, .. } = &mut operations(&mut module)[6].kind else {
        unreachable!()
    };
    access.volatile = true;
    let (_, plan) = plan_writes(module);
    assert!(plan.writes[0].access().volatile);
    assert!(matches!(
        plan.materialize(&mut setup()),
        Err(CanonicalRankedViewErrorV1::MissingWriteContracts { .. })
    ));
}

#[test]
fn source_diamond_and_ordered_guarded_stores_preserve_cfg() {
    let mut module = plain_module();
    let body = module.functions[0].body.as_mut().unwrap();
    let store = body.blocks[0].operations.pop().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(30),
        then_arguments: vec![],
        else_target: BlockId(20),
        else_arguments: vec![],
    });
    let mut yes = KirBlock::new(BlockId(30));
    yes.operations = vec![store.clone(), store];
    yes.terminator = Some(Terminator::Branch {
        target: BlockId(40),
        arguments: vec![],
    });
    let mut no = KirBlock::new(BlockId(20));
    no.terminator = Some(Terminator::Branch {
        target: BlockId(40),
        arguments: vec![],
    });
    let mut merge = KirBlock::new(BlockId(40));
    merge.terminator = Some(Terminator::Return { values: vec![] });
    // Physical source order is deliberately not topological.
    body.blocks.extend([merge, no, yes]);
    let (_, plan) = plan_writes(module);
    assert_eq!(plan.block_count(), 8);
    assert!(matches!(
        plan.blocks[0].terminator,
        Some(PlannedTerminator::LessThan { yes: 3, no: 2, .. })
    ));
    assert!(matches!(
        plan.blocks[7].terminator,
        Some(PlannedTerminator::Branch(1))
    ));
    assert_eq!(plan.writes[0].source_operation(), 0);
    assert_eq!(plan.writes[1].source_operation(), 1);
    assert_eq!(plan.writes[0].source_block(), BlockId(30));
    assert_eq!(plan.writes.len(), 2);
    assert_eq!(
        plan.blocks
            .iter()
            .flat_map(|b| &b.operations)
            .filter(|op| matches!(op, PlannedOp::Write { .. }))
            .count(),
        2
    );
}

#[test]
fn unsupported_control_and_block_arguments_reject_in_preflight() {
    let mut module = plain_module();
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Unreachable);
    reject(module, "unsupported CFG");

    let mut module = plain_module();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(1)],
    });
    let mut next = KirBlock::new(BlockId(1));
    next.parameters
        .push(ValueDef::new(ValueId(12), Type::INDEX));
    next.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(next);
    reject(module, "unsupported CFG");

    let mut module = plain_module();
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(0),
        arguments: vec![],
    });
    reject(module, "cyclic entry");
}

#[test]
fn context_epoch_and_canonical_subject_are_exact() {
    let (canonical, plan) = prepare(scalar_module("custody"));
    let mut context = setup();
    let view = plan.materialize(&mut context).unwrap();
    assert!(matches!(
        view.revalidate(&setup(), &canonical, 17),
        Err(CanonicalRankedViewErrorV1::ContextChanged)
    ));
    assert!(matches!(
        view.revalidate(&Context::new(), &canonical, 17),
        Err(CanonicalRankedViewErrorV1::MissingContextIdentity)
    ));
    assert!(matches!(
        view.revalidate(&context, &canonical, 18),
        Err(CanonicalRankedViewErrorV1::CanonicalSubjectChanged)
    ));
    let different = VerifiedCanonicalKernelIrV13::from_module(fill_module("different")).unwrap();
    assert!(matches!(
        view.revalidate(&context, &different, 17),
        Err(CanonicalRankedViewErrorV1::CanonicalSubjectChanged)
    ));
    let fresh =
        VerifiedCanonicalKernelIrV13::from_canonical_bytes(canonical.canonical_bytes().to_vec())
            .unwrap();
    view.revalidate(&context, &fresh, 17).unwrap();
}

#[test]
fn batch_views_are_attached_before_one_shared_final_seal() {
    let mut module = scalar_module("first");
    let other = scalar_module("second");
    module.functions.extend(other.functions);
    module.kernels.extend(other.kernels);
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let mut context = setup();
    let first = prepare_canonical_ranked_view_v1(&canonical, 17, &"first".into()).unwrap();
    let second = prepare_canonical_ranked_view_v1(&canonical, 17, &"second".into()).unwrap();
    assert_eq!(second.function_ordinal, 1);
    let root = ModuleOp::new(&mut context, "final_ranked_analysis".try_into().unwrap());
    let before = mutation_epoch(&context).unwrap();
    let views = materialize_canonical_ranked_views_v1(
        &mut context,
        &root,
        vec![Some(first), None, Some(second)],
    )
    .unwrap();
    assert!(views[1].is_none());
    let first = views[0].as_ref().unwrap();
    let second = views[2].as_ref().unwrap();
    assert_eq!(first.mutation_epoch, second.mutation_epoch);
    assert_eq!(first.mutation_epoch, mutation_epoch(&context).unwrap());
    assert!(first.mutation_epoch > before);
    let block = root
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .next()
        .unwrap();
    assert_eq!(
        block.deref(&context).iter(&context).collect::<Vec<_>>(),
        vec![
            first.pliron().get_operation(),
            second.pliron().get_operation()
        ]
    );
    first.revalidate(&context, &canonical, 17).unwrap();
    second.revalidate(&context, &canonical, 17).unwrap();
    assert!(crate::run_pliron_ranked_bounds_check_v1(&context, first.pliron()).is_clean());
    first.revalidate(&context, &canonical, 17).unwrap();
    second.revalidate(&context, &canonical, 17).unwrap();

    let allocation = find::<RankedViewOp>(&context, first);
    allocation.set_attr_kernel_allocation_origin(&context, AllocationOriginAttr(2));
    allocation.set_attr_kernel_allocation_origin(&context, AllocationOriginAttr(1));
    for view in [first, second] {
        assert!(
            derive_pliron_ir_structural_identity_v1(&context, view.pliron())
                .unwrap()
                .exactly_matches(&view.graph.structural)
        );
        assert!(matches!(
            view.revalidate(&context, &canonical, 17),
            Err(CanonicalRankedViewErrorV1::MutationEpochChanged)
        ));
    }
}

#[test]
fn single_view_attachment_after_seal_is_rejected() {
    let (canonical, plan) = prepare(scalar_module("single_attach"));
    let mut context = setup();
    let root = ModuleOp::new(&mut context, "analysis".try_into().unwrap());
    let view = plan.materialize(&mut context).unwrap();
    assert_eq!(view.mutation_epoch, mutation_epoch(&context).unwrap());
    view.revalidate(&context, &canonical, 17).unwrap();
    root.append_operation(&mut context, view.pliron().get_operation(), 0);
    assert!(
        derive_pliron_ir_structural_identity_v1(&context, view.pliron())
            .unwrap()
            .exactly_matches(&view.graph.structural)
    );
    assert!(matches!(
        view.revalidate(&context, &canonical, 17),
        Err(CanonicalRankedViewErrorV1::MutationEpochChanged)
    ));
}

#[test]
fn mutation_attempt_and_mutate_restore_cannot_revalidate() {
    for mutation in 0..3 {
        let (canonical, plan) = prepare(scalar_module("touch_restore"));
        let mut context = setup();
        let view = plan.materialize(&mut context).unwrap();
        let operation = view.pliron().get_operation();
        match mutation {
            0 => drop(operation.deref_mut(&context)),
            1 => {
                let allocation = find::<RankedViewOp>(&context, &view);
                allocation.set_attr_kernel_allocation_origin(&context, AllocationOriginAttr(2));
                allocation.set_attr_kernel_allocation_origin(&context, AllocationOriginAttr(1));
            }
            2 => {
                let shared = operation.deref(&context);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| drop(operation.deref_mut(&context)))).is_err()
                );
                drop(shared);
            }
            _ => unreachable!(),
        }
        assert!(
            derive_pliron_ir_structural_identity_v1(&context, view.pliron())
                .unwrap()
                .exactly_matches(&view.graph.structural)
        );
        assert!(
            matches!(
                view.revalidate(&context, &canonical, 17),
                Err(CanonicalRankedViewErrorV1::MutationEpochChanged)
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn later_build_cannot_reseal_a_prior_view() {
    for batch in [false, true] {
        let (canonical, plan) = prepare(scalar_module("prior"));
        let mut context = setup();
        let root = ModuleOp::new(&mut context, "analysis".try_into().unwrap());
        let prior = plan.materialize(&mut context).unwrap();
        let seal = prior.mutation_epoch;
        let allocation = find::<RankedViewOp>(&context, &prior);
        allocation.set_attr_kernel_allocation_origin(&context, AllocationOriginAttr(2));
        allocation.set_attr_kernel_allocation_origin(&context, AllocationOriginAttr(1));
        let plan = prepare_canonical_ranked_view_v1(&canonical, 17, prior.function_id()).unwrap();
        let later = if batch {
            materialize_canonical_ranked_views_v1(&mut context, &root, vec![Some(plan)])
                .unwrap()
                .pop()
                .unwrap()
                .unwrap()
        } else {
            plan.materialize(&mut context).unwrap()
        };
        later.revalidate(&context, &canonical, 17).unwrap();
        assert_eq!(prior.mutation_epoch, seal);
        assert!(matches!(
            prior.revalidate(&context, &canonical, 17),
            Err(CanonicalRankedViewErrorV1::MutationEpochChanged)
        ));
    }
}

#[test]
fn batch_slot_boundary_includes_unselected_plans() {
    let mut context = setup();
    let root = ModuleOp::new(&mut context, "analysis".try_into().unwrap());
    let before = mutation_epoch(&context).unwrap();
    for count in [
        0,
        MAX_CANONICAL_RANKED_VIEWS_V1,
        MAX_CANONICAL_RANKED_VIEWS_V1 + 1,
    ] {
        let plans = std::iter::repeat_with(|| None).take(count).collect();
        let result = materialize_canonical_ranked_views_v1(&mut context, &root, plans);
        if count <= MAX_CANONICAL_RANKED_VIEWS_V1 {
            let views = result.unwrap();
            assert_eq!(views.len(), count);
            assert!(views.iter().all(Option::is_none));
        } else {
            assert!(matches!(result, Err(CanonicalRankedViewErrorV1::Limit {
                resource: "batch plan slots", limit: MAX_CANONICAL_RANKED_VIEWS_V1, actual
            }) if actual == count));
        }
        assert_eq!(mutation_epoch(&context).unwrap(), before);
    }
}

#[test]
fn batch_subject_and_write_contract_preflight_precedes_allocation() {
    for mismatch in 0..4 {
        let (canonical, first) = prepare(scalar_module("first"));
        let second = match mismatch {
            0 => prepare(scalar_module("different_canonical")).1,
            1 => prepare_canonical_ranked_view_v1(&canonical, 18, &"first".into()).unwrap(),
            2 => prepare_canonical_ranked_view_v1(&canonical, 17, &"first".into()).unwrap(),
            3 => plan_writes(fill_module("unproved_write")).1,
            _ => unreachable!(),
        };
        let mut context = setup();
        let root = ModuleOp::new(&mut context, "analysis".try_into().unwrap());
        let before = mutation_epoch(&context).unwrap();
        let result = materialize_canonical_ranked_views_v1(
            &mut context,
            &root,
            vec![Some(first), Some(second)],
        );
        match mismatch {
            0 | 1 => assert!(matches!(
                result,
                Err(CanonicalRankedViewErrorV1::InvalidBatch(
                    "mixed canonical identities or final epochs"
                ))
            )),
            2 => assert!(matches!(
                result,
                Err(CanonicalRankedViewErrorV1::InvalidBatch(
                    "duplicate function"
                ))
            )),
            3 => assert!(matches!(
                result,
                Err(CanonicalRankedViewErrorV1::MissingWriteContracts { .. })
            )),
            _ => unreachable!(),
        }
        assert_eq!(mutation_epoch(&context).unwrap(), before);
        checked_empty_batch_root(&context, &root).unwrap();
    }
}

#[test]
fn batch_root_must_be_an_empty_single_block_module() {
    for invalid in 0..4 {
        let (_, plan) = prepare(scalar_module("invalid_root"));
        let mut context = setup();
        let root = if invalid == 0 {
            let ty = FunctionType::get(&mut context, vec![], vec![]);
            let function = FuncOp::new(&mut context, "not_module".try_into().unwrap(), ty);
            ModuleOp::from_operation(function.get_operation())
        } else {
            let root = ModuleOp::new(&mut context, "analysis".try_into().unwrap());
            match invalid {
                1 => {
                    let op = IndexConstantOp::new(&mut context, 0);
                    root.append_operation(&mut context, op.get_operation(), 0);
                }
                2 => {
                    let block = BasicBlock::new(&mut context, None, vec![]);
                    block.insert_at_back(root.get_region(&context), &context);
                }
                3 => {
                    let block = root
                        .get_region(&context)
                        .deref(&context)
                        .iter(&context)
                        .next()
                        .unwrap();
                    let index = IndexType::get(&mut context);
                    BasicBlock::push_argument(block, &context, index.into());
                }
                _ => unreachable!(),
            }
            root
        };
        let before = mutation_epoch(&context).unwrap();
        assert!(
            matches!(
                materialize_canonical_ranked_views_v1(&mut context, &root, vec![Some(plan)]),
                Err(CanonicalRankedViewErrorV1::InvalidBatchRoot)
            ),
            "root case {invalid}"
        );
        assert_eq!(mutation_epoch(&context).unwrap(), before);
    }
}

#[test]
fn batch_root_attributes_are_closed_before_verification() {
    for invalid in 0..5 {
        let mut context = setup();
        let root = ModuleOp::new(&mut context, "analysis".try_into().unwrap());
        match invalid {
            0 => root
                .get_operation()
                .deref_mut(&context)
                .attributes
                .0
                .clear(),
            1 => root.get_operation().deref_mut(&context).attributes.set(
                ATTR_KEY_SYM_NAME.clone(),
                StringAttr::new("wrong_type".into()),
            ),
            2 => root.get_operation().deref_mut(&context).attributes.set(
                "unknown".try_into().unwrap(),
                StringAttr::new("x".repeat(4096)),
            ),
            3 => {
                let mut operation = root.get_operation().deref_mut(&context);
                operation.attributes.0.clear();
                operation.attributes.set(
                    "x".repeat(4096).try_into().unwrap(),
                    StringAttr::new("unknown_key".into()),
                );
            }
            4 => {
                let block = root
                    .get_region(&context)
                    .deref(&context)
                    .iter(&context)
                    .next()
                    .unwrap();
                block.deref_mut(&context).attributes.set(
                    "unknown".try_into().unwrap(),
                    StringAttr::new("x".repeat(4096)),
                );
            }
            _ => unreachable!(),
        }
        let before = mutation_epoch(&context).unwrap();
        assert!(
            matches!(
                materialize_canonical_ranked_views_v1(&mut context, &root, vec![]),
                Err(CanonicalRankedViewErrorV1::InvalidBatchRoot)
            ),
            "attribute case {invalid}"
        );
        assert_eq!(mutation_epoch(&context).unwrap(), before);
    }
}

#[test]
fn batch_root_symbol_payload_boundary_is_checked_without_mutation() {
    for bytes in [
        MAX_CANONICAL_RANKED_ROOT_SYMBOL_BYTES_V1,
        MAX_CANONICAL_RANKED_ROOT_SYMBOL_BYTES_V1 + 1,
    ] {
        let mut context = setup();
        let root = ModuleOp::new(&mut context, "x".repeat(bytes).try_into().unwrap());
        let before = mutation_epoch(&context).unwrap();
        let result = materialize_canonical_ranked_views_v1(&mut context, &root, vec![]);
        if bytes == MAX_CANONICAL_RANKED_ROOT_SYMBOL_BYTES_V1 {
            assert!(result.unwrap().is_empty());
        } else {
            assert!(matches!(result, Err(CanonicalRankedViewErrorV1::Limit {
                resource: "batch root symbol bytes", limit: MAX_CANONICAL_RANKED_ROOT_SYMBOL_BYTES_V1, actual
            }) if actual == bytes));
        }
        assert_eq!(mutation_epoch(&context).unwrap(), before);
    }
}

#[test]
fn live_scalar_guard_origin_and_cfg_mutations_are_rejected() {
    for mutation in 0..4 {
        let mut module = fill_module("mutated");
        rhs_constant(&mut module, 0x3f80_0000);
        operations(&mut module).pop();
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(7),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        for id in [1, 2] {
            let mut block = KirBlock::new(BlockId(id));
            block.terminator = Some(Terminator::Return { values: vec![] });
            body.blocks.push(block);
        }
        let (canonical, plan) = prepare(module);
        let mut context = setup();
        let view = plan.materialize(&mut context).unwrap();
        match mutation {
            0 => find::<SemanticTypedConstantOp>(&context, &view)
                .set_attr_kernel_semantic_typed_constant_bits(&context, SemanticConstantAttr(0)),
            1 => {
                let zero = find::<IndexConstantOp>(&context, &view).result(&context);
                let guard = find::<IndexLessThanBranchOp>(&context, &view).get_operation();
                Operation::replace_operand(guard, &context, 0, zero);
            }
            2 => find::<RankedViewOp>(&context, &view)
                .set_attr_kernel_allocation_origin(&context, AllocationOriginAttr(2)),
            3 => {
                let guard = find::<IndexLessThanBranchOp>(&context, &view).get_operation();
                Operation::replace_successor(guard, &context, 0, view.graph.live_blocks[2]);
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                view.revalidate(&context, &canonical, 17),
                Err(CanonicalRankedViewErrorV1::MutationEpochChanged)
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn isomorphic_scalar_replacement_cannot_rebind_source_correspondence() {
    let (canonical, plan) = prepare(scalar_module("replacement"));
    let mut context = setup();
    let view = plan.materialize(&mut context).unwrap();
    let symbol = find::<SemanticTypedSymbolOp>(&context, &view);
    let scalar = symbol.scalar(&context).unwrap();
    let replacement = SemanticTypedSymbolOp::new(&mut context, 2, scalar);
    replacement
        .get_operation()
        .insert_before(&context, symbol.get_operation());
    symbol.get_operation().unlink(&context);
    let actual = derive_pliron_ir_structural_identity_v1(&context, view.pliron()).unwrap();
    assert!(actual.exactly_matches(&view.graph.structural));
    assert!(matches!(
        view.revalidate(&context, &canonical, 17),
        Err(CanonicalRankedViewErrorV1::MutationEpochChanged)
    ));
}

#[test]
fn no_owner_identity_means_no_materialization() {
    let (_, plan) = prepare(scalar_module("owner_required"));
    let mut context = Context::new();
    let epoch = context.ir_mutation_attempt_epoch().unwrap();
    assert!(matches!(
        plan.materialize(&mut context),
        Err(CanonicalRankedViewErrorV1::MissingContextIdentity)
    ));
    assert!(context.is_ir_empty());
    assert_eq!(context.ir_mutation_attempt_epoch().unwrap(), epoch);
}

#[test]
fn projection_selector_preserves_carrier_noop_path_but_not_unknown_ops() {
    let mut module = plain_module();
    assert!(needs_ranked_projection(&module.functions[0]));
    operations(&mut module).clear();
    assert!(!needs_ranked_projection(&module.functions[0]));
    operations(&mut module).push(KirOp::effect_free(
        ValueDef::new(ValueId(12), Type::INDEX),
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
    ));
    assert!(!needs_ranked_projection(&module.functions[0]));
    operations(&mut module).push(KirOp::effect_free(
        ValueDef::new(ValueId(13), Type::F64),
        OperationKind::Constant(Constant::F64Bits(0)),
    ));
    assert!(needs_ranked_projection(&module.functions[0]));

    let mut module = scalar_module("context_only");
    operations(&mut module).truncate(1);
    assert!(!needs_ranked_projection(&module.functions[0]));
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let module = decode_module_v13(canonical.canonical_bytes()).unwrap();
    assert!(!needs_ranked_projection(&module.functions[0]));
}

#[test]
fn synthetic_block_boundary_is_independent_of_source_operation_budget() {
    for stores in [256, 257] {
        let mut module = plain_module();
        let ops = operations(&mut module);
        let store = ops.pop().unwrap();
        ops.extend(std::iter::repeat_n(store, stores));
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let result = structural_plan(&canonical);
        if stores == 256 {
            let plan = result.unwrap();
            assert_eq!(plan.block_count(), 513);
            assert_eq!(plan.writes.len(), 256);
            assert!(plan.tree_work() >= 3 + plan.block_count() + 2 * (256 * 3 + 1));
        } else {
            assert!(matches!(
                result,
                Err(CanonicalRankedViewErrorV1::Limit {
                    resource: "synthetic blocks",
                    limit: 512,
                    ..
                })
            ));
        }
    }
}

#[test]
fn source_operation_boundary_and_sparse_ssa_are_bounded() {
    for count in [
        MAX_CANONICAL_RANKED_SOURCE_OPERATIONS_V1,
        MAX_CANONICAL_RANKED_SOURCE_OPERATIONS_V1 + 1,
    ] {
        let mut module = plain_module();
        let ops = operations(&mut module);
        ops.clear();
        ops.extend((0..count).map(|i| {
            KirOp::effect_free(
                ValueDef::new(ValueId(1_000_000 + i as u32), Type::INDEX),
                OperationKind::Constant(Constant::Index(i as u64)),
            )
        }));
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let result = prepare_canonical_ranked_view_v1(&canonical, 17, &"plain".into());
        if count == MAX_CANONICAL_RANKED_SOURCE_OPERATIONS_V1 {
            let plan = result.unwrap();
            assert_eq!(plan.covered_source_operations(), count);
            assert!(plan.nodes <= count + 4);
        } else {
            assert!(matches!(
                result,
                Err(CanonicalRankedViewErrorV1::Limit {
                    resource: "source operations",
                    ..
                })
            ));
        }
    }
}

#[test]
fn source_block_boundary_is_checked_before_cfg_projection() {
    for count in [
        MAX_CANONICAL_RANKED_SOURCE_BLOCKS_V1,
        MAX_CANONICAL_RANKED_SOURCE_BLOCKS_V1 + 1,
    ] {
        let mut module = plain_module();
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks.clear();
        for i in 0..count {
            let mut block = KirBlock::new(BlockId(i as u32));
            block.terminator = Some(if i + 1 == count {
                Terminator::Return { values: vec![] }
            } else {
                Terminator::Branch {
                    target: BlockId(i as u32 + 1),
                    arguments: vec![],
                }
            });
            body.blocks.push(block);
        }
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let result = prepare_canonical_ranked_view_v1(&canonical, 17, &"plain".into());
        if count == MAX_CANONICAL_RANKED_SOURCE_BLOCKS_V1 {
            assert_eq!(result.unwrap().block_count(), count);
        } else {
            assert!(matches!(
                result,
                Err(CanonicalRankedViewErrorV1::Limit {
                    resource: "source blocks",
                    ..
                })
            ));
        }
    }
}

#[test]
fn value_count_boundary_precedes_unsupported_block_arguments() {
    for values in [
        MAX_CANONICAL_RANKED_SOURCE_VALUES_V1,
        MAX_CANONICAL_RANKED_SOURCE_VALUES_V1 + 1,
    ] {
        let mut module = scalar_module("values");
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[0].operations.clear();
        let count = values - body.parameters.len();
        body.blocks[0].terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(1); count],
        });
        let mut block = KirBlock::new(BlockId(1));
        block.parameters = (0..count)
            .map(|i| ValueDef::new(ValueId(100 + i as u32), Type::INDEX))
            .collect();
        block.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.push(block);
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let error = prepare_canonical_ranked_view_v1(&canonical, 17, &"values".into()).unwrap_err();
        if values == MAX_CANONICAL_RANKED_SOURCE_VALUES_V1 {
            assert!(matches!(
                error,
                CanonicalRankedViewErrorV1::Unsupported { .. }
            ));
        } else {
            assert!(matches!(
                error,
                CanonicalRankedViewErrorV1::Limit {
                    resource: "source values",
                    limit: 32_768,
                    actual: 32_769
                }
            ));
        }
    }
}

#[test]
fn parameter_count_boundary_is_not_inflated_by_other_budgets() {
    for count in [64, 65] {
        let mut module = scalar_module("parameters");
        let function = &mut module.functions[0];
        function.signature.parameters = vec![Type::INDEX; count];
        let body = function.body.as_mut().unwrap();
        body.parameters = (0..count).map(|i| ValueId(i as u32)).collect();
        body.blocks[0].operations.clear();
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let result = prepare_canonical_ranked_view_v1(&canonical, 17, &"parameters".into());
        if count == 64 {
            assert_eq!(result.unwrap().arguments.len(), 64);
        } else {
            assert!(matches!(
                result,
                Err(CanonicalRankedViewErrorV1::Limit {
                    resource: "parameters",
                    limit: 64,
                    actual: 65
                })
            ));
        }
    }
}

#[test]
fn multiple_parameter_allocations_are_distinct_origins_not_noalias_proofs() {
    let mut module = scalar_module("origins");
    let function = &mut module.functions[0];
    function
        .signature
        .parameters
        .push(function.signature.parameters[0].clone());
    function.body.as_mut().unwrap().parameters.push(ValueId(14));
    let (_, plan) = prepare(module);
    let mut context = setup();
    let view = plan.materialize(&mut context).unwrap();
    let allocations = view
        .graph
        .live_operations
        .iter()
        .filter_map(|p| Operation::get_op::<RankedViewOp>(*p, &context))
        .collect::<Vec<_>>();
    assert_eq!(allocations.len(), 2);
    assert_eq!(allocations[0].allocation_origin(&context), Some(1));
    assert_eq!(allocations[1].allocation_origin(&context), Some(4));
    for allocation in allocations {
        assert_eq!(allocation.noalias_class(&context), Some(0));
    }
}

#[test]
fn unsupported_load_and_element_type_never_publish_an_empty_effect_view() {
    let mut module = plain_module();
    operations(&mut module).pop();
    module.functions[0].signature.parameters[0] =
        Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let ops = operations(&mut module);
    ops[4].results[0].ty = pointer.clone();
    ops[5].results[0].ty = pointer;
    ops.push(KirOp::effect_free(
        ValueDef::new(ValueId(12), Type::F32),
        OperationKind::Load {
            pointer: ValueId(11),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    reject(module, "closed scalar/global-store projection");

    let mut module = scalar_module("element");
    operations(&mut module).clear();
    module.functions[0].signature.parameters[0] =
        Type::slice(Type::F64, AddressSpace::Global, AccessMode::ReadOnly);
    reject(module, "global f32 slice");
}

#[test]
fn public_preflight_rejects_writes_even_in_other_source_blocks() {
    let mut module = plain_module();
    let body = module.functions[0].body.as_mut().unwrap();
    let store = body.blocks[0].operations.pop().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    let mut second = KirBlock::new(BlockId(7));
    second.operations.push(store);
    second.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(second);
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    assert!(matches!(
        prepare_canonical_ranked_view_v1(&canonical, 17, &"plain".into()),
        Err(CanonicalRankedViewErrorV1::MissingWriteContracts {
            block: BlockId(7),
            operation: 0
        })
    ));
}
