// These projection fixtures are not admitted compiler artifacts or proof receipts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReferenceWriteMutationV2 {
    None,
    Constant,
    WrongGet,
    WrongWrite,
    MissingWitness,
    RootBypass,
    IndexUseBypass,
    GetBypass,
    ReceiverBypass,
    RawUseBeforeDefinition,
    RawRedefined,
    RawEscaped,
    ReceiverRedefined,
    OutputRedefined,
    WrongAllocation,
    WrongValueType,
    Predicate,
    Shifted,
    ShiftedAndRanked,
    WrongRankedIndex,
}

fn reference_write_fixture_v2(
    mutation: ReferenceWriteMutationV2,
) -> (
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
    Vec<Option<ProjectedDisjointIndexV1>>,
    Vec<Option<AllocationContractV1>>,
) {
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let operand = |local, ty| SemanticOperandV1::Copy(place(local, ty));
    let call = |callee, args, destination, ty, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                args,
                Some(SemanticCallDestinationV1::new(
                    place(destination, ty),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let assign = |local, ty, value| {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            value,
        )))
    };
    let switch = |yes, no| SemanticTerminatorKindV1::SwitchInt {
        discriminant: operand(7, BOOL_TYPE),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                cfg_edge(SemanticEdgeRoleV1::SwitchValue, yes),
            )],
            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, no),
        )
        .unwrap(),
    };
    let value = assign(
        4,
        SCALAR_TYPE,
        if matches!(
            mutation,
            ReferenceWriteMutationV2::Constant
                | ReferenceWriteMutationV2::ReceiverBypass
                | ReferenceWriteMutationV2::IndexUseBypass
                | ReferenceWriteMutationV2::ShiftedAndRanked
        ) {
            SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(7)))
        } else {
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand: operand(3, U64_TYPE),
                },
            )
        },
    );
    let mut get_statements = vec![];
    let mut value_statements = vec![];
    if mutation == ReferenceWriteMutationV2::RawRedefined {
        value_statements.push(assign(
            3,
            U64_TYPE,
            SemanticRvalueV1::new(
                U64_TYPE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    U64_TYPE,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(3, 8).unwrap()),
                ))),
            ),
        ));
    }
    if mutation == ReferenceWriteMutationV2::RawEscaped {
        value_statements.push(assign(
            8,
            U64_POINTER_TYPE,
            SemanticRvalueV1::new(
                U64_POINTER_TYPE,
                SemanticRvalueKindV1::AddressOf {
                    mutability: SemanticMutabilityV1::Mutable,
                    place: place(3, U64_TYPE),
                },
            ),
        ));
    }
    if mutation == ReferenceWriteMutationV2::RawUseBeforeDefinition {
        get_statements.push(value);
    } else {
        value_statements.push(value);
    }
    let borrow = assign(
        5,
        POINTER_TYPE,
        SemanticRvalueV1::new(
            POINTER_TYPE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(1, POINTER_TYPE),
            },
        ),
    );
    let mut borrow_statements = vec![borrow.clone()];
    if mutation == ReferenceWriteMutationV2::ReceiverRedefined {
        borrow_statements.push(borrow);
    }
    if mutation == ReferenceWriteMutationV2::OutputRedefined {
        borrow_statements.push(assign(
            1,
            POINTER_TYPE,
            SemanticRvalueV1::new(
                POINTER_TYPE,
                SemanticRvalueKindV1::Use(operand(1, POINTER_TYPE)),
            ),
        ));
    }
    let get = call(1, vec![operand(2, U64_TYPE)], 3, U64_TYPE, 3);
    let mut blocks = vec![
        block(
            170,
            vec![],
            if mutation == ReferenceWriteMutationV2::IndexUseBypass {
                switch(1, 5)
            } else if mutation == ReferenceWriteMutationV2::RootBypass {
                switch(1, 2)
            } else {
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1))
            },
        ),
        block(171, vec![], call(0, vec![], 2, U64_TYPE, 2)),
        block(
            172,
            get_statements,
            if mutation == ReferenceWriteMutationV2::GetBypass {
                switch(7, 3)
            } else {
                get.clone()
            },
        ),
        block(
            173,
            value_statements,
            if mutation == ReferenceWriteMutationV2::ReceiverBypass {
                switch(4, 5)
            } else {
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4))
            },
        ),
        block(
            174,
            borrow_statements,
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 5)),
        ),
        block(
            175,
            vec![],
            call(
                2,
                vec![
                    operand(5, POINTER_TYPE),
                    operand(2, U64_TYPE),
                    if mutation == ReferenceWriteMutationV2::IndexUseBypass {
                        constant(7)
                    } else if mutation == ReferenceWriteMutationV2::WrongValueType {
                        operand(7, BOOL_TYPE)
                    } else {
                        operand(4, SCALAR_TYPE)
                    },
                ],
                6,
                BOOL_TYPE,
                6,
            ),
        ),
        block(176, vec![], SemanticTerminatorKindV1::Return),
    ];
    if mutation == ReferenceWriteMutationV2::GetBypass {
        blocks.push(block(177, vec![], get));
    }
    let function = projection_function_with_locals(
        blocks,
        vec![
            local(170, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(171, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
            local(172, U64_TYPE, SemanticLocalRoleV1::Temporary),
            local(173, U64_TYPE, SemanticLocalRoleV1::Temporary),
            local(174, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(175, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            local(176, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            local(177, BOOL_TYPE, SemanticLocalRoleV1::Argument(1)),
            local(178, U64_POINTER_TYPE, SemanticLocalRoleV1::Temporary),
        ],
    );
    let mut callables = vec![
        compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness: U64_TYPE,
            raw_index: U64_TYPE,
        }),
        compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
            index_witness: U64_TYPE,
            raw_index: U64_TYPE,
        }),
        compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                disjoint_slice: POINTER_TYPE,
                witness: U64_TYPE,
                element: SCALAR_TYPE,
                raw_index: U64_TYPE,
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
                kind: SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: false },
            },
        ),
    ];
    if mutation == ReferenceWriteMutationV2::WrongGet {
        callables[1] = SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0));
    }
    if mutation == ReferenceWriteMutationV2::WrongWrite {
        callables[2] = SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0));
    }
    let mut projected = ProjectedDisjointIndexV1 {
        value: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        mapping: SemanticDisjointIndexSpaceV1::Index1d,
        precondition: None,
        availability: None,
    };
    if mutation == ReferenceWriteMutationV2::Predicate {
        projected.precondition = Some((projected.value, ProductionRankedValueV1::Argument(0)));
    }
    if matches!(
        mutation,
        ReferenceWriteMutationV2::Shifted | ReferenceWriteMutationV2::ShiftedAndRanked
    ) {
        projected.value = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1));
    }
    let mut indices = vec![None; 9];
    indices[2] = (mutation != ReferenceWriteMutationV2::MissingWitness).then_some(projected);
    indices[3] = Some(projected);
    let mut allocations = vec![None; 9];
    allocations[5] = Some(AllocationContractV1 {
        allocation_origin: if mutation == ReferenceWriteMutationV2::WrongAllocation {
            2
        } else {
            1
        },
        noalias_class: 2,
        writable: true,
        singleton_object: false,
    });
    (function, callables, indices, allocations)
}

