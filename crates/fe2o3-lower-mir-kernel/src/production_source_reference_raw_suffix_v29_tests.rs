fn suffix_admit(
    base: &ProductionSemanticSsaOwnerV1,
    types: Vec<SemanticTypeDeclV1>,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let semantic = base.source_semantic();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(), types, vec![], vec![], vec![],
        vec![function(220, SemanticFunctionRoleV1::KernelRoot,
            semantic.functions()[0].abi().clone(), locals, blocks)],
        vec![SemanticCallableDeclV1::defined(ROOT)], vec![ROOT],
    ).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap()
}

fn with_suffix_plan(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(&SourceReferencePlanV29<'_, '_>, &mut ArgumentBudgetV1<'_>)
        -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_live_projection_builder(owner, |builder, budget| consume(&builder.plan, budget))
}

fn suffix_project(local: u32, steps: &[(SemanticProjectionKindV1, SemanticTypeIdV1)]) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local),
        steps.iter().map(|&(kind, ty)| SemanticProjectionV1::new(kind, ty).unwrap()).collect(),
        steps.last().unwrap().1).unwrap()
}

fn suffix_live(local: u32, live: bool) -> SemanticStatementV1 {
    SemanticStatementV1::new(source(), if live {
        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(local))
    } else { SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local)) })
}

fn suffix_load(destination: u32, target: SemanticPlaceV1) -> SemanticStatementV1 {
    assign(place(destination, target.ty()), SemanticRvalueKindV1::Load(
        SemanticMemoryLoadV1::new(target, SemanticVolatilityV1::NonVolatile, None)))
}

fn suffix_nested_owner(mode: u8) -> ProductionSemanticSsaOwnerV1 {
    let base = current_projection_owner();
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let pair = SemanticTypeIdV1::from_index(3);
    let array = SemanticTypeIdV1::from_index(4);
    let raw = reference(&mut types, array, SemanticMutabilityV1::Mutable, true);
    let root = &semantic.functions()[0];
    let mut locals = root.locals().to_vec();
    locals[3] = local(224, raw, SemanticLocalRoleV1::Temporary);
    locals[4] = local(225, raw, SemanticLocalRoleV1::Temporary);
    // Keep the inherited raw-U32 type in this fixture's exact local closure.
    locals.push(local(228, root.locals()[3].ty(), SemanticLocalRoleV1::Temporary));
    let mut statements = root.blocks()[0].statements()[..3].to_vec();
    statements.extend([
        assign(place(3, raw), SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Mutable, place: place(2, array),
        }),
        assign(place(4, raw), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, raw)))),
    ]);
    let component = |local, raw| {
        let mut steps = Vec::new();
        if raw { steps.push((SemanticProjectionKindV1::Dereference, array)); }
        steps.extend([
            (SemanticProjectionKindV1::ConstantIndex { offset: 1, minimum_length: 2, from_end: false }, pair),
            (SemanticProjectionKindV1::Field(0), U32),
        ]);
        suffix_project(local, &steps)
    };
    match mode {
        0 => statements.push(assign(component(2, false), SemanticRvalueKindV1::Use(address_literal(19)))),
        1 => statements.push(assign(component(4, true), SemanticRvalueKindV1::Use(address_literal(21)))),
        2 => statements.push(SemanticStatementV1::new(source(), SemanticStatementKindV1::Deinitialize(component(2, false)))),
        3 => statements.extend([suffix_live(2, false), suffix_live(2, true)]),
        _ => unreachable!(),
    }
    statements.push(suffix_load(5, component(4, true)));
    suffix_admit(&base, types, locals, vec![block(231, statements, SemanticTerminatorKindV1::Return)])
}

#[derive(Clone, Copy, Debug)]
enum SuffixChain {
    Read,
    Write,
    SharedHolderWrite,
    ImmutableTargetWrite,
    MutableReferenceThroughShared,
    IntermediateDeinit,
    FinalDeinit,
    HolderRestart,
    TargetRestart,
    Retarget,
}

