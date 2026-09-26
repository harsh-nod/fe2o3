#[test]
fn original_partial_array_read_requires_exact_source_and_scalar_backing() {
    struct Restore((u16, u64, bool), SourceArrayModeV29);
    impl Drop for Restore {
        fn drop(&mut self) { SOURCE_ARRAY_CASE_V29.set(self.0); SOURCE_ARRAY_MODE_V29.set(self.1); }
    }
    let _restore = Restore(SOURCE_ARRAY_CASE_V29.replace((32, 3, false)),
        SOURCE_ARRAY_MODE_V29.replace(SourceArrayModeV29::Scalar));
    let mut completed = false;
    with_original_scalar_array_plan_v29(|plan, budget| {
        with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
            let mut checked = 0;
            for access in &plan.accesses {
                if access.key.access != SourceReferenceAccessV29::Read
                    || access.local.index() != 2 || access.projections.len() != 1 { continue; }
                let instance = plan.instances.instance(access.instance).unwrap();
                if instance.function().index() != 2 { continue; }
                let original = instance.declaration();
                let SemanticStatementKindV1::Assign(assignment) = original.blocks()[access.key.site.block.index() as usize]
                    .statements()[access.key.site.statement.unwrap()].kind() else { panic!("original array read assignment"); };
                let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else { panic!("original array Load"); };
                let place = load.source();
                assert_eq!(access.key.source, place as *const SemanticPlaceV1 as usize);
                let read = ScopedMemoryReadV29 {
                    site: execution_site_v29(access.key.site.block, access.key.site.statement.map(|value| value as u32)),
                    role: ExecutionOperandV29::RvaluePlace, prefix: 1, ty: place.ty(),
                    // Inert gate input only. Final source occurrence/currentness
                    // authority is tested by the genuine consuming array tests.
                    occurrence: ScopedMemoryOccurrenceV29::Retained { event: usize::MAX },
                };
                let view = SourceFunctionBackingViewV29 { layouts, instance: access.instance };
                let expected = retained_array_slot_plan_v1(plan.instances.owner().source_semantic().types(),
                    original.locals()[2].ty(), usize::MAX)?;
                let slot = SemanticRetainedLocalSlotV1 { pointer: ValueId(u32::MAX),
                    semantic_type: expected.semantic_type, storage: expected.storage };
                let before = budget.storage();
                with_canonical_call_scratch_v1(budget, |budget| {
                    budget.reserve_storage(source_partial_array_read_headers_v29()?)?;
                    check_source_partial_array_read_v29(view, read, place, &slot, budget)?;
                    for fault in 0..11 {
                        let mut changed_slot = slot.clone();
                        let mut changed_read = read;
                        let mut changed_view = view;
                        let mut changed_place = place.clone();
                        let source = match fault {
                            0 => &changed_place,
                            1 => { changed_read.role = ExecutionOperandV29::RvalueOperand(0); place },
                            2 => { changed_view.instance = plan.instances.root(); place },
                            3 => { changed_slot.semantic_type = place.ty(); place },
                            4 => { let SemanticRetainedStorageV29::ScalarArray { alignment, .. } = &mut changed_slot.storage else { unreachable!() }; *alignment *= 2; place },
                            5 => { let SemanticRetainedStorageV29::ScalarArray { array: Some(array), .. } = &mut changed_slot.storage else { unreachable!() }; array.length += 1; place },
                            6 => { let SemanticRetainedStorageV29::ScalarArray { kernel_type, .. } = &mut changed_slot.storage else { unreachable!() }; *kernel_type = Type::Scalar(ScalarType::U64); place },
                            7 => {
                                changed_place = SemanticPlaceV1::new(place.local(), vec![SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::ConstantIndex { offset: 0, minimum_length: 3, from_end: false }, place.ty()).unwrap()], place.ty()).unwrap();
                                &changed_place
                            },
                            8 => { changed_read.site = execution_site_v29(access.key.site.block, Some(0)); place },
                            9 => { changed_read.ty = slot.semantic_type; place },
                            10 => { changed_read.prefix = 0; place },
                            _ => unreachable!(),
                        };
                        let error = check_source_partial_array_read_v29(changed_view, changed_read, source, &changed_slot, budget).unwrap_err();
                        assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
                            if detail == "typed allocation identity or representation requires its exact source contract"), "fault {fault}: {error:?}");
                    }
                    check_source_partial_array_read_v29(view, read, place, &slot, budget)
                })?;
                assert_eq!(budget.storage(), before);
                checked += 1;
            }
            assert_eq!(checked, 4, "two generations in each repeated original helper");
            completed = true;
            Ok(())
        })
    }).unwrap();
    assert!(completed);
}