fn resolve_reference_write_fixture_v2(
    mutation: ReferenceWriteMutationV2,
) -> Result<ProductionSemanticExpressionV2, String> {
    let (function, callables, indices, allocations) = reference_write_fixture_v2(mutation);
    let types = assertion_proof_types();
    let blocks = [ProductionRankedBlockV1::new(
        vec![ProductionRankedOperationV1::InvocationIndex {
            result: ProductionRankedValueIdV1::new(0),
            dimension: 0,
            launch_extent: 0,
        }],
        ProductionRankedTerminatorV1::Return,
    )];
    let mut resolver =
        GpuSemanticExpressionResolverV2::with_ranked_reads(&types, &function, &blocks, &[])
            .map_err(|error| format!("{error:?}"))?;
    resolver
        .bind_projected_indices_v2(&callables, &blocks, &indices)
        .map_err(|error| format!("{error:?}"))?;
    resolver.use_site = Some(ScalarAssignmentSiteV1 {
        block: 5,
        statement: 0,
    });
    let ranked_index = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(
        if mutation == ReferenceWriteMutationV2::ShiftedAndRanked {
            1
        } else if mutation == ReferenceWriteMutationV2::WrongRankedIndex {
            9
        } else {
            0
        },
    ));
    resolver
        .resolve_write_only_call_v2(
            &callables,
            function.blocks()[5].terminator().kind(),
            &allocations,
            &indices,
            1,
            &[ranked_index],
        )
        .map_err(str::to_owned)
}