fn suffix_chain_owner(mode: SuffixChain) -> ProductionSemanticSsaOwnerV1 {
    let immutable = matches!(mode, SuffixChain::ImmutableTargetWrite);
    let base = address_owner(if immutable { AddressFlow::SharedWrite } else { AddressFlow::Read });
    let semantic = base.source_semantic();
    let mut types = semantic.types()[..2].to_vec();
    let reference_target = matches!(mode, SuffixChain::MutableReferenceThroughShared);
    let inner = reference(&mut types, U32,
        if immutable { SemanticMutabilityV1::Immutable } else { SemanticMutabilityV1::Mutable },
        !reference_target);
    let shared = matches!(mode, SuffixChain::SharedHolderWrite);
    let outer = reference(&mut types, inner, SemanticMutabilityV1::Immutable, !shared);
    let root = &semantic.functions()[0];
    let mut locals = root.locals().to_vec();
    locals[3] = local(224, inner, SemanticLocalRoleV1::Temporary);
    locals[4] = local(225, inner, SemanticLocalRoleV1::Temporary);
    locals.push(local(227, outer, SemanticLocalRoleV1::Temporary));
    if matches!(mode, SuffixChain::Retarget) {
        locals.push(local(228, U32, SemanticLocalRoleV1::Temporary));
    }
    let initialize = || assign(place(2, U32), SemanticRvalueKindV1::Use(address_literal(7)));
    let form = || assign(place(3, inner), if reference_target {
        SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Mutable, place: place(2, U32) }
    } else { SemanticRvalueKindV1::AddressOf {
        mutability: if immutable { SemanticMutabilityV1::Immutable } else { SemanticMutabilityV1::Mutable },
        place: place(2, U32),
    } });
    let mut statements = vec![suffix_live(2, true), initialize(), suffix_live(3, true), form(),
        assign(place(6, outer), if shared {
            SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Shared, place: place(3, inner) }
        } else { SemanticRvalueKindV1::AddressOf {
            mutability: SemanticMutabilityV1::Immutable, place: place(3, inner),
        } })];
    match mode {
        SuffixChain::IntermediateDeinit => statements.push(SemanticStatementV1::new(source(),
            SemanticStatementKindV1::Deinitialize(place(3, inner)))),
        SuffixChain::FinalDeinit => statements.push(SemanticStatementV1::new(source(),
            SemanticStatementKindV1::Deinitialize(place(2, U32)))),
        SuffixChain::HolderRestart => statements.extend([suffix_live(3, false), suffix_live(3, true), form()]),
        SuffixChain::TargetRestart => statements.extend([suffix_live(2, false), suffix_live(2, true), initialize()]),
        SuffixChain::Retarget => statements.extend([
            suffix_live(7, true),
            assign(place(7, U32), SemanticRvalueKindV1::Use(address_literal(47))),
            assign(place(3, inner), SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable, place: place(7, U32),
            }),
        ]),
        _ => {}
    }
    let target = || suffix_project(6, &[
        (SemanticProjectionKindV1::Dereference, inner),
        (SemanticProjectionKindV1::Dereference, U32),
    ]);
    if matches!(mode, SuffixChain::Write | SuffixChain::SharedHolderWrite
        | SuffixChain::ImmutableTargetWrite | SuffixChain::MutableReferenceThroughShared)
    {
        statements.push(assign(target(), SemanticRvalueKindV1::Use(address_literal(23))));
    }
    statements.push(suffix_load(5, target()));
    suffix_admit(&base, types, locals, vec![block(231, statements, SemanticTerminatorKindV1::Return)])
}

fn suffix_source<'source>(plan: &SourceReferencePlanV29<'_, 'source>, row: &SourceReferenceRawAccessV29) -> &'source SemanticPlaceV1 {
    let declaration = plan.instances.instance(row.site.instance).unwrap().declaration();
    let statement = &declaration.blocks()[row.site.block.index() as usize].statements()[row.site.statement.unwrap()];
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else { panic!("original assignment") };
    match assignment.value().kind() {
        SemanticRvalueKindV1::Load(load) => load.source(),
        SemanticRvalueKindV1::Use(_) => assignment.destination(),
        _ => panic!("original suffix operation"),
    }
}

#[test]
fn original_raw_suffix_preserves_boundary_pointee_and_current_nested_leaf() {
    for mode in [0, 1] {
        let result = with_suffix_plan(
            suffix_nested_owner(mode), |plan, budget| {
                assert_eq!(plan.raw_origins.len(), 1);
                assert_eq!(plan.raw_accesses.len(), if mode == 0 { 1 } else { 2 });
                for (key, row) in &plan.raw_accesses {
                    let source = suffix_source(plan, row);
                    assert_eq!(key.1, 0);
                    assert_eq!(row.projection, 0);
                    assert_eq!(row.pointee, SemanticTypeIdV1::from_index(4));
                    assert_eq!(row.ty, U32);
                    assert!(row.access == row.crossing);
                    assert_eq!(source.projections().len(), 3);
                    assert!(plan.raw_projection_range(row.set, row.pointee, budget)?.is_empty());
                    let path = plan.raw_source_path(row.site, source, row.access, budget)?.unwrap();
                    assert_eq!(path.last_dereference, 0);
                }
                Ok(())
            });
        assert!(result.is_ok(), "mode {mode}: {result:?}");
    }
}

#[test]
fn original_raw_suffix_does_not_initialize_a_deinitialized_or_restarted_leaf() {
    for mode in [2, 3] {
        let result = with_suffix_plan(
            suffix_nested_owner(mode), |_, _| panic!("invalid suffix cannot reach consumer"));
        assert!(result.is_err(), "mode {mode}");
    }
}

