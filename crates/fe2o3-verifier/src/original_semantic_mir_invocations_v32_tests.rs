use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::{
    ProductionExecutionSourceInputV29, ProductionPendingScopedSourceOwnerV29,
    ProductionPreparedSourceV18, ProductionScopeCallableCandidateV29,
    ProductionSemanticKirLimitsV1, ProductionSourceLaunchInputV1,
    ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1,
    ProductionSourceOwnedViewErrorV18 as SourceError,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

const LIMIT: usize = 100_000_000;
const FLOOR: usize = 37;

fn prepared(budget: &mut Budget<'_>) -> Result<ProductionPreparedSourceV18> {
    prepared_variant(budget, false)
}

fn prepared_variant(
    budget: &mut Budget<'_>,
    unit_return: bool,
) -> Result<ProductionPreparedSourceV18> {
    prepared_control_variant(budget, unit_return, false)
}

fn prepared_control_variant(
    budget: &mut Budget<'_>,
    unit_return: bool,
    control: bool,
) -> Result<ProductionPreparedSourceV18> {
    prepared_root_variant(budget, unit_return, control, 2)
}

fn prepared_root_variant(
    budget: &mut Budget<'_>,
    unit_return: bool,
    control: bool,
    root_count: u8,
) -> Result<ProductionPreparedSourceV18> {
    prepared_root_storage_variant(budget, unit_return, control, root_count, 0)
}

fn prepared_root_storage_variant(
    budget: &mut Budget<'_>,
    unit_return: bool,
    control: bool,
    root_count: u8,
    retained_storage: u8,
) -> Result<ProductionPreparedSourceV18> {
    prepared_source_transform(
        budget,
        unit_return,
        control,
        root_count,
        retained_storage,
        |_, _| {},
    )
}

fn prepared_source_transform(
    budget: &mut Budget<'_>,
    unit_return: bool,
    control: bool,
    root_count: u8,
    retained_storage: u8,
    transform: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
) -> Result<ProductionPreparedSourceV18> {
    prepared_callable_transform(
        budget,
        unit_return,
        control,
        root_count,
        retained_storage,
        |types, functions, _| transform(types, functions),
    )
}

fn prepared_callable_transform(
    budget: &mut Budget<'_>,
    unit_return: bool,
    control: bool,
    root_count: u8,
    retained_storage: u8,
    transform: impl FnOnce(
        &mut Vec<SemanticTypeDeclV1>,
        &mut Vec<SemanticFunctionDeclV1>,
        &mut Vec<Callable>,
    ),
) -> Result<ProductionPreparedSourceV18> {
    assert!((2..=8).contains(&root_count));
    let root_names: Vec<_> = (0..root_count)
        .map(|root| match root {
            0 => "z_calls".to_string(),
            1 => "a_calls".to_string(),
            _ => format!("root_{root}_calls"),
        })
        .collect();
    let (mut types, mut helper) = super::super::tests::fixture(SemanticBinaryOpV1::BitXor, false);
    if control {
        assert!(!unit_return);
        let source = helper.source();
        let word = SemanticTypeIdV1::from_index(0);
        let place = |local| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], word).unwrap()
        };
        let edge = |target, role| SemanticControlFlowEdgeV1::new(role, Block::from_index(target));
        let branch = |local, cases: Vec<(u128, u32)>, otherwise| {
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(local)),
                    targets: SemanticSwitchTargetsV1::new(
                        cases
                            .into_iter()
                            .map(|(value, target)| {
                                SemanticSwitchTargetV1::new(
                                    value,
                                    edge(target, SemanticEdgeRoleV1::SwitchValue),
                                )
                            })
                            .collect(),
                        edge(otherwise, SemanticEdgeRoleV1::SwitchOtherwise),
                    )
                    .unwrap(),
                },
            )
        };
        let block = |id: u8, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([60 + id; 32]),
                source,
                statements,
                terminator,
            )
            .unwrap()
        };
        let statements = helper.blocks()[0].statements().to_vec();
        let blocks = vec![
            block(0, vec![], branch(1, vec![(0, 1), (1, 1)], 2)),
            block(
                1,
                statements.clone(),
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            ),
            block(2, statements.clone(), branch(2, vec![(0, 3)], 0)),
            block(
                3,
                statements,
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            ),
        ];
        helper = SemanticFunctionDeclV1::new(
            helper.identity(),
            helper.role(),
            helper.item_definition_identity(),
            helper.monomorphization_identity(),
            helper.generic_type_arguments_identity(),
            helper.const_generic_arguments_identity(),
            source,
            helper.abi().clone(),
            helper.locals().to_vec(),
            helper.entry(),
            blocks,
        )
        .unwrap();
    }
    let word = SemanticTypeIdV1::from_index(0);
    let unit = SemanticTypeIdV1::from_index(1);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([80; 32]),
        SemanticLayoutIdentityV1::from_sha256([81; 32]),
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
    ));
    let pair = SemanticTypeIdV1::from_index(types.len() as u32);
    if retained_storage == 1 {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([88; 32]),
            SemanticLayoutIdentityV1::from_sha256([89; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![word, word]).unwrap()),
        ));
    }
    let source = SemanticSourceProvenanceV1::unavailable();
    let unit_place =
        || SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], unit).unwrap();
    if unit_return {
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([50; 32]),
            SemanticLayoutIdentityV1::from_sha256([51; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            helper
                .abi()
                .arguments()
                .iter()
                .map(|row| row.value().clone())
                .collect(),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let mut locals = helper.locals().to_vec();
        locals[0] = SemanticLocalDeclV1::new(
            helper.locals()[0].identity(),
            unit,
            SemanticLocalRoleV1::Return,
            source,
        );
        helper = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([53; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([54; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([55; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([56; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([57; 32]),
            source,
            abi,
            locals,
            Block::from_index(0),
            vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([58; 32]),
                    source,
                    vec![SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            unit_place(),
                            SemanticRvalueV1::new(
                                unit,
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                                    SemanticConstantV1::new(
                                        unit,
                                        SemanticConstantValueV1::ZeroSized,
                                    ),
                                )),
                            ),
                        )),
                    )],
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            ],
        )
        .unwrap();
    }
    let place =
        |local| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], word).unwrap();
    let mut functions = Vec::new();
    for root in 0..root_count {
        let tag = if root < 2 {
            100 + 30 * root
        } else {
            150 + 15 * (root - 2)
        };
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            helper
                .abi()
                .arguments()
                .iter()
                .map(|row| row.value().clone())
                .collect(),
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let mut locals = helper.locals().to_vec();
        locals[0] = SemanticLocalDeclV1::new(
            helper.locals()[0].identity(),
            unit,
            SemanticLocalRoleV1::Return,
            source,
        );
        if retained_storage != 0 {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag + 15; 32]),
                if retained_storage == 1 { pair } else { word },
                SemanticLocalRoleV1::Temporary,
                source,
            ));
        }
        let mut blocks = Vec::new();
        for block in 0..4u32 {
            let terminator = if block == 2 {
                SemanticTerminatorKindV1::Return
            } else {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        FunctionId::from_index(root_count as u32),
                        vec![
                            SemanticOperandV1::Copy(place(1)),
                            SemanticOperandV1::Copy(place(2)),
                        ],
                        Some(SemanticCallDestinationV1::new(
                            if unit_return { unit_place() } else { place(3) },
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                Block::from_index(if block == 0 { 1 } else { 2 }),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                )
            };
            let mut statements = if control && block < 2 {
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(1),
                        SemanticRvalueV1::new(
                            word,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::BitXor,
                                left: SemanticOperandV1::Copy(place(1)),
                                right: SemanticOperandV1::Copy(place(2)),
                            },
                        ),
                    )),
                )]
            } else {
                vec![]
            };
            if retained_storage == 1 && block == 0 {
                let pair_place =
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(4), vec![], pair).unwrap();
                let field = |index| {
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(4),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), word)
                                .unwrap(),
                        ],
                        word,
                    )
                    .unwrap()
                };
                statements.extend([
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            pair_place,
                            SemanticRvalueV1::new(
                                pair,
                                SemanticRvalueKindV1::Aggregate(
                                    SemanticAggregateRvalueV1::new(
                                        SemanticAggregateKindV1::Tuple,
                                        vec![
                                            SemanticOperandV1::Copy(place(1)),
                                            SemanticOperandV1::Copy(place(2)),
                                        ],
                                    )
                                    .unwrap(),
                                ),
                            ),
                        )),
                    ),
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                            field(1),
                            SemanticOperandV1::Copy(place(2)),
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            place(1),
                            SemanticRvalueV1::new(
                                word,
                                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                                    field(0),
                                    SemanticVolatilityV1::NonVolatile,
                                    None,
                                )),
                            ),
                        )),
                    ),
                ]);
            }
            if matches!(retained_storage, 2 | 3) && block == 0 {
                if retained_storage == 3 {
                    statements.push(SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(4)),
                    ));
                }
                statements.extend([
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                            place(4),
                            SemanticOperandV1::Copy(place(1)),
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            place(1),
                            SemanticRvalueV1::new(
                                word,
                                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                                    place(4),
                                    SemanticVolatilityV1::NonVolatile,
                                    None,
                                )),
                            ),
                        )),
                    ),
                ]);
                if retained_storage == 3 {
                    statements.push(SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4)),
                    ));
                }
            }
            blocks.push(
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([tag + 10 + block as u8; 32]),
                    source,
                    statements,
                    SemanticTerminatorV1::new(source, terminator),
                )
                .unwrap(),
            );
        }
        functions.push(
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([tag + 3; 32]),
                SemanticFunctionRoleV1::KernelRoot,
                SemanticItemDefinitionIdentityV1::from_sha256([tag + 4; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([tag + 5; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag + 6; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([tag + 7; 32]),
                source,
                abi,
                locals,
                Block::from_index(0),
                blocks,
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(root_names[root as usize].as_bytes().to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([tag + 9; 32]),
                SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
            )),
        );
    }
    // Preserve all source function indices while sorting the appended helper
    // after every root's deterministic identity, including eight-root fixtures.
    functions.push(
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([255; 32]),
            helper.role(),
            helper.item_definition_identity(),
            helper.monomorphization_identity(),
            helper.generic_type_arguments_identity(),
            helper.const_generic_arguments_identity(),
            helper.source(),
            helper.abi().clone(),
            helper.locals().to_vec(),
            helper.entry(),
            helper.blocks().to_vec(),
        )
        .unwrap(),
    );
    assert!(
        functions
            .windows(2)
            .all(|pair| { pair[0].identity().as_bytes() < pair[1].identity().as_bytes() })
    );
    let mut callables = (0..=root_count as u32)
        .map(|i| Callable::defined(FunctionId::from_index(i)))
        .collect();
    transform(&mut types, &mut functions, &mut callables);
    let semantic = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        (0..root_count as u32).map(FunctionId::from_index).collect(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let launches: Vec<_> = semantic
        .roots()
        .iter()
        .enumerate()
        .map(|(root, function)| {
            let entry = semantic.functions()[function.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                &root_names[root],
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(
                    1,
                    Some(
                        entry
                            .source_contract()
                            .launch()
                            .and_then(|launch| launch.required())
                            .map(|required| required.as_array())
                            .unwrap_or([64, 1, 1]),
                    ),
                    [1, 1, 1],
                ),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(&semantic, &launches).unwrap();
    let callable_count = semantic.callables().len();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let semantic_sha256 = *owner.source_semantic_sha256();
    Ok(
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
            owner,
            launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: &semantic_sha256,
                roots: &[],
                classes: &vec![ProductionScopeCallableCandidateV29::Ordinary; callable_count],
                events: &[],
            },
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )?,
    )
}

fn retain(error: Error, source: &Source<'_>) -> Error {
    match error {
        Error::Resource(resource) => {
            Error::Source(source.retain_query_resource_error_v18(resource))
        }
        other => other,
    }
}

pub(in super::super) fn run(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_variant(work, storage, false, examine)
}

pub(in super::super) fn run_variant(
    work: usize,
    storage: usize,
    unit_return: bool,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_control_variant(work, storage, unit_return, false, examine)
}

pub(in super::super) fn run_control_variant(
    work: usize,
    storage: usize,
    unit_return: bool,
    control: bool,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_root_variant(work, storage, unit_return, control, 2, examine)
}

pub(in super::super) fn run_root_variant(
    work: usize,
    storage: usize,
    unit_return: bool,
    control: bool,
    root_count: u8,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_root_storage_variant(work, storage, unit_return, control, root_count, 0, examine)
}

pub(in super::super) fn run_allocation_variant(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_root_storage_variant(work, storage, false, false, 2, 1, examine)
}

pub(in super::super) fn run_scalar_allocation_variant(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_root_storage_variant(work, storage, false, false, 2, 2, examine)
}

pub(in super::super) fn run_scalar_lifetime_variant(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_root_storage_variant(work, storage, false, false, 2, 3, examine)
}

fn run_root_storage_variant(
    work: usize,
    storage: usize,
    unit_return: bool,
    control: bool,
    root_count: u8,
    retained_storage: u8,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_prepared(
        work,
        storage,
        |budget| {
            if retained_storage != 0 {
                prepared_root_storage_variant(
                    budget,
                    unit_return,
                    control,
                    root_count,
                    retained_storage,
                )
            } else if root_count != 2 {
                prepared_root_variant(budget, unit_return, control, root_count)
            } else if control {
                prepared_control_variant(budget, unit_return, true)
            } else if unit_return {
                prepared_variant(budget, true)
            } else {
                prepared(budget)
            }
        },
        examine,
    )
}

pub(in super::super) fn run_source_transform(
    work: usize,
    storage: usize,
    transform: impl FnOnce(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>),
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_prepared(
        work,
        storage,
        |budget| prepared_source_transform(budget, false, false, 2, 0, transform),
        examine,
    )
}

pub(in super::super) fn run_callable_transform(
    work: usize,
    storage: usize,
    transform: impl FnOnce(
        &mut Vec<SemanticTypeDeclV1>,
        &mut Vec<SemanticFunctionDeclV1>,
        &mut Vec<Callable>,
    ),
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_prepared(
        work,
        storage,
        |budget| prepared_callable_transform(budget, false, false, 2, 0, transform),
        examine,
    )
}

fn run_prepared(
    work: usize,
    storage: usize,
    prepare: impl FnOnce(&mut Budget<'_>) -> Result<ProductionPreparedSourceV18>,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(FLOOR).unwrap();
    let result = (|| {
        let prepared = prepare(&mut budget)?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let floor = budget.storage();
            let result = budget
                .with_prepaid_scope(
                    floor,
                    1,
                    1,
                    super::super::super::super::SOURCE_LIMIT,
                    |budget| {
                        let result = (|| {
                            let mut writer = Writer::new(budget)?;
                            let mut plan = InvocationPlan::derive(source, &mut writer)?;
                            examine(&mut plan, &mut writer)
                        })();
                        result.map_err(|error| retain(error, source))
                    },
                )
                .map_err(|error| retain(error, source));
            if result.is_ok() {
                assert_eq!(budget.storage(), floor);
            }
            result
        })
    })();
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

fn check_roster(plan: &mut InvocationPlan<'_, '_>, out: &mut Writer<'_, '_>) -> Result<()> {
    assert_eq!(plan.roots.len(), 2);
    let mut local_end = 0;
    let mut block_end = 0;
    for root in 0..2 {
        assert_eq!(plan.root(root, out)?.function.index(), root as u32);
        let count = plan.root(root, out)?.instances.len();
        assert!(count >= 3);
        let calls = plan.calls(root, 0, out)?;
        assert_eq!(calls.len(), 3);
        assert_eq!(
            calls
                .iter()
                .map(|row| row.block.index())
                .collect::<Vec<_>>(),
            [0, 1, 3]
        );
        assert!(calls[0].ssa_reachable && calls[1].ssa_reachable);
        assert!(!calls[2].ssa_reachable);
        assert_ne!(calls[0].child, calls[1].child);
        for call in &calls[..2] {
            let child = plan.instance(root, call.child.unwrap(), out)?;
            assert_eq!(child.function, FunctionId::from_index(2));
            assert_eq!(child.incoming, Some((0, call.block)));
            assert!(child.active);
        }
        if let Some(child) = calls[2].child {
            assert!(!plan.instance(root, child, out)?.active);
        }
        for instance in 0..count {
            let row = plan.instance(root, instance, out)?;
            assert_eq!(row.locals.start, local_end);
            assert_eq!(row.blocks.start, block_end);
            assert_eq!(row.locals.len(), 4);
            assert_eq!(row.blocks.len(), if instance == 0 { 4 } else { 1 });
            local_end = row.locals.end;
            block_end = row.blocks.end;
        }
    }
    Ok(())
}

#[test]
fn original_mir_invocation_roster_preserves_repeated_helpers_all_roots_and_inactive_calls() {
    let first = run(LIMIT, LIMIT, check_roster);
    let second = run(LIMIT, LIMIT, check_roster);
    first.0.unwrap();
    second.0.unwrap();
    assert_eq!((first.1, first.2, first.3), (second.1, FLOOR, second.3));
}

#[test]
fn original_mir_invocation_roster_has_exact_and_one_short_complete_resources() {
    let measured = run(LIMIT, LIMIT, check_roster);
    measured.0.unwrap();
    let exact = run(measured.1, measured.3, check_roster);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, FLOOR, measured.3));
    assert!(matches!(run(measured.1 - 1, measured.3, check_roster).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1));
    assert!(matches!(run(measured.1, measured.3 - 1, check_roster).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1));
}

