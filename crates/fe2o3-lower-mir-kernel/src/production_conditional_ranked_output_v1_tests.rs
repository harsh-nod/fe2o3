use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, derive_conditional_total_view_from_verified_v1,
    verify_module_ref,
};
use fe2o3_pliron::{
    ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedTerminatorV1,
    ProductionReferenceOutputSiteV2,
};

// These tests exercise occurrence checks on inert correspondence/candidates.
// They neither forge a production binding nor authenticate a ranked relation.
fn canonical_fixture() -> (Module, SemanticKirCorrespondenceV1) {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut entry = BasicBlock::new(BlockId(700));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), pointer.clone()),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(2),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(42),
        then_arguments: vec![],
        else_target: BlockId(9),
        else_arguments: vec![],
    });
    let mut tail = BasicBlock::new(BlockId(9));
    tail.terminator = Some(Terminator::Return { values: vec![] });
    let mut write = BasicBlock::new(BlockId(42));
    write.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(6),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    write.terminator = Some(Terminator::Branch {
        target: BlockId(9),
        arguments: vec![],
    });
    let mut module = Module::new("conditional-ranked-output");
    module.functions.push(Function::kernel_entry(
        "canonical_entry",
        Signature::new(
            vec![
                Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, tail, write],
    ));
    module.kernels.push(Kernel::new(
        "coverage",
        "canonical_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let association = SemanticKirFunctionCorrespondenceV1 {
        correspondence_owner: SemanticFunctionIdV1::from_index(2),
        semantic_function: SemanticFunctionIdV1::from_index(4),
        kernel_ir_function: FunctionId::new("canonical_entry"),
        role: SemanticKirFunctionRoleV1::KernelEntry,
    };
    let correspondence = SemanticKirCorrespondenceV1 {
        private_arrays: PrivateArrayCorrespondenceV1::default(),
        semantic_sha256: [0; 32],
        function_count: 5,
        lowered_functions: vec![association].into_boxed_slice(),
        blocks: Box::new([]),
        statement_operation_spans: vec![SemanticKirStatementOperationSpanV1 {
            correspondence_owner: SemanticFunctionIdV1::from_index(2),
            semantic_function: SemanticFunctionIdV1::from_index(4),
            semantic_block: SemanticBlockIdV1::from_index(8),
            statement_ordinal: 3,
            kernel_ir_block: BlockId(42),
            first_operation_ordinal: 0,
            operation_count: 1,
        }]
        .into_boxed_slice(),
        terminator_operation_spans: Box::new([]),
        generated_terminator_values: Box::new([]),
        call_returns: Box::new([]),
        call_result_components: Box::new([]),
        synthetic_operation_spans: Box::new([]),
        parameter_bindings: Box::new([]),
        parameter_component_bindings: Box::new([]),
        ignored_parameter_bindings: Box::new([]),
    };
    (module, correspondence)
}

fn site() -> SemanticAccessSiteV1 {
    SemanticAccessSiteV1 {
        block: 8,
        statement: Some(3),
        ordinal: 0,
    }
}
fn source() -> ProductionRankedAccessSourceV1 {
    ProductionRankedAccessSourceV1::new(8, Some(3), 0, 2, 0)
}
fn local(id: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(id))
}
fn access() -> ProductionRankedOperationV1 {
    ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Write,
        view: local(2),
        indices: vec![local(1)],
    }
}
fn ranked_kernel(
    extent: ProductionRankedValueV1,
    write: ProductionRankedOperationV1,
) -> ProductionRankedKernelV1 {
    ProductionRankedKernelV1::new(
        "different_ranked_symbol",
        2,
        vec![
            ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::IndexUnknown {
                        result: ProductionRankedValueIdV1::new(0),
                    },
                    ProductionRankedOperationV1::InvocationIndex {
                        result: ProductionRankedValueIdV1::new(1),
                        dimension: 0,
                        launch_extent: 0,
                    },
                    ProductionRankedOperationV1::ViewInSpace {
                        result: ProductionRankedValueIdV1::new(2),
                        element_width: 32,
                        writable: true,
                        shape: vec![DYNAMIC_EXTENT],
                        dynamic_extents: vec![extent],
                        memory_space: MemorySpaceAttr::Global,
                        allocation_origin: 1,
                        noalias_class: 1,
                    },
                    ProductionRankedOperationV1::SemanticConstant {
                        result: ProductionRankedValueIdV1::new(3),
                        value: 1,
                    },
                ],
                ProductionRankedTerminatorV1::Branch { target: 3 },
            ),
            ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
            ProductionRankedBlockV1::new(
                vec![write],
                ProductionRankedTerminatorV1::Branch { target: 1 },
            ),
            ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::IndexLessThan {
                    lhs: local(1),
                    rhs: extent,
                    true_block: 2,
                    false_block: 1,
                },
            ),
        ],
    )
    .expect("inert typed ranked recipe")
}
fn candidate<'a>(
    kernel: &'a ProductionRankedKernelV1,
    sources: &'a [ProductionRankedAccessSourceV1],
) -> NativeRankedSourceCandidateV1<'a> {
    NativeRankedSourceCandidateV1::from_untrusted_parts(
        2,
        1,
        kernel,
        sources,
        &[],
        "unused diagnostic text",
    )
}
fn contract(
    site: ProductionGpuWriteSiteV2,
    view: ProductionRankedValueV1,
    index: ProductionRankedValueV1,
    value: ProductionRankedValueV1,
) -> ProductionEffectRefinementContractV2 {
    ProductionEffectRefinementContractV2::new(
        1,
        site,
        ProductionReferenceOutputSiteV2::new(19, 71, 6),
        view,
        vec![index],
        vec![local(10)],
        vec![local(11)],
        local(12),
        local(12),
        local(12),
        local(12),
        value,
        local(13),
    )
    .expect("inert contract shape, not a proof subject")
}
fn source_query(
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    location: FunctionOperationLocation,
) -> JoinResult<SemanticAccessSiteV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    canonical_store_source(
        &module.functions[0],
        correspondence,
        &correspondence.lowered_functions[0],
        location,
        &mut budget,
    )
}