#[test]
fn original_raw_chain_reads_holders_without_widening_the_final_target() {
    for mode in [SuffixChain::Read, SuffixChain::Write, SuffixChain::SharedHolderWrite] {
        let result = with_suffix_plan(
            suffix_chain_owner(mode), |plan, _| {
                let write: Vec<_> = plan.raw_accesses.values().filter(|row| row.access == SourceReferenceAccessV29::Write).collect();
                if matches!(mode, SuffixChain::Write) {
                    assert_eq!(write.len(), 2);
                    assert!(write.iter().any(|row| row.projection == 0 && row.crossing == SourceReferenceAccessV29::Read));
                    assert!(write.iter().any(|row| row.projection == 1 && row.crossing == SourceReferenceAccessV29::Write));
                } else if matches!(mode, SuffixChain::SharedHolderWrite) {
                    assert_eq!(write.len(), 1);
                    assert_eq!(write[0].projection, 1);
                    assert!(write[0].crossing == SourceReferenceAccessV29::Write);
                    assert!(!plan.loans.is_empty());
                } else { assert!(write.is_empty()); }
                let mut ordinals: Vec<_> = plan.raw_accesses.values().map(|row| row.ordinal).collect();
                ordinals.sort_unstable();
                assert_eq!(ordinals, (0..ordinals.len()).collect::<Vec<_>>());
                for (key, row) in &plan.raw_accesses {
                    assert_eq!(key.1, row.projection);
                    assert_eq!(row.ty, U32);
                    assert_eq!(row.pointee, if row.projection == 0 { SemanticTypeIdV1::from_index(2) } else { U32 });
                }
                Ok(())
            });
        assert!(result.is_ok(), "{mode:?}: {result:?}");
    }
}

#[test]
fn original_raw_chain_rejects_shared_reference_upgrade_and_each_dead_boundary() {
    for mode in [SuffixChain::ImmutableTargetWrite, SuffixChain::MutableReferenceThroughShared,
        SuffixChain::IntermediateDeinit, SuffixChain::FinalDeinit,
        SuffixChain::HolderRestart, SuffixChain::TargetRestart]
    {
        let result = with_suffix_plan(
            suffix_chain_owner(mode), |_, _| panic!("invalid boundary cannot reach consumer"));
        assert!(result.is_err(), "{mode:?}");
    }
}

#[test]
fn original_raw_suffix_classification_rejects_foreign_source_and_site() {
    for changed in 0..3 {
        with_current_projection_builder(suffix_nested_owner(0), false, |builder, budget| {
            let instance = builder.plan.instances.root();
            let declaration = builder.plan.instances.instance(instance).unwrap().declaration();
            let index = declaration.blocks()[0].statements().len() - 1;
            let SemanticStatementKindV1::Assign(assignment) = declaration.blocks()[0].statements()[index].kind() else { unreachable!() };
            let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else { unreachable!() };
            let mut site = SourceReferenceSiteV29 { instance, block: SemanticBlockIdV1::from_index(0), statement: Some(index) };
            let clone = load.source().clone();
            if changed == 1 { site.statement = Some(index - 1); }
            if changed == 2 { site.statement = None; }
            let result = builder.plan.raw_source_path(site, if changed == 0 { &clone } else { load.source() }, SourceReferenceAccessV29::Read, budget);
            assert!(result.is_err());
            assert!(builder.plan.raw_accesses.is_empty());
            Ok(())
        }).unwrap();
    }
}

#[test]
fn original_raw_boundaries_are_reused_only_with_the_same_original_payload() {
    with_live_projection_builder(suffix_chain_owner(SuffixChain::Write), |builder, budget| {
        let rows: Vec<_> = builder.plan.raw_accesses.values().map(|row| **row).collect();
        let count = rows.len();
        for row in rows {
            let source = suffix_source(&builder.plan, &row);
            let path = builder.plan.raw_source_path(row.site, source, row.access, budget)?.unwrap();
            builder.retain_raw_access(row.site, source, row.access, &path, row.projection, row.set, row.holder, budget)?;
        }
        assert_eq!(builder.plan.raw_accesses.len(), count);
        Ok(())
    }).unwrap();
}

#[test]
fn original_raw_boundary_rejects_changed_crossing_pointee_and_leaf_metadata() {
    for changed in 0..3 {
        let mut reached = false;
        let result = with_live_projection_builder(suffix_chain_owner(SuffixChain::Read), |builder, budget| {
            reached = true;
            let (key, row) = builder.plan.raw_accesses.iter().next().map(|(key, row)| (**key, **row)).unwrap();
            let source = suffix_source(&builder.plan, &row);
            let path = builder.plan.raw_source_path(row.site, source, row.access, budget)?.unwrap();
            let old = builder.plan.raw_accesses.get_mut(&key).unwrap();
            match changed {
                0 => old.crossing = SourceReferenceAccessV29::Write,
                1 => old.pointee = UNIT,
                2 => old.ty = UNIT,
                _ => unreachable!(),
            }
            assert!(builder.retain_raw_access(row.site, source, row.access, &path,
                row.projection, row.set, row.holder, budget).is_err());
            assert_eq!(builder.plan.raw_accesses.len(), 2);
            Ok(())
        });
        assert!(reached);
        assert!(result.is_err(), "sticky original custody: {changed}");
    }
}