#[test]
fn original_mir_invocation_roster_rejects_changed_callee_callsite_and_duplicate_incoming() {
    let result = run(LIMIT, LIMIT, |plan, out| {
        let semantic = plan.source.source_semantic(out.budget)?;
        let original = &plan.instances[plan.roots[0].instances.clone()];
        for fault in 0..4 {
            let mut instances = original.to_vec();
            let mut calls = plan.calls.clone();
            for row in &instances {
                for call in &mut calls[row.calls.clone()] {
                    call.child = None;
                }
            }
            match fault {
                0 => instances[1].function = FunctionId::from_index(0),
                1 => instances[1].incoming = Some((0, Block::from_index(2))),
                2 => instances[2].incoming = instances[1].incoming,
                3 => calls[instances[0].calls.start].ssa_reachable = false,
                _ => unreachable!(),
            }
            let mut incoming: Vec<_> = instances
                .iter()
                .enumerate()
                .skip(1)
                .map(|(child, row)| {
                    let (caller, block) = row.incoming.unwrap();
                    Incoming {
                        caller,
                        block,
                        child,
                    }
                })
                .collect();
            assert!(matches!(
                link_root(
                    semantic.callables(),
                    &instances,
                    &mut calls,
                    &mut incoming,
                    out
                ),
                Err(Error::Statement(
                    "original MIR invocation coordinates differ from their source calls"
                ))
            ));
        }
        check_roster(plan, out)
    });
    result.0.unwrap();
    assert_eq!(result.2, FLOOR);
}

