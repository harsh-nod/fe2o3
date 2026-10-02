use super::super::super::source_roles::check_source;
use super::super::super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, OperationKind, WaveF32ReductionKindV1, WaveOperationKind,
};
include!("production_checked_output_source_roles_fixtures_v1_tests.rs");

#[path = "production_checked_output_wave_broadcast_source_v1_tests.rs"]
mod broadcast_source;

// Genuine admitted semantic source, not a collected ordinary-Rust fixture.
// Two unconditional ordered collectives retain an exact result-to-input edge.
// The no-global-effect ranked component claims no arithmetic/convergence authority.
fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}
fn local_place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    place(local, ty)
}
fn unit_type() -> SemanticTypeDeclV1 {
    types().remove(0)
}
fn scalar_type(tag: u8, scalar: SemanticScalarTypeV1) -> SemanticTypeDeclV1 {
    let (primitive, size, max) = match scalar {
        SemanticScalarTypeV1::Float { bits: 32 } => (
            SemanticBackendPrimitiveV1::float(32, 4),
            4,
            u32::MAX as u128,
        ),
        SemanticScalarTypeV1::Bool => (SemanticBackendPrimitiveV1::integer(false, 8, 1), 1, 1),
        _ => unreachable!(),
    };
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            size,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, max),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(scalar),
    )
}
fn direct_abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}
fn shared_reference_abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    true,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn compiler_intrinsic_callable(
    tag: u8,
    inputs: Vec<SemanticAbiValueV1>,
    output: SemanticAbiValueV1,
    operation: SemanticCompilerIntrinsicOperationV1,
) -> SemanticCallableDeclV1 {
    let arguments = inputs
        .into_iter()
        .map(SemanticAbiArgumentV1::source)
        .collect::<Vec<_>>();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        arguments.len() as u32,
        arguments,
        output,
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(tag)),
    }
}

fn reduction_admission(
    width: u32,
    kind: SemanticSubgroupReductionKindV1,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    reduction_admission_with_borrow_occurrences(width, kind, true)
}

fn reduction_admission_with_borrow_occurrences(
    width: u32,
    kind: SemanticSubgroupReductionKindV1,
    fresh_borrow: bool,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let unit = SemanticTypeIdV1::from_index(0);
    let context = SemanticTypeIdV1::from_index(1);
    let context_ref = SemanticTypeIdV1::from_index(2);
    let f32_ty = SemanticTypeIdV1::from_index(3);
    let source = SemanticSourceProvenanceV1::unavailable();
    let context_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(70)),
        SemanticLayoutIdentityV1::from_sha256(bytes(70)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    let context_reference_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(71)),
        SemanticLayoutIdentityV1::from_sha256(bytes(71)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                context,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    0,
                    1,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let call_edge = |target| {
        SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallReturn,
            SemanticBlockIdV1::from_index(target),
        )
    };
    let context_call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![],
            Some(SemanticCallDestinationV1::new(
                local_place(1, context),
                call_edge(1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let borrow = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local_place(2, context_ref),
            SemanticRvalueV1::new(
                context_ref,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: local_place(1, context),
                },
            ),
        )),
    );
    let initial_value = SemanticOperandV1::Constant(SemanticConstantV1::new(
        f32_ty,
        SemanticConstantValueV1::Scalar(
            SemanticScalarValueV1::new(f32::to_bits(1.0).into(), 4).unwrap(),
        ),
    ));
    let reduction_call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(2),
            vec![
                SemanticOperandV1::Copy(local_place(2, context_ref)),
                initial_value,
            ],
            Some(SemanticCallDestinationV1::new(
                local_place(3, f32_ty),
                call_edge(2),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let function_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(72)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = [unit, context, context_ref, f32_ty]
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(73 + index as u8)),
                ty,
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            )
        })
        .collect();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let contract = SemanticKernelSourceContractV1::new(
        Some(SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap()),
        None,
        None,
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(72)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(72)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(72)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(72)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(72)),
        source,
        function_abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            block(80, vec![], context_call),
            block(81, vec![borrow.clone()], reduction_call),
            block(
                82,
                // Exercise either a fresh source borrow or two independently
                // authenticated consumers of the same shared source borrow.
                if fresh_borrow { vec![borrow] } else { vec![] },
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(2),
                        vec![value(2, context_ref), value(3, f32_ty)],
                        Some(SemanticCallDestinationV1::new(
                            place(3, f32_ty),
                            call_edge(3),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(88, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"gfx950_collective_width".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(83)),
        contract,
    ));
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        compiler_intrinsic_callable(
            84,
            vec![],
            SemanticAbiValueV1::new(context, SemanticAbiPassModeV1::Ignore),
            SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupContextCurrent { context },
        ),
        compiler_intrinsic_callable(
            85,
            vec![
                shared_reference_abi_value(context_ref),
                direct_abi_value(f32_ty),
            ],
            direct_abi_value(f32_ty),
            SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupReduceF32 {
                context,
                width,
                kind,
            },
        ),
    ];
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![
            unit_type(),
            context_type,
            context_reference_type,
            scalar_type(86, SemanticScalarTypeV1::Float { bits: 32 }),
        ],
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
}