#[test]
fn original_raw_holder_revisit_rejects_changed_locator_without_publishing() {
    for changed in 0..8 {
        let mut completed = false;
        let mut expected_resource = None;
        let result = with_live_projection_builder(suffix_chain_owner(SuffixChain::Read), |builder, budget| {
            let (key, row) = builder.plan.raw_accesses.iter().next().map(|(key, row)| (**key, **row)).unwrap();
            expected_resource = Some(if changed == 3 && row.holder.count != 0 {
                ArgumentResourceV1::Arithmetic
            } else { ArgumentResourceV1::Accounting });
            let source = suffix_source(&builder.plan, &row);
            let path = builder.plan.raw_source_path(row.site, source, row.access, budget)?.unwrap();
            let mut holder = row.holder;
            match changed {
                0 => holder.instance = ProductionCallInstanceIdV1(usize::MAX),
                1 => holder.local = SemanticLocalIdV1::from_index(u32::MAX),
                2 => holder.node = usize::MAX,
                3 => holder.first = usize::MAX,
                4 => holder.parent = Some(usize::MAX),
                5 => holder.shared_path = !holder.shared_path,
                6 => holder.selector_source = Some((row.site, usize::MAX)),
                7 => holder.generation = u32::MAX,
                _ => unreachable!(),
            }
            let error = builder.retain_raw_access(row.site, source, row.access, &path,
                row.projection, row.set, holder, budget).unwrap_err();
            let retained = &builder.plan.raw_accesses[&key];
            assert_eq!((retained.set, retained.holder.node, retained.holder.generation),
                (row.set, row.holder.node, row.holder.generation));
            assert_eq!(builder.plan.raw_accesses.len(), 2);
            if changed == 7 {
                assert!(matches!(&error, ProductionSemanticKirErrorV1::Unsupported {
                    function: 0, block: None, statement: None,
                    detail: "source address has no live storage activation",
                }), "dead activation is a semantic refusal: {error:?}");
                completed = true;
                return Err(error);
            }
            assert!(matches!(&error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(actual)
                if Some(*actual) == expected_resource),
                "forged locator custody must leave a sticky resource refusal: {error:?}");
            // Only resource/custody failures are sticky when swallowed.
            completed = true;
            Ok(())
        });
        assert!(completed, "fault {changed} must finish every inner assertion: {result:?}");
        if changed == 7 {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0, block: None, statement: None,
                detail: "source address has no live storage activation",
            })), "semantic holder refusal must survive scope cleanup: {result:?}");
        } else {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(actual))
                if Some(actual) == expected_resource),
                "resource holder refusal must remain exact when swallowed: {result:?}");
        }
    }
}

fn suffix_enum_owner(changed: u8) -> ProductionSemanticSsaOwnerV1 {
    let base = address_owner(AddressFlow::Read);
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1), SemanticScalarValidityRangeV1::new(0, 255));
    let variants = (0..2).map(|variant| SemanticEnumVariantLayoutV1::from_rustc(
        variant, 8, 4, SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
        SemanticBackendReprV1::memory(true), None, false, None, 4, u64::from(variant),
        SemanticAggregateLayoutV1::new(vec![4], vec![SemanticPaddingV1::new(1, 3).unwrap()]).unwrap(),
    ).unwrap()).collect();
    let enumeration = declaration(&mut types,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(8, 4, SemanticBackendReprV1::memory(true),
            false, SemanticEnumLayoutV1::new(variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag))).unwrap()).unwrap(),
        SemanticTypeShapeV1::enum_type(U32, [3, 17].into_iter().map(|tag|
            SemanticEnumVariantV1::new(tag, SemanticAggregateTypeV1::new(vec![U32]).unwrap())).collect()).unwrap(), None);
    let raw = reference(&mut types, enumeration, SemanticMutabilityV1::Mutable, true);
    let mut locals = semantic.functions()[0].locals().to_vec();
    locals[2] = local(223, enumeration, SemanticLocalRoleV1::Temporary);
    locals[3] = local(224, raw, SemanticLocalRoleV1::Temporary);
    // The unused inherited local keeps the raw-U32 type in the exact closure.
    locals.push(local(227, U32, SemanticLocalRoleV1::Temporary));
    let go = |target| SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto, SemanticBlockIdV1::from_index(target)));
    let choose = |local, value, yes, no| SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(local, U32)),
        targets: SemanticSwitchTargetsV1::new(vec![SemanticSwitchTargetV1::new(value,
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::SwitchValue, SemanticBlockIdV1::from_index(yes)))],
            SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::SwitchOtherwise, SemanticBlockIdV1::from_index(no))).unwrap(),
    };
    let construct = |variant| assign(place(2, enumeration), SemanticRvalueKindV1::Aggregate(
        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::EnumVariant(variant), vec![address_literal(31)]).unwrap()));
    let exit = |variant| {
        let mut statements = Vec::new();
        if changed == 1 {
            statements.push(SemanticStatementV1::new(source(), SemanticStatementKindV1::SetDiscriminant {
                place: place(2, enumeration), variant_index: 1 - variant,
            }));
        } else if changed == 2 {
            statements.push(SemanticStatementV1::new(source(), SemanticStatementKindV1::Deinitialize(
                suffix_project(2, &[(SemanticProjectionKindV1::Downcast(variant), enumeration),
                    (SemanticProjectionKindV1::Field(0), U32)]))));
        }
        statements.push(suffix_load(6, suffix_project(3, &[
            (SemanticProjectionKindV1::Dereference, enumeration),
            (SemanticProjectionKindV1::Downcast(variant), enumeration),
            (SemanticProjectionKindV1::Field(0), U32),
        ])));
        block(235 + variant as u8, statements, SemanticTerminatorKindV1::Return)
    };
    suffix_admit(&base, types, locals, vec![
        block(231, vec![suffix_live(2, true)], choose(1, 0, 1, 2)),
        block(232, vec![construct(0)], go(3)),
        block(233, vec![construct(1)], go(3)),
        block(234, vec![
            assign(place(3, raw), SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable, place: place(2, enumeration),
            }),
            assign(place(5, U32), SemanticRvalueKindV1::Discriminant(place(2, enumeration))),
        ], choose(5, 3, 4, 5)),
        exit(0), exit(1),
    ])
}

