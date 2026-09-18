use super::*;
use fe2o3_mir_model::{SsaResolvedEventV1, SsaVariableIdV1};
use fe2o3_pliron::{
    ProductionSemanticSsaEntryOriginV1 as EntryOrigin, ProductionSemanticSsaErrorV1 as SsaError,
    ProductionSemanticSsaModuleLimitsV1 as ModuleLimits,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Case {
    Forward,
    Reborrow,
    Unused,
    CopiedReference,
    ReturnedReference,
    Cycle,
}

fn helper_source(case: Case, ignored_prefix: usize) -> AdmittedInertSemanticMirV1 {
    let with_provider = matches!(case, Case::Forward | Case::Reborrow | Case::CopiedReference);
    let relay_id = if with_provider { 3 } else { 2 };
    let issuer_id = if with_provider { 5 } else { 3 };
    let base = lifecycle_owner(false);
    let semantic = base.source_semantic();
    let original = &semantic.functions()[1];
    let reference_type = original.locals()[3].ty();
    let workgroup = original.locals()[4].ty();
    let reference_ordinal = ignored_prefix as u32 + 1;
    let mut inputs = vec![UNIT; ignored_prefix];
    inputs.extend([U32, reference_type]);
    let make_abi = |tag, output| {
        let mut ownership = vec![SemanticSourceArgumentOwnershipV1::ByValue; inputs.len()];
        ownership[reference_ordinal as usize] = SemanticSourceArgumentOwnershipV1::UniqueBorrow;
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            semantic.target().identity(),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            inputs.len() as u32,
            inputs
                .iter()
                .map(|ty| SemanticAbiArgumentV1::source(value_abi(semantic.types(), *ty)))
                .collect(),
            if output == reference_type {
                SemanticAbiValueV1::new(
                    output,
                    SemanticAbiPassModeV1::Direct(
                        SemanticAbiValueAttributesV1::new(
                            SemanticAbiRegularAttributesV1::new(
                                false, None, true, false, false, true,
                            ),
                            SemanticAbiExtensionV1::None,
                            0,
                            None,
                        )
                        .unwrap(),
                    ),
                )
            } else {
                value_abi(semantic.types(), output)
            },
        )
        .unwrap()
        .with_source_argument_ownership(ownership)
        .unwrap()
    };
    let arguments = |reference| {
        let mut arguments = vec![
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                UNIT,
                SemanticConstantValueV1::ZeroSized,
            ));
            ignored_prefix
        ];
        arguments.push(SemanticOperandV1::Copy(place(2, U32)));
        arguments.push(SemanticOperandV1::Move(place(reference, reference_type)));
        arguments
    };
    let invoke = |callee, arguments, destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    destination,
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let relay_output = if case == Case::ReturnedReference {
        reference_type
    } else {
        UNIT
    };
    let mut holder_locals = original.locals().to_vec();
    holder_locals.push(local(159, reference_type, SemanticLocalRoleV1::Temporary));
    let holder = function(
        100,
        SemanticFunctionRoleV1::InternalHelper,
        original.abi().clone(),
        holder_locals,
        vec![
            block(
                150,
                vec![assign(
                    place(3, reference_type),
                    SemanticRvalueKindV1::Borrow {
                        place: place(1, CONTEXT),
                        kind: SemanticBorrowKindV1::Mutable,
                    },
                )],
                invoke(
                    relay_id,
                    arguments(3),
                    if case == Case::ReturnedReference {
                        place(6, reference_type)
                    } else {
                        place(0, UNIT)
                    },
                    1,
                ),
            ),
            block(151, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let mut relay_locals = vec![
        local(161, relay_output, SemanticLocalRoleV1::Return),
        local(
            162,
            reference_type,
            SemanticLocalRoleV1::Argument(reference_ordinal),
        ),
        local(
            163,
            U32,
            SemanticLocalRoleV1::Argument(ignored_prefix as u32),
        ),
        local(164, reference_type, SemanticLocalRoleV1::Temporary),
    ];
    relay_locals.extend((0..ignored_prefix).map(|ordinal| {
        local(
            180 + ordinal as u8,
            UNIT,
            SemanticLocalRoleV1::Argument(ordinal as u32),
        )
    }));
    let mut statements = Vec::new();
    let forwarded = match case {
        Case::Reborrow => {
            statements.push(assign(
                place(3, reference_type),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(1),
                        vec![
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Dereference,
                                CONTEXT,
                            )
                            .unwrap(),
                        ],
                        CONTEXT,
                    )
                    .unwrap(),
                },
            ));
            3
        }
        Case::CopiedReference => {
            statements.push(assign(
                place(3, reference_type),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, reference_type))),
            ));
            1
        }
        Case::ReturnedReference => {
            statements.push(assign(
                place(0, reference_type),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, reference_type))),
            ));
            1
        }
        _ => 1,
    };
    let relay_blocks = if matches!(case, Case::ReturnedReference | Case::Unused) {
        vec![block(165, statements, SemanticTerminatorKindV1::Return)]
    } else {
        vec![
            block(
                165,
                statements,
                invoke(
                    if case == Case::Cycle { relay_id } else { 4 },
                    arguments(forwarded),
                    place(0, UNIT),
                    1,
                ),
            ),
            block(166, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    let relay = function(
        160,
        SemanticFunctionRoleV1::InternalHelper,
        make_abi(160, relay_output),
        relay_locals,
        relay_blocks,
    );
    let mut provider_locals = vec![
        local(171, UNIT, SemanticLocalRoleV1::Return),
        local(
            172,
            reference_type,
            SemanticLocalRoleV1::Argument(reference_ordinal),
        ),
        local(
            173,
            U32,
            SemanticLocalRoleV1::Argument(ignored_prefix as u32),
        ),
        local(174, reference_type, SemanticLocalRoleV1::Temporary),
        local(175, workgroup, SemanticLocalRoleV1::Temporary),
        local(176, U32, SemanticLocalRoleV1::Temporary),
    ];
    provider_locals.extend((0..ignored_prefix).map(|ordinal| {
        local(
            200 + ordinal as u8,
            UNIT,
            SemanticLocalRoleV1::Argument(ordinal as u32),
        )
    }));
    let provider_blocks = if case == Case::Unused {
        vec![block(177, vec![], SemanticTerminatorKindV1::Return)]
    } else {
        vec![
            block(
                177,
                vec![],
                invoke(
                    6,
                    vec![SemanticOperandV1::Move(place(1, reference_type))],
                    place(4, workgroup),
                    1,
                ),
            ),
            block(
                178,
                original.blocks()[1].statements().to_vec(),
                original.blocks()[1].terminator().kind().clone(),
            ),
            block(179, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    let provider = function(
        170,
        SemanticFunctionRoleV1::InternalHelper,
        make_abi(170, UNIT),
        provider_locals,
        provider_blocks,
    );
    let original_root = &semantic.functions()[0];
    let mut root_blocks = original_root.blocks().to_vec();
    root_blocks[0] = block(85, vec![], invoke(issuer_id, vec![], place(2, CONTEXT), 1));
    let root = function(
        80,
        SemanticFunctionRoleV1::KernelRoot,
        original_root.abi().clone(),
        original_root.locals().to_vec(),
        root_blocks,
    )
    .with_kernel_entry(original_root.kernel_entry().unwrap().clone());
    let functions = if with_provider {
        vec![
            root,
            holder,
            semantic.functions()[2].clone(),
            relay,
            provider,
        ]
    } else {
        vec![root, holder, relay]
    };
    let mut callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect::<Vec<_>>();
    callables.push(semantic.callables()[ISSUER.index() as usize].clone());
    if with_provider {
        callables.push(semantic.callables()[DERIVE.index() as usize].clone());
    }
    InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap_or_else(|error| panic!("{case:?} prefix={ignored_prefix}: {error:?}"))
}

fn owner_with_limits(
    source: AdmittedInertSemanticMirV1,
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<ProductionSemanticSsaOwnerV1, SsaError> {
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        limits,
    )
}

fn captured(case: Case, prefix: usize, check: impl FnOnce(&ProductionSemanticSsaOwnerV1)) {
    let mut owner = owner_with_limits(
        helper_source(case, prefix),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let before = owner.identity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(FLOOR).unwrap();
    let receipt = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    owner.verify_replay().unwrap();
    assert_eq!(owner.identity(), before);
    for plan in owner.plans() {
        assert!(plan.implicit_entry_variables().is_empty());
        assert!(
            owner
                .occurrences_v1()
                .unwrap()
                .function(plan.function())
                .unwrap()
                .entry_definitions()
                .iter()
                .all(|entry| entry.origin() != EntryOrigin::ImplicitCapability)
        );
    }
    check(&owner);
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn nominal_helper_bodies_close_transitive_borrows_without_fabricating_occurrences() {
    for case in [Case::Forward, Case::Reborrow, Case::Unused] {
        for prefix in [0, 8] {
            captured(case, prefix, |owner| {
                let occurrences = owner.occurrences_v1().unwrap();
                let holder = occurrences.function(HELPER).unwrap();
                let context = holder
                    .events()
                    .iter()
                    .find(|event| {
                        event.operand() == ExecutionOperandV29::RvaluePlace
                            && event.role() == ExecutionEventV29::BaseUse
                    })
                    .unwrap();
                assert_eq!(context.event().variable(), SsaVariableIdV1::new(1));
                assert!(context.is_promoted());
                assert!(context.resolved().is_some());
                let definition = holder
                    .events()
                    .iter()
                    .find(|event| {
                        event.role() == ExecutionEventV29::DestinationDefine
                            && event.event().variable() == SsaVariableIdV1::new(3)
                    })
                    .unwrap();
                let Some(SsaResolvedEventV1::Define { value, .. }) = definition.resolved() else {
                    panic!("borrow definition");
                };
                let argument = prefix as u32 + 1;
                let use_event = holder
                    .events()
                    .iter()
                    .find(|event| {
                        event.operand() == ExecutionOperandV29::CallArgument(argument)
                            && event.role() == ExecutionEventV29::BaseUse
                    })
                    .unwrap();
                let kill = holder
                    .events()
                    .iter()
                    .find(|event| {
                        event.operand() == ExecutionOperandV29::CallArgument(argument)
                            && event.role() == ExecutionEventV29::MoveKill
                    })
                    .unwrap();
                assert!(
                    matches!(use_event.resolved(), Some(SsaResolvedEventV1::Use { value: actual, .. }) if actual == value)
                );
                assert!(
                    matches!(kill.resolved(), Some(SsaResolvedEventV1::Kill { previous: Some(actual), .. }) if actual == value)
                );
                let relay = occurrences
                    .function(SemanticFunctionIdV1::from_index(if case == Case::Unused {
                        2
                    } else {
                        3
                    }))
                    .unwrap();
                assert!(
                    relay
                        .entry_definitions()
                        .iter()
                        .any(|entry| entry.variable() == SsaVariableIdV1::new(1)
                            && entry.origin() == EntryOrigin::Argument(argument))
                );
            });
        }
    }
}

#[test]
fn nominal_helper_alias_return_and_recursive_dependencies_remain_unapproved() {
    for case in [Case::CopiedReference, Case::ReturnedReference, Case::Cycle] {
        captured(case, 0, |owner| {
            let holder = owner.occurrences_v1().unwrap().function(HELPER).unwrap();
            let context = holder
                .events()
                .iter()
                .find(|event| {
                    event.operand() == ExecutionOperandV29::RvaluePlace
                        && event.role() == ExecutionEventV29::BaseUse
                })
                .unwrap();
            assert!(!context.is_promoted(), "{case:?}");
            assert!(context.resolved().is_none(), "{case:?}");
        });
    }
}

#[test]
fn isolated_planning_cannot_infer_the_missing_helper_body_summary() {
    let source = helper_source(Case::Forward, 0);
    let plan = fe2o3_pliron::plan_semantic_function_ssa_with_module_v1(
        HELPER,
        &source.functions()[1],
        source.types(),
        source.callables(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    assert!(
        !plan
            .plan()
            .promoted_variables()
            .contains(&SsaVariableIdV1::new(1))
    );
    let owner = owner_with_limits(source, ProductionSemanticSsaLimitsV1::default()).unwrap();
    assert!(
        owner
            .plan_for_function(HELPER)
            .unwrap()
            .plan()
            .promoted_variables()
            .contains(&SsaVariableIdV1::new(1))
    );
}

fn module_limits(work: usize, storage: usize) -> ProductionSemanticSsaLimitsV1 {
    let limits = ModuleLimits::production();
    ProductionSemanticSsaLimitsV1::with_module_limits(
        fe2o3_mir_model::SsaPlannerLimitsV1::default(),
        ModuleLimits::try_new(
            limits.max_variables(),
            limits.max_blocks(),
            limits.max_edges(),
            limits.max_events(),
            limits.max_edge_definitions(),
            limits.max_output_items(),
            storage,
            work,
        )
        .unwrap(),
    )
}

#[test]
fn nominal_helper_sparse_slots_and_closed_round_have_independent_limits() {
    use fe2o3_mir_model::SsaPlannerResourceV1::{StorageWords, WorkUnits};

    let limits = ModuleLimits::production();
    // Three functions, seven source slots, thirteen helper locals and one seed:
    // setup 114 + two row passes 20 + scan 1709 + closure 1 = 1844 work.
    // Retained rows 136 + peak scratch 400 = 536 words. Root sizing next costs 4.
    for (work, storage, resource, required, limit) in [
        (1843, limits.max_storage_words(), WorkUnits, 1844, 1843),
        (1844, limits.max_storage_words(), WorkUnits, 1848, 1844),
        (limits.max_work_units(), 535, StorageWords, 536, 535),
    ] {
        assert_eq!(
            owner_with_limits(helper_source(Case::Unused, 2), module_limits(work, storage),)
                .unwrap_err(),
            SsaError::AggregateResourceLimit {
                resource,
                required,
                limit,
            },
        );
    }
}

#[test]
fn nominal_helper_summary_work_and_storage_are_part_of_exact_module_limits() {
    for prefix in [0, 8] {
        let owner = owner_with_limits(
            helper_source(Case::Reborrow, prefix),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let summary = owner.summary();
        let work = summary.work_units();
        let storage = summary.storage_words();
        let exact = owner_with_limits(
            helper_source(Case::Reborrow, prefix),
            module_limits(work, storage),
        )
        .unwrap();
        assert_eq!(exact.identity(), owner.identity());
        for (work, storage) in [
            (work - 1, storage),
            (work, storage - 1),
            (1, storage),
            (work, 1),
        ] {
            assert!(matches!(
                owner_with_limits(
                    helper_source(Case::Reborrow, prefix),
                    module_limits(work, storage)
                ),
                Err(SsaError::AggregateResourceLimit { .. })
            ));
        }
    }
}
