use super::*;
#[path = "production_execution_helper_borrow_v29_tests.rs"]
mod helper_borrow_tests;

mod scoped_root_tests {
    include!("production_scoped_root_emission_v29_tests.rs");
}

use crate::{
    ProductionContextCallBoundaryV29 as Boundary, ProductionContextRootInputV29 as RootInput,
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    ProductionSourceLaunchRosterV1,
};

const CALLBACK: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(2);
const ISSUER: SemanticCallableIdV1 = SemanticCallableIdV1::from_index(3);
const DERIVE: SemanticCallableIdV1 = SemanticCallableIdV1::from_index(4);

// Admitted inert source/SSA fixtures. All nominal values come from actual source
// call emission, not entry seeds; this is not rustc producer authentication.
fn lifecycle_owner(branches: bool) -> ProductionSemanticSsaOwnerV1 {
    let template = nominal_owner(Input::Workgroup, false);
    let workgroup = template.source_semantic().functions()[1]
        .abi()
        .source_input_types()[0];
    let mut types = template.source_semantic().types().to_vec();
    let reference_type = reference(&mut types, CONTEXT, SemanticMutabilityV1::Mutable, false);
    let make_abi = |tag, inputs: &[SemanticTypeIdV1], output| {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            inputs.len() as u32,
            inputs
                .iter()
                .map(|ty| SemanticAbiArgumentV1::source(value_abi(&types, *ty)))
                .collect(),
            value_abi(&types, output),
        )
        .unwrap()
        .with_source_argument_ownership(vec![
            SemanticSourceArgumentOwnershipV1::ByValue;
            inputs.len()
        ])
        .unwrap()
    };
    let invoke = |callee, arguments, destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                callee,
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
    let root = function(
        80,
        SemanticFunctionRoleV1::KernelRoot,
        abi(81, true, &[U32]),
        vec![
            local(82, UNIT, SemanticLocalRoleV1::Return),
            local(83, U32, SemanticLocalRoleV1::Argument(0)),
            local(84, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(85, vec![], invoke(ISSUER, vec![], place(2, CONTEXT), 1)),
            block(
                86,
                vec![],
                invoke(
                    SemanticCallableIdV1::from_index(1),
                    vec![
                        SemanticOperandV1::Move(place(2, CONTEXT)),
                        SemanticOperandV1::Copy(place(1, U32)),
                    ],
                    place(0, UNIT),
                    2,
                ),
            ),
            block(87, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"lifecycle_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([88; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let mut provider_blocks = vec![
        block(
            90,
            vec![assign(
                place(3, reference_type),
                SemanticRvalueKindV1::Borrow {
                    place: place(1, CONTEXT),
                    kind: SemanticBorrowKindV1::Mutable,
                },
            )],
            invoke(
                DERIVE,
                vec![SemanticOperandV1::Move(place(3, reference_type))],
                place(4, workgroup),
                1,
            ),
        ),
        block(
            91,
            vec![],
            invoke(
                SemanticCallableIdV1::from_index(2),
                vec![
                    SemanticOperandV1::Move(place(4, workgroup)),
                    SemanticOperandV1::Copy(place(2, U32)),
                ],
                place(5, U32),
                2,
            ),
        ),
    ];
    if branches {
        provider_blocks.push(block(
            92,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(5, U32)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchValue,
                            SemanticBlockIdV1::from_index(3),
                        ),
                    )],
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::SwitchOtherwise,
                        SemanticBlockIdV1::from_index(4),
                    ),
                )
                .unwrap(),
            },
        ));
        provider_blocks.push(block(93, vec![], SemanticTerminatorKindV1::Return));
        provider_blocks.push(block(94, vec![], SemanticTerminatorKindV1::Return));
    } else {
        provider_blocks.push(block(92, vec![], SemanticTerminatorKindV1::Return));
    }
    let provider = function(
        100,
        SemanticFunctionRoleV1::InternalHelper,
        make_abi(101, &[CONTEXT, U32], UNIT),
        vec![
            local(102, UNIT, SemanticLocalRoleV1::Return),
            local(103, CONTEXT, SemanticLocalRoleV1::Argument(0)),
            local(104, U32, SemanticLocalRoleV1::Argument(1)),
            local(105, reference_type, SemanticLocalRoleV1::Temporary),
            local(106, workgroup, SemanticLocalRoleV1::Temporary),
            local(107, U32, SemanticLocalRoleV1::Temporary),
        ],
        provider_blocks,
    );
    let callback = function(
        110,
        SemanticFunctionRoleV1::InternalHelper,
        make_abi(111, &[workgroup, U32], U32),
        vec![
            local(112, U32, SemanticLocalRoleV1::Return),
            local(113, workgroup, SemanticLocalRoleV1::Argument(0)),
            local(114, U32, SemanticLocalRoleV1::Argument(1)),
        ],
        vec![block(
            115,
            vec![assign(
                place(0, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, U32))),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let intrinsic = |tag, abi, operation| SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            source(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Execution(operation),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    };
    let issuer = intrinsic(
        120,
        make_abi(120, &[], CONTEXT),
        SemanticExecutionOperationV29::ContextIssue { context: CONTEXT },
    );
    let derive = intrinsic(
        121,
        make_abi(121, &[reference_type], workgroup)
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::UniqueBorrow])
            .unwrap(),
        SemanticExecutionOperationV29::WorkgroupDerive {
            context: CONTEXT,
            workgroup,
        },
    );
    let semantic = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, provider, callback],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(HELPER),
            SemanticCallableDeclV1::defined(CALLBACK),
            issuer,
            derive,
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn root_input(owner: &ProductionSemanticSsaOwnerV1) -> RootInput<'_> {
    let semantic = owner.source_semantic();
    let SemanticTerminatorKindV1::Call(helper) =
        semantic.functions()[0].blocks()[1].terminator().kind()
    else {
        panic!("helper call");
    };
    RootInput {
        semantic_sha256: owner.source_semantic_sha256(),
        root: ROOT,
        root_identity: semantic.functions()[0].identity(),
        helper: HELPER,
        helper_identity: semantic.functions()[1].identity(),
        issuer: ISSUER,
        issuer_identity: SemanticFunctionIdentityV1::from_sha256([120; 32]),
        context_type: CONTEXT,
        context_identity: semantic.types()[CONTEXT.index() as usize].identity(),
        issuance: Boundary {
            block: SemanticBlockIdV1::from_index(0),
            statement_count: 0,
            destination: SemanticLocalIdV1::from_index(2),
            destination_type: CONTEXT,
            target: SemanticBlockIdV1::from_index(1),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        helper_call: Boundary {
            block: SemanticBlockIdV1::from_index(1),
            statement_count: 0,
            destination: SemanticLocalIdV1::from_index(0),
            destination_type: UNIT,
            target: SemanticBlockIdV1::from_index(2),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        helper_context_local: SemanticLocalIdV1::from_index(2),
        helper_arguments: helper.arguments(),
    }
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    None,
    MissingConsumer,
    ForeignSource,
    ChangedCatalog,
    SwappedEvents,
    LateLimit,
    Orchestrated {
        groups: u32,
        limits: ProductionSemanticKirLimitsV1,
        assertion: bool,
    },
}

fn run_lifecycle(
    branches: bool,
    fault: Fault,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<Vec<DeferredLifecycleEventV29>, ProductionSemanticKirErrorV1>,
    usize,
    usize,
) {
    let assertion = matches!(
        fault,
        Fault::Orchestrated {
            assertion: true,
            ..
        }
    );
    let mut owner = if assertion {
        assert!(!branches);
        scoped_root_tests::assertion_owner()
    } else {
        lifecycle_owner(branches)
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result = (|| {
        budget.reserve_storage(FLOOR)?;
        let capture = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .map_err(|error| match error {
                fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_call_error_v29(),
            })?;
        budget.reserve_storage(capture.retained_storage())?;
        let semantic = owner.source_semantic();
        let launch = ProductionSourceLaunchRosterV1::try_new(
            semantic,
            &[ProductionSourceLaunchRootInputV1::new(
                "lifecycle_fixture",
                [88; 32],
                ProductionSourceLaunchInputV1::new(
                    1,
                    Some([64, 1, 1]),
                    [
                        match fault {
                            Fault::Orchestrated { groups, .. } => groups,
                            _ => 2,
                        },
                        1,
                        1,
                    ],
                ),
            )],
        )
        .unwrap();
        let roots = [root_input(&owner)];
        let workgroup = semantic.functions()[2].abi().source_input_types()[0];
        let classes = [
            ProductionScopeCallableCandidateV29::Ordinary,
            ProductionScopeCallableCandidateV29::Provider {
                function: HELPER,
                identity: semantic.functions()[1].identity(),
            },
            ProductionScopeCallableCandidateV29::Ordinary,
            ProductionScopeCallableCandidateV29::Ordinary,
            ProductionScopeCallableCandidateV29::Derive {
                binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
                operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
                context: CONTEXT,
                workgroup,
            },
        ];
        let mut events = vec![
            (
                ROOT,
                1,
                0,
                ProductionScopeEventKindV29::Call {
                    callee: SemanticCallableIdV1::from_index(1),
                    kind: ProductionScopeCallKindV29::Provider,
                },
            ),
            (
                HELPER,
                0,
                1,
                ProductionScopeEventKindV29::Call {
                    callee: DERIVE,
                    kind: ProductionScopeCallKindV29::Derive,
                },
            ),
            (
                HELPER,
                1,
                0,
                ProductionScopeEventKindV29::Call {
                    callee: SemanticCallableIdV1::from_index(2),
                    kind: ProductionScopeCallKindV29::Ordinary,
                },
            ),
        ];
        for block in if branches { vec![3, 4] } else { vec![2] } {
            events.push((HELPER, block, 0, ProductionScopeEventKindV29::Return));
        }
        if assertion {
            events[2] = (HELPER, 1, 0, ProductionScopeEventKindV29::Assert);
            events.push((
                HELPER,
                3,
                0,
                ProductionScopeEventKindV29::Call {
                    callee: SemanticCallableIdV1::from_index(2),
                    kind: ProductionScopeCallKindV29::Ordinary,
                },
            ));
        }
        let events: Vec<_> = events
            .into_iter()
            .map(|(function, block, statement_count, kind)| {
                crate::ProductionScopeEventCandidateV29 {
                    function,
                    block: SemanticBlockIdV1::from_index(block),
                    statement_count,
                    kind,
                }
            })
            .collect();
        let source = ExecutionLifecycleSourceV29::new(
            &owner,
            &launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: owner.source_semantic_sha256(),
                roots: &roots,
                classes: &classes,
                events: &events,
            },
            &mut budget,
        )?;
        if let Fault::Orchestrated { groups, limits, .. } = fault {
            return scoped_root_tests::emit_checked(
                &source,
                &launch,
                roots[0],
                groups,
                limits,
                &mut budget,
            );
        }
        let floor = budget.storage();
        let outcome =
            with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                    with_execution_call_scope_v29(budget, |scope, budget| {
                        let mut sink = ExecutionDefinedCallSinkV29::new(scope, instances, budget)?;
                        let mut signatures = BTreeMap::new();
                        for index in 1..instances.instances().len() {
                            let id = instances.id_at(index).unwrap();
                            let row = instances.instance(id).unwrap();
                            signatures.insert(
                                row.function(),
                                execution_function_signature_v29(instances, id, budget)?,
                            );
                        }
                        let root_plan = kernel_entry_plan_v1(
                            semantic,
                            ROOT,
                            ROOT,
                            FunctionId::new("lifecycle_fixture"),
                            1024,
                            &mut ReachableClosureBudgetV1::new(1024),
                        )?;
                        let mut private = PrivateArrayLazyBudgetV1::new(1, 1024);
                        let placement = SemanticEmissionPlacementV1 {
                            first_block: 0,
                            first_value: 200,
                        };
                        let mut producer = ExecutionLifecycleProducerV29::new(
                            &source,
                            instances,
                            instances.root(),
                            placement,
                            budget,
                        )?;
                        if matches!(fault, Fault::ForeignSource) {
                            producer.pending.source.semantic[0] ^= 1;
                        }
                        if matches!(fault, Fault::ChangedCatalog) {
                            assert!(
                                producer
                                    .require_catalog_entry(
                                        ISSUER,
                                        &semantic.callables()[ISSUER.index() as usize].clone(),
                                        budget
                                    )
                                    .is_err()
                            );
                        }
                        let root = with_execution_availability_v29(
                            instances,
                            instances.root(),
                            budget,
                            |cursor, budget| {
                                lower_one_semantic_function_with_calls_v29(
                                    semantic,
                                    &root_plan,
                                    owner.plan_for_function(ROOT).unwrap(),
                                    &BTreeMap::new(),
                                    &signatures,
                                    Some([64, 1, 1]),
                                    BTreeSet::new(),
                                    1,
                                    false,
                                    1024,
                                    None,
                                    &mut private,
                                    None,
                                    budget,
                                    placement,
                                    Some(cursor),
                                    Some(&mut sink),
                                    if matches!(fault, Fault::MissingConsumer) {
                                        None
                                    } else {
                                        Some(&mut producer)
                                    },
                                )
                            },
                        )?;
                        assert!(producer.finished);
                        assert_eq!(producer.pending.retained_storage, 0);
                        let mut next_value = root.next_value;
                        let mut emitted = vec![None, None, None];
                        emitted[0] = Some(root);
                        while let Some(pending) = sink.pop_pending() {
                            let child = pending.child;
                            let row = instances.instance(child).unwrap();
                            let placement = SemanticEmissionPlacementV1 {
                                first_block: 17 * child.index() as u32,
                                first_value: next_value,
                            };
                            let plan = execution_instance_plan_v29(
                                instances,
                                child,
                                pending.kernel_ir_function,
                                placement,
                                budget,
                            )?;
                            let (_, parameters) = prepare_execution_parameters_v29(
                                instances,
                                child,
                                pending.arguments,
                                &plan,
                                budget,
                            )?;
                            let mut producer = ExecutionLifecycleProducerV29::new(
                                &source, instances, child, placement, budget,
                            )?;
                            let result = with_execution_availability_v29(
                                instances,
                                child,
                                budget,
                                |cursor, budget| {
                                    lower_one_semantic_function_with_calls_v29(
                                        semantic,
                                        &plan,
                                        row.ssa(),
                                        &BTreeMap::new(),
                                        &signatures,
                                        None,
                                        BTreeSet::new(),
                                        1,
                                        false,
                                        1024,
                                        None,
                                        &mut private,
                                        None,
                                        budget,
                                        placement,
                                        Some(cursor.with_call_parameters_v29(parameters)?),
                                        Some(&mut sink),
                                        Some(&mut producer),
                                    )
                                },
                            )?;
                            next_value = result.next_value;
                            emitted[child.index()] = Some(result);
                        }
                        sink.finish(budget)?;
                        if matches!(fault, Fault::SwappedEvents) {
                            let first = emitted[0].as_mut().unwrap().lifecycle_events.take();
                            let second = emitted[1].as_mut().unwrap().lifecycle_events.take();
                            emitted[0].as_mut().unwrap().lifecycle_events = second;
                            emitted[1].as_mut().unwrap().lifecycle_events = first;
                        }
                        let before: Vec<_> = emitted
                            .iter()
                            .map(|row| {
                                row.as_ref()
                                    .unwrap()
                                    .lifecycle_events
                                    .as_ref()
                                    .unwrap()
                                    .rows
                                    .as_ptr()
                            })
                            .collect();
                        let limits = if matches!(fault, Fault::LateLimit) {
                            ProductionSemanticKirLimitsV1::new_with_max_operations(16, 128, 128, 1)
                        } else {
                            ProductionSemanticKirLimitsV1::default()
                        };
                        let assembled = assemble_pending_scoped_root_v29(
                            instances,
                            &mut emitted,
                            limits,
                            budget,
                        )?;
                        assert!(emitted.iter().all(Option::is_none));
                        let mut observations = Vec::new();
                        for (index, sidecar) in assembled.sidecars.rows.iter().enumerate() {
                            let events = sidecar.lifecycle_events.as_ref().unwrap();
                            assert_eq!(events.rows.as_ptr(), before[index]);
                            assert_eq!(
                                events.retained_storage,
                                events.rows.capacity()
                                    * std::mem::size_of::<DeferredLifecycleEventV29>()
                            );
                            observations.extend_from_slice(&events.rows);
                        }
                        Ok(observations)
                    }),
                )
            })
            .map_err(|error| match error {
                production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_call_error_v29(),
            })?;
        // All emission owners have dropped. Test observations are not compiler output.
        budget.release_storage(budget.storage() - floor)?;
        assert_eq!(budget.storage(), floor);
        outcome
    })();
    let peak = budget.peak_storage();
    (result, work.work(), peak)
}