#[test]
fn original_raw_enum_suffix_requires_the_current_guard_and_payload_state() {
    let result = with_suffix_plan(
        suffix_enum_owner(0), |plan, _| {
            assert_eq!(plan.raw_accesses.len(), 2);
            for row in plan.raw_accesses.values() {
                assert_eq!(row.pointee, SemanticTypeIdV1::from_index(3));
                assert_eq!(row.ty, U32);
                assert!(row.crossing == SourceReferenceAccessV29::Read);
            }
            Ok(())
        });
    assert!(result.is_ok(), "{result:?}");
    for changed in [1, 2] {
        let result = with_suffix_plan(
            suffix_enum_owner(changed), |_, _| panic!("stale enum suffix must not reach consumer"));
        assert!(result.is_err(), "changed {changed}");
    }
}

#[test]
fn original_suffix_classification_prepays_the_actual_live_header() {
    for short in [true, false] {
        with_current_projection_builder(suffix_nested_owner(0), false, |builder, budget| {
            let instance = builder.plan.instances.root();
            let declaration = builder.plan.instances.instance(instance).unwrap().declaration();
            let index = declaration.blocks()[0].statements().len() - 1;
            let SemanticStatementKindV1::Assign(assignment) = declaration.blocks()[0].statements()[index].kind() else { unreachable!() };
            let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else { unreachable!() };
            let site = SourceReferenceSiteV29 { instance, block: SemanticBlockIdV1::from_index(0), statement: Some(index) };
            // Independent peak oracle, including the completed query's scratch.
            let scratch = 6 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>();
            let header = std::mem::size_of::<Option<SourceReferenceRawPathV29>>()
                + 2 * std::mem::size_of::<Result<Option<SourceReferenceRawPathV29>, ProductionSemanticKirErrorV1>>()
                + std::mem::size_of::<(bool, Option<usize>)>()
                + std::mem::size_of::<Option<SourceReferenceAccessV29>>()
                + 2 * std::mem::size_of::<Result<Option<SourceReferenceAccessV29>, ProductionSemanticKirErrorV1>>()
                + scratch;
            budget.reserve_storage(ADDRESS_TEST_LIMIT - budget.storage() - (header - usize::from(short)))?;
            let before = budget.storage();
            let result = builder.plan.raw_source_path(site, load.source(), SourceReferenceAccessV29::Read, budget);
            assert_eq!(result.is_ok(), !short);
            assert!(builder.plan.raw_accesses.is_empty());
            if !short {
                assert_eq!(budget.peak_storage(), before + header);
                assert_eq!(budget.storage(), before + header - scratch);
            }
            Ok(())
        }).unwrap();
    }
}

#[test]
fn original_suffix_classification_has_a_linear_independent_work_boundary() {
    for short in [0, 1] {
        with_current_projection_builder(suffix_nested_owner(0), false, |builder, budget| {
            let instance = builder.plan.instances.root();
            let declaration = builder.plan.instances.instance(instance).unwrap().declaration();
            let index = declaration.blocks()[0].statements().len() - 1;
            let SemanticStatementKindV1::Assign(assignment) = declaration.blocks()[0].statements()[index].kind() else { unreachable!() };
            let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else { unreachable!() };
            let site = SourceReferenceSiteV29 { instance, block: SemanticBlockIdV1::from_index(0), statement: Some(index) };
            // Owner5 + declaration5 + three per original projection + site2
            // + query scope2 + one destination3 + one load source3. No suffix rescans.
            let work = 20 + 3 * load.source().projections().len();
            budget.charge_work(ADDRESS_TEST_LIMIT - budget.work() - (work - short))?;
            let before = budget.work();
            let result = builder.plan.raw_source_path(site, load.source(), SourceReferenceAccessV29::Read, budget);
            if short == 0 {
                assert_eq!(result?.unwrap().last_dereference, 0);
                assert_eq!(budget.work() - before, work);
            } else {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))));
                assert!(builder.plan.raw_source_path(site, load.source(), SourceReferenceAccessV29::Read, budget).is_err());
            }
            assert!(builder.plan.raw_accesses.is_empty());
            Ok(())
        }).unwrap();
    }
}

