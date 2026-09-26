#[derive(Clone, Copy, Debug)]
enum FailureFixture {
    Shared,
    Direct,
    Abort,
    UnwindTerminate,
    Cleanup,
}

fn terminal_failure_owner(case: FailureFixture) -> ProductionSemanticSsaOwnerV1 {
    let owner = module_fixture_owner(ModuleFixture::LiveAssertion);
    let source = owner.source_semantic();
    let mut types = source.types().to_vec();
    let mut callables = source.callables().to_vec();
    let mut functions = source.functions().to_vec();
    let provider = &functions[2];
    let mut blocks = provider.blocks().to_vec();
    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message,
        ..
    } = blocks[1].terminator().kind().clone()
    else {
        panic!("original provider assertion");
    };
    match case {
        FailureFixture::Shared | FailureFixture::Cleanup => {
            let prior_derive = &blocks[0];
            let derive = SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([95; 32]),
                prior_derive.source(),
                prior_derive.statements().to_vec(),
                prior_derive.terminator().clone(),
            )
            .unwrap();
            assert_eq!(derive.source(), prior_derive.source());
            assert_eq!(derive.statements(), prior_derive.statements());
            assert_eq!(derive.terminator(), prior_derive.terminator());
            let target = blocks.len() as u32;
            blocks[0] = block(
                89,
                vec![],
                SemanticTerminatorKindV1::Assert {
                    condition,
                    expected,
                    message,
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        SemanticBlockIdV1::from_index(target),
                    ),
                    unwind: if matches!(case, FailureFixture::Cleanup) {
                        SemanticUnwindActionV1::Cleanup(SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::AssertUnwind,
                            SemanticBlockIdV1::from_index(2),
                        ))
                    } else {
                        SemanticUnwindActionV1::Unreachable
                    },
                },
            );
            blocks.push(derive);
        }
        FailureFixture::Direct | FailureFixture::Abort | FailureFixture::UnwindTerminate => {
            let returned = blocks.len() as u32;
            let failed = returned + 1;
            assert!(matches!(
                blocks[2].terminator().kind(),
                SemanticTerminatorKindV1::Return
            ));
            blocks[2] = block(
                92,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: condition,
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::SwitchValue,
                                SemanticBlockIdV1::from_index(failed),
                            ),
                        )],
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(returned),
                        ),
                    )
                    .unwrap(),
                },
            );
            blocks.push(block(95, vec![], SemanticTerminatorKindV1::Return));
            let terminator = match case {
                FailureFixture::Abort => SemanticTerminatorKindV1::Abort,
                FailureFixture::UnwindTerminate => SemanticTerminatorKindV1::UnwindTerminate,
                FailureFixture::Direct => {
                    let never = declaration(
                        &mut types,
                        SemanticTypeLayoutV1::with_exact_rustc_layout(
                            0,
                            1,
                            SemanticFieldsShapeV1::Primitive,
                            SemanticRustcVariantsV1::Empty,
                            SemanticBackendReprV1::memory(true),
                            None,
                            true,
                            None,
                            1,
                            0,
                            SemanticTypeLayoutDetailsV1::None,
                        )
                        .unwrap(),
                        SemanticTypeShapeV1::Never,
                        None,
                    );
                    let abi = SemanticFunctionAbiV1::from_rustc(
                        SemanticAbiIdentityV1::from_sha256([174; 32]),
                        SemanticLayoutIdentityV1::from_sha256([250; 32]),
                        SemanticCanonAbiV1::Rust,
                        SemanticExternAbiV1::Rust,
                        false,
                        false,
                        0,
                        vec![],
                        value_abi(&types, never),
                    )
                    .unwrap();
                    let callee = SemanticCallableIdV1::from_index(callables.len() as u32);
                    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
                        binding: SemanticNonBodyCallableBindingV1::new(
                            SemanticFunctionIdentityV1::from_sha256([174; 32]),
                            SemanticItemDefinitionIdentityV1::from_sha256([174; 32]),
                            SemanticMonomorphizationIdentityV1::from_sha256([174; 32]),
                            SemanticGenericTypeArgumentsIdentityV1::from_sha256([174; 32]),
                            SemanticConstGenericArgumentsIdentityV1::from_sha256([174; 32]),
                            SemanticSourceProvenanceV1::unavailable(),
                            abi,
                        ),
                        operation: SemanticCompilerIntrinsicOperationV1::Trap,
                        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(
                            [174; 32],
                        ),
                    });
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            callee,
                            vec![],
                            None,
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    )
                }
                _ => unreachable!(),
            };
            blocks.push(block(96, vec![], terminator));
        }
    }
    // Preserve the CFG's block indices while ordering original identities in
    // their global source roster, before the following callback's identity 115.
    let expected: &[u8] = match case {
        FailureFixture::Shared | FailureFixture::Cleanup => &[89, 91, 92, 93, 94, 95],
        _ => &[90, 91, 92, 93, 94, 95, 96],
    };
    assert_eq!(provider.blocks().len(), 5);
    let appended = if matches!(case, FailureFixture::Shared | FailureFixture::Cleanup) {
        1
    } else {
        2
    };
    assert_eq!(blocks.len(), provider.blocks().len() + appended);
    assert_eq!(&blocks[3..provider.blocks().len()], &provider.blocks()[3..]);
    assert_eq!(blocks.len(), expected.len());
    for (block, tag) in blocks.iter().zip(expected) {
        assert_eq!(
            block.identity(),
            SemanticBlockIdentityV1::from_sha256([*tag; 32])
        );
    }
    assert_eq!(blocks[1], provider.blocks()[1]);
    if matches!(case, FailureFixture::Shared | FailureFixture::Cleanup) {
        assert_eq!(blocks[2], provider.blocks()[2]);
    } else {
        assert_eq!(blocks[0], provider.blocks()[0]);
    }
    functions[2] = function(
        100,
        provider.role(),
        provider.abi().clone(),
        provider.locals().to_vec(),
        blocks,
    );
    assert_eq!(&functions[..2], &source.functions()[..2]);
    assert_eq!(&functions[3..], &source.functions()[3..]);
    assert_eq!(&callables[..source.callables().len()], source.callables());
    let original_blocks: Vec<_> = functions
        .iter()
        .flat_map(|function| function.blocks())
        .collect();
    assert_eq!(
        original_blocks.len(),
        source.functions().iter().map(|function| function.blocks().len()).sum::<usize>()
            + appended
    );
    assert!(original_blocks
        .windows(2)
        .all(|pair| pair[0].identity() < pair[1].identity()));
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(source.target_layout_identity()),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn terminal_source_inputs(
    case: FailureFixture,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ScopedSourceInputsV29, ScopedModuleErrorV29> {
    let owner = terminal_failure_owner(case);
    let (input, launch) = with_module_fixture_view(
        &owner,
        ModuleFixture::LiveAssertion,
        budget,
        |source, budget| OwnedExecutionInputV29::capture(source, budget),
    )?;
    Ok(ScopedSourceInputsV29 {
        owner,
        launch,
        input: input?,
    })
}

fn with_terminal_module(
    case: FailureFixture,
    check: impl FnOnce(&mut SourceOwnedScopedModuleV29, &mut ArgumentBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut donor = Some(terminal_source_inputs(case, &mut budget)
        .unwrap_or_else(|error| panic!("{case:?}: {error:?}")));
    let mut owner = SourceOwnedScopedModuleV29::try_new(
        &mut donor,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap_or_else(|error| panic!("{case:?}: {error:?}"));
    assert!(donor.is_none());
    let floor = budget.storage();
    owner.replay(&mut budget).unwrap();
    check(&mut owner, &mut budget);
    assert_eq!(budget.storage(), floor);
    let retained = owner.retained_storage;
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_terminal_failures_close_only_their_original_failure_edges() {
    for case in [
        FailureFixture::Shared,
        FailureFixture::Direct,
        FailureFixture::Abort,
        FailureFixture::UnwindTerminate,
    ] {
        with_terminal_module(case, |owner, budget| {
            let root = owner
                .pending
                .roots
                .iter()
                .find(|row| row.coordinates.root.index() == 1)
                .unwrap();
            let relation = root.terminal_failures.as_ref().unwrap();
            let body = owner.pending.graph.module().functions[root.function_ordinal]
                .body
                .as_ref()
                .unwrap();
            let locate = |id| body.blocks.iter().find(|block| block.id == id).unwrap();
            let provider: Vec<_> = relation
                .origins
                .rows
                .iter()
                .zip(&relation.closures)
                .filter(|(origin, _)| origin.function.index() == 2)
                .collect();
            assert_eq!(
                provider.len(),
                2,
                "{case:?}: original assertion and added terminal"
            );
            let mut retained_targets = Vec::new();
            for (origin, closure) in &provider {
                let block = locate(closure.block);
                assert_eq!(block.operations.len(), closure.diagnostic as usize + 1);
                assert!(matches!(block.terminator, Some(Terminator::Unreachable)));
                assert!(
                    terminal_failure_is_trap_v18(
                        &block.operations[closure.diagnostic as usize],
                        budget
                    )
                    .unwrap()
                );
                for operation in
                    &block.operations[closure.first as usize..closure.diagnostic as usize]
                {
                    assert!(matches!(
                        operation.kind,
                        OperationKind::Execution(
                            fe2o3_kernel_ir::ExecutionOperationV15::ScopeEnd { .. }
                        )
                    ));
                }
                if let TerminalFailureSiteV18::Edge {
                    block: source,
                    successor,
                    target,
                } = origin.site
                {
                    assert!(closure.generated);
                    assert_eq!(
                        terminal_failure_edge_v18(
                            locate(source).terminator.as_ref().unwrap(),
                            successor
                        )
                        .unwrap()
                        .0,
                        closure.block
                    );
                    let old = locate(target);
                    assert_eq!(old.operations.len(), 1);
                    assert!(matches!(old.terminator, Some(Terminator::Unreachable)));
                    retained_targets.push(target);
                    if let Some((condition, success)) = origin.normal {
                        let Terminator::ConditionalBranch {
                            condition: actual,
                            then_target,
                            else_target,
                            ..
                        } = locate(source).terminator.as_ref().unwrap()
                        else {
                            panic!("failure branch");
                        };
                        assert_eq!(*actual, condition);
                        assert_eq!(
                            if successor == 0 {
                                *else_target
                            } else {
                                *then_target
                            },
                            success
                        );
                    }
                } else {
                    assert!(!closure.generated);
                }
            }
            if matches!(case, FailureFixture::Shared) {
                assert_eq!(retained_targets.len(), 2);
                assert_eq!(retained_targets[0], retained_targets[1]);
                assert_ne!(provider[0].1.block, provider[1].1.block);
                assert_eq!(
                    provider
                        .iter()
                        .map(|(_, row)| row.scope_ends)
                        .collect::<Vec<_>>(),
                    [0, 1]
                );
            } else {
                assert!(provider.iter().all(|(_, row)| row.scope_ends == 1));
            }
        });
    }
}

#[test]
fn terminal_relation_replay_rejects_omission_reordering_foreign_source_and_wrong_edge() {
    for fault in 0..8 {
        with_terminal_module(FailureFixture::Shared, |owner, budget| {
            let root = owner
                .pending
                .roots
                .iter_mut()
                .find(|row| row.coordinates.root.index() == 1)
                .unwrap();
            let relation = root.terminal_failures.as_mut().unwrap();
            assert_eq!(relation.origins.rows.len(), 2);
            match fault {
                0 => {
                    relation.origins.rows.pop();
                    relation.closures.pop();
                }
                1 => relation.origins.rows.swap(0, 1),
                2 => relation.origins.source.semantic[0] ^= 1,
                3 => relation.closures[1].origin = 0,
                4 => relation.closures[1].scope_ends = 0,
                5 => relation.origins.rows[1].instance = relation.origins.rows[0].instance,
                6 => {
                    if let TerminalFailureSiteV18::Edge { successor, .. } =
                        &mut relation.origins.rows[1].site
                    {
                        *successor ^= 1;
                    }
                }
                7 => {
                    relation.origins.rows[1].normal.as_mut().unwrap().1 = relation.closures[1].block
                }
                _ => unreachable!(),
            }
            // Both source assertions belong to the same provider instance;
            // substitute the original root, not an equivalent sibling value.
            if fault == 5 {
                relation.origins.rows[1].instance = root.coordinates.sources.rows[0].instance;
            }
            let floor = budget.storage();
            assert!(owner.replay(budget).is_err(), "fault {fault}");
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn actual_cleanup_unwind_is_not_a_terminal_failure_permission() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let source = terminal_source_inputs(FailureFixture::Cleanup, &mut budget);
    match source {
        Err(_) => assert_eq!(budget.storage(), MODULE_FLOOR),
        Ok(source) => {
            let mut donor = Some(source);
            assert!(
                SourceOwnedScopedModuleV29::try_new(
                    &mut donor,
                    ProductionSemanticKirLimitsV1::default(),
                    &mut budget
                )
                .is_err()
            );
            assert!(
                donor.is_none(),
                "original owner was consumed before source validation"
            );
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn terminal_rewrite_does_not_relax_live_scope_diagnostic_or_unreachable_verification() {
    for fault in 0..3 {
        with_terminal_module(FailureFixture::Shared, |owner, budget| {
            let root = owner
                .pending
                .roots
                .iter()
                .find(|row| row.coordinates.root.index() == 1)
                .unwrap();
            let closure = *root
                .terminal_failures
                .as_ref()
                .unwrap()
                .closures
                .iter()
                .find(|row| row.scope_ends == 1)
                .unwrap();
            let mut graph = owner.pending.graph.module().clone();
            let block = graph.functions[root.function_ordinal]
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .find(|block| block.id == closure.block)
                .unwrap();
            assert_eq!(block.operations.len(), 2);
            match fault {
                0 => {
                    block.operations.remove(0);
                }
                1 => block.operations.clear(),
                2 => {
                    let unowned_call = block.operations[1].clone();
                    block.operations.insert(0, unowned_call);
                }
                _ => unreachable!(),
            }
            let floor = budget.storage();
            let result = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &graph, ProductionSemanticKirLimitsV1::default().storage_layout_limits(), budget);
            assert!(result.is_err(), "fault {fault}");
            drop(result);
            assert_eq!(budget.storage(), floor);
        });
    }
}
