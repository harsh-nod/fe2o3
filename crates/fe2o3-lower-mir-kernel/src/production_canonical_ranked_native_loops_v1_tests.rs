use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    CanonicalRankedPolicyFailureV1, PlironProgressFindingV1, ProductionPlironPreloweringErrorV2,
};

const LOOP_UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const LOOP_SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const LOOP_BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

fn loop_place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn loop_value(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(loop_place(local, ty))
}

fn loop_assignment(
    local: u32,
    ty: SemanticTypeIdV1,
    kind: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            loop_place(local, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}

fn loop_edge(role: SemanticEdgeRoleV1, block: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
}

fn loop_switch(condition: SemanticOperandV1, yes: u32, no: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: condition,
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                loop_edge(SemanticEdgeRoleV1::SwitchValue, yes),
            )],
            loop_edge(SemanticEdgeRoleV1::SwitchOtherwise, no),
        )
        .unwrap(),
    }
}

fn loop_block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, terminator),
    )
    .unwrap()
}

fn loop_scalar_type(tag: u8, width: u16, signed: bool, boolean: bool) -> SemanticTypeDeclV1 {
    let bytes = u64::from(width / 8);
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(signed, width, bytes),
                SemanticScalarValidityRangeV1::new(
                    0,
                    if boolean { 1 } else { (1_u128 << width) - 1 },
                ),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(if boolean {
            SemanticScalarTypeV1::Bool
        } else {
            SemanticScalarTypeV1::Integer {
                bits: width,
                signed,
            }
        }),
    )
}