#[test]
fn exact_store_uses_sparse_ids_not_physical_or_ranked_block_order() {
    let (module, mut correspondence) = canonical_fixture();
    let mut wrong_root = correspondence.statement_operation_spans[0];
    wrong_root.correspondence_owner = SemanticFunctionIdV1::from_index(99);
    let mut wrong_function = correspondence.statement_operation_spans[0];
    wrong_function.semantic_function = SemanticFunctionIdV1::from_index(99);
    correspondence.statement_operation_spans = vec![
        wrong_root,
        correspondence.statement_operation_spans[0],
        wrong_function,
    ]
    .into_boxed_slice();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let analysis = derive_conditional_total_view_from_verified_v1(
        verify_module_ref(&module).unwrap(),
        &module.kernels[0].id,
        &mut budget,
    )
    .unwrap();
    let facts = analysis.facts().unwrap();
    assert_eq!(
        facts.address_domain(),
        ConditionalTotalViewAddressDomainV1::GlobalLaunch
    );
    let selected = source_query(&module, &correspondence, facts.store_location()).unwrap();
    assert_eq!(selected, site());
    let kernel = ranked_kernel(ProductionRankedValueV1::Argument(0), access());
    let sources = [source()];
    let selected_row = ranked_source(&sources, selected, &mut budget).unwrap();
    let write = ranked_write(candidate(&kernel, &sources), selected_row, &mut budget).unwrap();
    assert_eq!(write.site, ProductionGpuWriteSiteV2::new(2, 0));
    assert_eq!(write.view, local(2));
    assert_eq!(write.index, local(1));
}