#[test]
fn source_calls_produce_issue_derive_and_each_provider_return_without_entry_seeds() {
    for branches in [false, true] {
        let rows = run_lifecycle(branches, Fault::None, 10_000_000, 10_000_000)
            .0
            .unwrap();
        assert_eq!(rows.len(), if branches { 4 } else { 3 });
        let DeferredLifecycleKindV29::Issue { result: context } = rows[0].kind else {
            panic!("issue");
        };
        let DeferredLifecycleKindV29::Derive {
            context: input,
            result: workgroup,
        } = rows[1].kind
        else {
            panic!("derive");
        };
        assert_eq!(input, context);
        assert_ne!(workgroup.value, context.value);
        for row in &rows[2..] {
            assert_eq!(row.kind, DeferredLifecycleKindV29::End { workgroup });
        }
        assert!(rows.iter().all(|row| row.original_gap == 0));
    }
}

#[test]
fn lifecycle_rejects_missing_consumers_foreign_sources_and_swapped_sidecars() {
    for fault in [
        Fault::MissingConsumer,
        Fault::ForeignSource,
        Fault::SwappedEvents,
        Fault::LateLimit,
    ] {
        assert!(
            run_lifecycle(true, fault, 10_000_000, 10_000_000)
                .0
                .is_err(),
            "{fault:?}"
        );
    }
    assert!(
        run_lifecycle(false, Fault::ChangedCatalog, 10_000_000, 10_000_000)
            .0
            .is_ok()
    );
}

#[test]
fn lifecycle_emission_has_exact_resource_boundaries() {
    let (result, work, peak) = run_lifecycle(true, Fault::None, 10_000_000, 10_000_000);
    result.unwrap();
    assert!(run_lifecycle(true, Fault::None, work, peak).0.is_ok());
    assert!(run_lifecycle(true, Fault::None, work - 1, peak).0.is_err());
    assert!(run_lifecycle(true, Fault::None, work, peak - 1).0.is_err());
    assert!(run_lifecycle(true, Fault::None, 0, peak).0.is_err());
}
