fn thread_write_callable_v2() -> SemanticCallableDeclV1 {
    compiler_intrinsic_callable(
        SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
            disjoint_slice: ARRAY_TYPE,
            witness: SCALAR_TYPE,
            element: SCALAR_TYPE,
            raw_index: U64_TYPE,
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
            kind: SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: false },
        },
    )
}

fn thread_write_call_v2(
    arguments: Vec<SemanticOperandV1>,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            arguments,
            Some(SemanticCallDestinationV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], BOOL_TYPE).unwrap(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn thread_write_arguments_v2(value: SemanticOperandV1) -> Vec<SemanticOperandV1> {
    vec![typed_operand(1, POINTER_TYPE), tensor_operand(2), value]
}

fn thread_write_function_v2(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    projection_function_with_locals(
        blocks,
        vec![
            local(201, BOOL_TYPE, SemanticLocalRoleV1::Return),
            local(202, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
            local(203, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)),
            local(204, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(205, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(206, BOOL_TYPE, SemanticLocalRoleV1::Argument(2)),
        ],
    )
}

fn thread_write_fixture_v2(
    statements: Vec<SemanticStatementV1>,
    arguments: Vec<SemanticOperandV1>,
) -> SemanticFunctionDeclV1 {
    thread_write_function_v2(vec![
        block(201, statements, thread_write_call_v2(arguments, 1)),
        block(202, vec![], SemanticTerminatorKindV1::Return),
    ])
}

fn thread_write_source_v2(block: usize) -> ProjectedAccessSourceV1 {
    ProjectedAccessSourceV1 {
        block: 0,
        operation: 1,
        access: AccessKindAttr::Write,
        memory_space: MemorySpaceAttr::Global,
        source: SemanticSourceProvenanceV1::unavailable(),
        semantic_site: Some(ProjectedSemanticAccessSiteV1 {
            block,
            statement: None,
        }),
    }
}

fn thread_write_projection_v2(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    source: ProjectedAccessSourceV1,
) -> crate::production_reference_effect_join_v2::RankedGpuWriteV2 {
    let view = ProductionRankedValueIdV1::new(0);
    let blocks = [ProductionRankedBlockV1::new(
        vec![
            ProductionRankedOperationV1::ViewInSpace {
                result: view,
                element_width: 32,
                writable: true,
                shape: vec![DYNAMIC_EXTENT],
                dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 7,
                noalias_class: 11,
            },
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Write,
                view: ProductionRankedValueV1::Local(view),
                indices: vec![ProductionRankedValueV1::Argument(1)],
            },
        ],
        ProductionRankedTerminatorV1::Return,
    )];
    let mut writes = projected_reference_gpu_writes_v2(
        &assertion_proof_types(),
        function,
        callables,
        &blocks,
        &[source],
    )
    .unwrap();
    assert_eq!(writes.len(), 1);
    let write = writes.pop().unwrap();
    assert_eq!(
        (write.block, write.operation, write.allocation_origin),
        (0, 1, 7)
    );
    assert_eq!(write.view, ProductionRankedValueV1::Local(view));
    assert_eq!(write.indices, vec![ProductionRankedValueV1::Argument(1)]);
    write
}

#[test]
fn gpu_write_only_thread_constant_preserves_ranked_correspondence_v2() {
    let function = thread_write_fixture_v2(vec![], thread_write_arguments_v2(constant(17)));
    for disjoint in [false, true] {
        let mut callable = thread_write_callable_v2();
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite { kind, .. },
            ..
        } = &mut callable
        else {
            unreachable!();
        };
        *kind = SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint };
        let write = thread_write_projection_v2(&function, &[callable], thread_write_source_v2(0));
        assert_eq!(
            write.value,
            Ok(ProductionSemanticExpressionV2::Constant {
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 32
                },
                bits: 17,
            })
        );
    }
}

#[test]
fn gpu_write_only_thread_alias_captures_value_before_reassignment_v2() {
    let function = thread_write_fixture_v2(
        vec![
            typed_assignment(3, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(17))),
            typed_assignment(4, SCALAR_TYPE, SemanticRvalueKindV1::Use(tensor_operand(3))),
            typed_assignment(3, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(23))),
        ],
        thread_write_arguments_v2(tensor_operand(4)),
    );
    assert!(matches!(
        thread_write_projection_v2(
            &function,
            &[thread_write_callable_v2()],
            thread_write_source_v2(0),
        )
        .value,
        Ok(ProductionSemanticExpressionV2::Constant { bits: 17, .. })
    ));
}