#[test]
fn missing_overlapping_and_same_source_duplicate_spans_are_rejected() {
    let (module, mut correspondence) = canonical_fixture();
    let original = correspondence.statement_operation_spans[0];
    let location = FunctionOperationLocation::new(BlockId(42), 0);
    correspondence.statement_operation_spans = Box::new([]);
    assert_eq!(
        source_query(&module, &correspondence, location),
        Err(JoinError::SourceSpan)
    );
    correspondence.statement_operation_spans = vec![original, original].into_boxed_slice();
    assert_eq!(
        source_query(&module, &correspondence, location),
        Err(JoinError::AmbiguousSourceSpan)
    );
    let mut other = original;
    other.kernel_ir_block = BlockId(9);
    other.operation_count = 0;
    correspondence.statement_operation_spans = vec![original, other].into_boxed_slice();
    assert_eq!(
        source_query(&module, &correspondence, location),
        Err(JoinError::AmbiguousSourceSpan)
    );
    other = original;
    other.operation_count = 2;
    correspondence.statement_operation_spans = vec![other].into_boxed_slice();
    assert_eq!(
        source_query(&module, &correspondence, location),
        Err(JoinError::SourceSpan)
    );
    other.first_operation_ordinal = u32::MAX;
    correspondence.statement_operation_spans = vec![other].into_boxed_slice();
    assert_eq!(
        source_query(&module, &correspondence, location),
        Err(JoinError::Resource(ResourceError::Arithmetic))
    );
}

#[test]
fn wrong_owner_or_function_does_not_supply_a_source_span() {
    let (module, correspondence) = canonical_fixture();
    for wrong_owner in [true, false] {
        let mut changed = correspondence.clone();
        if wrong_owner {
            changed.statement_operation_spans[0].correspondence_owner =
                SemanticFunctionIdV1::from_index(4);
        } else {
            changed.statement_operation_spans[0].semantic_function =
                SemanticFunctionIdV1::from_index(2);
        }
        assert_eq!(
            source_query(
                &module,
                &changed,
                FunctionOperationLocation::new(BlockId(42), 0)
            ),
            Err(JoinError::SourceSpan)
        );
    }
}

#[test]
fn terminator_access_uses_memory_consumer_ordinal_not_operation_ordinal() {
    let (mut module, mut correspondence) = canonical_fixture();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[2];
    block.operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    );
    block.operations.insert(
        1,
        Operation::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
    );
    verify_module_ref(&module).unwrap();
    correspondence.statement_operation_spans = Box::new([]);
    correspondence.terminator_operation_spans = vec![SemanticKirTerminatorOperationSpanV1 {
        correspondence_owner: SemanticFunctionIdV1::from_index(2),
        semantic_function: SemanticFunctionIdV1::from_index(4),
        semantic_block: SemanticBlockIdV1::from_index(8),
        kernel_ir_block: BlockId(42),
        first_operation_ordinal: 0,
        operation_count: 3,
    }]
    .into_boxed_slice();
    // This fixture has a read and is deliberately outside coverage analysis.
    let selected = source_query(
        &module,
        &correspondence,
        FunctionOperationLocation::new(BlockId(42), 2),
    )
    .unwrap();
    assert_eq!(
        selected,
        SemanticAccessSiteV1 {
            block: 8,
            statement: None,
            ordinal: 1
        }
    );
    assert_eq!(
        source_query(
            &module,
            &correspondence,
            FunctionOperationLocation::new(BlockId(42), 1)
        ),
        Err(JoinError::CanonicalStore)
    );
    let OperationKind::Load { access, .. } =
        &mut module.functions[0].body.as_mut().unwrap().blocks[2].operations[0].kind
    else {
        unreachable!()
    };
    access.address_space = AddressSpace::Private;
    assert_eq!(
        source_query(
            &module,
            &correspondence,
            FunctionOperationLocation::new(BlockId(42), 2)
        ),
        Err(JoinError::CanonicalStore)
    );
}

