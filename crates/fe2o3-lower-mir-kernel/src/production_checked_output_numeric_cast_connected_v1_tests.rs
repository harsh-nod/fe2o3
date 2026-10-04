use super::*;

// Rebuild genuine admitted semantic MIR, its SSA owner, materialized N and exact
// ranked receipt. The existing F32 fixture supplies only ordinary construction
// inputs; its old owner/receipt is never relabeled as the changed source.
fn cast_receipt(
    signed: bool,
    bits: u16,
    helper: bool,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    let base = fp_receipt(32, FpRecipe::Negate, helper);
    let source = base.materialized.semantic_ssa.source_semantic();
    let integer = SemanticTypeIdV1::from_index(4);
    let mut types = source.types().to_vec();
    let bytes = u64::from(bits / 8);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([5; 32]),
        SemanticLayoutIdentityV1::from_sha256([5; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, bits, bytes),
                SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
    ));
    let mut functions = source.functions().to_vec();
    let old = &functions[usize::from(helper)];
    let mut locals = old.locals().to_vec();
    let temporary = locals.len() as u32;
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([240; 32]),
        integer,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let mut blocks = old.blocks().to_vec();
    let old_block = &blocks[0];
    let mut statements = old_block.statements().to_vec();
    let assignment_index = usize::from(!helper);
    statements.splice(
        assignment_index..=assignment_index,
        [
            assignment(
                temporary,
                integer,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: value(if helper { 1 } else { 2 }, FP_VALUE),
                },
            ),
            assignment(
                if helper { 0 } else { 5 },
                FP_VALUE,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Float,
                    operand: value(temporary, integer),
                },
            ),
        ],
    );
    blocks[0] = SemanticBasicBlockV1::new(
        old_block.identity(),
        old_block.source(),
        statements,
        old_block.terminator().clone(),
    )
    .unwrap();
    let mut function = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        locals,
        old.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = old.kernel_entry() {
        function = function.with_kernel_entry(entry.clone());
    }
    functions[usize::from(helper)] = function;
    let request = InertSemanticMirRequestV1::new(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        source.roots().to_vec(),
    )
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "logical_0",
            [30; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let materialized = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let mut roots = base.roots.into_vec();
    if !helper {
        roots[0].access_sources[0].semantic_statement = Some(3);
    }
    ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        materialized,
        roots,
    )
    .unwrap()
}

fn prepared(
    signed: bool,
    bits: u16,
    helper: bool,
    profile: Profile,
    mutation: Option<fn(&mut Module)>,
) -> FpPrepared4 {
    fp_prepare_receipt4(cast_receipt(signed, bits, helper), profile, mutation)
}

fn cast_census(module: &Module, helper: bool) {
    let mut casts = 0;
    let mut calls = 0;
    for function in &module.functions {
        for op in function
            .body
            .iter()
            .flat_map(|b| &b.blocks)
            .flat_map(|b| &b.operations)
        {
            if matches!(
                op.kind,
                OperationKind::Cast {
                    kind: fe2o3_kernel_ir::CastKind::IntegerToFloat
                        | fe2o3_kernel_ir::CastKind::FloatToInteger,
                    ..
                }
            ) {
                casts += 1;
                assert_eq!(function.role == FunctionRole::InternalHelper, helper);
            }
            if matches!(op.kind, OperationKind::Call { .. }) {
                calls += 1;
            }
        }
    }
    assert_eq!((casts, calls), (2, usize::from(helper)));
}

#[test]
fn genuine_cast_source_replays_both_profiles_and_retains_exact_actual_o_native_casts() {
    for signed in [false, true] {
        for bits in [8, 16, 32, 64] {
            for helper in [false, true] {
                for profile in [Profile::Gfx942, Profile::Gfx950] {
                    let input = prepared(signed, bits, helper, profile, None);
                    cast_census(input.bound.module(), helper);
                    cast_census(
                        input.checked.intermediate_policy3().owner().module(),
                        helper,
                    );
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                    budget.reserve_storage(input.floor).unwrap();
                    let owner = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                        input.receipt,
                        input.bound,
                        input.checked,
                        &mut budget,
                    )
                    .unwrap();
                    cast_census(owner.output().module(), helper);
                    owner.verify_equivalence(&mut budget).unwrap();
                    assert_eq!(owner.kernels()[0].accesses().len(), 1);
                    assert_eq!(budget.storage(), input.floor);
                    let text = match profile {
                        Profile::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner.output()),
                        Profile::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner.output()),
                    }.unwrap();
                    let text =
                        dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&text).unwrap();
                    let sign = if signed { "s" } else { "u" };
                    assert!(text.contains(&format!("@llvm.fpto{sign}i.sat.i{bits}.f32(float ")));
                    assert!(text.contains(&format!(" = {sign}itofp i{bits} ")));
                    assert!(!text.contains(" fptosi ") && !text.contains(" fptoui "));
                }
            }
        }
    }
}

