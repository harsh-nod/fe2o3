#[derive(Clone, Copy)]
enum PrivateEntryFixtureV20 {
    Forward,
    Constant,
    Arithmetic,
    Phi,
    Captured,
    Neutral,
    NonNeutral,
    NonNeutralDeadAddress,
}

#[path = "production_source_private_writes_v22_tests.rs"]
mod source_writes_v22;

#[path = "production_source_private_spill_v25_tests.rs"]
mod source_spills_v25;

#[path = "production_source_private_raw_dereference_v26_tests.rs"]
mod source_raw_dereference_v26;

#[path = "production_source_scalar_boundaries_v31_tests.rs"]
mod scalar_boundaries_v31;

fn private_entry_owner_v20(case: PrivateEntryFixtureV20) -> ProductionSemanticSsaOwnerV1 {
    let base = module_fixture_owner(ModuleFixture::Ordinary);
    let semantic = base.source_semantic();
    let mut types = semantic.types()[..2].to_vec();
    let pointer = reference(&mut types, U32, SemanticMutabilityV1::Immutable, true);
    let unit = || {
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        )))
    };
    let captured = || {
        // The integer-only fixtures keep the formed address live so unrelated
        // pointer DCE cannot turn the non-neutral case into a changed output.
        let read = if matches!(
            case,
            PrivateEntryFixtureV20::Neutral | PrivateEntryFixtureV20::NonNeutral
        ) {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap(),
                ],
                U32,
            )
            .unwrap()
        } else {
            place(1, U32)
        };
        let mut statements = vec![
            assign(
                place(2, pointer),
                SemanticRvalueKindV1::AddressOf {
                    place: place(1, U32),
                    mutability: SemanticMutabilityV1::Immutable,
                },
            ),
            assign(
                place(3, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read)),
            ),
        ];
        if matches!(
            case,
            PrivateEntryFixtureV20::Neutral
                | PrivateEntryFixtureV20::NonNeutral
                | PrivateEntryFixtureV20::NonNeutralDeadAddress
        ) {
            statements.push(assign(
                place(1, U32),
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: SemanticOperandV1::Copy(place(3, U32)),
                    right: literal(if matches!(case, PrivateEntryFixtureV20::Neutral) {
                        0
                    } else {
                        1
                    }),
                },
            ));
        }
        statements.push(assign(place(0, UNIT), unit()));
        statements
    };
    let captured_locals = |tag| {
        vec![
            local(tag, UNIT, SemanticLocalRoleV1::Return),
            local(tag + 1, U32, SemanticLocalRoleV1::Argument(0)),
            local(tag + 2, pointer, SemanticLocalRoleV1::Temporary),
            local(tag + 3, U32, SemanticLocalRoleV1::Temporary),
        ]
    };
    let forwarding_locals = |tag| {
        vec![
            local(tag, UNIT, SemanticLocalRoleV1::Return),
            local(tag + 1, U32, SemanticLocalRoleV1::Argument(0)),
            local(tag + 2, U32, SemanticLocalRoleV1::Temporary),
        ]
    };
    let call = |callee, operand, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                vec![operand],
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
    };
    let rhs = match case {
        PrivateEntryFixtureV20::Forward
        | PrivateEntryFixtureV20::Phi
        | PrivateEntryFixtureV20::Captured
        | PrivateEntryFixtureV20::Neutral
        | PrivateEntryFixtureV20::NonNeutral
        | PrivateEntryFixtureV20::NonNeutralDeadAddress => {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32)))
        }
        PrivateEntryFixtureV20::Constant => SemanticRvalueKindV1::Use(literal(19)),
        PrivateEntryFixtureV20::Arithmetic => SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left: literal(7),
            right: literal(9),
        },
    };
    let root_blocks = if matches!(case, PrivateEntryFixtureV20::Phi) {
        let jump = || {
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(3),
            ))
        };
        vec![
            block(
                80,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(1, U32)),
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
                            SemanticBlockIdV1::from_index(2),
                        ),
                    )
                    .unwrap(),
                },
            ),
            block(
                81,
                vec![assign(place(2, U32), SemanticRvalueKindV1::Use(literal(7)))],
                jump(),
            ),
            block(
                82,
                vec![assign(place(2, U32), SemanticRvalueKindV1::Use(literal(9)))],
                jump(),
            ),
            block(
                83,
                vec![],
                call(2, SemanticOperandV1::Move(place(2, U32)), 4),
            ),
            block(84, vec![], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![
            block(
                80,
                vec![assign(place(2, U32), rhs)],
                call(2, SemanticOperandV1::Move(place(2, U32)), 1),
            ),
            block(81, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    let root = function(
        60,
        SemanticFunctionRoleV1::KernelRoot,
        abi(61, true, &[U32])
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
            .unwrap(),
        forwarding_locals(70),
        root_blocks,
    )
    .with_kernel_entry(semantic.functions()[0].kernel_entry().unwrap().clone());
    let companion = function(
        90,
        SemanticFunctionRoleV1::KernelRoot,
        abi(91, true, &[U32])
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
            .unwrap(),
        captured_locals(100),
        vec![block(110, captured(), SemanticTerminatorKindV1::Return)],
    )
    .with_kernel_entry(semantic.functions()[1].kernel_entry().unwrap().clone());
    let helper = if matches!(case, PrivateEntryFixtureV20::Captured) {
        function(
            120,
            SemanticFunctionRoleV1::InternalHelper,
            abi(121, false, &[U32]),
            captured_locals(130),
            vec![
                block(
                    140,
                    captured(),
                    call(3, SemanticOperandV1::Copy(place(1, U32)), 1),
                ),
                block(141, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
    } else {
        function(
            120,
            SemanticFunctionRoleV1::InternalHelper,
            abi(121, false, &[U32]),
            forwarding_locals(130),
            vec![
                block(
                    140,
                    vec![assign(
                        place(2, U32),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, U32))),
                    )],
                    call(3, SemanticOperandV1::Copy(place(2, U32)), 1),
                ),
                block(141, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
    };
    let leaf = function(
        150,
        SemanticFunctionRoleV1::InternalHelper,
        abi(151, false, &[U32]),
        captured_locals(160),
        vec![block(170, captured(), SemanticTerminatorKindV1::Return)],
    );
    let functions = vec![root, companion, helper, leaf];
    let callables = (0..functions.len())
        .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i as u32)))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
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

fn private_entry_forward_owner_v20() -> ProductionSemanticSsaOwnerV1 {
    private_entry_owner_v20(PrivateEntryFixtureV20::Forward)
}
fn private_entry_constant_owner_v20() -> ProductionSemanticSsaOwnerV1 {
    private_entry_owner_v20(PrivateEntryFixtureV20::Constant)
}
fn private_entry_arithmetic_owner_v20() -> ProductionSemanticSsaOwnerV1 {
    private_entry_owner_v20(PrivateEntryFixtureV20::Arithmetic)
}

fn private_entry_phi_owner_v20() -> ProductionSemanticSsaOwnerV1 {
    private_entry_owner_v20(PrivateEntryFixtureV20::Phi)
}

fn private_entry_captured_owner_v20() -> ProductionSemanticSsaOwnerV1 {
    private_entry_owner_v20(PrivateEntryFixtureV20::Captured)
}

fn private_entry_neutral_owner_v20() -> ProductionSemanticSsaOwnerV1 {
    private_entry_owner_v20(PrivateEntryFixtureV20::Neutral)
}

fn private_entry_non_neutral_owner_v20() -> ProductionSemanticSsaOwnerV1 {
    private_entry_owner_v20(PrivateEntryFixtureV20::NonNeutral)
}

fn private_entry_dead_address_owner_v20() -> ProductionSemanticSsaOwnerV1 {
    private_entry_owner_v20(PrivateEntryFixtureV20::NonNeutralDeadAddress)
}

fn private_entry_root_owner_v20() -> ProductionSemanticSsaOwnerV1 {
    let base = typed_root_entry_rhs_owner_v18();
    let source = base.source_semantic();
    let old = &source.functions()[0];
    let root = function(
        60,
        SemanticFunctionRoleV1::KernelRoot,
        abi(61, true, &[U32])
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
            .unwrap(),
        old.locals().to_vec(),
        old.blocks().to_vec(),
    )
    .with_kernel_entry(old.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
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

fn private_entry_typed_memory_census_v20(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
) -> [usize; 3] {
    let mut counts = [0; 3];
    for operation in owner
        .module()
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        match operation.kind {
            OperationKind::Alloca { .. } => counts[0] += 1,
            OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::ReadValue { .. }) => {
                counts[1] += 1
            }
            OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::WriteValue { .. }) => {
                counts[2] += 1
            }
            _ => {}
        }
    }
    counts
}

// Failure-only bounded diagnostic for these fixed source fixtures. This does
// not rewrite the owner or change the asserted changed/no-op classification.
fn private_entry_integer_diagnostic_v20(
    label: &str,
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
) {
    eprintln!(
        "private integer {label} typed memory {:?}",
        private_entry_typed_memory_census_v20(owner)
    );
    let mut remaining = 32;
    for (fi, function) in owner.module().functions.iter().enumerate() {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for (oi, operation) in block.operations.iter().enumerate() {
                if remaining == 0 {
                    eprintln!("private integer {label} operation limit");
                    return;
                }
                remaining -= 1;
                eprintln!(
                    "private integer {label} function={fi} block={:?} operation={oi} kind={:?} results={:?}",
                    block.id, operation.kind, operation.results
                );
                let OperationKind::Binary { lhs, rhs, .. } = operation.kind else {
                    continue;
                };
                for (side, value) in [("lhs", lhs), ("rhs", rhs)] {
                    let producer =
                        body.blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find(|operation| {
                                operation.results.iter().any(|result| result.id == value)
                            });
                    match producer.map(|operation| &operation.kind) {
                        Some(OperationKind::Constant(constant)) => eprintln!(
                            "private integer {label} {side}={value:?} constant={constant:?}"
                        ),
                        Some(kind) => {
                            eprintln!("private integer {label} {side}={value:?} producer={kind:?}")
                        }
                        None => eprintln!("private integer {label} {side}={value:?} parameter"),
                    }
                }
            }
        }
    }
}

#[test]
fn private_source_completion_owns_actual_integer_output_for_all_roots_and_nested_helpers() {
    for factory in [
        private_entry_forward_owner_v20 as fn() -> _,
        private_entry_constant_owner_v20,
        private_entry_captured_owner_v20,
        private_entry_root_owner_v20,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(factory, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let floor = budget.storage();
                let handoff = source.private_completed_integer_output_v20(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                let output = handoff.output(budget)?;
                assert_eq!(
                    output.input_audit_bytes(),
                    source.canonical(budget)?.canonical_bytes()
                );
                assert_eq!(output.owner().module().kernels.len(), roots.len());
                assert_eq!(output.report().passes().len(), 2);
                assert!(
                    private_entry_typed_memory_census_v20(output.owner())
                        .into_iter()
                        .all(|count| count > 0)
                );
                handoff.discard(budget)?;
                assert_eq!(budget.storage(), floor);
                completed.set(true);
                Ok::<_, ProductionPrivateSourceHandoffErrorV20>(())
            })
            .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn private_source_completion_does_not_guess_arithmetic_helper_arguments() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_arithmetic_owner_v20, &mut budget);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let error = source
            .private_completed_integer_output_v20(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )
            .err()
            .expect("unsupported arithmetic must refuse");
        assert!(
            error
                .to_string()
                .contains("private entry original expression requires unsupported derivation"),
            "{error}"
        );
        completed.set(true);
        Err::<(), _>(error)
    });
    assert!(result.is_err());
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_source_completion_checks_genuine_phi_without_selecting_a_predecessor() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_phi_owner_v20, &mut budget);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let owner = source.source_ssa(budget)?;
        let captured = owner.occurrences_v1().unwrap();
        let function = captured
            .function(SemanticFunctionIdV1::from_index(0))
            .unwrap();
        assert!(function.events().iter().any(|event| matches!(
            event.resolved(),
            Some(EntryEventV20::Use {
                value: EntryValueV20::BlockArgument { .. },
                ..
            })
        )));
        let floor = budget.storage();
        let handoff = source.private_completed_integer_output_v20(
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            budget,
        )?;
        handoff.check_original_source(source.source_ssa(budget)?, budget)?;
        handoff.discard(budget)?;
        assert_eq!(budget.storage(), floor);
        completed.set(true);
        Ok::<(), ProductionPrivateSourceHandoffErrorV20>(())
    });
    result.unwrap();
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_source_completion_retains_real_changed_and_noop_integer_owners() {
    for (factory, changed) in [
        (private_entry_neutral_owner_v20 as fn() -> _, true),
        (private_entry_non_neutral_owner_v20, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) = integer_handoff_prepared_v18(factory, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                assert!(
                    private_entry_typed_memory_census_v20(source.canonical(budget)?)
                        .into_iter()
                        .all(|count| count > 0)
                );
                let handoff = source.private_completed_integer_output_v20(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )?;
                let output = handoff.output(budget)?;
                assert!(
                    private_entry_typed_memory_census_v20(output.owner())
                        .into_iter()
                        .all(|count| count > 0)
                );
                if output.report().passes()[0].changed() != changed
                    || (output.owner().canonical_bytes() != output.input_audit_bytes()) != changed
                {
                    eprintln!(
                        "private integer expected changed={changed} input_bytes={} output_bytes={}",
                        output.input_audit_bytes().len(),
                        output.owner().canonical_bytes().len()
                    );
                    for (ordinal, pass) in output.report().passes().iter().enumerate() {
                        eprintln!(
                            "private integer pass={ordinal} kind={:?} changed={}",
                            pass.pass(),
                            pass.changed()
                        );
                    }
                    private_entry_integer_diagnostic_v20("input", source.canonical(budget)?);
                    private_entry_integer_diagnostic_v20("output", output.owner());
                }
                assert_eq!(output.report().passes()[0].changed(), changed);
                assert_eq!(
                    output.owner().canonical_bytes() != output.input_audit_bytes(),
                    changed
                );
                assert_eq!(output.map().output_identity(), output.owner().identity());
                assert!(!output.grants_authority());
                handoff.discard(budget)?;
                completed.set(true);
                Ok::<_, ProductionPrivateSourceHandoffErrorV20>(())
            })
            .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn private_source_completion_distinguishes_dead_address_dce_from_integer_rewrites() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_dead_address_owner_v20, &mut budget);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let restrict_count = |owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18| {
        owner
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Cast {
                        kind: fe2o3_kernel_ir::CastKind::RestrictPointerAccess,
                        ..
                    }
                )
            })
            .count()
    };
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| {
            let input = source.canonical(budget)?;
            assert_eq!(restrict_count(input), 2);
            assert_eq!(private_entry_typed_memory_census_v20(input), [2, 2, 4]);
            let handoff = source.private_completed_integer_output_v20(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let output = handoff.output(budget)?;
            assert_eq!(output.report().passes().len(), 2);
            assert!(!output.report().passes()[0].changed());
            assert!(output.report().passes()[1].changed());
            assert_ne!(output.owner().canonical_bytes(), output.input_audit_bytes());
            assert_eq!(restrict_count(output.owner()), 0);
            assert_eq!(
                private_entry_typed_memory_census_v20(output.owner()),
                [2, 2, 4]
            );
            assert_eq!(output.map().output_identity(), output.owner().identity());
            assert!(!output.grants_authority());
            handoff.discard(budget)?;
            completed.set(true);
            Ok::<_, ProductionPrivateSourceHandoffErrorV20>(())
        })
        .unwrap();
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_source_completion_rejects_incomplete_independent_abi_before_adoption() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_forward_owner_v20, &mut budget);
    let roots = fixture.roots();
    assert_eq!(roots.len(), 2);
    let completed = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let error = source
            .private_completed_integer_output_v20(
                ProductionKernelArgumentAbiInputV18 { roots: &roots[..1] },
                budget,
            )
            .err()
            .expect("missing companion root must refuse");
        assert!(
            matches!(
                error,
                ProductionPrivateSourceHandoffErrorV20::Check(
                    ProductionPrivateSourceCheckErrorV20::Source(
                        ProductionSourceOwnedViewErrorV18::Binding("kernel argument ABI profile differs from its original descriptor/source contract")
                    )
                )
            ),
            "{error:?}"
        );
        completed.set(true);
        Err::<(), _>(error)
    });
    assert!(result.is_err());
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_source_completion_keeps_original_owner_and_ledger_custody() {
    for foreign_ledger in [false, true] {
        let foreign = private_entry_forward_owner_v20();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let (prepared, fixture) =
            integer_handoff_prepared_v18(private_entry_forward_owner_v20, &mut budget);
        let roots = fixture.roots();
        let completed = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let handoff = source.private_completed_integer_output_v20(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )?;
            let paid_before_refusal = budget.storage();
            let error = if foreign_ledger {
                let mut other_work =
                    CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                let mut other = ArgumentBudgetV1::new(&mut other_work, MODULE_LIMIT);
                let error = handoff
                    .output(&mut other)
                    .err()
                    .expect("foreign ledger must refuse");
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ));
                assert_eq!((other.work(), other.storage()), (0, 0));
                error
            } else {
                assert_eq!(source.source_ssa(budget)?.identity(), foreign.identity());
                assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
                handoff.check_original_source(&foreign, budget).unwrap_err()
            };
            assert_eq!(
                budget.storage(),
                paid_before_refusal,
                "refusal cannot refund a live output"
            );
            let selected = error.to_string();
            assert_eq!(handoff.output(budget).err().unwrap().to_string(), selected);
            let _ = handoff.discard(budget);
            completed.set(true);
            Err::<(), ProductionPrivateSourceHandoffErrorV20>(error.into())
        });
        assert!(result.is_err());
        assert!(completed.get());
        if !foreign_ledger {
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn private_source_completion_header_refusal_is_first_and_sticky() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let (prepared, fixture) =
        integer_handoff_prepared_v18(private_entry_forward_owner_v20, &mut budget);
    let roots = fixture.roots();
    let completed = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let floor = budget.storage();
        let padding = MODULE_LIMIT - floor;
        budget.reserve_storage(padding)?;
        let error = source
            .private_completed_integer_output_v20(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                budget,
            )
            .err()
            .expect("helper header must refuse");
        assert!(
            matches!(
                error,
                ProductionPrivateSourceHandoffErrorV20::Check(
                    ProductionPrivateSourceCheckErrorV20::Source(
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(_))
                    )
                )
            ),
            "{error:?}"
        );
        let selected = error.to_string();
        budget.release_storage(padding)?;
        let before = (budget.work(), budget.storage());
        assert_eq!(
            source
                .private_completed_integer_output_v20(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget
                )
                .err()
                .unwrap()
                .to_string(),
            selected
        );
        assert_eq!((budget.work(), budget.storage()), before);
        completed.set(true);
        Err::<(), _>(error)
    });
    assert!(result.is_err());
    assert!(completed.get());
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn private_source_entry_completion_checks_rhs_instead_of_trusting_resolver_success() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget
        .reserve_storage(MODULE_FLOOR + private_source_completion_headers_v20().unwrap())
        .unwrap();
    let prepared =
        private_memory_prepared_v18(private_entry_constant_owner_v20, &mut budget).unwrap();
    let completed = std::cell::Cell::new(false);
    let result = with_production_optimizer_result_v18(
        prepared,
        &mut budget,
        |original, optimized, budget| {
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                let index = OriginalEntryIndexV20::build(original, budget)?;
                original.with_optimized_source_scalar_leaves_v18(
                    optimized,
                    0,
                    budget,
                    |leaves, budget| {
                        leaves.with_checked_entry_writes_v18(
                            budget,
                            |entry, budget| {
                                let actual = index.expression(leaves, entry, budget)?;
                                assert!(matches!(
                                    actual,
                                    ProductionSemanticExpressionV2::Constant { bits: 19, .. }
                                ));
                                let wrong = ProductionSemanticExpressionV2::Constant {
                                    scalar: actual.scalar(),
                                    bits: 20,
                                };
                                let error = entry.check_expression(&wrong, budget).unwrap_err();
                                assert!(
                                    matches!(error, ProductionSourceOwnedViewErrorV18::Binding("actual scalar expression differs from its original source value")),
                                    "{error:?}"
                                );
                                completed.set(true);
                                Err(error)
                            },
                            |_, _| panic!("wrong original expression completed entry roster"),
                        )
                    },
                )
            })
        },
    );
    assert!(result.is_err());
    assert!(completed.get());
    assert_eq!(
        budget.storage(),
        MODULE_FLOOR + private_source_completion_headers_v20().unwrap()
    );
}

