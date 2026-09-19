#[derive(Clone, Copy, Debug)]
enum ModuleFixture {
    Mixed,
    Array,
    LiveAssertion,
    Ordinary,
}

fn module_fixture_owner(kind: ModuleFixture) -> ProductionSemanticSsaOwnerV1 {
    let original = match kind {
        ModuleFixture::LiveAssertion => assertion_owner(),
        ModuleFixture::Array => fixtures::array_owner(false),
        _ => lifecycle_owner(false),
    };
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let boolean = types
        .iter()
        .position(|ty| {
            matches!(
                ty.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
            )
        })
        .map(|index| SemanticTypeIdV1::from_index(index as u32))
        .unwrap_or_else(|| {
            declaration(
                &mut types,
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(1),
                    1,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 8, 1),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
                None,
            )
        });
    let ordinary = |tag: u8, name: &[u8]| {
        function(
            tag,
            SemanticFunctionRoleV1::KernelRoot,
            abi(tag + 1, true, &[U32]),
            vec![
                local(tag + 2, UNIT, SemanticLocalRoleV1::Return),
                local(tag + 3, U32, SemanticLocalRoleV1::Argument(0)),
            ],
            vec![
                block(
                    tag + 4,
                    vec![],
                    SemanticTerminatorKindV1::Assert {
                        condition: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            boolean,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(1, 1).unwrap(),
                            ),
                        )),
                        expected: true,
                        message: SemanticAssertMessageV1::NullPointerDereference,
                        target: SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::AssertSuccess,
                            SemanticBlockIdV1::from_index(1),
                        ),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(tag + 5, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(name.to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([tag + 8; 32]),
            semantic.functions()[0]
                .kernel_entry()
                .unwrap()
                .source_contract()
                .clone(),
        ))
    };
    let mut functions = vec![ordinary(60, b"z_ordinary")];
    if !matches!(kind, ModuleFixture::Ordinary) {
        for (index, prior) in semantic.functions().iter().enumerate() {
            let blocks = prior
                .blocks()
                .iter()
                .enumerate()
                .map(|(block_index, prior)| {
                    let terminator = match prior.terminator().kind() {
                        SemanticTerminatorKindV1::Call(call) => {
                            let callee = match call.callee().index() {
                                0..=2 => call.callee().index() + 1,
                                3 => 5,
                                4 => 6,
                                _ => panic!("unexpected fixture callable"),
                            };
                            SemanticTerminatorKindV1::Call(
                                SemanticDirectCallV1::new_callable(
                                    SemanticCallableIdV1::from_index(callee),
                                    call.arguments().to_vec(),
                                    call.destination().cloned(),
                                    call.unwind(),
                                )
                                .unwrap(),
                            )
                        }
                        other => other.clone(),
                    };
                    block(
                        [85, 90, 115][index] + block_index as u8,
                        prior.statements().to_vec(),
                        terminator,
                    )
                })
                .collect();
            let mut copied = function(
                [80, 100, 110][index],
                prior.role(),
                prior.abi().clone(),
                prior.locals().to_vec(),
                blocks,
            );
            if let Some(entry) = prior.kernel_entry() {
                copied = copied.with_kernel_entry(entry.clone());
            }
            functions.push(copied);
        }
    }
    functions.push(ordinary(150, b"a_ordinary"));
    let roots = if matches!(kind, ModuleFixture::Ordinary) {
        vec![0, 1]
    } else {
        vec![0, 1, 4]
    };
    let mut callables: Vec<_> = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    if !matches!(kind, ModuleFixture::Ordinary) {
        callables.extend_from_slice(&semantic.callables()[3..]);
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        roots
            .into_iter()
            .map(SemanticFunctionIdV1::from_index)
            .collect(),
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

fn with_module_fixture<R>(
    kind: ModuleFixture,
    budget: &mut ArgumentBudgetV1<'_>,
    use_source: impl FnOnce(&ExecutionLifecycleSourceV29<'_>, &mut ArgumentBudgetV1<'_>) -> R,
) -> Result<R, ScopedModuleErrorV29> {
    let mut owner = module_fixture_owner(kind);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(budget)
        .map_err(|error| match error {
            fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error) => {
                ScopedModuleErrorV29::from(error)
            }
            _ => ScopedModuleErrorV29::from(execution_call_error_v29()),
        })?;
    budget.reserve_storage(capture.retained_storage())?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let semantic = owner.source_semantic();
        let launch_inputs: Vec<_> = semantic
            .roots()
            .iter()
            .enumerate()
            .map(|(ordinal, root)| {
                let entry = semantic.functions()[root.index() as usize]
                    .kernel_entry()
                    .unwrap();
                ProductionSourceLaunchRootInputV1::new(
                    std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                    *entry.kernel_binding_identity().as_bytes(),
                    ProductionSourceLaunchInputV1::new(
                        1,
                        Some([64, 1, 1]),
                        [ordinal as u32 + 1, 1, 1],
                    ),
                )
            })
            .collect();
        let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &launch_inputs).unwrap();
        let mut roots = Vec::new();
        let mut classes =
            vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
        let mut events = Vec::new();
        if !matches!(kind, ModuleFixture::Ordinary) {
            let SemanticTerminatorKindV1::Call(helper) =
                semantic.functions()[1].blocks()[1].terminator().kind()
            else {
                panic!("helper");
            };
            roots.push(RootInput {
                semantic_sha256: owner.source_semantic_sha256(),
                root: SemanticFunctionIdV1::from_index(1),
                root_identity: semantic.functions()[1].identity(),
                helper: SemanticFunctionIdV1::from_index(2),
                helper_identity: semantic.functions()[2].identity(),
                issuer: SemanticCallableIdV1::from_index(5),
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
            });
            classes[2] = ProductionScopeCallableCandidateV29::Provider {
                function: SemanticFunctionIdV1::from_index(2),
                identity: semantic.functions()[2].identity(),
            };
            classes[6] = ProductionScopeCallableCandidateV29::Derive {
                binding: SemanticFunctionIdentityV1::from_sha256([121; 32]),
                operation: SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32]),
                context: CONTEXT,
                workgroup: semantic.functions()[3].abi().source_input_types()[0],
            };
            for function_index in [1, 2] {
                for (block_index, block) in semantic.functions()[function_index]
                    .blocks()
                    .iter()
                    .enumerate()
                {
                    let kind = match block.terminator().kind() {
                        SemanticTerminatorKindV1::Call(call) if call.callee().index() == 2 => {
                            ProductionScopeEventKindV29::Call {
                                callee: call.callee(),
                                kind: ProductionScopeCallKindV29::Provider,
                            }
                        }
                        SemanticTerminatorKindV1::Call(call) if function_index == 2 => {
                            ProductionScopeEventKindV29::Call {
                                callee: call.callee(),
                                kind: if call.callee().index() == 6 {
                                    ProductionScopeCallKindV29::Derive
                                } else {
                                    ProductionScopeCallKindV29::Ordinary
                                },
                            }
                        }
                        SemanticTerminatorKindV1::Return if function_index == 2 => {
                            ProductionScopeEventKindV29::Return
                        }
                        SemanticTerminatorKindV1::Assert { .. } if function_index == 2 => {
                            ProductionScopeEventKindV29::Assert
                        }
                        _ => continue,
                    };
                    events.push(crate::ProductionScopeEventCandidateV29 {
                        function: SemanticFunctionIdV1::from_index(function_index as u32),
                        block: SemanticBlockIdV1::from_index(block_index as u32),
                        statement_count: block.statements().len(),
                        kind,
                    });
                }
            }
        }
        let source = ExecutionLifecycleSourceV29::new(
            &owner,
            &launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: owner.source_semantic_sha256(),
                roots: &roots,
                classes: &classes,
                events: &events,
            },
            budget,
        )?;
        Ok::<_, ScopedModuleErrorV29>(use_source(&source, budget))
    }));
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}