fn mutate_type(module: &mut Module, replacement: ScalarType) {
    for body in module.functions.iter_mut().filter_map(|f| f.body.as_mut()) {
        for op in body.blocks.iter_mut().flat_map(|b| &mut b.operations) {
            if let OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::FloatToInteger,
                to,
                ..
            } = &mut op.kind
            {
                *to = Type::Scalar(replacement);
                op.results[0].ty = to.clone();
                return;
            }
        }
    }
    panic!("actual conversion missing");
}

fn mutate_sign(module: &mut Module) {
    mutate_type(module, ScalarType::I32);
}
fn mutate_width(module: &mut Module) {
    mutate_type(module, ScalarType::U16);
}
fn mutate_kind(module: &mut Module) {
    for op in module
        .functions
        .iter_mut()
        .filter_map(|f| f.body.as_mut())
        .flat_map(|b| &mut b.blocks)
        .flat_map(|b| &mut b.operations)
    {
        if let OperationKind::Cast { kind, .. } = &mut op.kind
            && *kind == fe2o3_kernel_ir::CastKind::FloatToInteger
        {
            *kind = fe2o3_kernel_ir::CastKind::Bitcast;
            return;
        }
    }
    panic!("actual conversion missing");
}
fn mutate_operand(module: &mut Module) {
    for body in module.functions.iter_mut().filter_map(|f| f.body.as_mut()) {
        let replacement = *body.parameters.last().unwrap();
        for op in body.blocks.iter_mut().flat_map(|b| &mut b.operations) {
            if let OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::FloatToInteger,
                value,
                ..
            } = &mut op.kind
            {
                assert_ne!(*value, replacement);
                *value = replacement;
                return;
            }
        }
    }
    panic!("actual conversion missing");
}

#[test]
fn changed_cast_sign_width_kind_operand_and_unrelated_history_fail_exact_source_join() {
    for helper in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for mutation in [
                mutate_sign as fn(&mut Module),
                mutate_width,
                mutate_kind,
                mutate_operand,
            ] {
                let input = prepared(false, 32, helper, profile, Some(mutation));
                let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
                let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
                budget.reserve_storage(input.floor).unwrap();
                let result = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                    input.receipt,
                    input.bound,
                    input.checked,
                    &mut budget,
                );
                assert!(
                    matches!(
                        result,
                        Err(
                            crate::ProductionCheckedOutputAdmissionErrorPolicy4V1::Admission(
                                AdmissionError::Coordinates(_)
                            )
                        )
                    ),
                    "{result:?}"
                );
                assert_eq!(budget.storage(), input.floor);
            }
            let input = prepared(false, 32, helper, profile, None);
            let other = prepared(true, 32, helper, profile, None);
            let floor = input.floor + other.floor;
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            assert!(
                crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                    input.receipt,
                    input.bound,
                    other.checked,
                    &mut budget
                )
                .is_err()
            );
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn exact_numeric_cast_owner_work_storage_and_one_short_unwind() {
    for helper in [false, true] {
        let input = prepared(false, 32, helper, Profile::Gfx942, None);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(input.floor).unwrap();
        drop(
            crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget,
            )
            .unwrap(),
        );
        let exact_work = budget.work();
        let exact_storage = budget.peak_storage();
        assert_eq!(budget.storage(), input.floor);
        for (work_limit, storage_limit, success) in [
            (exact_work, exact_storage, true),
            (exact_work - 1, exact_storage, false),
            (exact_work, exact_storage - 1, false),
        ] {
            let input = prepared(false, 32, helper, Profile::Gfx942, None);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(input.floor).unwrap();
            let result = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget,
            );
            assert_eq!(result.is_ok(), success, "{result:?}");
            assert_eq!(budget.storage(), input.floor);
            if success {
                assert_eq!(budget.work(), exact_work);
            } else if work_limit < exact_work {
                assert!(work.failed_work().is_some());
            } else {
                assert!(budget.failed_storage().is_some());
            }
        }
    }
}
