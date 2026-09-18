//! Private custody fixtures, not authenticated Rust providers or executable proofs.
use super::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn unit_return() -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(0, UNIT),
            SemanticRvalueV1::new(
                UNIT,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ),
        )),
    )
}

fn abi(
    tag: u8,
    kernel: bool,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
) -> SemanticFunctionAbiV1 {
    let value = |ty| {
        SemanticAbiValueV1::new(
            ty,
            if ty == U32 {
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                        SemanticAbiExtensionV1::None,
                        0,
                        None,
                    )
                    .unwrap(),
                )
            } else {
                SemanticAbiPassModeV1::Ignore
            },
        )
    };
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if kernel {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if kernel {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        inputs.len() as u32,
        inputs
            .iter()
            .copied()
            .map(|ty| SemanticAbiArgumentV1::source(value(ty)))
            .collect(),
        value(output),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue;
        inputs.len()
    ])
    .unwrap()
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}

fn call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: u32,
    ty: SemanticTypeIdV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(destination, ty),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(target),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    Arguments,
    ArgumentKind,
    Issuer,
    ExtraIssue,
    HelperIdentity,
    ContextIdentity,
}

fn fixture(mutation: Mutation) -> AdmittedInertSemanticMirV1 {
    let aggregate_layout = |fields| {
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(vec![0; fields], vec![]).unwrap(),
        )
        .unwrap()
    };
    let declaration = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            layout,
            shape,
        )
    };
    let types = vec![
        declaration(
            1,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        declaration(
            2,
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
        ),
        declaration(
            3,
            aggregate_layout(0),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ),
        declaration(
            if matches!(mutation, Mutation::ContextIdentity) {
                5
            } else {
                4
            },
            aggregate_layout(5),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![MARKER; 5]).unwrap()),
        )
        .with_rust_type_kind(SemanticRustTypeKindV1::Execution(
            SemanticExecutionRoleV29::KernelContext,
        )),
    ];
    let function = |tag,
                    role,
                    signature: SemanticFunctionAbiV1,
                    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
                    blocks| {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            role,
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            signature,
            locals
                .iter()
                .enumerate()
                .map(|(index, (ty, role))| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([index as u8 + 1; 32]),
                        *ty,
                        *role,
                        SemanticSourceProvenanceV1::unavailable(),
                    )
                })
                .collect(),
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let mut arguments = vec![
        SemanticOperandV1::Move(place(3, CONTEXT)),
        SemanticOperandV1::Copy(place(1, U32)),
        SemanticOperandV1::Copy(place(2, U32)),
    ];
    if matches!(mutation, Mutation::Arguments) {
        arguments.swap(1, 2);
    }
    if matches!(mutation, Mutation::ArgumentKind) {
        arguments[1] = SemanticOperandV1::Move(place(1, U32));
    }
    let mut blocks = vec![
        block(10, vec![], call(2, vec![], 3, CONTEXT, 1)),
        block(11, vec![], call(1, arguments, 4, UNIT, 2)),
        block(12, vec![unit_return()], SemanticTerminatorKindV1::Return),
    ];
    if matches!(mutation, Mutation::ExtraIssue) {
        blocks[2] = block(12, vec![], call(2, vec![], 3, CONTEXT, 3));
        blocks.push(block(
            13,
            vec![unit_return()],
            SemanticTerminatorKindV1::Return,
        ));
    }
    let root = function(
        40,
        SemanticFunctionRoleV1::KernelRoot,
        abi(40, true, &[U32, U32], UNIT),
        &[
            (UNIT, SemanticLocalRoleV1::Return),
            (U32, SemanticLocalRoleV1::Argument(0)),
            (U32, SemanticLocalRoleV1::Argument(1)),
            (CONTEXT, SemanticLocalRoleV1::Temporary),
            (UNIT, SemanticLocalRoleV1::Temporary),
        ],
        blocks,
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"custody_root".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([60; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let helper = function(
        if matches!(mutation, Mutation::HelperIdentity) {
            51
        } else {
            50
        },
        SemanticFunctionRoleV1::InternalHelper,
        abi(50, false, &[CONTEXT, U32, U32], UNIT),
        &[
            (UNIT, SemanticLocalRoleV1::Return),
            (CONTEXT, SemanticLocalRoleV1::Argument(0)),
            (U32, SemanticLocalRoleV1::Argument(1)),
            (U32, SemanticLocalRoleV1::Argument(2)),
        ],
        vec![block(
            20,
            vec![unit_return()],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let mut callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
    ];
    let tag = if matches!(mutation, Mutation::Issuer) {
        71
    } else {
        70
    };
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi(tag, false, &[], CONTEXT),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Execution(
            SemanticExecutionOperationV29::ContextIssue { context: CONTEXT },
        ),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    });
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap()
}

fn completed() -> CompletedContextEntryV29 {
    let boundary = |block, destination, target| CallBoundaryV29 {
        block: SemanticBlockIdV1::from_index(block),
        statements: 0,
        destination: SemanticLocalIdV1::from_index(destination),
        destination_type: if destination == 3 { CONTEXT } else { UNIT },
        target: SemanticBlockIdV1::from_index(target),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    CompletedContextEntryV29 {
        function: SemanticFunctionIdV1::from_index(0),
        helper: SemanticFunctionIdV1::from_index(1),
        issuer: SemanticCallableIdV1::from_index(2),
        issuance: boundary(0, 3, 1),
        helper_call: boundary(1, 4, 2),
        helper_argument: SemanticLocalIdV1::from_index(3),
        arguments: vec![
            SemanticOperandV1::Move(place(3, CONTEXT)),
            SemanticOperandV1::Copy(place(1, U32)),
            SemanticOperandV1::Copy(place(2, U32)),
        ],
        root_identity: SemanticFunctionIdentityV1::from_sha256([40; 32]),
        helper_identity: SemanticFunctionIdentityV1::from_sha256([50; 32]),
        issuer_identity: SemanticFunctionIdentityV1::from_sha256([70; 32]),
        context_identity: SemanticTypeIdentityV1::from_sha256([4; 32]),
        _source_commitment: [90; 32],
    }
}

fn retained() -> RetainedContextEntryV29 {
    completed()
        .bind_function(&fixture(Mutation::None).functions()[0], |_| Ok(()))
        .unwrap()
}

#[test]
fn retained_context_custody_survives_the_existing_ssa_owner() {
    let semantic = fixture(Mutation::None);
    let retained =
        RetainedContextEntriesV29::seal(vec![retained()], &semantic, |_| Ok(())).unwrap();
    retained.validate_source(&semantic).unwrap();
    let owner = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    retained.validate_source(ssa.source_semantic()).unwrap();
    assert!(
        retained
            .validate_source(&fixture(Mutation::Arguments))
            .is_err()
    );
}

#[test]
fn retained_context_seal_checks_final_operands_identities_and_complete_roster() {
    assert!(
        completed()
            .bind_function(&fixture(Mutation::Arguments).functions()[0], |_| Ok(()))
            .is_err(),
        "same-typed argument substitution before receipt attachment"
    );
    let mut wrong_issuer = completed();
    wrong_issuer.issuer = SemanticCallableIdV1::from_index(1);
    assert!(
        wrong_issuer
            .bind_function(&fixture(Mutation::None).functions()[0], |_| Ok(()))
            .is_err()
    );
    for mutation in [
        Mutation::Arguments,
        Mutation::ArgumentKind,
        Mutation::Issuer,
        Mutation::ExtraIssue,
        Mutation::HelperIdentity,
        Mutation::ContextIdentity,
    ] {
        if matches!(mutation, Mutation::Arguments | Mutation::ArgumentKind) {
            assert!(
                completed()
                    .bind_function(&fixture(mutation).functions()[0], |_| Ok(()))
                    .is_err()
            );
        }
        assert!(
            RetainedContextEntriesV29::seal(vec![retained()], &fixture(mutation), |_| Ok(()))
                .is_err()
        );
    }
    let semantic = fixture(Mutation::None);
    assert!(RetainedContextEntriesV29::seal(vec![], &semantic, |_| Ok(())).is_err());
    assert!(
        RetainedContextEntriesV29::seal(vec![retained(), retained()], &semantic, |_| Ok(()))
            .is_err()
    );
    let mut wrong_root = retained();
    wrong_root.source.function = SemanticFunctionIdV1::from_index(1);
    assert!(RetainedContextEntriesV29::seal(vec![wrong_root], &semantic, |_| Ok(())).is_err());
}

#[test]
fn retained_context_seal_uses_exact_cumulative_work_budget() {
    let semantic = fixture(Mutation::None);
    let mut used = 0;
    RetainedContextEntriesV29::seal(vec![retained()], &semantic, |amount| {
        used += amount;
        Ok(())
    })
    .unwrap();
    for limit in [used - 1, used] {
        let mut remaining = limit;
        let result = RetainedContextEntriesV29::seal(vec![retained()], &semantic, |amount| {
            remaining = remaining.checked_sub(amount).ok_or_else(mismatch)?;
            Ok(())
        });
        assert_eq!(result.is_ok(), limit == used);
    }
}

#[test]
fn completed_context_rechecks_typed_call_boundaries_before_retention() {
    let semantic = fixture(Mutation::None);
    for mutation in 0..11 {
        let corrupt = |entry: &mut CompletedContextEntryV29| {
            if mutation == 10 {
                entry.helper_argument = SemanticLocalIdV1::from_index(2);
                return;
            }
            let call = if mutation < 5 {
                &mut entry.issuance
            } else {
                &mut entry.helper_call
            };
            match mutation % 5 {
                0 => call.destination_type = U32,
                1 => call.destination = SemanticLocalIdV1::from_index(0),
                2 => call.target = SemanticBlockIdV1::from_index(0),
                3 => call.statements = 1,
                4 => call.unwind = SemanticUnwindActionV1::Continue,
                _ => unreachable!(),
            }
        };
        let mut entry = completed();
        corrupt(&mut entry);
        assert!(
            matches!(
                entry.bind_function(&semantic.functions()[0], |_| Ok(())),
                Err(Error::IdentityTableMismatch {
                    table: "retained context entry custody"
                })
            ),
            "boundary {mutation}"
        );
        let mut entry = retained();
        corrupt(&mut entry.source);
        assert!(
            matches!(
                RetainedContextEntriesV29::seal(vec![entry], &semantic, |_| Ok(())),
                Err(Error::IdentityTableMismatch {
                    table: "retained context entry custody"
                })
            ),
            "sealed boundary {mutation}"
        );
    }
}