fn private_entry_index_cut_v20(cut: Option<(bool, usize)>) -> (usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let headers = private_source_completion_headers_v20().unwrap();
    budget.reserve_storage(MODULE_FLOOR + headers).unwrap();
    let prepared =
        private_memory_prepared_v18(private_entry_forward_owner_v20, &mut budget).unwrap();
    let completed = std::cell::Cell::new(None);
    let settled = std::cell::Cell::new(false);
    let result =
        with_production_optimizer_result_v18(prepared, &mut budget, |original, _, budget| {
            let floor = budget.storage();
            let result = source_scalar_normalization_scratch_v18(
                original.source.cleanup,
                budget,
                0,
                |budget| {
                    // Test-owned padding establishes an exact remaining budget after the
                    // genuine source/optimizer entry. It is not a production cap override.
                    if let Some((is_work, remaining)) = cut {
                        if is_work {
                            budget.charge_work(
                                OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - remaining,
                            )?;
                        } else {
                            budget.reserve_storage(MODULE_LIMIT - budget.storage() - remaining)?;
                        }
                    }
                    let before = (budget.work(), budget.storage());
                    match OriginalEntryIndexV20::build(original, budget) {
                        Ok(index) => {
                            let units = budget.work() - before.0;
                            let bytes = budget.storage() - before.1;
                            let expected_bytes = index.definitions.capacity()
                                * size_of::<OriginalEntryDefinitionRowV20>();
                            assert_eq!(bytes, expected_bytes);
                            assert!(!index.definitions.is_empty());
                            drop(index);
                            completed.set(Some((units, bytes, true)));
                            // Stop before any later query can consume a unit belonging to
                            // this constructor's exact-work boundary.
                            Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "selected index boundary stop",
                            ))
                        }
                        Err(error) => {
                            let Some((is_work, _)) = cut else {
                                panic!("unbounded original index refused: {error:?}")
                            };
                            match (is_work, &error) {
                                (
                                    true,
                                    ProductionSourceOwnedViewErrorV18::Resource(
                                        ArgumentResourceV1::Work(refusal),
                                    ),
                                ) => assert!(refusal.actual() > refusal.limit()),
                                (
                                    false,
                                    ProductionSourceOwnedViewErrorV18::Resource(
                                        ArgumentResourceV1::Storage(refusal),
                                    ),
                                ) => assert!(refusal.actual() > refusal.limit()),
                                _ => panic!("wrong exact index boundary: {error:?}"),
                            }
                            let after = (budget.work(), budget.storage());
                            assert_eq!(
                                OriginalEntryIndexV20::build(original, budget)
                                    .err()
                                    .unwrap()
                                    .to_string(),
                                error.to_string()
                            );
                            assert_eq!((budget.work(), budget.storage()), after);
                            completed.set(Some((0, 0, false)));
                            Err(error)
                        }
                    }
                },
            );
            assert_eq!(budget.storage(), floor);
            settled.set(true);
            result
        });
    assert!(result.is_err());
    assert_eq!(budget.storage(), MODULE_FLOOR + headers);
    assert!(
        settled.get(),
        "lexical index and padding credit must settle after all backing drops"
    );
    completed
        .get()
        .expect("index assertions must complete outside caught callbacks")
}

#[test]
fn private_source_entry_index_exact_and_one_short_work_and_storage_are_cumulative() {
    let (work, storage, accepted) = private_entry_index_cut_v20(None);
    assert!(accepted && work > 0 && storage > 0);
    for (is_work, limit) in [(true, work), (false, storage)] {
        assert!(private_entry_index_cut_v20(Some((is_work, limit))).2);
        assert!(!private_entry_index_cut_v20(Some((is_work, limit - 1))).2);
    }
}
#[path = "production_source_bound_private_handoff_v21_tests.rs"]
mod bound_private_tests;
