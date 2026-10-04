// Original SSA expressions complete only the retained Store value obligation.
// Bounds, initializedness, aliasing, races and runtime bindings stay independent.
impl OriginalEntryIndexV20<'_, '_> {
    fn global_store_expression_v23(
        &self,
        leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
        request: &ProductionOptimizedSourceScalarStoreV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.store_source_expression_v23(leaves, request, budget)
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    pub(super) fn global_expression_entry_v23(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || !optimized.has_original_account_v23(budget)
        {
            self.source.cleanup.deny_refund();
            return self.retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.retain_query(budget.check_prior_denials_v1().map_err(Into::into))?;
        optimized.pending_global_output_v18(self, budget)?;
        Ok(())
    }

    // One original index serves the complete root batch. The callback may use
    // the existing exact native/guard joins; none of their pending predicates
    // is replaced by the fact that this Store value was checked.
    pub(super) fn with_global_source_expressions_v23<'work, F>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: &mut F,
    ) -> SourceOwnedResultV18<()>
    where
        F: for<'scope> FnMut(
            usize,
            &PendingGlobalSourceAccessesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    {
        // Observe custody and the original first denial before the generic
        // attempt may reserve a header on the supplied account.
        self.global_expression_entry_v23(optimized, budget)?;
        let floor = budget.storage();
        self.retain_query(scoped_source_attempt_v29(
            self.source.cleanup,
            budget,
            floor,
            move |budget| {
                optimized_source_endpoints_v18(self, optimized, budget)?;
                source_output_correspondence_checks_v18(self, optimized, budget)?;
                let before = budget.storage();
                budget.reserve_storage(argument_sum_v1(&[
                    original_private_expression_headers_v22()?,
                    size_of::<OriginalEntryIndexV20<'_, '_>>(),
                    size_of::<SourceOwnedResultV18<OriginalEntryIndexV20<'_, '_>>>(),
                    4 * size_of::<SourceOwnedResultV18<()>>(),
                    4 * size_of::<&mut F>(),
                    8 * size_of::<&()>(),
                    6 * size_of::<usize>(),
                ])?)?;
                let index = OriginalEntryIndexV20::build(self, budget)?;
                let retained = budget
                    .storage()
                    .checked_sub(before)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                for root in 0..self.source.root_count(budget)? {
                    self.with_descriptor_source_roles_index_v23(
                        optimized,
                        root,
                        None,
                        Some(&index),
                        budget,
                        |roles, budget| {
                            let result = self.retain_query(consume(
                                root,
                                &PendingGlobalSourceAccessesV18 { roles },
                                budget,
                            ));
                            let postflight = self.global_expression_entry_v23(optimized, budget);
                            result.and(postflight)
                        },
                    )?;
                }
                index.check(budget)?;
                drop(index);
                optimized_source_endpoints_v18(self, optimized, budget)?;
                if before.checked_add(retained) != Some(budget.storage()) {
                    self.source.cleanup.deny_refund();
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                budget.release_storage(retained)?;
                Ok(())
            },
        ))
    }
}

#[cfg(test)]
pub(super) fn test_global_expression_namespace_v23(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    completed: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    let mut observations = [[0; 2]; 3];
    for (mode, namespace) in [
        SourceScalarNamespaceV18::SourceOnly,
        SourceScalarNamespaceV18::OriginalSourceExpressionsV23,
        SourceScalarNamespaceV18::PrivateSourceWritesV22,
    ]
    .iter()
    .enumerate()
    {
        original.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            0,
            namespace,
            budget,
            |leaves, budget| {
                let source = leaves.original_leaves(budget)?;
                assert_eq!(source.leaves.ordinary_values, mode != 0);
                observations[mode] = [
                    source
                        .leaves
                        .rows
                        .iter()
                        .filter(|row| row.typed_private)
                        .count(),
                    source.leaves.wrapping.len(),
                ];
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            },
        )?;
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(observations[0], [0, 0]);
    assert_eq!(observations[1][0], 0);
    assert!(observations[1][1] > 0);
    assert!(observations[2][0] > 0);
    assert_eq!(observations[1][1], observations[2][1]);
    completed.set(true);
    Ok(())
}

#[cfg(test)]
pub(super) fn test_global_source_expressions_v23(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    counts: &std::cell::Cell<[usize; 2]>,
    completed: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    original.with_global_source_expressions_v23(optimized, budget, &mut |root, accesses, budget| {
        assert_eq!(root, accesses.root());
        let joined = std::cell::Cell::new(false);
        with_native_global_test_view_v18(optimized, budget, |native, budget| {
            for row in accesses.roles.rows {
                if !matches!(
                    row.role,
                    Some(DescriptorSourceRoleV18::Read | DescriptorSourceRoleV18::Write)
                ) {
                    continue;
                }
                assert!(!row.write_recipe_pending);
                accesses
                    .with_native_access_v18(native, row.output, budget, |access, _| {
                        let access = access.expect("actual source/output/native access");
                        let mut found = counts.get();
                        found[usize::from(access.pair.output.writing)] += 1;
                        counts.set(found);
                        assert!(!access.grants_memory_or_launch_authority());
                        Ok(())
                    })
                    .unwrap();
            }
            joined.set(true);
        })?;
        assert!(joined.get(), "native callback must finish");
        completed.set(true);
        Ok(())
    })
}

#[cfg(test)]
pub(super) fn test_global_source_expression_pair_v23(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    fault: u8,
    completed: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    original.with_global_source_expressions_v23(optimized, budget, &mut |_, accesses, budget| {
        with_native_global_test_view_v18(optimized, budget, |native, budget| {
            let row = accesses
                .roles
                .rows
                .iter()
                .find(|row| matches!(row.role, Some(DescriptorSourceRoleV18::Write)))
                .unwrap();
            assert!(!row.write_recipe_pending);
            let pair = row.global.as_ref().unwrap();
            assert_eq!(accesses.access(row.output, budget).unwrap(), Some(pair));
            let mut changed = *pair;
            match fault {
                0 => changed.instance = usize::MAX,
                1 => changed.output.value = changed.output.pointer,
                2 => changed.input.logical.root = changed.input.logical.index,
                3 => changed.output.logical.guard_edge.successor = u32::MAX,
                4 => changed.output.writing = false,
                _ => unreachable!(),
            }
            assert!(matches!(
                accesses.check_native_pair_v18(native, &changed, budget),
                Err(PendingGlobalNativeErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "pending global native substituted checked source pair",
                    )
                ))
            ));
            completed.set(true);
        })
    })
}

