use super::*;

#[derive(Clone, Copy, Debug)]
enum Lane {
    Constant(u32),
    Masked(u32),
    Dynamic,
}

fn u32_constant(ty: SemanticTypeIdV1, bits: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits.into(), 4).unwrap()),
    ))
}

fn admitted(width: u32, lane: Lane) -> AdmittedInertSemanticMirV1 {
    let original = reduction_admission(width, SemanticSubgroupReductionKindV1::Sum).unwrap();
    let f = &original.functions()[0];
    let context = SemanticTypeIdV1::from_index(1);
    let context_ref = SemanticTypeIdV1::from_index(2);
    let float = SemanticTypeIdV1::from_index(3);
    let uint = SemanticTypeIdV1::from_index(4);
    let mut types = original.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(91)),
        SemanticLayoutIdentityV1::from_sha256(bytes(91)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    ));
    let mut locals = f.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(92)),
        uint,
        SemanticLocalRoleV1::Argument(0),
        f.source(),
    ));
    if matches!(lane, Lane::Masked(_)) {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(93)),
            uint,
            SemanticLocalRoleV1::Temporary,
            f.source(),
        ));
    }
    let abi = SemanticFunctionAbiV1::from_rustc(
        f.abi().identity(),
        f.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(direct_abi_value(uint))],
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap();
    let mut blocks = f.blocks().to_vec();
    assert_eq!(blocks.len(), 4);
    for index in [1usize, 2] {
        let b = &blocks[index];
        let SemanticTerminatorKindV1::Call(call) = b.terminator().kind() else {
            unreachable!()
        };
        let mut statements = b.statements().to_vec();
        if index == 1 {
            if let Lane::Masked(mask) = lane {
                statements.push(SemanticStatementV1::new(
                    f.source(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(5, uint),
                        SemanticRvalueV1::new(
                            uint,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::BitAnd,
                                left: value(4, uint),
                                right: u32_constant(uint, mask),
                            },
                        ),
                    )),
                ));
            }
        }
        let mut arguments = call.arguments().to_vec();
        assert_eq!(arguments.len(), 2);
        arguments.push(match lane {
            Lane::Constant(v) => u32_constant(uint, v),
            Lane::Masked(_) => value(5, uint),
            Lane::Dynamic => value(4, uint),
        });
        blocks[index] = SemanticBasicBlockV1::new(
            b.identity(),
            b.source(),
            statements,
            SemanticTerminatorV1::new(
                b.terminator().source(),
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable(
                        SemanticCallableIdV1::from_index(2),
                        arguments,
                        call.destination().cloned(),
                        call.unwind(),
                    )
                    .unwrap(),
                ),
            ),
        )
        .unwrap();
    }
    let function = SemanticFunctionDeclV1::new(
        f.identity(),
        f.role(),
        f.item_definition_identity(),
        f.monomorphization_identity(),
        f.generic_type_arguments_identity(),
        f.const_generic_arguments_identity(),
        f.source(),
        abi,
        locals,
        f.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(f.kernel_entry().unwrap().clone());
    let mut callables = original.callables().to_vec();
    callables[2] = compiler_intrinsic_callable(
        94,
        vec![
            shared_reference_abi_value(context_ref),
            direct_abi_value(float),
            direct_abi_value(uint),
        ],
        direct_abi_value(float),
        SemanticCompilerIntrinsicOperationV1::SubgroupBroadcastF32 { context, width },
    );
    InertSemanticMirRequestV1::new_with_callables(
        original.target(),
        types,
        original.allocations().to_vec(),
        original.statics().to_vec(),
        original.vtables().to_vec(),
        vec![function],
        callables,
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn receipt(width: u32, lane: Lane) -> ProductionMaterializedRankedModuleReceiptV1 {
    collective_receipt(admitted(width, lane))
}

fn roster(module: &fe2o3_kernel_ir::Module, width: u32) {
    let mut results = Vec::new();
    let mut values = Vec::new();
    for function in &module.functions {
        for block in &function.body.as_ref().unwrap().blocks {
            for op in &block.operations {
                if let OperationKind::Wave(wave) = &op.kind {
                    let WaveOperationKind::BroadcastF32 {
                        value, tile_width, ..
                    } = wave.kind
                    else {
                        panic!("not a Broadcast");
                    };
                    assert_eq!(tile_width, width);
                    assert_eq!(op.results.len(), 1);
                    assert_eq!(op.results[0].ty, fe2o3_kernel_ir::Type::F32);
                    values.push(value);
                    results.push(op.results[0].id);
                }
            }
        }
    }
    assert_eq!(results.len(), 2);
    assert_eq!(values[1], results[0]);
}

#[test]
fn wave_broadcast_census_genuine_source_all_tiles_bounded_lanes_and_four_policies() {
    for width in [1, 2, 4, 8, 16, 32, 64] {
        for lane in [Lane::Constant(width - 1), Lane::Masked(width - 1)] {
            for policy in 3..=6 {
                let receipt = receipt(width, lane);
                let original = receipt.materialized.executable().module().clone();
                roster(&original, width);
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
                macro_rules! finish {
                    ($checked:expr, $storage:expr, $owner:ty, $admit:ident) => {{
                        let checked = $checked; budget.reserve_storage(($storage)(&checked)).unwrap();
                        let floor = budget.storage(); let bytes = checked.owner().canonical().canonical_bytes().to_vec();
                        let owner = <$owner>::$admit(receipt, bound, checked, &mut budget).unwrap();
                        assert_eq!(owner.source_semantic_kir().module(), &original);
                        assert_eq!(owner.output().canonical().canonical_bytes(), bytes);
                        assert!(!owner.grants_artifact_or_launch_authority());
                        owner.verify_equivalence(&mut budget).unwrap(); roster(owner.output().module(), width);
                        let llvm = dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner.output()).unwrap();
                        assert!(llvm.contains("llvm.amdgcn.ds.bpermute")); assert_eq!(budget.storage(), floor);
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

#[test]
fn wave_broadcast_census_genuine_source_unbounded_lane_is_not_laundered() {
    for lane in [Lane::Dynamic, Lane::Constant(16), Lane::Masked(31)] {
        let semantic = ProductionSemanticMirOwnerV1::try_new(
            admitted(16, lane),
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa = ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        ssa.verify_replay().unwrap();
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
        let error = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                crate::ProductionPreRankedKirErrorV1::Lowering(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: Some(1),
                        statement: None,
                        detail: "gfx950 subgroup broadcast requires f32, a valid width, and a statically bounded source lane"
                    }
                )
            ),
            "{error:?}"
        );
        assert_eq!(budget.storage(), FLOOR);
    }
}

fn mutate(module: &mut fe2o3_kernel_ir::Module, fault: usize) {
    let mut count = 0;
    let mut first_value = None;
    for function in &mut module.functions {
        let body = function.body.as_mut().unwrap();
        let bounded_other_lane = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|op| {
                matches!(
                    op.kind,
                    OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(15))
                )
                .then(|| op.results[0].id)
            })
            .unwrap();
        for block in &mut body.blocks {
            for op in &mut block.operations {
                let OperationKind::Wave(wave) = &mut op.kind else {
                    continue;
                };
                let WaveOperationKind::BroadcastF32 {
                    value,
                    source_lane,
                    tile_width,
                } = &mut wave.kind
                else {
                    unreachable!()
                };
                if count == 0 {
                    first_value = Some(*value);
                    match fault {
                        0 | 2 | 3 => {}
                        1 => *tile_width = 32,
                        4 => {
                            assert_ne!(*source_lane, bounded_other_lane);
                            *source_lane = bounded_other_lane;
                        }
                        _ => unreachable!(),
                    }
                } else if fault == 3 {
                    assert_ne!(*value, first_value.unwrap());
                    *value = first_value.unwrap();
                }
                if count == 0 && fault == 2 {
                    wave.kind = WaveOperationKind::ReduceF32 {
                        value: *value,
                        tile_width: *tile_width,
                        kind: WaveF32ReductionKindV1::Sum,
                    };
                }
                count += 1;
            }
        }
    }
    assert_eq!(count, 2);
}

#[test]
fn wave_broadcast_census_source_bound_and_output_reject_both_operand_and_payload_substitutions() {
    for fault in 1..=4 {
        let receipt = receipt(16, Lane::Masked(15));
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
        let mut module = receipt.materialized.executable().module().clone();
        mutate(&mut module, fault);
        let binding = dialect_amdgcn::bind_production_target_v1(&module, Profile::Gfx950).unwrap();
        let (bound, storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(binding.module(), &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
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
        assert!(
            matches!(error, E::Coordinates(_)),
            "fault {fault}: {error:?}"
        );
        assert_eq!(budget.storage(), floor);
    }
    let receipt = receipt(16, Lane::Masked(15));
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
    let (input, storage) = CanonicalKirInventoryV1::derive(&bound, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let baseline = budget.storage();
    for fault in [0, 1, 2, 3, 4, 0] {
        let mut module = checked.owner().module().clone();
        mutate(&mut module, fault);
        let (owner, owned) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        let (output, stored) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
        budget.reserve_storage(stored.retained_storage()).unwrap();
        let floor = budget.storage();
        assert_eq!(
            output.uses().len() + usize::from(fault == 2),
            checked.occurrences().candidate().uses.len(),
            "only the Broadcast-to-Reduce mutation removes an operand"
        );
        let result = fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
            &input,
            &output,
            checked.occurrences().candidate(),
            &mut budget,
        );
        if fault == 0 {
            let (view, retained) = result.unwrap();
            budget.reserve_storage(retained.retained_storage()).unwrap();
            drop(view);
            budget.release_storage(retained.retained_storage()).unwrap();
        } else if fault == 2 {
            assert!(
                matches!(
                    result,
                    Err(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::IncompleteRows)
                ),
                "fault {fault}: {:?}",
                result.as_ref().err()
            );
        } else if fault == 1 {
            assert!(
                matches!(
                    result,
                    Err(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
                        "retained operation payload"
                    ))
                ),
                "fault {fault}: {:?}",
                result.as_ref().err()
            );
        } else {
            assert!(
                matches!(
                    result,
                    Err(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
                        "final operand has no exact descendant"
                    ))
                ),
                "fault {fault}: {:?}",
                result.as_ref().err()
            );
        }
        assert_eq!(budget.storage(), floor);
        drop(output);
        budget.release_storage(stored.retained_storage()).unwrap();
        drop(owner);
        budget.release_storage(owned.retained_storage()).unwrap();
        assert_eq!(budget.storage(), baseline);
    }
}

fn admit_limits(
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
    let receipt = receipt(16, Lane::Masked(15));
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
fn wave_broadcast_census_full_source_exact_and_one_short_keep_typed_cause_and_floor() {
    let (result, work, peak, floor, failed_work, failed_storage) = admit_limits(WORK, STORAGE);
    drop(result.unwrap());
    assert!(work > 0 && peak > floor);
    assert_eq!((failed_work, failed_storage), (None, None));
    for (work_limit, storage_limit, success) in [
        (work, peak, true),
        (work - 1, peak, false),
        (work, peak - 1, false),
    ] {
        let (result, accepted, actual_peak, same_floor, failed_work, failed_storage) =
            admit_limits(work_limit, storage_limit);
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
                cause = cause
                    .source()
                    .unwrap_or_else(|| panic!("missing typed resource cause: {error:?}"));
            };
            if work_limit < work {
                assert!(
                    matches!(resource, AssertOriginResourceV1::Work(e) if e.actual() == work && e.limit() == work_limit)
                );
                assert_eq!((failed_work, failed_storage), (Some(work), None));
            } else {
                assert!(
                    matches!(resource, AssertOriginResourceV1::Storage(e) if e.actual() == peak && e.limit() == storage_limit)
                );
                assert_eq!((failed_work, failed_storage), (None, Some(peak)));
            }
        }
        assert!(accepted <= work_limit && actual_peak <= storage_limit);
    }
}
