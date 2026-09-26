use super::*;

include!("production_execution_identity_emission_v1_tests.rs");

mod function_frame_tests {
    use super::*;
    include!("production_function_frame_v1_tests.rs");
}

mod fresh_stack_tests {
    use super::*;
    include!("production_scoped_function_stack_v1_tests.rs");
}

const LIMIT: usize = 20_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SuffixCase {
    Ordinary,
    Cells,
    Array,
    Root,
    Loop,
    CyclicEntry,
    Branch,
}

fn literal(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

fn store(destination: SemanticPlaceV1, value: SemanticOperandV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            destination,
            value,
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    )
}

fn load(source: SemanticPlaceV1) -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
        source,
        SemanticVolatilityV1::NonVolatile,
        None,
    ))
}

fn call_pair(
    first: SemanticOperandV1,
    second: u128,
    destination: u32,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(3),
            vec![first, literal(second)],
            Some(SemanticCallDestinationV1::new(
                place(destination, U32),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(next),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn switch(discriminant: SemanticOperandV1, zero: u32, otherwise: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(zero),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(otherwise),
            ),
        )
        .unwrap(),
    }
}

fn suffix_owner(case: SuffixCase) -> ProductionSemanticSsaOwnerV1 {
    let original = scoped_root_tests::fixtures::repeated_owner();
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let mut functions = semantic.functions().to_vec();
    let mut locals = vec![
        local(132, U32, SemanticLocalRoleV1::Return),
        local(133, U32, SemanticLocalRoleV1::Argument(0)),
        local(135, U32, SemanticLocalRoleV1::Argument(1)),
        local(136, U32, SemanticLocalRoleV1::Temporary),
    ];
    let mut statements = if case == SuffixCase::Cells {
        let reference_type = reference(&mut types, U32, SemanticMutabilityV1::Mutable, false);
        locals.push(local(137, reference_type, SemanticLocalRoleV1::Temporary));
        locals.push(local(138, reference_type, SemanticLocalRoleV1::Temporary));
        let dereference = |local| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(local),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap(),
                ],
                U32,
            )
            .unwrap()
        };
        vec![
            assign(
                place(4, reference_type),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(1, U32),
                },
            ),
            assign(
                place(5, reference_type),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(2, U32),
                },
            ),
            assign(dereference(4), SemanticRvalueKindV1::Use(literal(17))),
            assign(dereference(5), SemanticRvalueKindV1::Use(literal(23))),
            assign(
                place(3, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(dereference(4))),
            ),
            assign(
                place(0, U32),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(dereference(5))),
            ),
        ]
    } else {
        vec![
            store(place(1, U32), SemanticOperandV1::Copy(place(1, U32))),
            store(place(2, U32), SemanticOperandV1::Copy(place(2, U32))),
            assign(place(3, U32), load(place(1, U32))),
            assign(place(0, U32), load(place(2, U32))),
        ]
    };
    if case == SuffixCase::Array {
        let array = declaration(
            &mut types,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                8,
                4,
                SemanticFieldsShapeV1::array(4, 2),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: U32,
                length: 2,
            },
            None,
        );
        locals.push(local(137, array, SemanticLocalRoleV1::Temporary));
        locals.push(local(138, U32, SemanticLocalRoleV1::Temporary));
        let indexed = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(5)),
                    U32,
                )
                .unwrap(),
            ],
            U32,
        )
        .unwrap();
        statements.extend([
            assign(place(5, U32), SemanticRvalueKindV1::Use(literal(0))),
            assign(
                place(4, array),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Array,
                        vec![literal(11), literal(13)],
                    )
                    .unwrap(),
                ),
            ),
            assign(indexed, SemanticRvalueKindV1::Use(literal(99))),
        ]);
    }
    statements.push(assign(
        place(0, U32),
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::Add,
            left: SemanticOperandV1::Copy(place(3, U32)),
            right: SemanticOperandV1::Copy(place(0, U32)),
        },
    ));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([131; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        2,
        vec![SemanticAbiArgumentV1::source(direct(U32)); 2],
        direct(U32),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap();
    functions[3] = function(
        130,
        SemanticFunctionRoleV1::InternalHelper,
        abi,
        locals,
        vec![block(134, statements, SemanticTerminatorKindV1::Return)],
    );
    let callback = &functions[CALLBACK.index() as usize];
    let blocks = match case {
        SuffixCase::Loop => vec![
            block(
                115,
                vec![],
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            ),
            block(
                117,
                vec![],
                call_pair(SemanticOperandV1::Copy(place(2, U32)), 31, 0, 2),
            ),
            block(
                118,
                vec![assign(
                    place(2, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: SemanticOperandV1::Copy(place(2, U32)),
                        right: literal(1),
                    },
                )],
                switch(SemanticOperandV1::Copy(place(2, U32)), 3, 1),
            ),
            block(119, vec![], SemanticTerminatorKindV1::Return),
        ],
        SuffixCase::CyclicEntry => vec![
            block(
                115,
                vec![],
                call_pair(SemanticOperandV1::Copy(place(2, U32)), 31, 0, 1),
            ),
            block(
                117,
                vec![assign(
                    place(2, U32),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: SemanticOperandV1::Copy(place(2, U32)),
                        right: literal(1),
                    },
                )],
                switch(SemanticOperandV1::Copy(place(2, U32)), 2, 0),
            ),
            block(118, vec![], SemanticTerminatorKindV1::Return),
        ],
        SuffixCase::Branch => vec![
            block(
                115,
                vec![],
                switch(SemanticOperandV1::Copy(place(2, U32)), 1, 2),
            ),
            block(
                117,
                vec![],
                call_pair(SemanticOperandV1::Copy(place(2, U32)), 31, 0, 3),
            ),
            block(
                118,
                vec![],
                call_pair(SemanticOperandV1::Copy(place(2, U32)), 47, 0, 3),
            ),
            block(119, vec![], SemanticTerminatorKindV1::Return),
        ],
        _ => vec![
            block(
                115,
                vec![],
                call_pair(SemanticOperandV1::Copy(place(2, U32)), 31, 3, 1),
            ),
            block(
                117,
                vec![],
                call_pair(SemanticOperandV1::Move(place(3, U32)), 47, 0, 2),
            ),
            block(118, vec![], SemanticTerminatorKindV1::Return),
        ],
    };
    functions[CALLBACK.index() as usize] = function(
        110,
        callback.role(),
        callback.abi().clone(),
        callback.locals().to_vec(),
        blocks,
    );
    if case == SuffixCase::Root {
        let root = &functions[0];
        let mut blocks = root.blocks().to_vec();
        let mut statements = vec![store(place(1, U32), SemanticOperandV1::Copy(place(1, U32)))];
        statements.extend_from_slice(blocks[0].statements());
        blocks[0] = block(85, statements, blocks[0].terminator().kind().clone());
        functions[0] = function(
            80,
            root.role(),
            root.abi().clone(),
            root.locals().to_vec(),
            blocks,
        )
        .with_kernel_entry(root.kernel_entry().unwrap().clone());
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        vec![ROOT],
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

struct Original {
    function: Function,
    statements: Vec<SemanticKirStatementOperationSpanV1>,
    terminators: Vec<SemanticKirTerminatorOperationSpanV1>,
    synthetic: Vec<SemanticKirSyntheticOperationSpanV1>,
    calls: Vec<SemanticKirCallReturnV1>,
}

#[derive(Default)]
struct Snapshot {
    originals: Vec<Original>,
    slots: Vec<ScopedSourceSlotV29>,
    initializers: Vec<(
        ProductionCallInstanceIdV1,
        PrivateArrayPhysicalLocationV1,
        Operation,
    )>,
}

thread_local! {
    static SNAPSHOT: std::cell::RefCell<Snapshot> = std::cell::RefCell::new(Snapshot::default());
    static REACHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn capture(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut snapshot = Snapshot::default();
    snapshot.slots = slots.slots.clone();
    for row in &slots.instances {
        let lowered = emitted[row.instance.index()].as_ref().unwrap();
        let body = lowered.function.body.as_ref().unwrap();
        visit_scoped_slot_initializers_v29(
            instances,
            row.instance,
            body.blocks[0].id,
            &slots.slots[row.slots.clone()],
            budget,
            |_, slot, location, _| {
                let operation = body.blocks[0].operations[location.operation].clone();
                assert!(matches!(operation.kind, OperationKind::Store { .. }));
                if instances.instance(row.instance).unwrap().function().index() == 3 {
                    let OperationKind::Store { pointer, value, .. } = &operation.kind else {
                        unreachable!()
                    };
                    assert_eq!(*pointer, slot.origin.pointer);
                    assert_eq!(*value, body.parameters[slot.legacy_local().unwrap() as usize - 1]);
                    assert_eq!(body.parameters.len(), 2);
                    if slot.legacy_local().unwrap() == 1 {
                        assert_ne!(location.operation, slot.allocation.operation + 1);
                    }
                }
                snapshot
                    .initializers
                    .push((row.instance, location, operation));
                Ok(())
            },
        )?;
        snapshot.originals.push(Original {
            function: lowered.function.clone(),
            statements: lowered.statement_operation_spans.clone(),
            terminators: lowered.terminator_operation_spans.clone(),
            synthetic: lowered.synthetic_operation_spans.clone(),
            calls: lowered.call_returns.sites.rows.clone(),
        });
    }
    SNAPSHOT.with(|saved| *saved.borrow_mut() = snapshot);
    REACHED.set(REACHED.get() + 1);
    Ok(())
}

fn verify(output: &OwnedPendingScopedRootV29, case: SuffixCase) {
    SNAPSHOT.with(|saved| {
        let snapshot = saved.borrow();
        let pending = &output.pending;
        let body = pending.function.body.as_ref().unwrap();
        let root = body.blocks[0].id;
        assert_eq!(output.source_slots.slots, snapshot.slots);
        let mut initializers = 0;
        for row in &pending.coordinates.spans.rows {
            let original_span = row.source.coordinates().2;
            let original = &snapshot.originals[row.instance.index()];
            let block = original
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == original_span.block)
                .unwrap();
            let removed = row.removed_call.map(|call| {
                let witness = original
                    .calls
                    .iter()
                    .find(|site| site.semantic_block == call.block)
                    .unwrap();
                let SemanticKirCallReturnKindV1::Call { call_operation, .. } = witness.kind else {
                    panic!("removed call requires exact original call witness")
                };
                call_operation as usize
            });
            let expected: Vec<_> = block
                .operations
                .iter()
                .enumerate()
                .skip(original_span.first as usize)
                .take(original_span.count as usize)
                .filter(|(index, _)| Some(*index) != removed)
                .map(|(_, operation)| operation)
                .collect();
            let actual: Vec<_> = row
                .segments
                .iter()
                .flatten()
                .flat_map(|span| {
                    let block = body
                        .blocks
                        .iter()
                        .find(|block| block.id == span.block)
                        .unwrap();
                    &block.operations[span.first as usize..span.end().unwrap() as usize]
                })
                .collect();
            assert_eq!(
                actual, expected,
                "source payload must retain exact order: {:?}",
                row.source
            );
            if matches!(row.source, InstanceSpanSourceV1::Synthetic(source)
                if source.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage)
            {
                let count = snapshot
                    .initializers
                    .iter()
                    .filter(|(instance, _, _)| *instance == row.instance)
                    .count();
                let first = row.segments[0].unwrap();
                assert_eq!(first.block, root);
                if count != 0 {
                    let suffix =
                        row.segments[1].expect("initializers require a separate exact segment");
                    assert_eq!(suffix.count as usize, count);
                    assert_eq!(suffix.block, original_span.block);
                    if suffix.block != root {
                        assert_eq!(suffix.first, 0);
                    }
                    assert!(
                        body.blocks
                            .iter()
                            .find(|block| block.id == suffix.block)
                            .unwrap()
                            .operations
                            [suffix.first as usize..suffix.end().unwrap() as usize]
                            .iter()
                            .all(|operation| matches!(operation.kind, OperationKind::Store { .. }))
                    );
                    initializers += count;
                }
            }
        }
        assert_eq!(initializers, snapshot.initializers.len());
        assert_eq!(
            initializers,
            if matches!(case, SuffixCase::Loop | SuffixCase::CyclicEntry) {
                2
            } else if case == SuffixCase::Root {
                5
            } else {
                4
            }
        );
        for (index, row) in pending.sidecars.rows.iter().enumerate() {
            assert_eq!(
                row.statement_operation_spans,
                snapshot.originals[index].statements
            );
            assert_eq!(
                row.terminator_operation_spans,
                snapshot.originals[index].terminators
            );
            assert_eq!(
                row.synthetic_operation_spans,
                snapshot.originals[index].synthetic
            );
        }
        for block in &body.blocks {
            for operation in &block.operations {
                if matches!(operation.kind, OperationKind::Alloca { .. }) {
                    assert_eq!(block.id, root);
                }
            }
        }
        if case == SuffixCase::Array {
            assert_eq!(
                snapshot
                    .slots
                    .iter()
                    .filter(|slot| slot.scalar_array().unwrap().count.is_some())
                    .count(),
                2
            );
        }
    });
}

mod instance_layout_emission_tests {
    use super::*;
    include!("production_execution_instance_layout_emission_v29_tests.rs");
}

fn run_suffix(
    case: SuffixCase,
    observer: ScopedSlotObserverV29,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    run_suffix_owner(
        || suffix_owner(case),
        observer,
        work_limit,
        storage_limit,
        |output, _, _| {
            verify(output, case);
            Ok(())
        },
    )
}

fn run_suffix_owner(
    owner: impl FnOnce() -> ProductionSemanticSsaOwnerV1,
    observer: ScopedSlotObserverV29,
    work_limit: usize,
    storage_limit: usize,
    verify_output: impl FnOnce(
        &OwnedPendingScopedRootV29,
        &ExecutionLifecycleSourceV29<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    run_suffix_owner_with_abi(owner, false, observer, work_limit, storage_limit, verify_output)
}

fn run_profiled_suffix_owner(
    owner: impl FnOnce() -> ProductionSemanticSsaOwnerV1,
    observer: ScopedSlotObserverV29,
    work_limit: usize,
    storage_limit: usize,
    verify_output: impl FnOnce(
        &OwnedPendingScopedRootV29,
        &ExecutionLifecycleSourceV29<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    run_suffix_owner_with_abi(
        || kernel_argument_abi_v18::tests::fixture_descriptor_ownership_v18(owner()),
        true, observer, work_limit, storage_limit, verify_output,
    )
}

fn run_suffix_owner_with_abi(
    owner: impl FnOnce() -> ProductionSemanticSsaOwnerV1,
    profiled: bool,
    observer: ScopedSlotObserverV29,
    work_limit: usize,
    storage_limit: usize,
    verify_output: impl FnOnce(
        &OwnedPendingScopedRootV29,
        &ExecutionLifecycleSourceV29<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let _guard = ObserverGuard::install(observer);
    REACHED.set(0);
    SNAPSHOT.with(|saved| *saved.borrow_mut() = Snapshot::default());
    let mut owner = owner();
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
        let profile = if profiled {
            budget.reserve_storage(size_of::<Option<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>>())?;
            let fixture = kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner);
            let roots = fixture.roots();
            Some(kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(
                &owner, ProductionKernelArgumentAbiInputV18 { roots: &roots }, &mut budget)?)
        } else { None };
        let semantic = owner.source_semantic();
        let launch = ProductionSourceLaunchRosterV1::try_new(
            semantic,
            &[ProductionSourceLaunchRootInputV1::new(
                "lifecycle_fixture",
                [88; 32],
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
            )],
        )
        .unwrap();
        let roots = [root_input(&owner)];
        let classes: Vec<_> = semantic
            .callables()
            .iter()
            .map(|callable| match callable {
                SemanticCallableDeclV1::Defined { function } if *function == HELPER => {
                    ProductionScopeCallableCandidateV29::Provider {
                        function: *function,
                        identity: semantic.functions()[function.index() as usize].identity(),
                    }
                }
                SemanticCallableDeclV1::CompilerIntrinsic {
                    binding,
                    operation:
                        SemanticCompilerIntrinsicOperationV1::Execution(
                            SemanticExecutionOperationV29::WorkgroupDerive { context, workgroup },
                        ),
                    operation_identity,
                } => {
                    assert_eq!(
                        binding.identity(),
                        SemanticFunctionIdentityV1::from_sha256([121; 32])
                    );
                    assert_eq!(
                        *operation_identity,
                        SemanticCompilerIntrinsicIdentityV1::from_sha256([121; 32])
                    );
                    assert_eq!(*context, CONTEXT);
                    assert_eq!(
                        *workgroup,
                        semantic.functions()[2].abi().source_input_types()[0]
                    );
                    ProductionScopeCallableCandidateV29::Derive {
                        binding: binding.identity(),
                        operation: *operation_identity,
                        context: *context,
                        workgroup: *workgroup,
                    }
                }
                _ => ProductionScopeCallableCandidateV29::Ordinary,
            })
            .collect();
        assert_eq!(classes.len(), semantic.callables().len());
        assert_eq!(
            classes
                .iter()
                .filter(|class| matches!(
                    class,
                    ProductionScopeCallableCandidateV29::Provider { .. }
                ))
                .count(),
            1
        );
        assert_eq!(
            classes
                .iter()
                .filter(|class| matches!(class, ProductionScopeCallableCandidateV29::Derive { .. }))
                .count(),
            1
        );
        let mut events = vec![crate::ProductionScopeEventCandidateV29 {
            function: ROOT,
            block: SemanticBlockIdV1::from_index(1),
            statement_count: 0,
            kind: ProductionScopeEventKindV29::Call {
                callee: SemanticCallableIdV1::from_index(1),
                kind: ProductionScopeCallKindV29::Provider,
            },
        }];
        let provider = &semantic.functions()[HELPER.index() as usize];
        for (ordinal, block) in provider.blocks().iter().enumerate() {
            let kind = match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    let kind = match classes[call.callee().index() as usize] {
                        ProductionScopeCallableCandidateV29::Ordinary => {
                            ProductionScopeCallKindV29::Ordinary
                        }
                        ProductionScopeCallableCandidateV29::Provider { .. } => {
                            ProductionScopeCallKindV29::Provider
                        }
                        ProductionScopeCallableCandidateV29::Derive { .. } => {
                            ProductionScopeCallKindV29::Derive
                        }
                    };
                    ProductionScopeEventKindV29::Call {
                        callee: call.callee(),
                        kind,
                    }
                }
                SemanticTerminatorKindV1::Assert { .. } => ProductionScopeEventKindV29::Assert,
                SemanticTerminatorKindV1::Return => ProductionScopeEventKindV29::Return,
                SemanticTerminatorKindV1::Unreachable => ProductionScopeEventKindV29::Unreachable,
                SemanticTerminatorKindV1::UnwindResume => ProductionScopeEventKindV29::UnwindResume,
                SemanticTerminatorKindV1::UnwindTerminate => {
                    ProductionScopeEventKindV29::UnwindTerminate
                }
                SemanticTerminatorKindV1::Abort => ProductionScopeEventKindV29::Abort,
                SemanticTerminatorKindV1::Goto(_) | SemanticTerminatorKindV1::SwitchInt { .. } => {
                    continue;
                }
                other => panic!("unexpected suffix provider terminator: {other:?}"),
            };
            events.push(crate::ProductionScopeEventCandidateV29 {
                function: HELPER,
                block: SemanticBlockIdV1::from_index(u32::try_from(ordinal).unwrap()),
                statement_count: block.statements().len(),
                kind,
            });
        }
        let assertion_sites: Vec<_> = provider
            .blocks()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, block)| {
                matches!(
                    block.terminator().kind(),
                    SemanticTerminatorKindV1::Assert { .. }
                )
                .then_some((
                    SemanticBlockIdV1::from_index(u32::try_from(ordinal).unwrap()),
                    block.statements().len(),
                ))
            })
            .collect();
        let assertion_events: Vec<_> = events
            .iter()
            .filter_map(|event| {
                (event.kind == ProductionScopeEventKindV29::Assert)
                    .then_some((event.block, event.statement_count))
            })
            .collect();
        assert_eq!(assertion_events, assertion_sites);
        for (site, _) in &assertion_sites {
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event.function == HELPER && event.block == *site)
                    .count(),
                1
            );
            assert!(events.iter().any(|event| event.function == HELPER
                && event.block == *site
                && event.kind == ProductionScopeEventKindV29::Assert));
        }
        let source = ExecutionLifecycleSourceV29::with_kernel_arguments(
            &owner,
            &launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: owner.source_semantic_sha256(),
                roots: &roots,
                classes: &classes,
                events: &events,
            },
            profile.as_ref(),
            &mut budget,
        )?;
        let table_floor = budget.storage();
        let result = with_scoped_source_test_layouts_v29(
            &source,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
            |demands, layouts, budget| {
                let floor = budget.storage();
                let persistent = layouts.persistent_storage_for_test();
                let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    crate::with_checked_context_root_v29(
                        &owner,
                        &launch,
                        roots[0],
                        budget,
                        |checked, budget| {
                            let limits = ProductionSemanticKirLimitsV1::default();
                            Ok(emit_pending_scoped_root_v29(
                                &checked,
                                &source,
                                demands,
                                layouts,
                                limits,
                                &mut ReachableClosureBudgetV1::new(limits.max_blocks),
                                &mut PrivateArrayLazyBudgetV1::new(1, limits.max_operations),
                                PrivateArrayPayloadV1 {
                                    occupied: 0,
                                    capacity: 0,
                                },
                                budget,
                            ))
                        },
                    )
                }));
                let result = match attempt {
                    Ok(result) => result,
                    Err(payload) => {
                        assert_eq!(
                            budget.storage(),
                            floor + layouts.persistent_storage_for_test() - persistent,
                            "root unwind must release its own reservations"
                        );
                        std::panic::resume_unwind(payload)
                    }
                }
                .map_err(|error| match error {
                    crate::ProductionContextRootErrorV29::Resource(error) => error.into(),
                    _ => execution_lifecycle_error_v29(),
                })?;
                match result {
                    Ok(output) => {
                        let schema_growth = layouts.persistent_storage_for_test() - persistent;
                        assert_eq!(
                            budget.storage() - floor,
                            output.retained_emission_storage + schema_growth
                        );
                        let verified = verify_output(&output, &source, budget);
                        let retained = output.retained_emission_storage;
                        drop(output);
                        budget.release_storage(retained)?;
                        assert_eq!(budget.storage(), floor + schema_growth);
                        verified
                    }
                    Err(error) => {
                        assert_eq!(
                            budget.storage(),
                            floor + layouts.persistent_storage_for_test() - persistent,
                            "{error:?}"
                        );
                        Err(error)
                    }
                }
            },
        );
        assert_eq!(budget.storage(), table_floor);
        result
    })();
    let peak = budget.peak_storage();
    drop(owner);
    let remaining = budget.storage();
    budget.release_storage(remaining).unwrap();
    assert_eq!(budget.storage(), 0);
    (result, work.work(), peak)
}

