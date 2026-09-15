//! Actual registered scalar source through ranked verification and the sole KIR lowerer.
use super::*;
use fe2o3_kernel_ir::{
    ExecutionCapabilityOperationV1 as E, ExecutionCapabilityRoleV1 as R, ExecutionTypeIdentityV1,
    OperationKind, Type,
};
use fe2o3_lower_mir_kernel::{ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

#[path = "lowering/mutations.rs"]
mod mutations;
#[path = "lowering/allocation_mutations.rs"]
mod allocation_mutations;

pub(super) const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{KernelContext, kernel};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn reusable(mut context: KernelContext<'_>, enabled: u32) {
    if enabled == 0 { return; }
    context.with_workgroup(|workgroup| {
        let _reusable = workgroup.allocate_lds::<f32, 64>().into_reusable();
    });
}
"#;

fn stage<T, E: std::fmt::Debug>(name: &str, result: Result<T, E>) -> T {
    let value = result.unwrap_or_else(|error| {
        panic!("[fe2o3-reusable-source-kir-v1] stage={name} rejected: {error:?}")
    });
    eprintln!("[fe2o3-reusable-source-kir-v1] stage={name} passed");
    value
}

pub(super) fn check(
    imported: crate::collector::ConstructedProductionSemanticMirV1,
    typed_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
) {
    let typed_roots = stage(
        "descriptor-order",
        crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
            typed_roots,
            &imported.semantic_mir,
        ),
    );
    stage(
        "descriptor-ownership",
        crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
            &typed_roots,
            &imported.semantic_mir,
        ),
    );
    let semantic = stage(
        "mir-owner",
        ProductionSemanticMirOwnerV1::try_new(
            imported.semantic_mir,
            ProductionSemanticMirLimitsV1::default(),
        ),
    );
    let ssa = stage(
        "ssa-owner",
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default()),
    );
    stage("ssa-replay", ssa.verify_replay());
    let mir = ssa.source_semantic();
    let [root] = mir.roots() else {
        panic!("one registered scalar root")
    };
    let [row] = ssa
        .execution_plan_for_root(*root)
        .unwrap()
        .defined_reusable_lds_results()
    else {
        panic!("one checked consuming converter receipt")
    };
    let row = *row;
    let record = row.record();
    let source = record.source();
    assert_eq!(
        (
            record.elements(),
            record.element_size(),
            record.element_align()
        ),
        (64, 4, 4)
    );
    assert_eq!(record.provenance().root(), *root);
    assert_eq!(
        mir.types()[record.types().element.index() as usize].shape(),
        &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    );
    let SemanticCallableDeclV1::CompilerIntrinsic {
        operation:
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
                contract: allocator,
            },
        ..
    } = &mir.callables()[source.allocation_callable.index() as usize]
    else {
        panic!("the source receipt must name its actual allocation callable")
    };
    assert!(matches!(
        allocator.operation(),
        SemanticExecutionCapabilityOperationV1::LdsAllocate { .. }
    ));
    let allocator_identity = *allocator.source_identity().as_bytes();
    assert_ne!(row.allocation(), row.parameter());
    assert_ne!(row.parameter(), row.return_local());
    assert!(row.erased());
    let identity = |ty: SemanticTypeIdV1| {
        ExecutionTypeIdentityV1::new(*mir.types()[ty.index() as usize].identity().as_bytes())
    };
    let SemanticExecutionCapabilityOperationV1::LdsAllocate { workgroup: reference, .. } = allocator.operation() else { unreachable!() };
    let SemanticTypeShapeV1::Pointer(pointer) = mir.types()[reference.index() as usize].shape() else { panic!("actual shared Workgroup source reference"); };
    assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
    assert_eq!(pointer.mutability(), SemanticMutabilityV1::Immutable);
    let expected_workgroup_pair = (identity(reference), identity(pointer.pointee()));
    assert_ne!(expected_workgroup_pair.0, expected_workgroup_pair.1);
    let expected_types = [
        identity(record.types().input),
        identity(record.types().output),
        identity(record.types().element),
    ];
    let original_caller = *mir.functions()[source.caller.index() as usize]
        .identity()
        .as_bytes();
    let root_source = *mir.functions()[root.index() as usize].identity().as_bytes();
    let expansion = *ssa.execution_expansion().identity();
    let expanded_root = *ssa.execution_view_for_root(*root).unwrap().identity();
    // Fixed-size source coordinates survive failure after the owner is consumed.
    eprintln!(
        "[fe2o3-reusable-source-kir-v1] root={root:?} original_caller={:?} original_allocation={:?} original_conversion={:?} caller_instance={:?} callee_instance={:?} expanded_allocation={:?} allocation={:?} parameter_site={:?}:{} parameter={:?} return_site={:?}:{} returned={:?} destination={:?}",
        source.caller,
        source.allocation_block,
        source.conversion_block,
        row.caller(),
        row.callee(),
        row.allocation_block(),
        row.allocation(),
        row.parameter_block(),
        row.parameter_statement(),
        row.parameter(),
        row.return_block(),
        row.return_statement(),
        row.return_local(),
        row.destination(),
    );
    let inputs = typed_roots
        .iter()
        .map(|typed| {
            crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                typed.logical_name(),
                typed.kernel_binding_bytes(),
                typed.source_launch().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let ranked = stage(
        "ranked-verification",
        crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_with_contexts_v1(
            ssa,
            &inputs,
            &imported.reference_effect_bindings,
            Some(&imported.kernel_contexts),
        ),
    );
    let contexts = stage(
        "live-context-lowering-inputs",
        imported.kernel_contexts.into_lowering_inputs(
            &imported.rustc_identity_inventory,
            &imported.rustc_target,
            ranked.roots(),
            &typed_roots,
        ),
    );
    let receipt = stage(
        "ranked-roster-receipt",
        ranked.into_verified_roster_receipt(),
    );
    let (receipt, verification) = stage(
        "ranked-module-receipt",
        receipt.into_module_verified_receipt(),
    );
    assert!(verification.every_functional_verification_is_coherent());
    let lowered = stage(
        "production-target-neutral-lowering",
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks_with_kernel_contexts(
            receipt,
            ProductionSemanticKirLimitsV1::default(),
            contexts,
        ),
    );
    stage("kir-equivalence", lowered.verify_equivalence());
    let canonical = lowered
        .canonical_kernel_ir_v13()
        .expect("[fe2o3-reusable-source-kir-v1] stage=kir-v13 missing canonical owner");
    stage("kir-revalidation", canonical.revalidate());
    let module = lowered.module();
    let mut allocation = None;
    let mut workgroup = None;
    let mut conversion = None;
    for (function_index, function) in module.functions.iter().enumerate() {
        let Some(body) = &function.body else { continue };
        for (block_index, block) in body.blocks.iter().enumerate() {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                    continue;
                };
                match contract.operation {
                    E::LdsAllocateBorrowed { .. } => assert!(
                        allocation
                            .replace((function_index, block_index, operation_index, operation, contract))
                            .is_none()
                    ),
                    E::ReusableLdsConversion(value) => {
                        assert!(
                            conversion
                                .replace((
                                    function_index,
                                    block_index,
                                    operation_index,
                                    operation,
                                    contract,
                                    value
                                ))
                                .is_none()
                        );
                    }
                    E::WorkgroupDerive { .. } => assert!(workgroup.replace((function_index, operation)).is_none()),
                    _ => panic!(
                        "unexpected capability operation in allocation-only source: {:?}",
                        contract.operation
                    ),
                }
            }
        }
    }
    let (allocation_function, allocation_block, allocation_index, allocation_op, allocation) =
        allocation.expect("one actual allocation");
    let (conversion_function, block, index, conversion_op, conversion, value) =
        conversion.expect("one actual conversion");
    assert_eq!(allocation_function, conversion_function);
    let (workgroup_function, workgroup_op) = workgroup.expect("actual Workgroup issuer");
    assert_eq!(workgroup_function, allocation_function);
    let [issued] = workgroup_op.results.as_slice() else { panic!("one actual Workgroup SSA result"); };
    assert_eq!(allocation.operands, [issued.id]);
    let Type::ExecutionCapability(owned) = &issued.ty else { panic!("owned Workgroup capability"); };
    assert_eq!(owned.role, R::Workgroup);
    assert_eq!(owned.source_type, expected_workgroup_pair.1);
    let E::LdsAllocateBorrowed { workgroup_reference, workgroup, .. } = allocation.operation else { unreachable!() };
    assert_eq!((workgroup_reference, workgroup), expected_workgroup_pair);
    assert_eq!(allocation.signature.arguments().collect::<Vec<_>>(), [expected_workgroup_pair.0]);
    let allocation_payload = fe2o3_kernel_ir::encode_execution_capability_contract_v1(allocation).unwrap();
    assert_eq!(&allocation_payload[..2], &[5, 30]);
    assert_eq!(fe2o3_kernel_ir::decode_execution_capability_contract_v1(&allocation_payload, allocation.operands.clone()), Some(allocation.clone()));
    assert_eq!(allocation.obligations.bits(), fe2o3_kernel_ir::required_execution_obligations_v1(&allocation.operation));
    let [allocated] = allocation_op.results.as_slice() else {
        panic!("one allocated handle")
    };
    let [converted] = conversion_op.results.as_slice() else {
        panic!("one reusable handle")
    };
    assert_eq!(conversion.operands, [allocated.id]);
    assert_ne!(allocated.id, converted.id);
    let Type::ExecutionCapability(input) = &allocated.ty else {
        panic!("typed allocation")
    };
    let Type::ExecutionCapability(output) = &converted.ty else {
        panic!("typed reusable handle")
    };
    assert_eq!([value.input, value.output, value.element], expected_types);
    assert_eq!(
        (
            value.layout.byte_size,
            value.layout.byte_alignment,
            value.elements
        ),
        (4, 4, 64)
    );
    assert_eq!(value.defined_function, *record.source_identity().as_bytes());
    assert_eq!(value.defined_abi, *record.abi_identity().as_bytes());
    assert_eq!(value.defined_body, *record.body_identity());
    assert_eq!(value.source_binding, source.source_binding);
    assert_eq!(input.role, value.input_role());
    assert_eq!(
        output.role,
        R::ReusableLds {
            element: value.element,
            layout: value.layout,
            elements: 64
        }
    );
    assert_eq!(value.output_type(input), Some(output.clone()));
    assert_eq!(conversion.workgroup_brand, Some(*record.brand().as_bytes()));
    assert_eq!(conversion.epoch_before, Some(*record.epoch().as_bytes()));
    assert_eq!(conversion.epoch_after, None);
    assert_eq!(allocation.provenance, conversion.provenance);
    assert_eq!(allocation.workgroup_brand, conversion.workgroup_brand);
    assert_eq!(allocation.epoch_before, conversion.epoch_before);
    assert_eq!(allocation.source.function, original_caller);
    assert_eq!(allocation.source.operation, allocator_identity);
    assert_eq!(allocation.source.block, source.allocation_block.index());
    let allocation_occurrence = allocation
        .source
        .occurrence
        .expect("actual allocator occurrence");
    assert_eq!(allocation_occurrence.root_source_identity(), root_source);
    assert_eq!(allocation_occurrence.expansion_identity(), expansion);
    assert_eq!(
        allocation_occurrence.expanded_root_identity(),
        expanded_root
    );
    assert_eq!(
        allocation_occurrence.caller_instance(),
        row.caller().index()
    );
    assert_eq!(
        allocation_occurrence.expanded_block(),
        row.allocation_block().index()
    );
    let provenance = record.provenance();
    assert_eq!(
        conversion.provenance.root,
        module.functions[conversion_function].id
    );
    assert_eq!(
        conversion.provenance.kernel_binding,
        *provenance.kernel_binding().as_bytes()
    );
    assert_eq!(
        conversion.provenance.frontend_unit,
        *provenance.frontend_unit().as_bytes()
    );
    assert_eq!(
        conversion.provenance.kernel_marker,
        *provenance.kernel_marker().as_bytes()
    );
    assert_eq!(
        conversion.provenance.target_brand,
        *provenance.target_brand().as_bytes()
    );
    assert_eq!(
        conversion.provenance.launch_brand,
        *provenance.launch_brand().as_bytes()
    );
    assert_eq!(
        conversion.provenance.issuance,
        *provenance.issuance().as_bytes()
    );
    assert_eq!(conversion.source.function, original_caller);
    assert_eq!(conversion.source.operation, value.defined_function);
    assert_eq!(conversion.source.block, source.conversion_block.index());
    let occurrence = conversion
        .source
        .occurrence
        .expect("checked source occurrence");
    assert_eq!(occurrence.root_source_identity(), root_source);
    assert_eq!(occurrence.expansion_identity(), expansion);
    assert_eq!(occurrence.expanded_root_identity(), expanded_root);
    assert_eq!(occurrence.caller_instance(), row.caller().index());
    assert_eq!(occurrence.expanded_block(), row.return_block().index());
    assert!(conversion.operation.memory_effects().is_empty());
    assert!(!conversion.operation.transitions_epoch());
    assert_eq!(
        conversion.obligations.bits(),
        fe2o3_kernel_ir::required_execution_obligations_v1(&conversion.operation)
    );
    let wire = stage("kir-encode", fe2o3_kernel_ir::encode_module_v13(module));
    assert_eq!(
        stage("kir-decode", fe2o3_kernel_ir::decode_module_v13(&wire)),
        *module
    );
    let payload = fe2o3_kernel_ir::encode_execution_capability_contract_v1(conversion).unwrap();
    assert_eq!(&payload[..2], &[6, 29]);
    assert_eq!(
        fe2o3_kernel_ir::decode_execution_capability_contract_v1(
            &payload,
            conversion.operands.clone()
        ),
        Some(conversion.clone())
    );
    mutations::check(module, conversion_function, block, index);
    allocation_mutations::check(module, allocation_function, allocation_block, allocation_index);
    eprintln!(
        "[fe2o3-reusable-source-kir-v1] stage=source-to-kir passed allocations=1 conversions=1 bytes=256 initialization=false epoch_transition=false"
    );
}

#[test]
#[ignore = "requires cached authenticated AMD metadata and real ranked checks; no Cargo"]
fn reusable_lds_scalar_source_kir_gfx942() {
    super::run_with_lowering(
        "gfx942",
        "lowering::reusable_lds_scalar_source_kir_gfx942",
        true,
    );
}

#[test]
#[ignore = "requires cached authenticated AMD metadata and real ranked checks; no Cargo"]
fn reusable_lds_scalar_source_kir_gfx950() {
    super::run_with_lowering(
        "gfx950",
        "lowering::reusable_lds_scalar_source_kir_gfx950",
        true,
    );
}