#[test]
fn source_rows_must_be_unique_in_both_directions() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    assert_eq!(
        ranked_source(&[], site(), &mut budget),
        Err(JoinError::SourceAccess)
    );
    for rows in [
        vec![source(), source()],
        vec![
            source(),
            ProductionRankedAccessSourceV1::new(8, Some(3), 0, 1, 0),
        ],
        vec![
            source(),
            ProductionRankedAccessSourceV1::new(91, None, 0, 2, 0),
        ],
    ] {
        assert_eq!(
            ranked_source(&rows, site(), &mut budget),
            Err(JoinError::AmbiguousSourceAccess)
        );
    }
    let rows = [ProductionRankedAccessSourceV1::new(8, Some(3), 1, 2, 0)];
    assert_eq!(
        ranked_source(&rows, site(), &mut budget),
        Err(JoinError::SourceAccess)
    );
}

#[test]
fn shifted_wrong_and_allocation_only_writes_are_rejected() {
    let kernel = ranked_kernel(ProductionRankedValueV1::Argument(0), access());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    for (block, operation) in [(0, 1), (2, 1), (99, 0)] {
        let row = ProductionRankedAccessSourceV1::new(8, Some(3), 0, block, operation);
        assert!(matches!(
            ranked_write(candidate(&kernel, &[]), &row, &mut budget),
            Err(JoinError::RankedWrite)
        ));
    }
    let (origin, class) = dialect_kernel::neutral_workgroup_allocation_contract_v1([13; 32]);
    // Global allocation-only writes are already rejected by the recipe
    // constructor. Use valid effects to reach this query's own refusal.
    for (kind, memory_space, allocation_origin, noalias_class) in [
        (AccessKindAttr::Read, MemorySpaceAttr::Global, 1, 1),
        (
            AccessKindAttr::Write,
            MemorySpaceAttr::Workgroup,
            origin,
            class,
        ),
    ] {
        let fallback = ranked_kernel(
            ProductionRankedValueV1::Argument(0),
            ProductionRankedOperationV1::AllocationEffect {
                kind,
                memory_space,
                allocation_origin,
                noalias_class,
            },
        );
        assert!(matches!(
            ranked_write(candidate(&fallback, &[]), &source(), &mut budget),
            Err(JoinError::RankedWrite)
        ));
    }
}

#[test]
fn dynamic_extent_identity_is_retained_but_never_bound_to_canonical_length() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    for extent in [
        ProductionRankedValueV1::Argument(0),
        ProductionRankedValueV1::Argument(1),
        local(0),
    ] {
        let kernel = ranked_kernel(extent, access());
        let candidate = candidate(&kernel, &[]);
        let write = ranked_write(candidate, &source(), &mut budget).unwrap();
        let (observed, contract) =
            ranked_view_and_contract(candidate, &write, 0, 4, &mut budget).unwrap();
        assert_eq!(
            observed,
            ProductionConditionalRankedExtentV1::Unbound(extent)
        );
        assert!(
            contract.is_none(),
            "the public query must refuse absent contracts"
        );
        assert!(matches!(
            ranked_view_and_contract(candidate, &write, 1, 4, &mut budget),
            Err(JoinError::View)
        ));
        assert!(matches!(
            ranked_view_and_contract(candidate, &write, 0, 8, &mut budget),
            Err(JoinError::View)
        ));
        assert!(matches!(
            ranked_view_and_contract(candidate, &write, 0, u64::MAX, &mut budget),
            Err(JoinError::Resource(ResourceError::Arithmetic))
        ));
    }
}

#[test]
fn contract_join_checks_write_view_and_indices_not_reference_logical_abi() {
    let kernel = ranked_kernel(ProductionRankedValueV1::Argument(0), access());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    let write = ranked_write(candidate(&kernel, &[]), &source(), &mut budget).unwrap();
    let matching = contract(write.site, write.view, write.index, local(3));
    let mut selected = None;
    retain_contract(&mut selected, &matching, &write, &mut budget).unwrap();
    assert!(std::ptr::eq(selected.unwrap(), &matching));
    assert_eq!(
        matching.reference_output_site().argument(),
        19,
        "inert contract's normalized logical ABI relation is not validated here"
    );
    assert_eq!(
        retain_contract(&mut selected, &matching, &write, &mut budget),
        Err(JoinError::AmbiguousContract)
    );
    let shifted = contract(
        ProductionGpuWriteSiteV2::new(2, 1),
        write.view,
        write.index,
        local(3),
    );
    let mut selected = None;
    retain_contract(&mut selected, &shifted, &write, &mut budget).unwrap();
    assert!(selected.is_none());
    for changed in [
        contract(write.site, local(99), write.index, local(3)),
        contract(write.site, write.view, local(99), local(3)),
    ] {
        assert_eq!(
            retain_contract(&mut None, &changed, &write, &mut budget),
            Err(JoinError::Contract)
        );
    }
}