#[cfg(test)]
pub(super) fn test_global_source_expression_work_v23(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    work_limit: usize,
    remaining: Option<usize>,
    used: &std::cell::Cell<usize>,
    settled: &std::cell::Cell<bool>,
    selected: &std::cell::Cell<Option<(usize, usize)>>,
) -> SourceOwnedResultV18<()> {
    if let Some(remaining) = remaining {
        budget.charge_work(work_limit - budget.work() - remaining)?;
    }
    let floor = budget.storage();
    let work = budget.work();
    let result = original.with_global_source_expressions_v23(
        optimized,
        budget,
        &mut |_, accesses, budget| {
            assert!(accesses.roles.rows.iter().any(|row| matches!(
                row.role,
                Some(DescriptorSourceRoleV18::Write)
            ) && !row.write_recipe_pending));
            accesses.roles.check(budget)
        },
    );
    used.set(budget.work() - work);
    assert_eq!(budget.storage(), floor);
    if let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error))) =
        &result
    {
        assert_eq!(error.limit(), work_limit);
        selected.set(Some((error.actual(), error.limit())));
    }
    settled.set(true);
    result.and_then(|()| {
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "test stops after exact global source expression scope",
        ))
    })
}

#[cfg(test)]
pub(super) fn test_global_source_expression_entry_v23(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    limit: usize,
    fault: u8,
    completed: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    let mut consume = |_, _: &PendingGlobalSourceAccessesV18<'_>, _: &mut ArgumentBudgetV1<'_>| {
        panic!("entry refusal must precede every global source callback");
    };
    let before = (budget.work(), budget.storage());
    let error = match fault {
        0 => {
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 0);
            assert!(matches!(
                foreign.charge_work(1),
                Err(ArgumentResourceV1::Work(_))
            ));
            assert!(matches!(
                foreign.reserve_storage(1),
                Err(ArgumentResourceV1::Storage(_))
            ));
            let foreign_before = (foreign.work(), foreign.storage());
            let error = original
                .with_global_source_expressions_v23(optimized, &mut foreign, &mut consume)
                .unwrap_err();
            assert!(matches!(
                error,
                ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting,)
            ));
            assert_eq!((foreign.work(), foreign.storage()), foreign_before);
            assert_eq!((budget.work(), budget.storage()), before);
            assert!(original.source.cleanup.is_denied());
            error
        }
        1 => {
            // The move closure retains exactly three thin references: original,
            // optimized and the borrowed concrete callback. No owned F is moved.
            type Capture<'a> = (&'a (), &'a (), &'a mut ());
            let header = scoped_source_attempt_header_oracle_v29::<
                (),
                ProductionSourceOwnedViewErrorV18,
                Capture<'_>,
            >();
            let padding = limit - budget.storage() - (header - 1);
            budget.reserve_storage(padding).unwrap();
            let padded = budget.storage();
            let error = original
                .with_global_source_expressions_v23(optimized, budget, &mut consume)
                .unwrap_err();
            let ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(selected)) =
                &error
            else {
                panic!("actual attempt header Storage denial: {error:?}");
            };
            assert_eq!((selected.actual(), selected.limit()), (limit + 1, limit));
            // The generic attempt prepays bounded disposal (32) and result
            // settlement (7) before its first storage reservation.
            assert_eq!((budget.work(), budget.storage()), (before.0 + 32 + 7, padded));
            budget.release_storage(padding).unwrap();
            assert_eq!(budget.storage(), before.1);
            error
        }
        2 => {
            let selected = budget.charge_work(limit).unwrap_err();
            let ArgumentResourceV1::Work(selected) = selected else {
                unreachable!()
            };
            let error = original
                .with_global_source_expressions_v23(optimized, budget, &mut consume)
                .unwrap_err();
            let ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(actual)) =
                &error
            else {
                panic!("original entry Work denial: {error:?}");
            };
            assert_eq!(
                (actual.actual(), actual.limit()),
                (selected.actual(), selected.limit())
            );
            assert_eq!((budget.work(), budget.storage()), before);
            error
        }
        _ => unreachable!(),
    };
    let retry_before = (budget.work(), budget.storage());
    let retry = original
        .with_global_source_expressions_v23(optimized, budget, &mut consume)
        .unwrap_err();
    match (&error, &retry) {
        (
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting),
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting),
        ) => {}
        (
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(first)),
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(next)),
        ) => assert_eq!(
            (first.actual(), first.limit()),
            (next.actual(), next.limit())
        ),
        (
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(first)),
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage(next)),
        ) => assert_eq!(
            (first.actual(), first.limit()),
            (next.actual(), next.limit())
        ),
        _ => panic!("changed first refusal: {error:?}; {retry:?}"),
    }
    assert_eq!((budget.work(), budget.storage()), retry_before);
    completed.set(true);
    Err(error)
}