#[test]
fn reference_write_only_uses_the_terminal_value_and_exact_index_cast() {
    assert_eq!(
        resolve_reference_write_fixture_v2(ReferenceWriteMutationV2::None).unwrap(),
        ProductionSemanticExpressionV2::Cast {
            kind: ProductionSemanticCastV2::Integer,
            source: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 64
            },
            target: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32
            },
            operand: Box::new(ProductionSemanticExpressionV2::Symbol {
                symbol: 0,
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 64
                },
            }),
        }
    );
    assert_eq!(
        resolve_reference_write_fixture_v2(ReferenceWriteMutationV2::Constant).unwrap(),
        ProductionSemanticExpressionV2::Constant {
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32
            },
            bits: 7
        }
    );
}

#[test]
fn reference_write_only_rejects_provenance_control_and_value_mutations() {
    for mutation in [
        ReferenceWriteMutationV2::WrongGet,
        ReferenceWriteMutationV2::WrongWrite,
        ReferenceWriteMutationV2::MissingWitness,
        ReferenceWriteMutationV2::RootBypass,
        ReferenceWriteMutationV2::IndexUseBypass,
        ReferenceWriteMutationV2::GetBypass,
        ReferenceWriteMutationV2::ReceiverBypass,
        ReferenceWriteMutationV2::RawUseBeforeDefinition,
        ReferenceWriteMutationV2::RawRedefined,
        ReferenceWriteMutationV2::RawEscaped,
        ReferenceWriteMutationV2::ReceiverRedefined,
        ReferenceWriteMutationV2::OutputRedefined,
        ReferenceWriteMutationV2::WrongAllocation,
        ReferenceWriteMutationV2::WrongValueType,
        ReferenceWriteMutationV2::Predicate,
        ReferenceWriteMutationV2::Shifted,
        ReferenceWriteMutationV2::ShiftedAndRanked,
        ReferenceWriteMutationV2::WrongRankedIndex,
    ] {
        assert!(
            resolve_reference_write_fixture_v2(mutation).is_err(),
            "accepted {mutation:?}"
        );
    }
}

#[test]
fn reference_write_only_constant_rhs_still_requires_current_index_and_receiver() {
    for mutation in [
        ReferenceWriteMutationV2::IndexUseBypass,
        ReferenceWriteMutationV2::ReceiverBypass,
    ] {
        assert_eq!(
            resolve_reference_write_fixture_v2(mutation).unwrap_err(),
            "GPU write-only operand does not dominate the write"
        );
    }
}

#[test]
fn write_only_guard_failure_reaches_the_following_effect_without_writing() {
    let (function, callables, _, _) = reference_write_fixture_v2(ReferenceWriteMutationV2::None);
    let mut projected = (0..function.blocks().len())
        .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
        .collect::<Vec<_>>();
    projected[5].items = vec![
        ProjectedBlockItemV1::Guarded(GuardedRankedAccessV1 {
            view: ProductionRankedValueIdV1::new(1),
            indices: vec![ProductionRankedValueV1::Local(
                ProductionRankedValueIdV1::new(0),
            )],
            checked_success: None,
            failure: GuardedAccessFailureV1::Continue,
            comparisons: vec![(
                ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
                ProductionRankedValueV1::Argument(0),
            )],
            access: AccessKindAttr::Write,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: Some(ProjectedSemanticAccessSiteV1 {
                block: 5,
                statement: None,
            }),
        }),
        ProjectedBlockItemV1::Effect {
            operation: ProductionRankedOperationV1::AllocationEffect {
                kind: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 2,
                noalias_class: 3,
            },
            source: None,
        },
    ];
    let (blocks, sources, _) = build_ranked_cfg(
        &assertion_proof_types(),
        &function,
        &callables,
        &vec![None; function.locals().len()],
        &vec![None; function.blocks().len()],
        &[],
        vec![],
        projected,
    )
    .unwrap();
    let ProductionRankedTerminatorV1::IndexLessThan {
        true_block,
        false_block,
        ..
    } = blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator(),
                ProductionRankedTerminatorV1::IndexLessThan { .. }
            )
        })
        .unwrap()
        .terminator()
    else {
        panic!("missing write guard")
    };
    let success = &blocks[*true_block as usize];
    let failure = &blocks[*false_block as usize];
    assert!(matches!(
        success.operations(),
        [ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Write,
            ..
        }]
    ));
    assert!(failure.operations().is_empty());
    assert_eq!(success.terminator(), failure.terminator());
    let ProductionRankedTerminatorV1::Branch { target } = failure.terminator() else {
        panic!("failed write must continue")
    };
    assert!(matches!(
        blocks[*target as usize].operations(),
        [ProductionRankedOperationV1::AllocationEffect { .. }]
    ));
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].block, *true_block as usize);
    assert_eq!(
        sources[0].semantic_site,
        Some(ProjectedSemanticAccessSiteV1 {
            block: 5,
            statement: None
        })
    );
    assert!(
        !blocks
            .iter()
            .any(|block| matches!(block.terminator(), ProductionRankedTerminatorV1::Trap))
    );
}