fn reduction_receipt(
    width: u32,
    kind: SemanticSubgroupReductionKindV1,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    collective_receipt(reduction_admission(width, kind).unwrap())
}

fn collective_receipt(
    admitted: AdmittedInertSemanticMirV1,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    assert!(
        ssa.plan_for_function(SemanticFunctionIdV1::from_index(0))
            .unwrap()
            .plan()
            .promoted_variables()
            .iter()
            .any(|variable| variable.get() == 1),
        "the genuine per-call context borrows must not require opaque storage"
    );
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "logical_0",
            bytes(83),
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let name = source.executable().module().kernels[0]
        .entry
        .as_str()
        .to_owned();
    let layout = source.source_launch().roots()[0].layout();
    let kernel = ProductionRankedKernelV1::new(
        &name,
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
        ProductionConstructionV1::ranked_kernel(&name, kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(lowering.all_mandatory_reports_are_clean());
    let root = ProductionRankedSemanticProjectionRootV1::new(
        SemanticFunctionIdV1::from_index(0), 1, lowering,
        "genuine original collective source; ranked component proves only the empty global-access roster".to_owned(),
        vec![], vec![],
    );
    ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        source,
        vec![root],
    )
    .unwrap()
}

#[test]
fn wave_reduction_census_reused_context_reference_authenticates_both_shared_consumers() {
    // Shared fanout is admitted only when every outgoing use closes at an
    // authenticated intrinsic. Both original reduction occurrences must remain.
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        reduction_admission_with_borrow_occurrences(
            64,
            SemanticSubgroupReductionKindV1::Sum,
            false,
        )
        .unwrap(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    ssa.verify_replay().unwrap();
    assert!(
        ssa.plan_for_function(SemanticFunctionIdV1::from_index(0))
            .unwrap()
            .plan()
            .promoted_variables()
            .iter()
            .any(|variable| variable.get() == 1)
    );
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "logical_0",
            bytes(83),
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let lowered = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    require_wave_roster(
        lowered.executable().module(),
        64,
        SemanticSubgroupReductionKindV1::Sum,
    );
    lowered.executable().revalidate().unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn reduction_bound(
    receipt: &ProductionMaterializedRankedModuleReceiptV1,
    mutation: Option<fn(&mut fe2o3_kernel_ir::Module)>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
    let mut module = receipt.materialized.executable().module().clone();
    if let Some(mutate) = mutation {
        mutate(&mut module);
    }
    let binding = dialect_amdgcn::bind_production_target_v1(&module, Profile::Gfx950).unwrap();
    let (bound, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        binding.module(), budget,
    ).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    bound
}

fn mutate_reduction(module: &mut fe2o3_kernel_ir::Module, fault: usize) {
    let mut changed = 0;
    let mut first_input = None;
    for function in &mut module.functions {
        for block in &mut function.body.as_mut().unwrap().blocks {
            for operation in &mut block.operations {
                if let OperationKind::Wave(wave) = &mut operation.kind {
                    if let WaveOperationKind::ReduceF32 { value, .. } = &mut wave.kind {
                        if changed == 0 {
                            first_input = Some(*value);
                        } else if fault == 3 {
                            assert_ne!(*value, first_input.unwrap());
                            *value = first_input.unwrap();
                        }
                    }
                    if changed == 0 {
                        let WaveOperationKind::ReduceF32 {
                            tile_width, kind, ..
                        } = &mut wave.kind
                        else {
                            unreachable!()
                        };
                        match fault {
                            0 | 3 => {}
                            1 => *kind = WaveF32ReductionKindV1::Maximum,
                            2 => *tile_width = 8,
                            _ => unreachable!(),
                        }
                    }
                    changed += 1;
                }
            }
        }
    }
    assert_eq!(changed, 2);
}

#[test]
fn wave_reduction_census_exact_source_to_bound_rejects_valid_kind_and_tile_substitution() {
    for mutate in [
        (|module: &mut fe2o3_kernel_ir::Module| mutate_reduction(module, 1)) as fn(&mut _),
        (|module: &mut fe2o3_kernel_ir::Module| mutate_reduction(module, 2)) as fn(&mut _),
        (|module: &mut fe2o3_kernel_ir::Module| mutate_reduction(module, 3)) as fn(&mut _),
    ] {
        let receipt = reduction_receipt(16, SemanticSubgroupReductionKindV1::Sum);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget
            .reserve_storage(
                FLOOR
                    + receipt
                        .materialized
                        .unit_local_source_storage_floor_v1()
                        .unwrap(),
            )
            .unwrap();
        let bound = reduction_bound(&receipt, Some(mutate), &mut budget);
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut budget)
                .unwrap();
        budget
            .reserve_storage(checked.storage().retained_storage())
            .unwrap();
        let floor = budget.storage();
        let error = crate::ProductionCheckedOutputOwnerPolicy3V1::try_admit_general_v1(
            receipt,
            bound,
            checked,
            &mut budget,
        )
        .unwrap_err();
        assert!(matches!(error, E::Coordinates(_)), "{error:?}");
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn wave_reduction_census_bound_to_output_rejects_valid_payload_changes() {
    let receipt = reduction_receipt(16, SemanticSubgroupReductionKindV1::Sum);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(
            FLOOR
                + receipt
                    .materialized
                    .unit_local_source_storage_floor_v1()
                    .unwrap(),
        )
        .unwrap();
    let bound = reduction_bound(&receipt, None, &mut budget);
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut budget)
            .unwrap();
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    let (input, input_storage) = CanonicalKirInventoryV1::derive(&bound, &mut budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let baseline = budget.storage();
    for fault in [0, 1, 2, 3, 0] {
        let mut output = checked.owner().module().clone();
        mutate_reduction(&mut output, fault);
        let (owner, owner_storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&output, &mut budget).unwrap();
        budget
            .reserve_storage(owner_storage.retained_storage())
            .unwrap();
        let (output, output_storage) =
            CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
        budget
            .reserve_storage(output_storage.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let result = fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
            &input,
            &output,
            checked.occurrences().candidate(),
            &mut budget,
        );
        if fault == 0 {
            let (view, storage) = result.unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            drop(view);
            budget.release_storage(storage.retained_storage()).unwrap();
        } else if fault == 3 {
            assert!(matches!(
                result,
                Err(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
                    "final operand has no exact descendant"
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
                    "retained operation payload"
                ))
            ));
        }
        assert_eq!(budget.storage(), floor);
        drop(output);
        budget
            .release_storage(output_storage.retained_storage())
            .unwrap();
        drop(owner);
        budget
            .release_storage(owner_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), baseline);
    }
}

fn require_wave_roster(
    module: &fe2o3_kernel_ir::Module,
    width: u32,
    kind: SemanticSubgroupReductionKindV1,
) {
    let expected = match kind {
        SemanticSubgroupReductionKindV1::Sum => WaveF32ReductionKindV1::Sum,
        SemanticSubgroupReductionKindV1::Maximum => WaveF32ReductionKindV1::Maximum,
    };
    let mut count = 0;
    let mut consumed_by_reduction = 0;
    for function in &module.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            for operation in &block.operations {
                if let OperationKind::Wave(wave) = &operation.kind {
                    assert!(
                        matches!(wave.kind, WaveOperationKind::ReduceF32 { tile_width, kind, .. }
                        if tile_width == width && kind == expected)
                    );
                    assert_eq!(operation.results.len(), 1);
                    count += 1;
                    let result = operation.results[0].id;
                    consumed_by_reduction += function.body.as_ref().unwrap().blocks.iter()
                        .flat_map(|block| &block.operations)
                        .filter(|row| matches!(&row.kind, OperationKind::Wave(wave)
                            if matches!(wave.kind, WaveOperationKind::ReduceF32 { value, .. } if value == result)))
                        .count();
                }
            }
        }
    }
    assert_eq!(count, 2);
    assert_eq!(consumed_by_reduction, 1);
}