#[test]
fn two_retained_arguments_preserve_original_values_and_exact_split_payloads() {
    for case in [
        SuffixCase::Ordinary,
        SuffixCase::Cells,
        SuffixCase::Array,
        SuffixCase::Root,
    ] {
        let result = run_suffix(case, capture, LIMIT, LIMIT).0;
        assert!(result.is_ok(), "{case:?}: {result:?}");
        assert_eq!(REACHED.get(), 1);
    }
}

#[test]
fn helper_initializers_stay_inside_mutating_loop_and_branch_call_entries() {
    for case in [SuffixCase::Loop, SuffixCase::Branch] {
        let result = run_suffix(case, capture, LIMIT, LIMIT).0;
        assert!(result.is_ok(), "{case:?}: {result:?}");
        assert_eq!(REACHED.get(), 1);
    }
}

#[test]
fn cyclic_source_entry_materializes_original_ssa_through_one_time_invocation() {
    let result = run_suffix(
        SuffixCase::CyclicEntry,
        capture_cyclic_invocation,
        LIMIT,
        LIMIT,
    )
    .0;
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(REACHED.get(), 1);
}

include!("production_invocation_entry_emission_v1_tests.rs");

#[path = "production_scoped_slot_custody_lifecycle_v29_tests.rs"]
mod custody_lifecycle_tests;

