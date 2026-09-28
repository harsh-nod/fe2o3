use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

#[path = "production_source_reference_abi_v29_tests.rs"]
mod abi_tests;
#[path = "production_source_reference_fixture_closure_v29_tests.rs"]
mod fixture_closure;

#[path = "production_source_reference_validation_scratch_v29_tests.rs"]
mod validation_scratch;

// Retained semantic-source component fixtures copied from the frozen A owner
// fixtures. These tests do not replace the genuine rustc source acceptance gates.

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const CAPTURE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Case {
    Shared,
    UniqueRead,
    Writeback,
    DeadReferent,
    Conflict,
    Reborrow,
    SuspendedParent,
    Address,
}

impl Case {
    fn mutable(self) -> bool {
        self != Self::Shared && self != Self::Address
    }
}

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn projected(
    local: u32,
    projections: &[(SemanticProjectionKindV1, SemanticTypeIdV1)],
) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        projections
            .iter()
            .map(|&(kind, ty)| SemanticProjectionV1::new(kind, ty).unwrap())
            .collect(),
        projections.last().unwrap().1,
    )
    .unwrap()
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(source(), kind)
}
fn assign(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let ty = destination.ty();
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        destination,
        SemanticRvalueV1::new(ty, value),
    )))
}
fn unit() -> SemanticStatementV1 {
    assign(
        place(0, UNIT),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ))),
    )
}
fn dead(local: u32) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::StorageDead(
        SemanticLocalIdV1::from_index(local),
    ))
}
fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([tag; 32]),
        ty,
        role,
        source(),
    )
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), kind),
    )
    .unwrap()
}
fn call(callee: u32, argument: SemanticOperandV1, target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            vec![argument],
            Some(SemanticCallDestinationV1::new(
                place(0, UNIT),
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
fn abi(tag: u8, kernel: bool, input: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    let mode = if input == CAPTURE {
        SemanticAbiPassModeV1::cast(
            false,
            SemanticAbiCastV1::new(
                [None; 8],
                None,
                SemanticAbiUniformV1::new(
                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                    8,
                )
                .unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        )
    } else {
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    false,
                    None,
                    false,
                    false,
                    false,
                    input == WORD,
                ),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
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
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            input, mode,
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap()
}
fn function(
    tag: u8,
    kernel: bool,
    input: SemanticTypeIdV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if kernel {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        source(),
        abi(tag, kernel, input),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

// Structural source/SSA component fixtures, not rustc provenance or allocation
// authority. The unchanged managed 24 remain the real source acceptance gate.
fn owner(case: Case) -> ProductionSemanticSsaOwnerV1 {
    owner_with(case, |_, _| {})
}

fn later_addressable_instance_owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::UniqueRead, |_, functions| {
        let mut statements = vec![
            assign(
                place(2, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
            ),
            assign(
                place(3, WORD),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
            ),
        ];
        for (referent, holder, capture) in [(2, 4, 6), (3, 5, 7)] {
            statements.extend([
                assign(
                    place(holder, REFERENCE),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: place(referent, WORD),
                    },
                ),
                assign(
                    place(capture, CAPTURE),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Tuple,
                            vec![SemanticOperandV1::Move(place(holder, REFERENCE))],
                        )
                        .unwrap(),
                    ),
                ),
                dead(holder),
            ]);
        }
        let entry = functions[0].kernel_entry().unwrap().clone();
        functions[0] = function(
            10,
            true,
            WORD,
            vec![
                local(10, UNIT, SemanticLocalRoleV1::Return),
                local(11, WORD, SemanticLocalRoleV1::Argument(0)),
                local(12, WORD, SemanticLocalRoleV1::Temporary),
                local(13, WORD, SemanticLocalRoleV1::Temporary),
                local(14, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(15, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(16, CAPTURE, SemanticLocalRoleV1::Temporary),
                local(17, CAPTURE, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    10,
                    statements,
                    call(1, SemanticOperandV1::Move(place(6, CAPTURE)), 1),
                ),
                block(
                    11,
                    vec![assign(
                        projected(
                            7,
                            &[
                                (SemanticProjectionKindV1::Field(0), REFERENCE),
                                (SemanticProjectionKindV1::Dereference, WORD),
                            ],
                        ),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(
                                WORD,
                                SemanticConstantValueV1::Scalar(
                                    SemanticScalarValueV1::new(17, 8).unwrap(),
                                ),
                            ),
                        )),
                    )],
                    call(1, SemanticOperandV1::Move(place(7, CAPTURE)), 2),
                ),
                block(12, vec![unit()], SemanticTerminatorKindV1::Return),
            ],
        )
        .with_kernel_entry(entry);
        functions[1] = functions[2].clone();
        functions.truncate(2);
    })
}

fn owner_with(
    case: Case,
    mutate: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
) -> ProductionSemanticSsaOwnerV1 {
    try_owner_with(case, mutate).unwrap()
}

fn try_owner_with(
    case: Case,
    mutate: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    let unit_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([1; 32]),
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
    );
    let word_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([2; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    );
    let reference_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([3; 32]),
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
                WORD,
                SemanticPointerKindV1::Reference,
                if case.mutable() {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
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
                    if case.mutable() {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                    },
                    8,
                    8,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let capture_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([4; 32]),
        SemanticLayoutIdentityV1::from_sha256([4; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            8,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![REFERENCE]).unwrap()),
    );
    let root = function(
        10,
        true,
        WORD,
        vec![
            local(10, UNIT, SemanticLocalRoleV1::Return),
            local(11, WORD, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![
            block(
                10,
                vec![],
                call(1, SemanticOperandV1::Copy(place(1, WORD)), 1),
            ),
            block(
                11,
                vec![],
                call(1, SemanticOperandV1::Copy(place(1, WORD)), 2),
            ),
            block(12, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"source_reference_component".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([10; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let borrow_kind = if case.mutable() {
        SemanticBorrowKindV1::Mutable
    } else {
        SemanticBorrowKindV1::Shared
    };
    let mut statements = vec![
        assign(
            place(2, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: borrow_kind,
                place: place(1, WORD),
            },
        ),
        assign(
            place(3, CAPTURE),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![SemanticOperandV1::Move(place(2, REFERENCE))],
                )
                .unwrap(),
            ),
        ),
        dead(2),
    ];
    if case == Case::DeadReferent {
        statements.push(dead(1));
    }
    if case == Case::Conflict {
        statements.push(assign(
            place(4, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(1, WORD),
            },
        ));
    }
    let worker = function(
        20,
        false,
        WORD,
        vec![
            local(20, UNIT, SemanticLocalRoleV1::Return),
            local(21, WORD, SemanticLocalRoleV1::Argument(0)),
            local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
            local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
            local(24, REFERENCE, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                20,
                statements,
                call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 1),
            ),
            block(21, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let mut statements = vec![assign(
        place(2, REFERENCE),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
            1,
            &[(SemanticProjectionKindV1::Field(0), REFERENCE)],
        ))),
    )];
    if matches!(case, Case::Reborrow | Case::SuspendedParent) {
        statements.push(assign(
            place(4, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)]),
            },
        ));
    }
    let read = if case == Case::Reborrow { 4 } else { 2 };
    statements.push(assign(
        place(3, WORD),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
            read,
            &[(SemanticProjectionKindV1::Dereference, WORD)],
        ))),
    ));
    if case == Case::Writeback {
        statements.push(assign(
            projected(
                1,
                &[
                    (SemanticProjectionKindV1::Field(0), REFERENCE),
                    (SemanticProjectionKindV1::Dereference, WORD),
                ],
            ),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                WORD,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(17, 8).unwrap()),
            ))),
        ));
    }
    if case == Case::Reborrow {
        statements.push(dead(4));
        statements.push(assign(
            place(3, WORD),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                2,
                &[(SemanticProjectionKindV1::Dereference, WORD)],
            ))),
        ));
    }
    statements.push(unit());
    let closure = function(
        30,
        false,
        CAPTURE,
        vec![
            local(30, UNIT, SemanticLocalRoleV1::Return),
            local(31, CAPTURE, SemanticLocalRoleV1::Argument(0)),
            local(32, REFERENCE, SemanticLocalRoleV1::Temporary),
            local(33, WORD, SemanticLocalRoleV1::Temporary),
            local(34, REFERENCE, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(30, statements, SemanticTerminatorKindV1::Return)],
    );
    let mut types = vec![unit_type, word_type, reference_type, capture_type];
    let mut functions = vec![root, worker, closure];
    if case == Case::Address {
        fixture_closure::add_address_observation(&mut types, &mut functions);
    }
    mutate(&mut types, &mut functions);
    fixture_closure::retain_fixture_closure(&types, &mut functions);
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(
                u32::try_from(index).unwrap(),
            ))
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let mut owner =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())?;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    Ok(owner)
}

