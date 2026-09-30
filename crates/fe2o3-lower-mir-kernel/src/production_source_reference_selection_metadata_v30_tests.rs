thread_local! {
    static COMPLETED_SELECTED_METADATA_CHECKS_V30: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0; 3]) };
}

fn completed_metadata_scope_oracle_v30() -> usize {
    use std::mem::{align_of, size_of};
    type Capture<'a> = (
        &'a SourceReferencePlanV29<'a, 'a>,
        SourceExternalReferenceOriginV29,
    );
    2 * size_of::<Capture<'_>>()
        + 2 * align_of::<Capture<'_>>()
        + size_of::<()>()
        + 2 * size_of::<Result<(), ProductionSemanticKirErrorV1>>()
        + size_of::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()
        + size_of::<Box<dyn std::any::Any + Send>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 4 * size_of::<usize>()
        + 2 * size_of::<&SourceReferencePlanV29<'_, '_>>()
        + 2 * size_of::<&SemanticPlaceV1>()
        + 2 * size_of::<&mut ArgumentBudgetV1<'_>>()
}

fn audit_completed_descriptor_metadata_v30(
    plan: &mut SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let descriptor = plan
        .external_borrows
        .iter()
        .find_map(|row| match row.origin {
            SourceExternalReferenceOriginV29::Descriptor {
                instance,
                descriptor,
            } => Some((instance, descriptor)),
            _ => None,
        });
    let Some((instance, descriptor)) = descriptor else {
        return Ok(());
    };
    let observation = SOURCE_EMISSION_OBSERVATION_V29
        .get()
        .expect("source-owned metadata completion phase");
    assert_eq!(
        observation.owner,
        std::ptr::from_ref(plan.instances.owner()) as usize
    );
    assert_eq!(observation.slot, std::ptr::from_ref(budget) as usize);
    assert!(observation.ledger == budget.work_ledger_identity_v1());
    let phase = match observation.phase {
        SourceEmissionPhaseV29::Admission => 0,
        SourceEmissionPhaseV29::ConstructionReplay => 1,
        SourceEmissionPhaseV29::ConsumerReplay => 2,
    };
    assert_eq!(
        COMPLETED_SELECTED_METADATA_CHECKS_V30.get(),
        match phase {
            0 => [0, 0, 0],
            1 => [1, 0, 0],
            2 => [1, 1, 0],
            _ => unreachable!(),
        },
        "metadata completion must follow the exact original owner lifecycle"
    );
    let origin = SourceExternalReferenceOriginV29::Descriptor {
        instance,
        descriptor,
    };
    let capture = SourceExternalMetadataCaptureV30 { plan, origin };
    assert_eq!(
        source_reference_selection_call_headers_v29::<()>(&capture)?,
        completed_metadata_scope_oracle_v30()
    );
    check_source_external_origin_metadata_v30(plan, origin, budget)?;
    let row = plan.descriptors[descriptor];
    let source = row.check(plan.instances, budget)?;
    let occurrences = plan.instances.occurrences(instance).unwrap();
    let occurrence = &occurrences.events()[row.holder_occurrence];
    let key =
        source_reference_selector_site_v29(instance, occurrence.site(), source, row.projection - 1);
    let original = *plan
        .descriptor_values
        .get(&key)
        .expect("completed original holder metadata");
    assert!(
        original.descriptor.is_some(),
        "genuine kernel ABI metadata, not an unknown fallback"
    );
    let floor = budget.storage();
    for fault in 0..4 {
        match fault {
            0 => {
                assert!(plan.descriptor_values.remove(&key).is_some());
            }
            1 => {
                plan.descriptor_values.get_mut(&key).unwrap().ty =
                    SemanticTypeIdV1::from_index(u32::MAX);
            }
            2 => {
                plan.descriptor_values.get_mut(&key).unwrap().descriptor = Some(usize::MAX);
            }
            _ => (),
        }
        let selected = if fault == 3 {
            SourceExternalReferenceOriginV29::Descriptor {
                instance,
                descriptor: usize::MAX,
            }
        } else {
            origin
        };
        let result = check_source_external_origin_metadata_v30(plan, selected, budget);
        if fault == 2 {
            assert!(
                matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ),
                "{result:?}"
            );
        } else {
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "source runtime slice descriptor/index/extent correspondence differs",
                    })
                ),
                "{result:?}"
            );
        }
        plan.descriptor_values.insert(key, original);
        assert_eq!(budget.storage(), floor);
        check_source_external_origin_metadata_v30(plan, origin, budget)?;
    }
    check_source_external_metadata_complete_v30(plan, budget)?;
    let mut checked = COMPLETED_SELECTED_METADATA_CHECKS_V30.get();
    checked[phase] += 1;
    COMPLETED_SELECTED_METADATA_CHECKS_V30.set(checked);
    Ok(())
}

#[test]
fn selected_descriptor_metadata_is_mandatory_after_all_original_cfg_arms_complete() {
    struct Restore(Option<SourceExternalMetadataAuditV30>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_EXTERNAL_METADATA_AUDIT_V30.set(self.0);
        }
    }
    let _restore = Restore(
        SOURCE_EXTERNAL_METADATA_AUDIT_V30.replace(Some(audit_completed_descriptor_metadata_v30)),
    );
    COMPLETED_SELECTED_METADATA_CHECKS_V30.set([0; 3]);
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        mixed_selected_memory_owner(),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, _| {
            assert_eq!(retained_selected_memory(original).selected.len(), 1);
            assert_eq!(COMPLETED_SELECTED_METADATA_CHECKS_V30.get(), [1, 1, 1]);
            reached.set(true);
            Ok(())
        },
    );
    result.unwrap();
    assert!(reached.get());
    assert_eq!(
        COMPLETED_SELECTED_METADATA_CHECKS_V30.get(),
        [1, 1, 1],
        "admission, construction replay, and source-consumer replay C1 plans"
    );
}

#[test]
fn selected_completed_metadata_frames_have_independent_exact_and_one_short_boundaries() {
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let expected = h::<SourceExternalReferenceOriginV29>()
        + h::<&SemanticPlaceV1>()
        + h::<AddressSpace>()
        + h::<&SourceReferenceDescriptorV29>()
        + h::<&SemanticFunctionDeclV1>()
        + h::<SourceReferenceSelectionSubjectV29>()
        + h::<&SourceReferenceSelectionNodeV29>()
        + h::<Option<&SemanticPlaceV1>>()
        + h::<()>();
    assert_eq!(source_external_metadata_headers_v30().unwrap(), expected);
    let expected = expected + completed_metadata_scope_oracle_v30();
    for short in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 17 + expected - short);
        budget.reserve_storage(17).unwrap();
        let result = budget.reserve_storage(
            source_external_metadata_headers_v30().unwrap() + completed_metadata_scope_oracle_v30(),
        );
        if short == 0 {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        } else {
            assert!(
                matches!(result, Err(ArgumentResourceV1::Storage(error)) if error.actual() == 17 + expected && error.limit() == 16 + expected)
            );
        }
        assert_eq!((budget.storage(), budget.work()), (17, 0));
    }
}
