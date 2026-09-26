fn pointer_holder_source_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = fixture(Case::Ordinary, false);
    let mut types = base.source_semantic().types()[..2].to_vec();
    let pointer = SemanticTypeIdV1::from_index(2);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([201; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8,
            fe2o3_mir_model::semantic_mir_v1::SemanticBackendReprV1::scalar(
                fe2o3_mir_model::semantic_mir_v1::SemanticBackendScalarV1::initialized(
                    fe2o3_mir_model::semantic_mir_v1::SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    fe2o3_mir_model::semantic_mir_v1::SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)))),
            false).unwrap(),
        SemanticTypeShapeV1::Pointer(SemanticPointerTypeV1::new_with_kind(
            U32, SemanticPointerKindV1::Raw, SemanticMutabilityV1::Mutable,
            0, 64, SemanticPointerMetadataV1::None).unwrap()),
    ).with_rustc_abi_properties(
        fe2o3_mir_model::semantic_mir_v1::SemanticTypeAbiPropertiesV1::new(false, false)
            .with_scalar_pointee_info(Some(
                fe2o3_mir_model::semantic_mir_v1::SemanticAbiPointeeInfoV1::new(
                    fe2o3_mir_model::semantic_mir_v1::SemanticAbiPointeeKindV1::Raw, 0, 1,
                ).unwrap()), None),
    ));
    let destination = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![
        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap(),
    ], U32).unwrap();
    let root = function(203, SemanticFunctionRoleV1::KernelRoot, abi(202, true, &[pointer]),
        vec![local(204, UNIT, SemanticLocalRoleV1::Return),
            local(205, pointer, SemanticLocalRoleV1::Argument(0))],
        vec![block(206, vec![assign(destination, SemanticRvalueKindV1::Use(scalar(1)))],
            SemanticTerminatorKindV1::Return)],
    ).with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"original_pointer_holder".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([207; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types, vec![], vec![], vec![], vec![root], vec![SemanticCallableDeclV1::defined(ROOT)], vec![ROOT],
    ).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap()
}

#[test]
fn original_pointer_holder_identity_requires_claimed_use_and_exact_archive() {
    // Genuine original MIR/SSA capture, with inert physical binding hypotheses.
    // This tests the archive join only, not raw-pointer memory permission.
    let mut owner = pointer_holder_source_owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let capture = owner.try_capture_occurrences_with_budget_v1(&mut budget).unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let mut completed = false;
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        with_execution_availability_v29(instances, instances.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            let site = execution_site_v29(block, Some(0));
            let role = ExecutionOperandV29::Destination;
            cursor.begin_block(block, budget)?;
            let original = scoped_object_original_place_v29(cursor.function, site, role).unwrap();
            let definition = cursor.current[1].unwrap();
            let binding = SemanticValueBindingV1::Value { id: ValueId(91), ty: Type::pointer(
                Type::Scalar(ScalarType::U32), AddressSpace::Generic, AccessMode::ReadWrite) };
            let archive = SemanticSsaBindingsV1::from([(definition, binding.clone())]);
            let locals = [Some(SemanticValueBindingV1::Unit), Some(binding.clone())];
            let carriers = ExecutionCfgCarriersV29::default();
            let check = |cursor: &ExecutionAvailabilityV29<'_>, place, site, role, definition, locals: &[Option<SemanticValueBindingV1>], archive: &SemanticSsaBindingsV1, budget: &mut dyn SemanticEmissionBudgetV1| {
                check_source_use_archive_v29(cursor, &carriers, locals, archive, site, role, place, definition, budget)
            };
            assert!(check(&cursor, original, site, role, definition, &locals, &archive, budget).is_err());
            assert_eq!(cursor.use_place(site, role, original, false, budget)?, definition);
            check(&cursor, original, site, role, definition, &locals, &archive, budget)?;
            let cloned = original.clone();
            for (place, site, role) in [
                (&cloned, site, role),
                (original, execution_site_v29(block, Some(1)), role),
                (original, site, ExecutionOperandV29::StoreDestination),
            ] {
                assert!(matches!(check(&cursor, place, site, role, definition, &locals, &archive, budget),
                    Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                    if detail == "execution availability differs from its source SSA instance"));
            }
            for mutation in 0..4 {
                let mut candidate = binding.clone();
                let SemanticValueBindingV1::Value { id, ty } = &mut candidate else { unreachable!() };
                match mutation {
                    0 => *id = ValueId(92),
                    1 => *ty = Type::pointer(Type::Scalar(ScalarType::U64), AddressSpace::Generic, AccessMode::ReadWrite),
                    2 => *ty = Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Global, AccessMode::ReadWrite),
                    _ => *ty = Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Generic, AccessMode::ReadOnly),
                }
                let swapped = SemanticSsaBindingsV1::from([(definition, candidate)]);
                assert!(matches!(check(&cursor, original, site, role, definition, &locals, &swapped, budget),
                    Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                    if detail == "execution availability differs from its source SSA instance"));
            }
            let missing = SemanticSsaBindingsV1::default();
            assert!(check(&cursor, original, site, role, definition, &locals, &missing, budget).is_err());
            let wrong_definition = SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(99), variable: fe2o3_mir_model::SsaVariableIdV1::new(1),
            };
            let wrong = SemanticSsaBindingsV1::from([(wrong_definition, binding)]);
            assert!(check(&cursor, original, site, role, wrong_definition, &locals, &wrong, budget).is_err());
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(1000);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 0);
            assert!(check(&cursor, original, site, role, definition, &locals, &archive, &mut foreign).is_err());
            assert_eq!((foreign.work(), foreign.storage()), (0, 0));
            check(&cursor, original, site, role, definition, &locals, &archive, budget)?;
            completed = true;
            Ok(())
        }).unwrap();
        Ok::<_, crate::production_semantic_kir_v1::production_call_instances_v1::ProductionCallInstanceErrorV1>(())
    }).unwrap();
    assert!(completed);
    assert_eq!(budget.storage(), capture.retained_storage());
}