fn run(
    case: Case,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    run_owner(owner(case), consume)
}

fn run_owner(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = with_source_reference_plan_v29(instances, budget, consume);
            assert!(budget.storage() >= floor);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap()
}

fn capture_instance(
    plan: &SourceReferencePlanV29<'_, '_>,
    ordinal: usize,
) -> ProductionCallInstanceIdV1 {
    let index = plan
        .instances
        .instances()
        .iter()
        .enumerate()
        .filter(|(_, row)| row.function() == SemanticFunctionIdV1::from_index(2))
        .nth(ordinal)
        .unwrap()
        .0;
    plan.instances.id_at(index).unwrap()
}

fn capture_node(
    plan: &SourceReferencePlanV29<'_, '_>,
    child: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> usize {
    source_reference_entry_node_v29(plan, child, SemanticLocalIdV1::from_index(1), None, budget)
        .unwrap()
        .unwrap()
}

fn capture_binding(
    references: &SourceReferenceEmissionV29<'_, '_>,
    child: ProductionCallInstanceIdV1,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SemanticValueBindingV1 {
    let node = capture_node(references.plan, child, budget);
    let values = [ValueDef::new(value, Type::Scalar(ScalarType::U64))];
    source_reference_rebuild_node_v29(
        references,
        node,
        true,
        &mut [].iter(),
        &mut values.iter(),
        &mut 0,
        budget,
    )
    .unwrap()
}

#[test]
fn source_reference_emission_index_is_exact_source_occurrence_and_single_use() {
    run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let mut expected = 0;
        for (instance, row) in plan.instances.instances().iter().enumerate() {
            for (block, source) in row.declaration().blocks().iter().enumerate() {
                for (statement, original) in source.statements().iter().enumerate() {
                    let SemanticStatementKindV1::Assign(assignment) = original.kind() else {
                        continue;
                    };
                    let SemanticRvalueKindV1::Borrow { kind, place } = assignment.value().kind()
                    else {
                        continue;
                    };
                    let site = SourceReferenceSiteV29 {
                        instance: plan.instances.id_at(instance).unwrap(),
                        block: SemanticBlockIdV1::from_index(block as u32),
                        statement: Some(statement),
                    };
                    let index = references.loan_at(site, budget)?.unwrap();
                    let loan = &plan.loans[index];
                    assert_eq!(loan.site, site);
                    assert_eq!(loan.kind, *kind);
                    assert_eq!(loan.source_type, assignment.destination().ty());
                    let origin = &plan.origins[loan.origin];
                    assert_eq!(origin.instance, site.instance);
                    assert_eq!(origin.local, place.local());
                    assert_eq!(origin.ty, place.ty());
                    assert_eq!(references.claim(site, budget)?, index);
                    assert!(references.claim(site, budget).is_err());
                    expected += 1;
                }
            }
        }
        assert_eq!(
            expected, 2,
            "two calls to the same source worker create distinct loans"
        );
        assert_eq!(references.sites.len(), expected);
        references.finish(budget)?;
        let missing = SourceReferenceSiteV29 {
            instance: plan.root,
            block: SemanticBlockIdV1::from_index(0),
            statement: Some(0),
        };
        assert!(references.loan_at(missing, budget)?.is_none());
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_call_parameter_raw_denial_is_sticky_after_valid_control() {
    for denied in [false, true] {
        let result = run(Case::Shared, |plan, budget| {
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let child = capture_instance(plan, 0);
            let incoming = plan.instances.incoming(child).unwrap();
            let call_plan = execution_instance_plan_with_references_v29(
                plan.instances,
                child,
                FunctionId::new("reference.helper"),
                SemanticEmissionPlacementV1::default(),
                Some(plan),
                budget,
            )?;
            let signature = execution_function_signature_with_references_v29(
                plan.instances,
                child,
                Some(plan),
                budget,
            )?;
            let binding = capture_binding(&references, child, ValueId(913), budget);
            with_execution_call_scope_v29(budget, |scope, budget| {
                let prepared = PreparedDefinedCallArgumentsV1 {
                    arguments: vec![ValueId(913)],
                    source_bindings: vec![binding],
                    execution: Some(PreparedExecutionCallOriginV29 {
                        scope,
                        ledger: budget.work_ledger_identity_v1(),
                        source: ExecutionCallSourceV29::from_instances(plan.instances, budget)?,
                        function: plan
                            .instances
                            .instance(incoming.occurrence().caller)
                            .unwrap()
                            .function(),
                        occurrence: incoming.occurrence(),
                        callee: plan.instances.instance(child).unwrap().function(),
                        projections: signature.call_arguments,
                        parameter_types: signature.parameter_types,
                    }),
                };
                if denied {
                    // Five five-unit checks precede the constructor's atomic
                    // twelve-unit debit. Leave eleven after those checks.
                    budget.charge_work(usize::MAX - budget.work() - 36)?;
                }
                let before = budget.work();
                let result = prepare_execution_parameters_with_references_v29(
                    plan.instances,
                    child,
                    prepared,
                    &call_plan,
                    Some(&references),
                    budget,
                );
                if denied {
                    let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first)) =
                        result
                    else {
                        panic!("call parameter constructor must report its work denial");
                    };
                    assert!(matches!(first, ArgumentResourceV1::Work(_)));
                    assert_eq!(budget.work() - before, 25);
                    assert_eq!(plan.failure.get(), Some(first));
                    assert!(matches!(references.check(budget),
                        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
                } else {
                    let (arguments, parameters) = result?;
                    assert_eq!(arguments, [ValueId(913)]);
                    drop(parameters);
                }
                Ok(())
            })
        });
        if denied {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn source_reference_component_repeated_helper_keeps_distinct_instance_payloads() {
    run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let left = capture_instance(plan, 0);
        let right = capture_instance(plan, 1);
        assert_eq!(
            plan.instances.instance(left).unwrap().function(),
            plan.instances.instance(right).unwrap().function()
        );
        let left_node = capture_node(plan, left, budget);
        let right_node = capture_node(plan, right, budget);
        let left_binding = capture_binding(&references, left, ValueId(801), budget);
        let right_binding = capture_binding(&references, right, ValueId(802), budget);
        assert_eq!(
            source_reference_call_shape_v29(&references, left_node, &left_binding, budget)?.1,
            [Some(ValueId(801))]
        );
        assert_eq!(
            source_reference_call_shape_v29(&references, right_node, &right_binding, budget)?.1,
            [Some(ValueId(802))]
        );
        assert!(
            source_reference_call_shape_v29(&references, right_node, &left_binding, budget)
                .is_err()
        );
        assert!(
            source_reference_call_shape_v29(&references, left_node, &right_binding, budget)
                .is_err()
        );
        let left_signature = execution_function_signature_with_references_v29(
            plan.instances,
            left,
            Some(plan),
            budget,
        )?;
        let right_signature = execution_function_signature_with_references_v29(
            plan.instances,
            right,
            Some(plan),
            budget,
        )?;
        source_reference_same_signature_v29(&left_signature, &right_signature, budget)?;
        assert_eq!(left_signature.parameter_semantic_types, [CAPTURE]);
        assert_eq!(
            left_signature.parameter_types,
            [Type::Scalar(ScalarType::U64)]
        );
        assert_eq!(left_signature.call_arguments.len(), 1);
        let projection = &left_signature.call_arguments[0];
        assert_eq!(
            (
                projection.source_argument,
                projection.tuple_field,
                projection.component
            ),
            (0, None, Some(0))
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_component_payload_requires_owner_and_explicit_projection() {
    run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let binding = capture_binding(&references, capture_instance(plan, 0), ValueId(711), budget);
        let SemanticValueBindingV1::Aggregate(fields) = binding else {
            panic!("capture")
        };
        let SemanticValueBindingV1::SourceReference(reference) = &fields[0] else {
            panic!("reference")
        };
        assert!(fields[0].value().is_err());
        assert!(fields[0].values().is_err());
        source_reference_validate_binding_v29(plan, reference, budget)?;
        let mut wrong = reference.clone();
        wrong.owner ^= 1;
        assert!(source_reference_validate_binding_v29(plan, &wrong, budget).is_err());
        wrong = reference.clone();
        wrong.source[0] ^= 1;
        assert!(source_reference_validate_binding_v29(plan, &wrong, budget).is_err());
        wrong = reference.clone();
        wrong.values[0].ty = Type::Scalar(ScalarType::U32);
        assert!(source_reference_validate_binding_v29(plan, &wrong, budget).is_err());
        let cloned = emission_clone_binding_v1(&fields[0], budget)?;
        let SemanticValueBindingV1::SourceReference(cloned) = cloned else {
            panic!("clone")
        };
        assert_eq!(&cloned, reference);
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_lowering_keeps_writeback_and_address_obligations() {
    for case in [Case::Writeback, Case::Address] {
        super::source_reference_plan_v29_tests::run_owner_with_storage(
            owner(case),
            |plan, budget| {
                for ordinal in 0..2 {
                    let child = capture_instance(plan, ordinal);
                    assert!(
                        execution_function_signature_with_references_v29(
                            plan.instances,
                            child,
                            Some(plan),
                            budget
                        )
                        .is_err()
                    );
                }
                for loan in &plan.loans {
                    let origin = &plan.origins[loan.origin];
                    assert!(matches!(
                        loan.representation,
                        SourceReferenceRepresentationV29::NeedsAddressable(_)
                    ));
                    assert!(
                        source_reference_existing_value_local_v29(
                            plan,
                            origin.instance,
                            origin.local.index(),
                            budget
                        )
                        .is_err()
                    );
                }
                Ok(())
            },
        )
        .unwrap();
    }
}

#[test]
fn source_reference_optional_owner_preserves_untracked_loop_signature_route() {
    let owner = owner_with(Case::Shared, |_, functions| {
        functions[1] = function(
            20,
            false,
            WORD,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, WORD, SemanticLocalRoleV1::Argument(0)),
            ],
            vec![
                block(
                    20,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(place(1, WORD)),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::SwitchValue,
                                    SemanticBlockIdV1::from_index(1),
                                ),
                            )],
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchOtherwise,
                                SemanticBlockIdV1::from_index(0),
                            ),
                        )
                        .unwrap(),
                    },
                ),
                block(21, vec![unit()], SemanticTerminatorKindV1::Return),
            ],
        );
    });
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            assert_eq!(instances.instances().len(), 3);
            let result =
                with_optional_source_reference_plan_v29(instances, budget, |references, budget| {
                    assert!(references.is_none());
                    for index in 1..instances.instances().len() {
                        let floor = budget.storage();
                        let instance = instances.id_at(index).unwrap();
                        let baseline =
                            execution_function_signature_v29(instances, instance, budget)?;
                        let actual = execution_function_signature_with_references_v29(
                            instances, instance, references, budget,
                        )?;
                        source_reference_same_signature_v29(&baseline, &actual, budget)?;
                        assert_eq!(actual.parameter_semantic_types, [WORD]);
                        assert_eq!(actual.parameter_types, [Type::Scalar(ScalarType::U64)]);
                        assert!(actual.result_types.is_empty());
                        assert_eq!(actual.result_semantic_type, UNIT);
                        assert_eq!(actual.call_arguments.len(), 1);
                        assert_eq!(
                            (
                                actual.call_arguments[0].source_argument,
                                actual.call_arguments[0].tuple_field,
                                actual.call_arguments[0].component
                            ),
                            (0, None, Some(0))
                        );
                        drop(actual);
                        drop(baseline);
                        budget.release_storage(budget.storage() - floor)?;
                    }
                    Ok(())
                });
            assert!(result.is_ok());
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[test]
fn source_reference_optional_owner_checks_borrows_in_expanded_nonroot_instances() {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result =
                with_optional_source_reference_plan_v29(instances, budget, |references, budget| {
                    let references = references.expect("nonroot tracked-loan producer was missed");
                    references.check_owner(instances, budget)?;
                    assert_eq!(references.loans.len(), 2);
                    assert!(
                        references
                            .loans
                            .iter()
                            .all(|loan| loan.site.instance != instances.root())
                    );
                    Ok(())
                });
            assert!(result.is_ok());
            assert_eq!(
                budget.storage(),
                floor + source_reference_emission_headers_v29::<()>().unwrap()
            );
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        },
    )
    .unwrap();
}

#[path = "production_source_reference_general_flow_v29_tests.rs"]
mod general_flow_tests;

#[path = "production_source_reference_cell_custody_v29_tests.rs"]
mod cell_custody_tests;
#[path = "production_source_reference_cell_emission_v29_tests.rs"]
mod cell_emission_tests;
#[path = "production_source_reference_cells_v29_tests.rs"]
pub(super) mod cells_tests;