// Fresh semantic-source model requests, not rustc extraction. The seed supplies
// only real root identity/unit layout; admission, SSA and owner materialization
// remain unchanged and no correspondence, effects or assertion facts are edited.
fn source_loop(
    width: u16,
    signed: bool,
    initial: i64,
    unknown_entry: bool,
) -> ProductionPreRankedKirOwnerV1 {
    let seed = noop_semantic_owner(&["source_native_loop"]);
    let semantic = seed.semantic();
    let root = &semantic.functions()[0];
    let source = SemanticSourceProvenanceV1::unavailable();
    let parameter = if unknown_entry {
        LOOP_BOOL
    } else {
        LOOP_SCALAR
    };
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        if unknown_entry {
            SemanticAbiExtensionV1::ZeroExtend
        } else if width < 32 {
            if signed {
                SemanticAbiExtensionV1::SignExtend
            } else {
                SemanticAbiExtensionV1::ZeroExtend
            }
        } else {
            SemanticAbiExtensionV1::None
        },
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([221; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            parameter,
            SemanticAbiPassModeV1::Direct(attributes),
        ))],
        SemanticAbiValueV1::new(LOOP_UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let constant = |value: i64| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            LOOP_SCALAR,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(
                    (value as u128) & ((1_u128 << width) - 1),
                    u8::try_from(width / 8).unwrap(),
                )
                .unwrap(),
            ),
        ))
    };
    let initialize = loop_assignment(2, LOOP_SCALAR, SemanticRvalueKindV1::Use(constant(initial)));
    let blocks = if unknown_entry {
        vec![
            loop_block(
                230,
                vec![initialize],
                loop_switch(loop_value(1, LOOP_BOOL), 1, 2),
            ),
            loop_block(
                231,
                vec![],
                SemanticTerminatorKindV1::Goto(loop_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            loop_block(232, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![
            loop_block(
                230,
                vec![initialize],
                SemanticTerminatorKindV1::Goto(loop_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            loop_block(
                231,
                vec![loop_assignment(
                    3,
                    LOOP_BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: loop_value(2, LOOP_SCALAR),
                        right: loop_value(1, LOOP_SCALAR),
                    },
                )],
                loop_switch(loop_value(3, LOOP_BOOL), 2, 3),
            ),
            loop_block(
                232,
                vec![loop_assignment(
                    2,
                    LOOP_SCALAR,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: loop_value(2, LOOP_SCALAR),
                        right: constant(1),
                    },
                )],
                SemanticTerminatorKindV1::Goto(loop_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            loop_block(233, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    let function = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        source,
        abi,
        [
            (LOOP_UNIT, SemanticLocalRoleV1::Return),
            (parameter, SemanticLocalRoleV1::Argument(0)),
            (LOOP_SCALAR, SemanticLocalRoleV1::Temporary),
            (LOOP_BOOL, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([240 + index as u8; 32]),
                ty,
                role,
                source,
            )
        })
        .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        vec![
            semantic.types()[0].clone(),
            loop_scalar_type(222, width, signed, false),
            loop_scalar_type(223, 8, false, true),
        ],
        vec![],
        vec![],
        vec![],
        vec![function],
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    assert_eq!(
        ssa.summary().reachable_blocks(),
        if unknown_entry { 3 } else { 4 }
    );
    assert_eq!(ssa.summary().pruned_blocks(), 0);
    let owner = cr_owner_from_ssa(ssa);
    let module = owner.executable().module();
    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    let body = module.functions[0].body.as_ref().unwrap();
    assert_eq!(body.parameters.len(), 1);
    assert_eq!(body.blocks.len(), if unknown_entry { 3 } else { 4 });
    assert_eq!(
        module.functions[0].signature.parameters,
        vec![if unknown_entry {
            Type::BOOL
        } else {
            Type::Scalar(match (width, signed) {
                (8, false) => ScalarType::U8,
                (32, false) => ScalarType::U32,
                (32, true) => ScalarType::I32,
                _ => panic!("outside explicit test roster"),
            })
        }]
    );
    assert!(body.blocks.iter().any(|block| !block.operations.is_empty()));
    if !unknown_entry {
        let seed = match (width, signed) {
            (8, false) => Constant::U8(u8::try_from(initial).unwrap()),
            (32, false) => Constant::U32(u32::try_from(initial).unwrap()),
            (32, true) => Constant::I32(i32::try_from(initial).unwrap()),
            _ => unreachable!(),
        };
        assert!(body.blocks.iter().flat_map(|block| &block.operations).any(
            |operation| matches!(&operation.kind, OperationKind::Constant(value) if *value == seed)
        ));
        assert!(body.blocks.iter().flat_map(|block| &block.operations).any(|operation|
            matches!(operation.kind, OperationKind::Compare { rhs, .. } if rhs == body.parameters[0])));
        assert_eq!(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(operation.kind, OperationKind::Compare { .. }))
                .count(),
            1
        );
        assert_eq!(
            body.blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| matches!(
                    operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                        ..
                    }
                ))
                .count(),
            1
        );
    }
    owner
}

fn check_source_loop(owner: &ProductionPreRankedKirOwnerV1, unknown_entry: bool) {
    let before = owner.executable().canonical().canonical_bytes().to_vec();
    let roots = owner.executable().module().kernels.clone();
    let names = owner
        .executable()
        .module()
        .functions
        .iter()
        .map(|function| function.id.clone())
        .collect::<Vec<_>>();
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let mut checked_entered = false;
    let mut policies_entered = false;
    let result =
        owner
            .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
                checked_entered = true;
                let metadata = view.metadata(budget)?;
                assert!(std::ptr::eq(
                    metadata.inventory(budget)?.owner(),
                    owner.executable()
                ));
                assert_eq!(metadata.arguments(budget)?.len(), 1);
                assert!(metadata.arguments(budget)?.iter().all(
                    |row| row.source_ownership() == SemanticSourceArgumentOwnershipV1::ByValue
                ));
                assert!(metadata.assertions(budget)?.is_empty());
                assert!(metadata.effects(budget)?.is_empty());
                assert!(metadata.catalog(budget)?.definitions().is_empty());
                assert!(metadata.catalog(budget)?.bindings().is_empty());
                Ok(view.with_policy_checks_v1(budget, |policies, budget| {
                policies_entered = true;
                assert!(
                    !unknown_entry,
                    "unknown-entry cycle must not obtain a policy callback"
                );
                let metadata = policies.metadata(budget)?;
                assert!(std::ptr::eq(
                    metadata.inventory(budget)?.owner(),
                    owner.executable()
                ));
                assert_eq!(metadata.arguments(budget)?.len(), 1);
                let reports = policies.policies(budget)?;
                let exact = reports.owner(budget)?;
                assert!(std::ptr::eq(exact, owner.executable()));
                assert_eq!(exact.canonical().canonical_bytes(), before);
                assert_eq!(exact.module().kernels, roots);
                assert!(
                    exact
                        .module()
                        .functions
                        .iter()
                        .map(|function| &function.id)
                        .eq(names.iter())
                );
                assert_eq!(reports.function_count(budget)?, 1);
                let report = reports.report(0, budget)?;
                assert_eq!(
                    report.pass_order(),
                    &fe2o3_pliron::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
                );
                assert!(report.is_clean());
                let [certificate] = report.semantics().progress().certificates() else {
                    panic!(
                        "genuine emitted source loop must have one nonempty progress certificate"
                    );
                };
                assert_eq!(certificate.step(), 1);
                assert_ne!(certificate.header(), certificate.exit());
                assert_ne!(certificate.body(), certificate.exit());
                assert!(!certificate.induction().is_empty());
                assert!(!certificate.bound().is_empty());
                assert!(!report.semantics().progress().grants_launch_or_liveness_authority());
                assert_eq!(reports.history(0, budget)?.function(), 0);
                assert!(reports.observation(budget)?.work_upper_bound() > 0);
                assert_eq!(reports.pending_obligations().iter().count(), 19);
                assert!(!policies.ranked_verification_is_complete());
                assert!(!policies.grants_artifact_or_launch_authority());
                Ok(())
            }))
            })
            .unwrap();
    assert!(checked_entered);
    if unknown_entry {
        assert!(!policies_entered);
        let error = result.unwrap_err();
        let ProductionCanonicalRankedPolicyErrorV1::Policy(error) = error else {
            panic!("expected real policy progress refusal, not a fabricated success: {error:?}");
        };
        let CanonicalRankedPolicyFailureV1::Analysis {
            function: 0,
            cause: ProductionPlironPreloweringErrorV2::Semantic(cause),
        } = error.failure()
        else {
            panic!("expected actual progress analysis: {error:?}");
        };
        assert!(
            cause
                .report()
                .progress()
                .findings()
                .iter()
                .any(|finding| matches!(
                    finding,
                    PlironProgressFindingV1::ProgressIncomplete { .. }
                ))
        );
        assert!(
            !cause
                .report()
                .progress()
                .findings()
                .iter()
                .any(|finding| matches!(
                    finding,
                    PlironProgressFindingV1::NonTerminatingCycle { .. }
                ))
        );
        assert!(error.observation().work_upper_bound() > 0);
        assert_eq!(error.last_invocation().unwrap().function(), 0);
    } else {
        result.unwrap();
        assert!(policies_entered);
    }
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(owner.executable().canonical().canonical_bytes(), before);
    assert_eq!(owner.executable().module().kernels, roots);
    assert!(
        owner
            .executable()
            .module()
            .functions
            .iter()
            .map(|function| &function.id)
            .eq(names.iter())
    );
}

#[test]
fn source_canonical_native_dynamic_finite_loop_reaches_real_policy_callback() {
    check_source_loop(&source_loop(32, false, 0, false), false);
}

#[test]
fn source_canonical_native_nonzero_seed_loop_keeps_source_argument_and_certificate() {
    check_source_loop(&source_loop(32, false, 7, false), false);
}

#[test]
fn source_canonical_native_signed_and_narrow_loops_keep_typed_certificates() {
    for (width, signed, initial) in [(32, true, -3), (8, false, 2)] {
        check_source_loop(&source_loop(width, signed, initial, false), false);
    }
}

#[test]
fn source_canonical_native_unknown_entry_cycle_is_real_progress_incomplete() {
    check_source_loop(&source_loop(32, false, 7, true), true);
}