#[test]
fn original_raw_boundary_publication_prepays_larger_key_row_and_map_without_partial_insert() {
    use std::mem::size_of;
    for storage in [false, true] {
        for short in [0, 1] {
            let mut reached = false;
            let result = with_live_projection_builder(suffix_nested_owner(0), |builder, budget| {
                reached = true;
                let (key, row) = builder.plan.raw_accesses.iter().next().map(|(key, row)| (**key, **row)).unwrap();
                let source = suffix_source(&builder.plan, &row);
                let path = builder.plan.raw_source_path(row.site, source, row.access, budget)?.unwrap();
                assert!(builder.plan.raw_accesses.remove(&key).is_some());
                assert!(builder.plan.raw_accesses.is_empty());
                // Original map allocator reserves two split levels at empty.
                // Mirror concrete envelopes, boxed node tuple and both owners.
                let bytes = size_of::<SourceReferenceRawAccessV29>()
                    + 2 * size_of::<Result<SourceReferenceRawAccessV29, ProductionSemanticKirErrorV1>>()
                    + size_of::<SourceReferenceRawAccessKeyV29>()
                    + 2 * size_of::<Result<SourceReferenceRawAccessKeyV29, ProductionSemanticKirErrorV1>>()
                    + 2 * 32 * size_of::<(Box<SourceReferenceRawAccessKeyV29>, Box<SourceReferenceRawAccessV29>, usize)>()
                    + size_of::<SourceReferenceRawAccessKeyV29>()
                    + size_of::<SourceReferenceRawAccessV29>();
                // Owner5, original adjacent boundary8, holder5, lookup32, insert lookup32.
                let work = 82;
                if storage {
                    budget.reserve_storage(ADDRESS_TEST_LIMIT - budget.storage() - (bytes - short))?;
                } else {
                    budget.charge_work(ADDRESS_TEST_LIMIT - budget.work() - (work - short))?;
                }
                let before = (budget.storage(), budget.work());
                let recorded = builder.retain_raw_access(row.site, source, row.access, &path,
                    row.projection, row.set, row.holder, budget);
                if short == 0 {
                    recorded?;
                    assert_eq!(budget.storage() - before.0, bytes);
                    assert_eq!(budget.work() - before.1, work);
                    assert_eq!(builder.plan.raw_accesses.len(), 1);
                    assert_eq!(builder.plan.raw_accesses[&key].ordinal, 0);
                } else {
                    assert!(matches!(recorded, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))));
                    assert!(builder.plan.raw_accesses.is_empty());
                    assert!(builder.retain_raw_access(row.site, source, row.access, &path,
                        row.projection, row.set, row.holder, budget).is_err());
                    assert!(builder.plan.raw_accesses.is_empty());
                }
                Ok(())
            });
            assert!(reached);
            assert_eq!(result.is_ok(), short == 0, "storage {storage} short {short}: {result:?}");
        }
    }
}