#[path = "production_source_cell_payload_v29_tests.rs"]
mod cell_payload_tests;

#[test]
fn initialized_relocation_and_cell_coordinate_scratch_have_exact_resource_bounds() {
    for case in [SuffixCase::Cells, SuffixCase::Array, SuffixCase::Root] {
        let (result, work, storage) = run_suffix(case, capture, LIMIT, LIMIT);
        assert!(result.is_ok(), "{case:?}: {result:?}");
        assert!(run_suffix(case, capture, work, storage).0.is_ok());
        for (work, storage, work_failure) in [(work - 1, storage, true), (work, storage - 1, false)]
        {
            let result = run_suffix(case, capture, work, storage).0;
            assert_resource(
                result.as_ref().err().expect("one below must fail"),
                work_failure,
            );
        }
        assert!(run_suffix(case, capture, LIMIT, LIMIT).0.is_ok());
    }
}

fn reject_suffix_faults(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    fn physical_census(
        instances: &ExecutionInstancesV29<'_>,
        emitted: &[Option<LoweredFunctionResultV1>],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<OwnedScopedSourceSlotsV29, ProductionSemanticKirErrorV1> {
        // Replay the original demands, including typed arrays, independently
        // of the emitted slot identities that the hostile cases modify.
        derive_scoped_source_slots_with_demanded_plan_v29(instances, emitted, 1024, budget)
    }
    fn unchanged_census(
        instances: &ExecutionInstancesV29<'_>,
        emitted: &[Option<LoweredFunctionResultV1>],
        slots: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let checked = physical_census(instances, emitted, budget)?;
        assert_eq!(checked.slots, slots.slots);
        assert_eq!(checked.instances, slots.instances);
        let retained = checked.retained_storage;
        drop(checked);
        budget.release_storage(retained)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    }
    unchanged_census(instances, emitted, slots, budget)?;
    let row = slots
        .instances
        .iter()
        .find(|row| row.function.index() == 3)
        .unwrap();
    let other = slots
        .instances
        .iter()
        .find(|other| other.function.index() == 3 && other.instance != row.instance)
        .unwrap();
    let lowered = emitted[row.instance.index()].as_ref().unwrap();
    let block = &lowered.function.body.as_ref().unwrap().blocks[0];
    let operations = block.operations.clone();
    let spans = lowered.synthetic_operation_spans.clone();
    let origins = lowered.scoped_slot_origins.clone();
    let source_instance = lowered.source_call_instance;
    let mut locations = Vec::new();
    visit_scoped_slot_initializers_v29(
        instances,
        row.instance,
        block.id,
        &slots.slots[row.slots.clone()],
        budget,
        |_, _, location, _| {
            locations.push(location);
            Ok(())
        },
    )?;
    assert_eq!(locations.len(), 2);
    let first = locations[0].operation;
    let second = locations[1].operation;
    let other_parameter = lowered.function.body.as_ref().unwrap().parameters[1];
    let other_pointer = slots.slots[row.slots.start + 1].origin.pointer;
    for fault in 0..16 {
        {
            let lowered = emitted[row.instance.index()].as_mut().unwrap();
            let operations = &mut lowered.function.body.as_mut().unwrap().blocks[0].operations;
            match fault {
                0 => {
                    let OperationKind::Store { value, .. } = &mut operations[first].kind else {
                        unreachable!()
                    };
                    *value = other_parameter;
                }
                1 => {
                    let OperationKind::Store { pointer, .. } = &mut operations[first].kind else {
                        unreachable!()
                    };
                    *pointer = other_pointer;
                }
                2 | 3 => {
                    let OperationKind::Store { access, .. } = &mut operations[first].kind else {
                        unreachable!()
                    };
                    *access = MemoryAccess::new(
                        if fault == 2 {
                            AddressSpace::Private
                        } else {
                            AddressSpace::Global
                        },
                        1,
                    );
                }
                4 => operations.swap(first, second),
                5 => operations.swap(first, slots.slots[row.slots.start].allocation.operation + 1),
                6 => {
                    operations.remove(first);
                }
                7 => {
                    let duplicate = operations[first].clone();
                    operations.insert(first, duplicate);
                }
                8 => lowered.source_call_instance = Some(other.instance),
                9 => lowered.scoped_slot_origins.as_mut().unwrap()[0].semantic_type = UNIT,
                10 => lowered.scoped_slot_origins.as_mut().unwrap()[0].identity = ScopedAllocationIdentityV29::LegacyLocal(0),
                11 | 12 | 13 | 14 => {
                    let span = lowered
                        .synthetic_operation_spans
                        .iter_mut()
                        .find(|span| {
                            span.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage
                        })
                        .unwrap();
                    match fault {
                        11 => span.operation_count += 1,
                        12 => span.operation_count -= 1,
                        13 => span.first_operation_ordinal = 1,
                        14 => span.kernel_ir_block = BlockId(u32::MAX),
                        _ => unreachable!(),
                    }
                }
                15 => {
                    let span = *lowered
                        .synthetic_operation_spans
                        .iter()
                        .find(|span| {
                            span.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage
                        })
                        .unwrap();
                    lowered.synthetic_operation_spans.push(span);
                }
                _ => unreachable!(),
            }
        }
        let floor = budget.storage();
        assert!(
            physical_census(instances, emitted, budget).is_err(),
            "fault {fault}"
        );
        assert_eq!(
            budget.storage(),
            floor,
            "fault {fault} leaked census storage"
        );
        let lowered = emitted[row.instance.index()].as_mut().unwrap();
        lowered.function.body.as_mut().unwrap().blocks[0].operations = operations.clone();
        lowered.synthetic_operation_spans = spans.clone();
        lowered.scoped_slot_origins = origins.clone();
        lowered.source_call_instance = source_instance;
        unchanged_census(instances, emitted, slots, budget)?;
        REACHED.set(REACHED.get() + 1);
    }
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn source_suffix_census_rejects_same_type_value_position_span_and_custody_changes() {
    for case in [SuffixCase::Ordinary, SuffixCase::Cells, SuffixCase::Array] {
        let result = run_suffix(case, reject_suffix_faults, LIMIT, LIMIT).0;
        assert!(is_stopped(&result), "{case:?}: {result:?}");
        assert_eq!(REACHED.get(), 16);
        assert!(run_suffix(case, capture, LIMIT, LIMIT).0.is_ok());
    }
}

fn reject_schedule_faults(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let row = slots
        .instances
        .iter()
        .find(|row| row.function.index() == 3)
        .unwrap();
    let original = &slots.slots[row.slots.clone()];
    assert_eq!(original.len(), 3);
    let entry = emitted[row.instance.index()]
        .as_ref()
        .unwrap()
        .function
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .id;
    let floor = budget.storage();
    let unchanged = visit_scoped_slot_initializers_v29(
        instances,
        row.instance,
        entry,
        original,
        budget,
        |_, _, _, _| Ok(()),
    )?;
    assert_eq!(unchanged, (4, 2));
    assert_eq!(budget.storage(), floor);
    for fault in 0..7 {
        let mut changed = original.to_vec();
        match fault {
            0 => changed[0].instance = instances.root(),
            1 => changed[1].origin.identity = changed[0].origin.identity,
            2 => changed.swap(0, 1),
            3 => changed[0].origin.semantic_type = UNIT,
            4 => changed[0].allocation.operation += 1,
            5 => {
                let ScopedSlotRepresentationV29::ScalarArray(scalar) = &mut changed[2].representation else { panic!("array fixture"); };
                scalar.count.as_mut().unwrap().1.operation += 1;
            }
            6 => changed[0].allocation.block_ordinal = 1,
            _ => unreachable!(),
        }
        let floor = budget.storage();
        assert!(
            visit_scoped_slot_initializers_v29(
                instances,
                row.instance,
                entry,
                &changed,
                budget,
                |_, _, _, _| Ok(())
            )
            .is_err(),
            "schedule fault {fault}"
        );
        assert_eq!(budget.storage(), floor);
        REACHED.set(REACHED.get() + 1);
    }
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn suffix_schedule_is_bound_to_original_instance_local_order_type_and_array_prefix() {
    let result = run_suffix(SuffixCase::Array, reject_schedule_faults, LIMIT, LIMIT).0;
    assert!(is_stopped(&result), "{result:?}");
    assert_eq!(REACHED.get(), 7);
}

#[test]
fn initialized_relocation_callback_panic_restores_root_floor_and_observer_scope() {
    fn panic_after_census(
        source: &ExecutionLifecycleSourceV29<'_>,
        instances: &ExecutionInstancesV29<'_>,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        slots: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        capture(source, instances, emitted, slots, budget)?;
        panic!("injected initializer census unwind")
    }
    let result = run_suffix(SuffixCase::Cells, panic_after_census, LIMIT, LIMIT).0;
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference callback panicked",
            ..
        })
    ));
    assert_eq!(REACHED.get(), 1);
    assert!(
        run_suffix(SuffixCase::Cells, capture, LIMIT, LIMIT)
            .0
            .is_ok()
    );
}