fn discriminant_source_owner() -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBackendPrimitiveV1, SemanticBackendReprV1, SemanticBackendScalarV1,
        SemanticDirectEnumEncodingV1, SemanticEnumEncodingV1, SemanticEnumLayoutV1,
        SemanticEnumVariantLayoutV1, SemanticFieldsShapeV1, SemanticPaddingV1,
        SemanticScalarValidityRangeV1,
    };
    let original = cfg_owner(Shape::Mixed);
    let model = original.source_semantic();
    let base = &model.functions()[ROOT.index() as usize];
    let enumeration = SemanticTypeIdV1::from_index(5);
    let mut types = model.types().to_vec();
    types[PAIR.index() as usize] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([201; 32]),
        SemanticTypeLayoutV1::aggregate(Some(8), 4,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap()).unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![CONTEXT, enumeration]).unwrap()),
    );
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([202; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticTypeLayoutV1::enum_layout(8, 4, SemanticEnumLayoutV1::new(
            vec![
                SemanticEnumVariantLayoutV1::from_rustc(0, 8, 4,
                    SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                    SemanticBackendReprV1::memory(true), None, false, None, 4, 0,
                    SemanticAggregateLayoutV1::new(vec![], vec![SemanticPaddingV1::new(4, 4).unwrap()]).unwrap(),
                ).unwrap(),
                SemanticEnumVariantLayoutV1::from_rustc(1, 8, 4,
                    SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
                    SemanticBackendReprV1::memory(true), None, false, None, 4, 0,
                    SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
                ).unwrap(),
            ],
            SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0,
                SemanticBackendScalarV1::initialized(SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into())))),
        ).unwrap()).unwrap(),
        SemanticTypeShapeV1::Enum {
            discriminant: U32,
            variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![U32]).unwrap()),
            ].into_boxed_slice(),
        },
    ));
    let mut locals = base.locals().to_vec();
    locals.push(local(227, enumeration, SemanticLocalRoleV1::Temporary));
    let selected = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(4), vec![
        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), enumeration).unwrap(),
    ], enumeration).unwrap();
    let root = function(76, SemanticFunctionRoleV1::KernelRoot, base.abi().clone(), locals,
        vec![block(77, vec![
            assign(place(7, enumeration), SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::EnumVariant(1), vec![scalar(42)]).unwrap()),
            assign(place(4, PAIR), SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Tuple, vec![
                    SemanticOperandV1::Move(place(1, CONTEXT)),
                    SemanticOperandV1::Copy(place(7, enumeration)),
                ]).unwrap()),
            assign(place(3, U32), SemanticRvalueKindV1::Discriminant(selected)),
        ], SemanticTerminatorKindV1::Return)],
    ).with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"original_discriminant_use".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([78; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types, vec![], vec![], vec![], vec![root], vec![SemanticCallableDeclV1::defined(ROOT)], vec![ROOT],
    ).unwrap().admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default()).unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap()
}

