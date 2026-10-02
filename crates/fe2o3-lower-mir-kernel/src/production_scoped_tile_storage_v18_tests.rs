// Source-owned table coverage. The additional array is admitted as semantic
// source before planning; no hand-edited graph becomes a Pending source owner.
use super::*;

fn tile_with_array_source_v29() -> ProductionSemanticSsaOwnerV1 {
    let template = source_owner(SourceCase::Repeated);
    let array_template = scoped_root_tests::fixtures::array_owner(false);
    let array_declaration = array_template
        .source_semantic()
        .types()
        .iter()
        .find(|ty| {
            matches!(ty.shape(), SemanticTypeShapeV1::Array { element, length: 1 }
            if *element == U32)
        })
        .expect("original scalar-array source layout")
        .clone();
    let semantic = template.source_semantic();
    let mut types = semantic.types().to_vec();
    let array = SemanticTypeIdV1::from_index(types.len() as u32);
    let identity = SemanticTypeIdentityV1::from_sha256([239; 32]);
    assert!(types.iter().all(|ty| ty.identity() != identity));
    types.push(SemanticTypeDeclV1::new(
        identity,
        SemanticLayoutIdentityV1::from_sha256([239; 32]),
        array_declaration.layout().clone(),
        array_declaration.shape().clone(),
    ));
    let mut functions = semantic.functions().to_vec();
    let prior = functions[3].clone();
    let mut locals = prior.locals().to_vec();
    let array_local = locals.len() as u32;
    let index_local = array_local + 1;
    locals.push(local(241, array, SemanticLocalRoleV1::Temporary));
    locals.push(local(242, U32, SemanticLocalRoleV1::Temporary));
    let indexed = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(array_local),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(index_local)),
                U32,
            )
            .unwrap(),
        ],
        U32,
    )
    .unwrap();
    let mut blocks = prior.blocks().to_vec();
    let mut statements = vec![
        assign(
            place(index_local, U32),
            SemanticRvalueKindV1::Use(scalar(0)),
        ),
        assign(
            place(array_local, array),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Array, vec![scalar(11)])
                    .unwrap(),
            ),
        ),
        assign(indexed, SemanticRvalueKindV1::Use(scalar(99))),
    ];
    statements.extend_from_slice(blocks[0].statements());
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        statements,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    functions[3] = replace_extra_function_v29(&prior, prior.abi().clone(), locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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

fn storage_candidate_case_v29(
    check: impl FnOnce(&ScopedTileScalarCandidateV29, &mut ArgumentBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let candidate = scalar_candidate_from_source_v29(
        tile_with_array_source_v29,
        ScopedTileOrderV29::Blocked,
        64,
        &mut budget,
    );
    let original = candidate.input.pending.pending_module();
    assert!(!original.storage_layouts.is_empty());
    assert_eq!(
        candidate.output.module().storage_layouts,
        original.storage_layouts
    );
    assert!(
        original
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                OperationKind::Execution(kir::ExecutionOperationV15::MaskedTileLoadU32 { .. })
            ))
    );
    assert!(
        !candidate
            .output
            .module()
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::Execution(_)))
    );
    let floor = budget.storage();
    candidate.replay_with_budget(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    check(&candidate, &mut budget);
    candidate.replay_with_budget(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

#[test]
fn source_owned_storage_table_survives_tile_scalarization_and_replay() {
    storage_candidate_case_v29(|candidate, budget| {
        fresh_graph_subject(candidate, budget, true, |_, _| {});
        let floor = budget.storage();
        let legacy =
            kir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                candidate.output.module(),
                budget,
            );
        assert!(
            legacy.is_err(),
            "nonempty V18 source table cannot become legacy V12"
        );
        drop(legacy);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn fresh_v18_table_substitutions_refuse_source_correspondence() {
    storage_candidate_case_v29(|candidate, budget| {
        fresh_graph_subject(candidate, budget, false, |module, _| {
            module.storage_layouts.clear();
        });
        fresh_graph_subject(candidate, budget, false, |module, _| {
            module.storage_layouts.push(kir::StorageLayoutV1 {
                size: 4,
                alignment: 4,
                kind: kir::StorageLayoutKindV1::Scalar(kir::ScalarType::I32),
            });
        });
        fresh_graph_subject(candidate, budget, false, |module, _| {
            let row = module
                .storage_layouts
                .iter_mut()
                .find(|row| {
                    matches!(
                        row.kind,
                        kir::StorageLayoutKindV1::Scalar(kir::ScalarType::U32)
                    )
                })
                .expect("source u32 storage layout");
            row.kind = kir::StorageLayoutKindV1::Scalar(kir::ScalarType::I32);
        });
    });
}

#[test]
fn storage_source_kernel_role_substitution_fails_fresh_v18_admission() {
    storage_candidate_case_v29(|candidate, budget| {
        let mut module = candidate.output.module().clone();
        let root = candidate.input.pending.inner.pending.roots[0].function_ordinal;
        assert_eq!(module.functions[root].role, kir::FunctionRole::KernelEntry);
        module.functions[root].role = kir::FunctionRole::InternalHelper;
        let floor = budget.storage();
        let result =
            kir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                candidate.input.pending.inner.limits.storage_layout_limits(),
                budget,
            );
        assert!(
            result.is_err(),
            "fresh admission must not infer the original explicit role"
        );
        drop(result);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn v18_resource_summaries_preserve_all_nested_resource_paths() {
    use kir::{
        BorrowedKernelIrVerificationErrorV1 as V, CanonicalKernelIrReplayAdmissionErrorV18 as C,
        KernelIrDecodeError as D, KernelIrEncodeError as E, StorageLayoutErrorV1 as L,
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let work_limit = work.charge_work(1).unwrap_err();
    let mut storage_work = CanonicalKernelIrWorkBudgetV1::new(1);
    let mut budget = ArgumentBudgetV1::new(&mut storage_work, 0);
    let storage = budget.reserve_storage(1).unwrap_err();
    for resource in [
        ArgumentResourceV1::Work(work_limit),
        storage,
        ArgumentResourceV1::Allocation,
        ArgumentResourceV1::Arithmetic,
        ArgumentResourceV1::Accounting,
    ] {
        for error in [
            C::Resource(resource),
            C::Decode(D::Resource(resource)),
            C::Layout(L::Resource(resource)),
            C::Verification(V::Resource(resource)),
        ] {
            assert_eq!(
                ScopedTileFailureKindV29::from(ScopedModuleErrorV29::Canonical(error)),
                ScopedTileFailureKindV29::Resource(resource)
            );
        }
    }
    for error in [
        C::Encode(E::Allocation),
        C::Decode(D::Encode(E::Allocation)),
    ] {
        assert_eq!(
            ScopedTileFailureKindV29::from(ScopedModuleErrorV29::Canonical(error)),
            ScopedTileFailureKindV29::Resource(ArgumentResourceV1::Allocation)
        );
    }
}

#[test]
fn raw_graph_scalar_emitter_preserves_storage_operations_and_explicit_roles() {
    // Graph boundary coverage only, not fabricated source ownership.
    let mut module = Module::new("storage_tile_emitter_boundary");
    module.storage_layouts.push(kir::StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: kir::StorageLayoutKindV1::Scalar(kir::ScalarType::U32),
    });
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(0),
                Type::pointer(
                    Type::StorageObject(kir::StorageLayoutIdV1(0)),
                    kir::AddressSpace::Private,
                    kir::AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Alloca {
                element: Type::StorageObject(kir::StorageLayoutIdV1(0)),
                count: None,
                address_space: kir::AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), Type::Scalar(kir::ScalarType::U32)),
            OperationKind::Constant(Constant::U32(37)),
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(kir::StorageOperationV1::WriteValue {
                address: ValueId(0),
                value: ValueId(1),
                access: kir::MemoryAccess::new(kir::AddressSpace::Private, 4),
            }),
        ),
        Operation::new(
            vec![ValueDef::new(
                ValueId(2),
                Type::Scalar(kir::ScalarType::U32),
            )],
            OperationKind::Storage(kir::StorageOperationV1::ReadValue {
                address: ValueId(0),
                access: kir::MemoryAccess::new(kir::AddressSpace::Private, 4),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    module.functions.push(Function::definition(
        "storage_helper",
        kir::Signature::new(vec![], vec![Type::Scalar(kir::ScalarType::U32)]),
        vec![],
        vec![block],
    ));
    max_id_graph_boundary_tests::graph_only_emit_and_check(&module, |output, _| {
        assert_eq!(output, &module);
        assert_eq!(output.functions[0].role, kir::FunctionRole::InternalHelper);
    });
    // Cross both replacement-vector growth boundaries while preserving the
    // original storage read/write and every added scalar operation.
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    for value in 3..10 {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(value), Type::Scalar(kir::ScalarType::U32)),
            OperationKind::Constant(Constant::U32(value)),
        ));
    }
    assert_eq!(block.operations.len(), 11);
    max_id_graph_boundary_tests::graph_only_emit_and_check(&module, |output, _| {
        assert_eq!(output, &module);
        assert_eq!(output.functions[0].role, kir::FunctionRole::InternalHelper);
    });
}
