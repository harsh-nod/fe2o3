fn audit_ordered_descriptor_sources_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instances: &ExecutionInstancesV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) {
    assert!(!plan.descriptors.is_empty());
    for row in &plan.descriptors {
        let source = row.check(instances, budget).unwrap();
        let function = instances.instance(row.instance).unwrap().declaration();
        let occurrences = instances.occurrences(row.instance).unwrap();
        let event = &occurrences.events()[row.occurrence];
        let ExecutionSiteV29::Statement { block, statement } = event.site() else {
            unreachable!()
        };
        let site = SourceReferenceSiteV29 {
            instance: row.instance,
            block: SemanticBlockIdV1::from_index(block.get()),
            statement: Some(statement as usize),
        };
        let work = budget.work();
        let storage = budget.storage();
        assert!(
            plan.ordered_descriptor_effect(site, source, event.operand(), budget)
                .unwrap()
        );
        // Owner/selector 25; descriptor lookup 13+log; row 10; holder 6;
        // shape 8+projection. All checks borrow existing original records.
        assert_eq!(
            budget.work() - work,
            62 + row.projection + plan.descriptor_sites.len().checked_ilog2().unwrap_or(0) as usize
        );
        assert_eq!(budget.storage(), storage);
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(10 - usize::from(short));
            let mut exact = ArgumentBudgetV1::new(&mut work, 0);
            let result = source_descriptor_volatile_source_v29(
                function,
                event.site(),
                source,
                event.operand(),
                &mut exact,
            );
            if short {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
                assert_eq!(exact.work(), 0);
            } else {
                assert!(result.unwrap());
                assert_eq!(exact.work(), 10);
            }
            assert_eq!(exact.storage(), 0);
        }
        let cloned = source.clone();
        assert!(
            !source_descriptor_volatile_source_v29(
                function,
                event.site(),
                &cloned,
                event.operand(),
                budget,
            )
            .unwrap()
        );
        assert!(
            !source_descriptor_volatile_source_v29(
                function,
                ExecutionSiteV29::Terminator { block },
                source,
                event.operand(),
                budget,
            )
            .unwrap()
        );
        for role in [
            ExecutionOperandV29::Destination,
            ExecutionOperandV29::RvalueOperand(0),
            if event.operand() == ExecutionOperandV29::RvaluePlace {
                ExecutionOperandV29::StoreDestination
            } else {
                ExecutionOperandV29::RvaluePlace
            },
        ] {
            assert!(
                !plan
                    .ordered_descriptor_effect(site, source, role, budget)
                    .unwrap()
            );
        }
        assert!(
            !plan
                .ordered_descriptor_effect(site, &cloned, event.operand(), budget)
                .unwrap()
        );
        assert!(
            plan.ordered_descriptor_effect(site, source, event.operand(), budget)
                .unwrap()
        );
    }
}

#[test]
fn original_volatile_descriptor_effects_keep_exact_source_roles_and_replay() {
    for write in [false, true] {
        for looped in [false, true] {
            for metadata_length in [false, true] {
                let case = DescriptorCase {
                    write,
                    explicit: true,
                    looped,
                    metadata_length,
                    ..DescriptorCase::READ
                };
                run_descriptor_module(
                    case,
                    DescriptorFault::OrderedSourceAudit,
                    MODULE_LIMIT,
                    MODULE_LIMIT,
                )
                .0
                .unwrap_or_else(|error| panic!("{case:?}: {error:?}"));
                descriptor_resource_assertions_completed(true);
                assert!(DESCRIPTOR_EMITTED.get() > 0);
                assert_eq!(DESCRIPTOR_TAMPERED.get(), 0);
            }
        }
    }
}

#[test]
fn original_descriptor_atomic_effects_remain_closed_before_emission() {
    for write in [false, true] {
        let case = DescriptorCase {
            write,
            explicit: true,
            atomic: true,
            ..DescriptorCase::READ
        };
        let error = run_descriptor_module(case, DescriptorFault::None, MODULE_LIMIT, MODULE_LIMIT)
            .0
            .unwrap_err();
        assert!(
            matches!(error, ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
            detail, ..
        }) if detail == if write {
            "source reference ordered store requires checked addressable effects"
        } else { "source reference ordered load requires checked addressable effects" })
        );
        descriptor_resource_assertions_completed(false);
        assert_eq!(DESCRIPTOR_EMITTED.get(), 0);
        assert_eq!(DESCRIPTOR_ASSERTIONS_STARTED.get(), 0);
        assert_eq!(DESCRIPTOR_TAMPERED.get(), 0);
    }
}

#[test]
fn actual_descriptor_volatility_cannot_be_added_or_removed() {
    for write in [false, true] {
        for explicit in [false, true] {
            let case = DescriptorCase {
                write,
                explicit,
                ..DescriptorCase::READ
            };
            run_descriptor_module(case, DescriptorFault::None, MODULE_LIMIT, MODULE_LIMIT)
                .0
                .unwrap();
            descriptor_resource_assertions_completed(true);
            let error = run_descriptor_module(
                case,
                DescriptorFault::Volatility,
                MODULE_LIMIT,
                MODULE_LIMIT,
            )
            .0
            .unwrap_err();
            assert!(
                matches!(
                    error,
                    ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { .. })
                ),
                "actual source-effect mismatch must not be a resource failure: {error:?}"
            );
            descriptor_resource_assertions_completed(false);
            assert_eq!(DESCRIPTOR_TAMPERED.get(), 1);
            assert!(DESCRIPTOR_EMITTED.get() > 0);
            run_descriptor_module(case, DescriptorFault::None, MODULE_LIMIT, MODULE_LIMIT)
                .0
                .unwrap();
            descriptor_resource_assertions_completed(true);
        }
    }
}

#[test]
fn ordered_descriptor_effects_still_require_the_original_extent_and_success_edge() {
    for write in [false, true] {
        let base = DescriptorCase {
            write,
            explicit: true,
            ..DescriptorCase::READ
        };
        run_descriptor_module(base, DescriptorFault::None, MODULE_LIMIT, MODULE_LIMIT)
            .0
            .unwrap();
        descriptor_resource_assertions_completed(true);
        for case in [
            DescriptorCase {
                foreign_extent: true,
                ..base
            },
            DescriptorCase {
                changed_index: true,
                ..base
            },
            DescriptorCase {
                bypass: true,
                ..base
            },
        ] {
            let error =
                run_descriptor_module(case, DescriptorFault::None, MODULE_LIMIT, MODULE_LIMIT)
                    .0
                    .unwrap_err();
            assert!(
                matches!(
                    error,
                    ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { .. })
                ),
                "ordered effects cannot replace the original bounds proof: {case:?}: {error:?}"
            );
            descriptor_resource_assertions_completed(false);
            assert!(
                DESCRIPTOR_EMITTED.get() > 0,
                "the actual descriptor gate must run"
            );
        }
    }
}