#[test]
fn original_discriminant_place_is_consumed_by_the_actual_emitter() {
    let source_event = std::cell::Cell::new(None);
    for omit in [false, true] {
        let mut completed = false;
        lower_cfg_owner_with_limits(discriminant_source_owner(), Shape::Mixed,
            (10_000_000, 10_000_000), |_| {}, |cursor| {
                let index = cursor.occurrences.events().iter().position(|event|
                    event.site() == execution_site_v29(SemanticBlockIdV1::from_index(0), Some(2))
                        && event.operand() == ExecutionOperandV29::RvaluePlace
                        && event.role() == ExecutionEventV29::BaseUse).unwrap();
                assert!(cursor.events.required.contains(&index));
                assert!(cursor.occurrences.events()[index].is_promoted());
                source_event.set(Some(index));
                if omit { cursor.skipped_event = Some(index); }
            }, |_, seed, result| {
                if omit {
                    assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                        if detail == "execution availability differs from its source SSA instance"));
                } else {
                    let result = result.unwrap();
                    let body = result.function.body.as_ref().unwrap();
                    let tag = body.blocks[0].operations.iter()
                        .find(|operation| operation.kind == OperationKind::Constant(Constant::U32(1)))
                        .expect("original enum discriminant constant").results[0].id;
                    let observed = result.execution_observation.unwrap();
                    assert_eq!(observed.locals[3].as_ref().unwrap().value().unwrap(), (tag, Type::Scalar(ScalarType::U32)));
                    let Some(SemanticValueBindingV1::Aggregate(fields)) = &observed.locals[4] else { panic!("original holder"); };
                    assert!(matches!(&fields[0], SemanticValueBindingV1::Execution(value) if value == seed));
                }
                completed = true;
            }).unwrap();
        assert!(completed && source_event.get().is_some());
    }
}

#[test]
fn claimed_original_use_requires_exact_source_place_role_and_definition() {
    captured_execution(Flow::Linear, |instances, budget| {
        let instance = instances.calls(instances.root()).unwrap()[1].child().unwrap();
        with_execution_availability_v29(instances, instance, budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(0);
            let site = execution_site_v29(block, Some(0));
            let role = ExecutionOperandV29::RvalueOperand(0);
            cursor.begin_block(block, budget)?;
            let original = scoped_object_original_place_v29(cursor.function, site, role).unwrap();
            let definition = cursor.current[original.local().index() as usize].unwrap();
            assert!(cursor.check_claimed_original_use_v29(site, role, original, definition, budget).is_err());
            let actual = cursor.use_place(site, role, original, true, budget)?;
            assert_eq!(actual, definition);
            assert_eq!(cursor.current[original.local().index() as usize], None);
            cursor.check_claimed_original_use_v29(site, role, original, definition, budget)?;
            let cloned = original.clone();
            for (candidate_site, candidate_role, candidate, candidate_definition) in [
                (site, role, &cloned, definition),
                (site, ExecutionOperandV29::RvalueOperand(1), original, definition),
                (execution_site_v29(block, Some(1)), role, original, definition),
                (site, role, original, source_definition(&cursor, 0, 0)),
            ] {
                assert!(matches!(cursor.check_claimed_original_use_v29(candidate_site, candidate_role,
                    candidate, candidate_definition, budget), Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                    if detail == "execution availability differs from its source SSA instance"));
            }
            cursor.check_claimed_original_use_v29(site, role, original, definition, budget)?;
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(100);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 100);
            assert!(cursor.check_claimed_original_use_v29(site, role, original, definition, &mut foreign).is_err());
            assert_eq!(foreign.work(), 0);
            assert_eq!(foreign.storage(), 0);
            Ok(())
        }).unwrap();
    });
}

#[test]
fn claimed_original_use_has_prepaid_logarithmic_work_and_no_storage() {
    captured_execution(Flow::Linear, |instances, _| {
        let instance = instances.calls(instances.root()).unwrap()[1].child().unwrap();
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
            budget.reserve_storage(37).unwrap();
            let mut completed = false;
            let result = with_execution_availability_v29(instances, instance, &mut budget, |mut cursor, budget| {
                let block = SemanticBlockIdV1::from_index(0);
                let site = execution_site_v29(block, Some(0));
                let role = ExecutionOperandV29::RvalueOperand(0);
                cursor.begin_block(block, budget)?;
                let original = scoped_object_original_place_v29(cursor.function, site, role).unwrap();
                let definition = cursor.use_place(site, role, original, true, budget)?;
                let key = unit_local_source_key_v1(site, role, Some(ExecutionEventV29::BaseUse));
                let position = cursor.index.iter().position(|entry| entry.key == key).unwrap();
                // Independently count tree depth by ordinal, without calling
                // the query or comparing its retained source keys.
                let (mut start, mut end, mut depth) = (0, cursor.index.len(), 0);
                loop {
                    depth += 1;
                    let middle = start + (end - start) / 2;
                    if middle == position { break; }
                    if middle < position { start = middle + 1; } else { end = middle; }
                }
                let required = 5 + 8 * depth;
                budget.charge_work(1_000_000 - budget.work() - required + usize::from(short))?;
                let before = (budget.work(), budget.storage(), budget.peak_storage());
                let queried = cursor.check_claimed_original_use_v29(site, role, original, definition, budget);
                if short {
                    assert!(matches!(queried, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))));
                } else {
                    queried.as_ref().unwrap();
                    assert_eq!(budget.work() - before.0, required);
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (before.1, before.2));
                completed = true;
                queried
            });
            assert!(completed);
            assert_eq!(result.is_ok(), !short);
            assert_eq!(budget.storage(), 37);
        }
    });
}