#[cfg(test)]
pub(super) fn test_global_source_expression_counterfeit_v23(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    fault: u8,
    output: bool,
    completed: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    original.global_expression_entry_v23(optimized, budget)?;
    let floor = budget.storage();
    original.retain_query(scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        budget.reserve_storage(private_source_completion_headers_v20()?)?;
        let index = OriginalEntryIndexV20::build(original, budget)?;
        original.with_optimized_scalar_leaf_namespace_v18(optimized, 0,
            &SourceScalarNamespaceV18::OriginalSourceExpressionsV23, budget, |leaves, budget| {
            leaves.visit_store_inputs(budget, |disposition, budget| {
                let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) = disposition
                else { panic!("genuine retained helper Store"); };
                let mut expression = index.global_store_expression_v23(leaves, &request, budget)?;
                request.original.check_expression(&expression, budget)?;
                request.check_expression(&expression, budget)?;
                let ProductionSemanticExpressionV2::Binary { operation, scalar, lhs, rhs, .. } = &mut expression
                else { panic!("source helper arithmetic must remain a Binary"); };
                match fault {
                    0 => **rhs = ProductionSemanticExpressionV2::Constant { scalar: *scalar, bits: 3 },
                    1 => *operation = ProductionSemanticBinaryOpV2::Multiply,
                    2 => **lhs = ProductionSemanticExpressionV2::Constant { scalar: *scalar, bits: 0 },
                    _ => unreachable!(),
                }
                let error = if output {
                    request.check_expression(&expression, budget)
                } else {
                    request.original.check_expression(&expression, budget)
                }.unwrap_err();
                let expected = if output {
                    "actual optimized scalar expression differs from its original source value"
                } else {
                    "actual scalar expression differs from its original source value"
                };
                assert!(matches!(&error, ProductionSourceOwnedViewErrorV18::Binding(message) if *message == expected), "{error:?}");
                drop(expression);
                completed.set(true);
                Err(error)
            })
        })
    })).map(|_| ())
}

#[cfg(test)]
pub(super) fn test_global_source_expression_callback_denial_v23(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    storage: bool,
    limit: usize,
    completed: &std::cell::Cell<bool>,
    selected: &std::cell::Cell<Option<(usize, usize)>>,
) -> SourceOwnedResultV18<()> {
    original.with_global_source_expressions_v23(optimized, budget, &mut |_, accesses, budget| {
        assert!(accesses.roles.rows.iter().any(|row| matches!(
            row.role,
            Some(DescriptorSourceRoleV18::Write)
        ) && !row.write_recipe_pending));
        let before = (budget.work(), budget.storage());
        let error = if storage {
            budget.reserve_storage(limit)
        } else {
            budget.charge_work(limit)
        }
        .unwrap_err();
        match error {
            ArgumentResourceV1::Storage(error) if storage => {
                selected.set(Some((error.actual(), error.limit())))
            }
            ArgumentResourceV1::Work(error) if !storage => {
                selected.set(Some((error.actual(), error.limit())))
            }
            other => panic!("exact original callback denial: {other:?}"),
        }
        assert_eq!((budget.work(), budget.storage()), before);
        completed.set(true);
        Ok(())
    })
}