#[test]
fn value_access_keeps_site_and_checks_only_ranked_contract_value_identity() {
    let kernel = ranked_kernel(
        ProductionRankedValueV1::Argument(0),
        ProductionRankedOperationV1::ValueAccess {
            kind: AccessKindAttr::Write,
            view: local(2),
            indices: vec![local(1)],
            value: local(3),
        },
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    let write = ranked_write(candidate(&kernel, &[]), &source(), &mut budget).unwrap();
    assert_eq!(write.site, ProductionGpuWriteSiteV2::new(2, 0));
    let matching = contract(write.site, write.view, write.index, local(3));
    retain_contract(&mut None, &matching, &write, &mut budget).unwrap();
    let changed = contract(write.site, write.view, write.index, local(99));
    assert_eq!(
        retain_contract(&mut None, &changed, &write, &mut budget),
        Err(JoinError::Contract)
    );
}

#[test]
fn same_work_ledger_exact_boundary_and_storage_floor_are_preserved() {
    let (module, correspondence) = canonical_fixture();
    let kernel = ranked_kernel(ProductionRankedValueV1::Argument(0), access());
    let rows = [source()];
    let candidate = candidate(&kernel, &rows);
    let matching = contract(
        ProductionGpuWriteSiteV2::new(2, 0),
        local(2),
        local(1),
        local(3),
    );
    let run = |budget: &mut Budget<'_>| -> JoinResult<()> {
        let site = canonical_store_source(
            &module.functions[0],
            &correspondence,
            &correspondence.lowered_functions[0],
            FunctionOperationLocation::new(BlockId(42), 0),
            budget,
        )?;
        let source = ranked_source(&rows, site, budget)?;
        let write = ranked_write(candidate, source, budget)?;
        ranked_view_and_contract(candidate, &write, 0, 4, budget)?;
        retain_contract(&mut None, &matching, &write, budget)
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 37);
    budget.reserve_storage(37).unwrap();
    budget.charge_work(17).unwrap();
    run(&mut budget).unwrap();
    let exact = budget.work();
    assert!(exact > 17);
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.peak_storage(), 37);
    for (limit, succeeds) in [(exact - 1, false), (exact, true)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, 37);
        budget.reserve_storage(37).unwrap();
        budget.charge_work(17).unwrap();
        let outcome = run(&mut budget);
        if succeeds {
            outcome.unwrap();
        } else {
            assert!(matches!(
                outcome,
                Err(JoinError::Resource(ResourceError::Work(_)))
            ));
        }
        assert_eq!(budget.storage(), 37);
        assert_eq!(budget.peak_storage(), 37);
        assert!(matches!(
            run(&mut budget),
            Err(JoinError::Resource(ResourceError::Work(_)))
        ));
    }
}

fn extent_source(extent: ProductionRankedValueV1) -> ProductionRankedAccessSourceV1 {
    source().with_output_extent(ProductionRankedOutputExtentSourceV1::new(
        0,
        local(2),
        extent,
        local(1),
    ))
}

fn extent_query(
    kernel: &ProductionRankedKernelV1,
    rows: &[ProductionRankedAccessSourceV1],
    extent: ProductionRankedValueV1,
    budget: &mut Budget<'_>,
) -> JoinResult<()> {
    let candidate = candidate(kernel, rows);
    let selected = ranked_source(rows, site(), budget)?;
    let write = ranked_write(candidate, selected, budget)?;
    ranked_view_and_contract(candidate, &write, 0, 4, budget)?;
    rederive_extent_source(candidate, selected, &write, extent, 0, budget)
}