#[test]
fn write_only_padded_static_launch_proves_total_view_but_underlaunch_does_not() {
    // These exact finite graphs exercise the real nine-pass pipeline. They do
    // not replace the production kernel's runtime extent with a guessed size.
    for length in [64_u64, 65, 4097] {
        for underlaunch in [false, true] {
            let global = if underlaunch {
                64
            } else {
                length.div_ceil(64) * 64
            };
            if underlaunch && length == 64 {
                continue;
            }
            let invocation = ProductionRankedValueIdV1::new(0);
            let view = ProductionRankedValueIdV1::new(2);
            let extent = ProductionRankedValueIdV1::new(1);
            let entry = vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [global, 1, 1],
                    workgroup_extents: [64, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::InvocationIndex {
                    result: invocation,
                    dimension: 0,
                    launch_extent: global,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: extent,
                    value: length,
                },
                ProductionRankedOperationV1::ViewInSpace {
                    result: view,
                    element_width: 32,
                    writable: true,
                    shape: vec![length],
                    dynamic_extents: vec![],
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::OwnershipContract {
                    view: ProductionRankedValueV1::Local(view),
                    coverage: dialect_kernel::OwnershipCoverageAttr::TotalView,
                    partition: dialect_kernel::OwnershipPartitionAttr::ExactSets,
                },
            ];
            let (blocks, _, _) = single_guarded_cfg(
                entry,
                GuardedRankedAccessV1 {
                    view,
                    indices: vec![ProductionRankedValueV1::Local(invocation)],
                    checked_success: None,
                    failure: GuardedAccessFailureV1::Continue,
                    comparisons: vec![(
                        ProductionRankedValueV1::Local(invocation),
                        ProductionRankedValueV1::Local(extent),
                    )],
                    access: AccessKindAttr::Write,
                    memory_space: MemorySpaceAttr::Global,
                    source: SemanticSourceProvenanceV1::unavailable(),
                    semantic_site: None,
                },
            );
            let kernel = ProductionRankedKernelV1::new("padded_write_only", 0, blocks).unwrap();
            let construction =
                ProductionConstructionV1::ranked_kernel("padded_module", kernel).unwrap();
            let result = compile_ranked_kernel_for_lowering_v1(
                construction,
                ProductionSessionLimitsV1::default(),
            );
            if underlaunch {
                let error = result.unwrap_err().to_string();
                assert!(error.contains("FE2O3-OWN"), "unexpected failure: {error}");
            } else {
                let lowering = result.unwrap();
                assert!(lowering.all_mandatory_reports_are_clean());
                assert_eq!(
                    lowering
                        .ownership_report()
                        .coverage_summary()
                        .total_view_proved(),
                    1
                );
            }
        }
    }
}

#[test]
fn write_only_boolean_result_does_not_mint_exact_control() {
    let (function, callables, _, _) = reference_write_fixture_v2(ReferenceWriteMutationV2::None);
    let mut blocks = function.blocks().to_vec();
    blocks[6] = block(
        176,
        vec![],
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(6), vec![], BOOL_TYPE).unwrap(),
            ),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    1,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, 7),
                )],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 8),
            )
            .unwrap(),
        },
    );
    blocks.push(block(177, vec![], SemanticTerminatorKindV1::Return));
    blocks.push(block(178, vec![], SemanticTerminatorKindV1::Return));
    let function = projection_function_with_locals(blocks, function.locals().to_vec());
    let predicates = switch_predicates(
        &function,
        &vec![None; function.locals().len()],
        &vec![None; function.locals().len()],
    )
    .unwrap();
    assert!(predicates.iter().all(Option::is_none));
    assert!(matches!(
        projected_cfg_terminator(
            &function,
            6,
            &callables,
            false,
            &constant_locals(&function).unwrap(),
            &predicates,
            &vec![None; function.blocks().len()],
        )
        .unwrap(),
        ProjectedCfgTerminatorV1::AnalysisSplit {
            first_block: 7,
            second_block: 8
        }
    ));
}
