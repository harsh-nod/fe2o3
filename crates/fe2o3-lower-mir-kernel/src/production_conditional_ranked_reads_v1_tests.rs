// Exercises the read join used by the consuming continuation. These candidate
// checks alone are not source authentication or a final-graph bounds proof.
fn canonical_read_fixture() -> (Module, SemanticKirCorrespondenceV1) {
    let (mut module, mut correspondence) = canonical_fixture();
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    module.functions[0].signature.parameters.push(Type::slice(
        scalar.clone(),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    ));
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[0].operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(21), pointer.clone()),
            OperationKind::SliceData { slice: ValueId(20) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(22), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(21),
                offset: ValueId(2),
            },
        ),
    ]);
    body.blocks[2].operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(23), scalar),
            OperationKind::Load {
                pointer: ValueId(22),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    );
    let OperationKind::Store { value, .. } = &mut body.blocks[2].operations[1].kind else {
        unreachable!()
    };
    *value = ValueId(23);
    correspondence.statement_operation_spans[0].operation_count = 2;
    (module, correspondence)
}

#[test]
fn exact_canonical_read_join_checks_origin_index_space_width_and_source_occurrence() {
    let (module, correspondence) = canonical_read_fixture();
    let verified = verify_module_ref(&module).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let fe2o3_kernel_ir::ConditionalTotalViewAnalysisV1::Established(facts) =
        derive_conditional_total_view_from_verified_v1(
            verified,
            &KernelId::new("coverage"),
            &mut budget,
        )
        .unwrap()
    else {
        panic!("canonical read coverage");
    };
    let mut read = None;
    facts
        .visit_reads_v1(&mut budget, |value| {
            read = Some(value);
            Ok(())
        })
        .unwrap();
    let read = read.unwrap();
    assert_eq!(
        read.access_domain(),
        ConditionalTotalViewAddressDomainV1::GuardedOutput
    );
    assert_eq!(
        read.address_domain(),
        ConditionalTotalViewAddressDomainV1::GlobalLaunch
    );
    let source_site = canonical_store_source(
        &module.functions[0],
        &correspondence,
        &correspondence.lowered_functions[0],
        read.location(),
        AccessKindAttr::Read,
        &mut budget,
    )
    .unwrap();
    for change in 0..6 {
        let mut blocks = ranked_kernel(
            ProductionRankedValueV1::Argument(0),
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::Read,
                view: local(4),
                indices: vec![local(if change == 2 { 0 } else { 1 })],
            },
        )
        .blocks()
        .to_vec();
        let mut entry = blocks[0].operations().to_vec();
        let (space, origin, class) = if change == 4 {
            let (origin, class) =
                dialect_kernel::neutral_workgroup_allocation_contract_v1([24; 32]);
            (MemorySpaceAttr::Workgroup, origin, class)
        } else {
            (MemorySpaceAttr::Global, if change == 1 { 4 } else { 3 }, 3)
        };
        entry.push(ProductionRankedOperationV1::ViewInSpace {
            result: ProductionRankedValueIdV1::new(4),
            element_width: if change == 3 { 64 } else { 32 },
            writable: false,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![ProductionRankedValueV1::Argument(1)],
            memory_space: space,
            allocation_origin: origin,
            noalias_class: class,
        });
        blocks[0] = ProductionRankedBlockV1::new(entry, blocks[0].terminator().clone());
        let kernel = ProductionRankedKernelV1::new("read_join", 2, blocks).unwrap();
        let sources = [ProductionRankedAccessSourceV1::new(
            source_site.block,
            source_site.statement,
            source_site.ordinal,
            2,
            if change == 5 { 1 } else { 0 },
        )];
        let source = ranked_source(&sources, source_site, &mut budget).unwrap();
        let result = check_ranked_read_v1(
            candidate(&kernel, &sources),
            source,
            read,
            local(1),
            2,
            &mut budget,
        );
        if change == 0 {
            assert_eq!(result, Ok(local(4)));
        } else {
            assert!(result.is_err(), "mutation {change}");
        }
    }
}