fn reduction_admit_with_limits(
    kind: SemanticSubgroupReductionKindV1,
    work_limit: usize,
    storage_limit: usize,
) -> (
    R<crate::ProductionCheckedOutputOwnerPolicy3V1>,
    usize,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let receipt = reduction_receipt(16, kind);
    let mut preparation_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut preparation = AssertOriginBudgetV1::new(&mut preparation_work, STORAGE);
    preparation
        .reserve_storage(
            FLOOR
                + receipt
                    .materialized
                    .unit_local_source_storage_floor_v1()
                    .unwrap(),
        )
        .unwrap();
    let bound = reduction_bound(&receipt, None, &mut preparation);
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(&bound, &mut preparation)
            .unwrap();
    preparation
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    let floor = preparation.storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = crate::ProductionCheckedOutputOwnerPolicy3V1::try_admit_general_v1(
        receipt,
        bound,
        checked,
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        floor,
        budget.failed_work(),
        budget.failed_storage(),
    )
}

#[test]
fn wave_reduction_census_full_source_exact_and_one_short_admission_keeps_caller_floor() {
    for kind in [
        SemanticSubgroupReductionKindV1::Sum,
        SemanticSubgroupReductionKindV1::Maximum,
    ] {
        let (owner, work, peak, floor, failed_work, failed_storage) =
            reduction_admit_with_limits(kind, WORK, STORAGE);
        drop(owner.unwrap());
        assert_eq!((failed_work, failed_storage), (None, None));
        assert!(peak > floor && work > 0);
        for (work_limit, storage_limit, success) in [
            (work, peak, true),
            (work - 1, peak, false),
            (work, peak - 1, false),
        ] {
            let (result, accepted, actual_peak, same_floor, failed_work, failed_storage) =
                reduction_admit_with_limits(kind, work_limit, storage_limit);
            assert_eq!(same_floor, floor);
            if success {
                drop(result.unwrap());
                assert_eq!((accepted, actual_peak), (work, peak));
                assert_eq!((failed_work, failed_storage), (None, None));
            } else {
                let error = result.unwrap_err();
                let mut cause: &(dyn std::error::Error + 'static) = &error;
                let resource = loop {
                    if let Some(resource) = cause.downcast_ref::<AssertOriginResourceV1>() {
                        break *resource;
                    }
                    cause = cause.source().unwrap_or_else(|| {
                        panic!("{kind:?}: missing typed resource cause: {error:?}")
                    });
                };
                if work_limit < work {
                    assert!(matches!(resource, AssertOriginResourceV1::Work(error)
                        if error.actual() == work && error.limit() == work_limit));
                    assert_eq!((failed_work, failed_storage), (Some(work), None));
                } else {
                    assert!(matches!(resource, AssertOriginResourceV1::Storage(error)
                        if error.actual() == peak && error.limit() == storage_limit));
                    assert_eq!((failed_work, failed_storage), (None, Some(peak)));
                }
            }
            assert!(accepted <= work_limit && actual_peak <= storage_limit);
        }
    }
}

#[test]
fn wave_reduction_census_genuine_source_composes_all_four_policy_prefixes_and_native() {
    for kind in [
        SemanticSubgroupReductionKindV1::Sum,
        SemanticSubgroupReductionKindV1::Maximum,
    ] {
        for width in [1, 2, 4, 8, 16, 32, 64] {
            for policy in 3..=6 {
                let receipt = reduction_receipt(width, kind);
                let original = receipt.materialized.executable().module().clone();
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget
                    .reserve_storage(
                        FLOOR
                            + receipt
                                .materialized
                                .unit_local_source_storage_floor_v1()
                                .unwrap(),
                    )
                    .unwrap();
                require_wave_roster(&original, width, kind);
                let bound = reduction_bound(&receipt, None, &mut budget);
                macro_rules! finish {
                    ($checked:expr, $storage:expr, $owner:ty, $admit:ident) => {{
                        let checked = $checked;
                        budget.reserve_storage(($storage)(&checked)).unwrap();
                        let floor = budget.storage();
                        let output = checked.owner().canonical().canonical_bytes().to_vec();
                        let owner = <$owner>::$admit(receipt, bound, checked, &mut budget).unwrap();
                        assert_eq!(owner.source_semantic_kir().module(), &original);
                        assert_eq!(owner.output().canonical().canonical_bytes(), output);
                        assert!(!owner.grants_artifact_or_launch_authority());
                        owner.verify_equivalence(&mut budget).unwrap();
                        require_wave_roster(owner.output().module(), width, kind);
                        let llvm = dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner.output()).unwrap();
                        assert!(!llvm.is_empty());
                        assert_eq!(budget.storage(), floor);
                    }};
                }
                match policy {
                    3 => finish!(
                        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1(
                            &bound,
                            &mut budget
                        )
                        .unwrap(),
                        |checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1| checked
                            .storage()
                            .retained_storage(),
                        crate::ProductionCheckedOutputOwnerPolicy3V1,
                        try_admit_general_v1
                    ),
                    4 => finish!(
                        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(
                            &bound,
                            &mut budget
                        )
                        .unwrap(),
                        |checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1| {
                            checked.retained_storage()
                        },
                        crate::ProductionCheckedOutputOwnerPolicy4V1,
                        try_admit_v1
                    ),
                    5 => finish!(
                        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(
                            &bound,
                            &mut budget
                        )
                        .unwrap(),
                        |checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy5V1| {
                            checked.retained_storage()
                        },
                        crate::ProductionCheckedOutputOwnerPolicy5V1,
                        try_admit_v1
                    ),
                    6 => {
                        let prefix =
                            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy5_v1(
                                &bound,
                                &mut budget,
                            )
                            .unwrap();
                        budget.reserve_storage(prefix.retained_storage()).unwrap();
                        finish!(
                            fe2o3_kernel_opt::continue_checked_canonical_kernel_ir_policy6_v1(
                                &bound,
                                prefix,
                                &mut budget
                            )
                            .unwrap(),
                            |checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1| {
                                checked.retained_storage()
                            },
                            crate::ProductionCheckedOutputOwnerPolicy6V1,
                            try_admit_v1
                        );
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}