#[test]
fn growing_boxed_raw_accesses_keep_original_rows_and_independent_insertion_debits() {
    use std::mem::size_of;
    for count in [1usize, 2, 32, 64, 128] {
        let base = suffix_nested_owner(0);
        let semantic = base.source_semantic();
        let original = &semantic.functions()[0];
        let source_block = &original.blocks()[0];
        let mut statements = source_block.statements()[..source_block.statements().len() - 1].to_vec();
        let read = source_block.statements().last().unwrap();
        for _ in 0..count {
            statements.push(read.clone());
        }
        let owner = suffix_admit(&base, semantic.types().to_vec(), original.locals().to_vec(),
            vec![block(231, statements, SemanticTerminatorKindV1::Return)]);
        let mut completed = false;
        with_live_projection_builder(owner, |builder, budget| {
            assert_eq!(builder.plan.raw_accesses.len(), count);
            let mut rows: Vec<_> = builder.plan.raw_accesses.iter()
                .map(|(key, row)| (**key, **row)).collect();
            rows.sort_unstable_by_key(|(_, row)| row.ordinal);
            builder.plan.raw_accesses.clear();
            let envelopes = size_of::<SourceReferenceRawAccessV29>()
                + 2 * size_of::<Result<SourceReferenceRawAccessV29, ProductionSemanticKirErrorV1>>()
                + size_of::<SourceReferenceRawAccessKeyV29>()
                + 2 * size_of::<Result<SourceReferenceRawAccessKeyV29, ProductionSemanticKirErrorV1>>();
            let mut addresses = Vec::new();
            for (ordinal, (key, row)) in rows.iter().enumerate() {
                assert_eq!(row.ordinal, ordinal);
                let source = suffix_source(&builder.plan, row);
                let path = builder.plan.raw_source_path(row.site, source, row.access, budget)?.unwrap();
                let before = (budget.storage(), budget.work());
                builder.retain_raw_access(row.site, source, row.access, &path,
                    row.projection, row.set, row.holder, budget)?;
                let levels = if ordinal == 0 { 2 } else {
                    usize::BITS as usize - ordinal.leading_zeros() as usize + 1
                };
                let nodes = levels * 32
                    * size_of::<(Box<SourceReferenceRawAccessKeyV29>, Box<SourceReferenceRawAccessV29>, usize)>();
                assert_eq!(budget.storage() - before.0,
                    envelopes + nodes + size_of::<SourceReferenceRawAccessKeyV29>()
                        + size_of::<SourceReferenceRawAccessV29>());
                assert_eq!(budget.work() - before.1, 18 + 32 * levels);
                let published = &builder.plan.raw_accesses[key];
                assert_eq!((published.ordinal, published.site, published.source, published.access,
                    published.crossing, published.projection, published.set, published.pointee, published.ty),
                    (ordinal, row.site, row.source, row.access, row.crossing, row.projection,
                        row.set, row.pointee, row.ty));
                let (published_key, _) = builder.plan.raw_accesses.get_key_value(key).unwrap();
                assert_eq!(published_key.as_ref(), key);
                let mut foreign = *key;
                foreign.0.0 = usize::MAX;
                assert!(builder.plan.raw_accesses.get(&foreign).is_none());
                addresses.push((std::ptr::from_ref(published_key.as_ref()),
                    std::ptr::from_ref(published.as_ref())));
            }
            for ((key, row), (key_address, address)) in rows.iter().zip(addresses) {
                assert_eq!(std::ptr::from_ref(builder.plan.raw_accesses.get_key_value(key).unwrap().0.as_ref()),
                    key_address, "tree growth preserves the exact owned key");
                assert_eq!(std::ptr::from_ref(builder.plan.raw_accesses[key].as_ref()), address);
                let source = suffix_source(&builder.plan, row);
                let path = builder.plan.raw_source_path(row.site, source, row.access, budget)?.unwrap();
                builder.retain_raw_access(row.site, source, row.access, &path,
                    row.projection, row.set, row.holder, budget)?;
                assert_eq!(std::ptr::from_ref(builder.plan.raw_accesses[key].as_ref()), address,
                    "an authenticated revisit updates the same owned record");
                assert_eq!(std::ptr::from_ref(builder.plan.raw_accesses.get_key_value(key).unwrap().0.as_ref()),
                    key_address, "an authenticated revisit retains the original key");
                assert_eq!(builder.plan.raw_accesses[key].ordinal, row.ordinal);
            }
            assert_eq!(builder.plan.raw_accesses.len(), count);
            completed = true;
            Ok(())
        }).unwrap();
        assert!(completed, "original raw access growth count {count}");
    }
}

#[test]
fn original_raw_boundary_rejects_source_role_ordinal_and_origin_pointee_substitution() {
    for changed in 0..4 {
        let mut reached = false;
        let result = with_live_projection_builder(suffix_chain_owner(SuffixChain::Write), |builder, budget| {
            reached = true;
            let row = **builder.plan.raw_accesses.values().find(|row|
                row.access == SourceReferenceAccessV29::Write && row.projection == 0).unwrap();
            let source = suffix_source(&builder.plan, &row);
            let path = builder.plan.raw_source_path(row.site, source, row.access, budget)?.unwrap();
            let before = builder.plan.raw_accesses.len();
            match changed {
                0 => assert!(builder.plan.raw_source_path(row.site, source, SourceReferenceAccessV29::Read, budget).is_err()),
                1 => assert!(builder.retain_raw_access(row.site, source, row.access, &path, 2, row.set, row.holder, budget).is_err()),
                2 => {
                    let inner = builder.plan.raw_accesses.values().find(|other|
                        other.site == row.site && other.projection == 1).unwrap().set;
                    assert!(builder.retain_raw_access(row.site, source, row.access, &path, 0, inner, row.holder, budget).is_err());
                }
                3 => {
                    let clone = source.clone();
                    assert!(builder.retain_raw_access(row.site, &clone, row.access, &path, 0, row.set, row.holder, budget).is_err());
                }
                _ => unreachable!(),
            }
            assert_eq!(builder.plan.raw_accesses.len(), before);
            Ok(())
        });
        assert!(reached);
        assert!(result.is_err(), "changed {changed}");
    }
}

#[test]
fn original_raw_chain_uses_the_current_intermediate_pointer_not_formation_time_value() {
    let mut reached = false;
    let result = with_suffix_plan(suffix_chain_owner(SuffixChain::Retarget), |plan, _| {
        reached = true;
        assert_eq!(plan.raw_origins.len(), 3);
        let inner = plan.raw_accesses.values().find(|row| row.projection == 1).unwrap();
        let set = plan.raw_sets[inner.set];
        assert_eq!(set.local.index(), 7);
        for choice in &plan.raw_choices[set.first..set.first + set.count] {
            assert_eq!(plan.raw_origins[choice.origin].local.index(), 7);
            assert!(!choice.expired);
        }
        Ok(())
    });
    assert!(result.is_ok(), "{result:?}");
    assert!(reached);
}