#[test]
fn original_carrier_owner_equation_rejects_repeated_caller_and_foreign_headers() {
    // Exercise only the owner equation with captured original instances. This
    // empty hypothesis is not a carrier recipe or an admission positive.
    captured_execution(Flow::Linear, |instances, budget| {
        let calls = instances.calls(instances.root()).unwrap();
        let first = calls[1].child().unwrap();
        let second = calls[3].child().unwrap();
        assert_ne!(first, second);
        for instance in [first, second] {
            with_execution_availability_v29(instances, instance, budget, |cursor, budget| {
                let original = ExecutionCfgCarriersV29 {
                    source_owner: Some(std::ptr::from_ref(cursor.function).addr()),
                    source_plan: None, instance: Some(cursor.instance),
                    ledger: Some(budget.work_ledger_identity_v1()), locals: BTreeMap::new(),
                };
                let floor = budget.storage();
                let before = budget.work();
                original.check_owner(&cursor, budget)?;
                assert_eq!(budget.work() - before, 4);
                for fault in 0..4 {
                    let mut changed = original.clone();
                    match fault {
                        0 => changed.source_owner = None,
                        1 => changed.source_plan = Some(1),
                        2 => changed.instance = Some(if instance == first { second } else { first }),
                        _ => changed.ledger = None,
                    }
                    assert!(matches!(changed.check_owner(&cursor, budget),
                        Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                        if detail == "execution CFG transport differs from its captured SSA state"));
                }
                original.check_owner(&cursor, budget)?;
                assert_eq!(budget.storage(), floor);
                Ok(())
            }).unwrap();
        }
    });
}

#[test]
fn ordinary_archive_fallback_refuses_a_substituted_carrier_without_a_recipe() {
    captured_execution(Flow::Linear, |instances, budget| {
        with_execution_availability_v29(instances, instances.root(), budget, |mut cursor, budget| {
            let block = SemanticBlockIdV1::from_index(1);
            let site = execution_site_v29(block, None);
            let role = ExecutionOperandV29::CallArgument(1);
            cursor.begin_block(block, budget)?;
            let nominal = scoped_object_original_place_v29(cursor.function, site, ExecutionOperandV29::CallArgument(0)).unwrap();
            cursor.use_place(site, ExecutionOperandV29::CallArgument(0), nominal, true, budget)?;
            let original = scoped_object_original_place_v29(cursor.function, site, role).unwrap();
            let definition = cursor.use_place(site, role, original, false, budget)?;
            assert_eq!(cursor.cfg.nominal_locals[original.local().index() as usize], 0);
            let seed = SemanticValueBindingV1::Value { id: ValueId(90), ty: Type::Scalar(ScalarType::U32) };
            let mut locals = vec![None; cursor.function.locals().len()];
            locals[original.local().index() as usize] = Some(seed.clone());
            let mut archive = SemanticSsaBindingsV1::from([(definition, seed)]);
            let no_carriers = ExecutionCfgCarriersV29::default();
            check_source_use_archive_v29(&cursor, &no_carriers, &locals, &archive,
                site, role, original, definition, budget)?;
            // Either side alone must force a missing-recipe refusal. Equality
            // of two invented carrier-shaped bindings is not source evidence.
            for side in 0..3 {
                let mut changed_locals = locals.clone();
                let mut changed_archive = archive.clone();
                if side != 1 { changed_locals[original.local().index() as usize] = Some(SemanticValueBindingV1::MathContext); }
                if side != 0 { changed_archive.insert(definition, SemanticValueBindingV1::MathContext); }
                assert!(matches!(check_source_use_archive_v29(&cursor, &no_carriers,
                    &changed_locals, &changed_archive, site, role, original, definition, budget),
                    Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                    if detail == "execution CFG transport differs from its captured SSA state"));
            }
            archive.clear();
            assert!(check_source_use_archive_v29(&cursor, &no_carriers, &locals, &archive,
                site, role, original, definition, budget).is_err());
            Ok(())
        }).unwrap();
    });
}