fn change_ranked_block(
    kernel: &ProductionRankedKernelV1,
    block: usize,
    change: impl FnOnce(&mut Vec<ProductionRankedOperationV1>, &mut ProductionRankedTerminatorV1),
) -> ProductionRankedKernelV1 {
    let mut blocks = kernel.blocks().to_vec();
    let mut operations = blocks[block].operations().to_vec();
    let mut terminator = blocks[block].terminator().clone();
    change(&mut operations, &mut terminator);
    blocks[block] = ProductionRankedBlockV1::new(operations, terminator);
    ProductionRankedKernelV1::new("inert_extent_candidate", 2, blocks).unwrap()
}

#[test]
fn extent_proposal_is_borrowed_inline_and_argument_ordinal_has_no_meaning() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    for extent in [
        ProductionRankedValueV1::Argument(0),
        ProductionRankedValueV1::Argument(1),
    ] {
        let kernel = ranked_kernel(extent, access());
        let rows = [extent_source(extent)];
        let candidate = candidate(&kernel, &rows);
        assert!(std::ptr::eq(candidate.access_sources(), rows.as_slice()));
        assert_eq!(
            candidate.access_sources()[0]
                .output_extent()
                .unwrap()
                .extent(),
            extent
        );
        extent_query(&kernel, &rows, extent, &mut budget).unwrap();
        assert_eq!(
            extent_query(&kernel, &[source()], extent, &mut budget),
            Err(JoinError::MissingExtentSource)
        );
    }
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), 0);
}

#[test]
fn base_field_reconstruction_loses_proposal_and_never_authenticates_extent() {
    let extent = ProductionRankedValueV1::Argument(0);
    let proposed = extent_source(extent);
    // Model the legacy constructor boundary, not a new wire codec.
    let reconstructed = ProductionRankedAccessSourceV1::new(
        proposed.semantic_block(),
        proposed.semantic_statement(),
        proposed.semantic_access_ordinal(),
        proposed.ranked_block(),
        proposed.ranked_operation(),
    );
    assert_eq!(reconstructed, source());
    assert_eq!(reconstructed.cmp(&source()), std::cmp::Ordering::Equal);
    assert_ne!(proposed, reconstructed);
    assert_ne!(proposed.cmp(&reconstructed), std::cmp::Ordering::Equal);
    assert_eq!(reconstructed.output_extent(), None);
    let kernel = ranked_kernel(extent, access());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    assert_eq!(
        extent_query(&kernel, &[reconstructed], extent, &mut budget),
        Err(JoinError::MissingExtentSource)
    );
}