fn suffix_repeated_owner(helpers: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = suffix_nested_owner(0);
    let semantic = base.source_semantic();
    let original = &semantic.functions()[0];
    if !helpers {
        let mut statements = original.blocks()[0].statements().to_vec();
        statements.push(suffix_live(2, false));
        statements.extend_from_slice(original.blocks()[0].statements());
        return suffix_admit(&base, semantic.types().to_vec(), original.locals().to_vec(),
            vec![block(231, statements, SemanticTerminatorKindV1::Return)]);
    }
    let helper = SemanticFunctionIdV1::from_index(1);
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([229; 32]), SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust, SemanticExternAbiV1::Rust, false, false, 1,
        vec![SemanticAbiArgumentV1::source(value_abi(semantic.types(), U32))], value_abi(semantic.types(), UNIT),
    ).unwrap().with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue]).unwrap();
    let invoke = |target| SemanticTerminatorKindV1::Call(SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1), vec![SemanticOperandV1::Copy(place(1, U32))],
        Some(SemanticCallDestinationV1::new(place(0, UNIT), SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallReturn, SemanticBlockIdV1::from_index(target)))),
        SemanticUnwindActionV1::Unreachable,
    ).unwrap());
    let functions = vec![
        function(220, SemanticFunctionRoleV1::KernelRoot, original.abi().clone(), vec![
            local(210, UNIT, SemanticLocalRoleV1::Return), local(211, U32, SemanticLocalRoleV1::Argument(0)),
        ], vec![block(240, vec![], invoke(1)), block(241, vec![], invoke(2)),
            block(242, vec![], SemanticTerminatorKindV1::Return)]),
        function(221, SemanticFunctionRoleV1::InternalHelper, helper_abi, original.locals().to_vec(), original.blocks().to_vec()),
    ];
    let admitted = InertSemanticMirRequestV1::new_with_callables(semantic.target(), semantic.types().to_vec(),
        vec![], vec![], vec![], functions,
        vec![SemanticCallableDeclV1::defined(ROOT), SemanticCallableDeclV1::defined(helper)], vec![ROOT])
        .unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default()).unwrap()
}

#[test]
fn original_suffix_boundaries_preserve_distinct_helper_instances_and_fresh_generations() {
    for helpers in [false, true] {
        let mut reached = false;
        let result = with_suffix_plan(suffix_repeated_owner(helpers), |plan, _| {
            reached = true;
            assert_eq!(plan.raw_accesses.len(), 2);
            assert_eq!(plan.raw_origins.len(), 2);
            let a = plan.raw_origins[0];
            let b = plan.raw_origins[1];
            assert_eq!(a.local, b.local);
            if helpers { assert_ne!(a.instance, b.instance); }
            else { assert_eq!(a.instance, b.instance); assert_ne!(a.generation, b.generation); }
            for row in plan.raw_accesses.values() {
                assert_eq!(row.projection, 0);
                assert_eq!(row.pointee, SemanticTypeIdV1::from_index(4));
                assert_eq!(row.ty, U32);
                let set = plan.raw_sets[row.set];
                assert_eq!(set.count, 1);
                let choice = plan.raw_choices[set.first];
                assert!(!choice.expired);
                assert_eq!(plan.raw_origins[choice.origin].instance, row.site.instance);
            }
            Ok(())
        });
        assert!(result.is_ok(), "helpers {helpers}: {result:?}");
        assert!(reached);
    }
}

#[test]
fn original_suffix_paid_classification_preserves_callback_error_and_unwind_cleanup() {
    for mode in 0..3 {
        let result = std::panic::catch_unwind(|| with_current_projection_builder(
            suffix_nested_owner(0), false, |builder, budget| {
                let instance = builder.plan.instances.root();
                let declaration = builder.plan.instances.instance(instance).unwrap().declaration();
                let index = declaration.blocks()[0].statements().len() - 1;
                let SemanticStatementKindV1::Assign(assignment) = declaration.blocks()[0].statements()[index].kind() else { unreachable!() };
                let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else { unreachable!() };
                let site = SourceReferenceSiteV29 { instance, block: SemanticBlockIdV1::from_index(0), statement: Some(index) };
                let classified = builder.plan.raw_source_path(site, load.source(), SourceReferenceAccessV29::Read, budget)?.unwrap();
                assert_eq!(classified.last_dereference, 0);
                match mode {
                    0 => Ok(()),
                    1 => Err(source_reference_error_v29("raw suffix callback sentinel")),
                    _ => panic!("raw suffix cleanup probe"),
                }
            }));
        match mode {
            0 => result.unwrap().unwrap(),
            1 => assert!(format!("{:?}", result.unwrap()).contains("raw suffix callback sentinel")),
            _ => assert!(result.is_err()),
        }
    }
}