#[test]
fn original_mir_invocation_roster_rejects_funded_foreign_budget_before_mutation() {
    let result = run(LIMIT, LIMIT, |plan, _| {
        let mut work = Work::new(LIMIT);
        let mut foreign = Budget::new(&mut work, LIMIT);
        foreign
            .reserve_storage(super::super::super::super::SOURCE_LIMIT + plan.required)
            .unwrap();
        let writer = Writer::new(&mut foreign)?;
        let before = (
            writer.budget.work(),
            writer.budget.storage(),
            writer.budget.peak_storage(),
        );
        let result = plan.root(0, &writer);
        assert!(matches!(
            result,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(
            (
                writer.budget.work(),
                writer.budget.storage(),
                writer.budget.peak_storage()
            ),
            before
        );
        result.map(|_| ())
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn original_mir_invocation_roster_headers_have_independent_field_and_result_oracles() {
    fn vector<T>() -> usize {
        size_of::<Vec<T>>() + 2 * size_of::<Result<Vec<T>>>()
    }
    fn query<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T>>()
            + 2 * size_of::<std::result::Result<T, SourceError>>()
    }
    type PlanFields = (
        &'static Source<'static>,
        Vec<Root>,
        Vec<Instance>,
        Vec<CallSite>,
        Ledger,
        usize,
        usize,
    );
    type RootFields = (FunctionId, usize, Range<usize>);
    type InstanceFields = (
        FunctionId,
        Option<(usize, Block)>,
        bool,
        Range<usize>,
        Range<usize>,
        Range<usize>,
    );
    type CallFields = (usize, Block, CallableId, CallKind, bool, Option<usize>);
    type IncomingFields = (usize, Block, usize);
    assert_eq!(size_of::<InvocationPlan<'_, '_>>(), size_of::<PlanFields>());
    assert_eq!(size_of::<Root>(), size_of::<RootFields>());
    assert_eq!(size_of::<Instance>(), size_of::<InstanceFields>());
    assert_eq!(size_of::<CallSite>(), size_of::<CallFields>());
    assert_eq!(size_of::<Incoming>(), size_of::<IncomingFields>());
    let expected = size_of::<PlanFields>()
        + 2 * size_of::<Result<InvocationPlan<'_, '_>>>()
        + vector::<Root>()
        + vector::<Instance>()
        + vector::<CallSite>()
        + vector::<Incoming>()
        + vector::<usize>()
        + size_of::<RootFields>()
        + size_of::<InstanceFields>()
        + size_of::<CallFields>()
        + size_of::<IncomingFields>()
        + size_of::<(&Source<'_>, &mut Writer<'_, '_>, Ledger)>()
        + size_of::<Result<&Root>>()
        + size_of::<Result<&Instance>>()
        + size_of::<Result<&[CallSite]>>()
        + size_of::<Result<&Source<'_>>>()
        + 2 * size_of::<&()>()
        + query::<()>()
        + query::<usize>()
        + query::<bool>()
        + query::<(FunctionId, usize)>()
        + query::<(FunctionId, Option<(usize, Block)>)>()
        + query::<&AdmittedInertSemanticMirV1>()
        + query::<&ProductionSemanticSsaOwnerV1>()
        + size_of::<Option<(CallKind, CallableId)>>()
        + size_of::<(&[Callable], &[Instance], &mut [CallSite], &mut [Incoming])>()
        + 18 * size_of::<usize>();
    assert_eq!(headers(), expected);
}