#[test]
fn serialized_extent_proposals_still_require_exact_operand_checks() {
    use crate::{decode_production_ranked_source_rows_v1, encode_production_ranked_source_rows_v1};
    let extent = ProductionRankedValueV1::Argument(0);
    let kernel = ranked_kernel(extent, access());
    let valid = extent_source(extent);
    let rows = [
        valid,
        source(),
        source().with_output_extent(ProductionRankedOutputExtentSourceV1::new(
            1,
            local(2),
            extent,
            local(1),
        )),
        source().with_output_extent(ProductionRankedOutputExtentSourceV1::new(
            0,
            local(0),
            extent,
            local(1),
        )),
        source().with_output_extent(ProductionRankedOutputExtentSourceV1::new(
            0,
            local(2),
            ProductionRankedValueV1::Argument(1),
            local(1),
        )),
        source().with_output_extent(ProductionRankedOutputExtentSourceV1::new(
            0,
            local(2),
            extent,
            local(0),
        )),
    ];
    for (ordinal, row) in rows.into_iter().enumerate() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(37).unwrap();
        let (bytes, encoded_storage) =
            encode_production_ranked_source_rows_v1(&[row], &[], &mut budget).unwrap();
        budget
            .reserve_storage(encoded_storage.retained_storage())
            .unwrap();
        let (decoded, storage) =
            decode_production_ranked_source_rows_v1(&bytes, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(decoded.access_sources(), [row]);
        let expected = match ordinal {
            0 => Ok(()),
            1 => Err(JoinError::MissingExtentSource),
            _ => Err(JoinError::ExtentSource),
        };
        assert_eq!(
            extent_query(&kernel, decoded.access_sources(), extent, &mut budget),
            expected
        );
        drop(decoded);
        budget.release_storage(storage.retained_storage()).unwrap();
        drop(bytes);
        budget
            .release_storage(encoded_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn extent_proposal_requires_exact_source_view_index_and_operand() {
    let extent = ProductionRankedValueV1::Argument(0);
    let kernel = ranked_kernel(extent, access());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    for proposal in [
        ProductionRankedOutputExtentSourceV1::new(1, local(2), extent, local(1)),
        ProductionRankedOutputExtentSourceV1::new(0, local(0), extent, local(1)),
        ProductionRankedOutputExtentSourceV1::new(0, local(2), extent, local(0)),
        ProductionRankedOutputExtentSourceV1::new(
            0,
            local(2),
            ProductionRankedValueV1::Argument(1),
            local(1),
        ),
    ] {
        assert_eq!(
            extent_query(
                &kernel,
                &[source().with_output_extent(proposal)],
                extent,
                &mut budget
            ),
            Err(JoinError::ExtentSource)
        );
    }
    let local_extent = ranked_kernel(local(0), access());
    assert_eq!(
        extent_query(
            &local_extent,
            &[extent_source(local(0))],
            local(0),
            &mut budget
        ),
        Err(JoinError::ExtentSource)
    );
}

#[test]
fn conflicting_extent_meanings_and_duplicate_occurrences_are_refused() {
    let extent = ProductionRankedValueV1::Argument(0);
    let kernel = ranked_kernel(extent, access());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    let selected = extent_source(extent);
    for other in [
        ProductionRankedOutputExtentSourceV1::new(7, local(0), extent, local(1)),
        ProductionRankedOutputExtentSourceV1::new(
            0,
            local(2),
            ProductionRankedValueV1::Argument(1),
            local(1),
        ),
        selected.output_extent().unwrap(),
    ] {
        let rows = [
            selected,
            ProductionRankedAccessSourceV1::new(99, None, 0, 1, 0).with_output_extent(other),
        ];
        assert_eq!(
            extent_query(&kernel, &rows, extent, &mut budget),
            Err(JoinError::ConflictingExtentSource)
        );
    }
    assert_eq!(
        extent_query(&kernel, &[selected, selected], extent, &mut budget),
        Err(JoinError::AmbiguousSourceAccess)
    );
}

#[test]
fn unrelated_extent_uses_in_arithmetic_and_other_views_are_refused() {
    let extent = ProductionRankedValueV1::Argument(0);
    let kernel = ranked_kernel(extent, access());
    let rows = [extent_source(extent)];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    for extra in [
        ProductionRankedOperationV1::IndexBinary {
            result: ProductionRankedValueIdV1::new(4),
            kind: dialect_kernel::IndexBinaryKindAttr::Add,
            lhs: local(1),
            rhs: extent,
        },
        ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(4),
            element_width: 32,
            writable: true,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![extent],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: 2,
            noalias_class: 2,
        },
    ] {
        let changed = change_ranked_block(&kernel, 0, |ops, _| ops.push(extra));
        assert_eq!(
            extent_query(&changed, &rows, extent, &mut budget),
            Err(JoinError::ExtentUse)
        );
    }
}

#[test]
fn ranked_constructor_rejects_extent_as_a_semantic_store_value_before_the_join() {
    let extent = ProductionRankedValueV1::Argument(0);
    let kernel = ranked_kernel(extent, access());
    let mut blocks = kernel.blocks().to_vec();
    blocks[2] = ProductionRankedBlockV1::new(
        vec![ProductionRankedOperationV1::ValueAccess {
            kind: AccessKindAttr::Write,
            view: local(2),
            indices: vec![local(1)],
            value: extent,
        }],
        ProductionRankedTerminatorV1::Branch { target: 1 },
    );
    assert!(matches!(
        ProductionRankedKernelV1::new("inert_extent_value", 2, blocks),
        Err(fe2o3_pliron::ProductionRankedKernelErrorV1::ExpectedSemantic(value))
            if value == extent
    ));
}

#[test]
fn extent_index_must_be_dynamic_global_x_and_guard_must_target_the_write() {
    let extent = ProductionRankedValueV1::Argument(0);
    let kernel = ranked_kernel(extent, access());
    let rows = [extent_source(extent)];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    let extra_index = change_ranked_block(&kernel, 0, |ops, _| {
        ops.push(ProductionRankedOperationV1::InvocationIndex {
            result: ProductionRankedValueIdV1::new(4),
            dimension: 0,
            launch_extent: 0,
        });
    });
    assert_eq!(
        extent_query(&extra_index, &rows, extent, &mut budget),
        Err(JoinError::ExtentIndex)
    );
    for (dimension, launch_extent) in [(1, 0), (0, 64)] {
        let changed = change_ranked_block(&kernel, 0, |ops, _| {
            ops[1] = ProductionRankedOperationV1::InvocationIndex {
                result: ProductionRankedValueIdV1::new(1),
                dimension,
                launch_extent,
            };
        });
        assert_eq!(
            extent_query(&changed, &rows, extent, &mut budget),
            Err(JoinError::ExtentIndex)
        );
    }
    let changed = change_ranked_block(&kernel, 0, |ops, _| {
        ops[1] = ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(1),
            value: 0,
        };
    });
    assert_eq!(
        extent_query(&changed, &rows, extent, &mut budget),
        Err(JoinError::ExtentIndex)
    );
    for (terminator, error) in [
        (ProductionRankedTerminatorV1::Return, JoinError::ExtentGuard),
        (
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs: local(1),
                rhs: extent,
                true_block: 1,
                false_block: 2,
            },
            JoinError::ExtentGuard,
        ),
        (
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs: local(0),
                rhs: extent,
                true_block: 2,
                false_block: 1,
            },
            JoinError::ExtentUse,
        ),
        (
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs: local(1),
                rhs: extent,
                true_block: 2,
                false_block: 2,
            },
            JoinError::ExtentUse,
        ),
        (
            ProductionRankedTerminatorV1::IndexEqual {
                lhs: local(1),
                rhs: extent,
                true_block: 2,
                false_block: 1,
            },
            JoinError::ExtentUse,
        ),
    ] {
        let changed = change_ranked_block(&kernel, 3, |_, term| *term = terminator);
        assert_eq!(
            extent_query(&changed, &rows, extent, &mut budget),
            Err(error)
        );
    }
}

#[test]
fn extent_rederivation_charges_same_ledger_and_never_changes_storage() {
    let extent = ProductionRankedValueV1::Argument(0);
    let kernel = ranked_kernel(extent, access());
    let rows = [extent_source(extent)];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 37);
    budget.reserve_storage(37).unwrap();
    budget.charge_work(17).unwrap();
    extent_query(&kernel, &rows, extent, &mut budget).unwrap();
    let exact = budget.work();
    for (limit, succeeds) in [(17, false), (exact - 1, false), (exact, true)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, 37);
        budget.reserve_storage(37).unwrap();
        budget.charge_work(17).unwrap();
        let outcome = extent_query(&kernel, &rows, extent, &mut budget);
        if succeeds {
            outcome.unwrap();
        } else {
            assert!(matches!(
                outcome,
                Err(JoinError::Resource(ResourceError::Work(_)))
            ));
        }
        assert_eq!(budget.storage(), 37);
        assert_eq!(budget.peak_storage(), 37);
        budget.release_storage(37).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