#[test]
fn gpu_write_only_thread_requires_exact_call_site_v2() {
    let function = thread_write_fixture_v2(vec![], thread_write_arguments_v2(constant(17)));
    let types = assertion_proof_types();
    let callables = [thread_write_callable_v2()];
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
    for (site, expected) in [
        (
            ProjectedSemanticAccessSiteV1 {
                block: 2,
                statement: None,
            },
            "GPU write call site is outside the exact semantic function",
        ),
        (
            ProjectedSemanticAccessSiteV1 {
                block: 0,
                statement: Some(0),
            },
            "GPU write call site does not identify the exact semantic terminator",
        ),
        (
            ProjectedSemanticAccessSiteV1 {
                block: 1,
                statement: None,
            },
            "GPU write semantic terminator is not a direct call",
        ),
    ] {
        assert_eq!(
            resolver
                .resolve_thread_write_v2(
                    &callables,
                    site,
                    SemanticSourceProvenanceV1::unavailable(),
                )
                .unwrap_err(),
            expected
        );
        assert!(resolver.use_site.is_none());
    }
    let origin = SemanticSourceOriginV1::new(
        SemanticSourceFileIdentityV1::from_sha256(bytes(210)),
        0,
        1,
        1,
        1,
        1,
        2,
    )
    .unwrap();
    assert_eq!(
        resolver
            .resolve_thread_write_v2(
                &callables,
                ProjectedSemanticAccessSiteV1 {
                    block: 0,
                    statement: None
                },
                SemanticSourceProvenanceV1::new(Some(origin), None),
            )
            .unwrap_err(),
        "GPU write call site does not identify the exact semantic terminator"
    );
}

#[test]
fn gpu_write_only_thread_rejects_other_callees_and_write_kinds_v2() {
    let function = thread_write_fixture_v2(vec![], thread_write_arguments_v2(constant(17)));
    let mut exclusive = thread_write_callable_v2();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut exclusive else {
        unreachable!();
    };
    let SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite { kind, .. } = operation
    else {
        unreachable!();
    };
    *kind = SemanticWriteOnlyDisjointWriteKindV1::GridExclusive;
    for callables in [
        vec![],
        vec![compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::ColdPath,
        )],
        vec![exclusive],
    ] {
        assert_eq!(
            thread_write_projection_v2(&function, &callables, thread_write_source_v2(0),)
                .value
                .unwrap_err(),
            "GPU write call is not an authenticated write-only Thread intrinsic"
        );
    }
}

#[test]
fn gpu_write_only_thread_rejects_wrong_arity_v2() {
    for arguments in [
        thread_write_arguments_v2(constant(17))[..2].to_vec(),
        vec![
            typed_operand(1, POINTER_TYPE),
            tensor_operand(2),
            constant(17),
            constant(23),
        ],
    ] {
        let function = thread_write_fixture_v2(vec![], arguments);
        assert_eq!(
            thread_write_projection_v2(
                &function,
                &[thread_write_callable_v2()],
                thread_write_source_v2(0),
            )
            .value
            .unwrap_err(),
            "GPU write-only Thread call argument count changed"
        );
    }
}

#[test]
fn gpu_write_only_thread_rejects_wrong_operand_types_v2() {
    for argument in [1, 2] {
        let mut arguments = thread_write_arguments_v2(constant(17));
        arguments[argument] = typed_constant(U64_TYPE, 17, 8);
        let function = thread_write_fixture_v2(vec![], arguments);
        assert_eq!(
            thread_write_projection_v2(
                &function,
                &[thread_write_callable_v2()],
                thread_write_source_v2(0),
            )
            .value
            .unwrap_err(),
            "GPU write-only Thread call operand type changed"
        );
    }
}

#[test]
fn gpu_write_only_thread_rejects_non_dominating_value_v2() {
    let function = thread_write_function_v2(vec![
        block(201, vec![], zero_switch(5, BOOL_TYPE, 1, 2)),
        block(
            202,
            vec![typed_assignment(
                3,
                SCALAR_TYPE,
                SemanticRvalueKindV1::Use(constant(17)),
            )],
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
        ),
        block(
            203,
            vec![],
            thread_write_call_v2(thread_write_arguments_v2(tensor_operand(3)), 3),
        ),
        block(204, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let types = assertion_proof_types();
    let mut resolver = GpuSemanticExpressionResolverV2::new(&types, &function).unwrap();
    assert_eq!(
        resolver
            .resolve_thread_write_v2(
                &[thread_write_callable_v2()],
                ProjectedSemanticAccessSiteV1 {
                    block: 2,
                    statement: None
                },
                SemanticSourceProvenanceV1::unavailable(),
            )
            .unwrap_err(),
        "GPU semantic local has no exact reaching assignment"
    );
    assert!(resolver.use_site.is_none());
    assert!(resolver.visiting.is_empty());
}

#[test]
fn gpu_write_only_thread_does_not_admit_other_memory_effects_v2() {
    let function = thread_write_fixture_v2(vec![], thread_write_arguments_v2(constant(17)));
    let mut workgroup = thread_write_source_v2(0);
    workgroup.memory_space = MemorySpaceAttr::Workgroup;
    let mut atomic = thread_write_source_v2(0);
    atomic.access = AccessKindAttr::AtomicReadModifyWrite;
    let mut missing = thread_write_source_v2(0);
    missing.semantic_site = None;
    for source in [workgroup, atomic, missing] {
        assert_eq!(
            thread_write_projection_v2(&function, &[thread_write_callable_v2()], source,)
                .value
                .unwrap_err(),
            "GPU write has no authenticated semantic MIR statement"
        );
    }
}
