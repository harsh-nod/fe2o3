/// Fresh original source proofs joined through every actual scalar history pair
/// to real final-F private/call checks and all nine reports per definition.
/// Original metadata is never relabeled as final metadata.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalPrivateCallPoliciesV1;
/// fn forge() { let _ = ProductionCanonicalPrivateCallPoliciesV1 {}; }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalPrivateCallPoliciesV1;
/// fn clone(x: &ProductionCanonicalPrivateCallPoliciesV1<'_, '_, '_>) { let _ = (*x).clone(); }
/// ```
pub struct ProductionCanonicalPrivateCallPoliciesV1<'s, 'm, 'g> {
    source: &'s ProductionCanonicalRankedMetadataV1<'m>,
    lineage: ProductionCanonicalScalarLineageV1<'s>,
    assertions: &'s [ProductionCanonicalScalarAssertionV1],
    operations: &'s [ProductionCanonicalPrivateOperationV1],
    aliases: &'s [ProductionCanonicalPrivateSourceAliasV1],
    calls: &'s [ProductionCanonicalPrivateCallSiteV1],
    callables: &'s [ProductionCanonicalAssertionCallableV1],
    native: &'s fe2o3_pliron::CheckedCanonicalPrivateMemoryPoliciesV1<'s, 'g>,
    memory: [usize; 2],
}
impl ProductionCanonicalPrivateCallPoliciesV1<'_, '_, '_> {
    fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<()> {
        self.lineage.guard.query(budget)?;
        self.source.guard.query(budget)?;
        self.native.owner(budget)?;
        Ok(())
    }
    /// Complete original source obligations, including every zero-operation span.
    pub fn original_metadata(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&ProductionCanonicalRankedMetadataV1<'_>> {
        self.query(budget)?;
        Ok(self.source)
    }
    /// Actual final inventory, independently verified and checked by the native facade.
    pub fn final_inventory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&CanonicalKirInventoryV1<'_>> {
        self.query(budget)?;
        Ok(self.lineage.output)
    }
    /// Full numeric definition/use/control lineage, not operation proof-grant unions.
    pub fn lineage(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&ProductionCanonicalScalarLineageV1<'_>> {
        self.query(budget)?;
        Ok(&self.lineage)
    }
    /// Complete original private/call occurrence roster, retaining removed sites.
    pub fn operation_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<usize> {
        self.query(budget)?;
        Ok(self.operations.len())
    }
    /// One occurrence's current coordinate or actual first unreachable-removal event.
    pub fn operation(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&ProductionCanonicalPrivateOperationV1> {
        self.query(budget)?;
        self.operations.get(ordinal).ok_or_else(|| {
            self.lineage
                .guard
                .missing("private operation ordinal")
                .into()
        })
    }
    /// All original root-qualified private/call source aliases, never deduplicated.
    pub fn source_aliases(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&[ProductionCanonicalPrivateSourceAliasV1]> {
        self.query(budget)?;
        Ok(self.aliases)
    }
    /// Every ordinary source call alias with its unchanged original ABI/frame bindings.
    pub fn calls(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&[ProductionCanonicalPrivateCallSiteV1]> {
        self.query(budget)?;
        Ok(self.calls)
    }
    /// Fresh original callable classifications; PrivateFrame is not an empty summary.
    pub fn original_callables(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&[ProductionCanonicalAssertionCallableV1]> {
        self.query(budget)?;
        Ok(self.callables)
    }
    /// Complete source assertion aliases advanced by the existing assertion tracker.
    pub fn assertion_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<usize> {
        self.query(budget)?;
        Ok(self.assertions.len())
    }
    /// One fresh proof and checked retained, selected, source-elided or removed route.
    pub fn assertion(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&ProductionCanonicalScalarAssertionV1> {
        self.query(budget)?;
        self.assertions.get(ordinal).ok_or_else(|| {
            self.lineage
                .guard
                .missing("private assertion ordinal")
                .into()
        })
    }
    /// Exact final physical allocation and access counts after source lifetime checks.
    pub fn memory_census(&self, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<[usize; 2]> {
        self.query(budget)?;
        Ok(self.memory)
    }
    /// Full actual F fixed-nine reports, including definitions with no trap pairs.
    pub fn policies(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<&CheckedCanonicalTrapPoliciesV1<'_, '_>> {
        self.query(budget)?;
        Ok(self.native.trap_policies(budget)?)
    }
    /// Current source-ledger observations, separate from native analysis units.
    pub fn source_resources(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<ProductionCanonicalAssertionResourcesV1> {
        self.query(budget)?;
        Ok(observation(budget))
    }
    /// General rewriting, source families and external obligations remain pending.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// This borrowed analysis view grants no artifact, target or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[allow(clippy::too_many_arguments)]
fn cpc_callback_v1<'w, T>(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    lineage: &CsLineageV1,
    transport: &CpcTransportV1<'_, '_, '_>,
    callables: &[ProductionCanonicalAssertionCallableV1],
    native: &fe2o3_pliron::CheckedCanonicalPrivateMemoryPoliciesV1<'_, '_>,
    memory: [usize; 2],
    budget: &mut ArgumentBudgetV1<'w>,
    callback: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalPrivateCallPoliciesV1<'s, 'm, 'g>,
        &mut ArgumentBudgetV1<'w>,
    ) -> CsResultV1<T>,
) -> CsResultV1<T> {
    cs_check_subjects_v1(owner, source, output, native.owner(budget)?, budget)?;
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<ProductionCanonicalPrivateCallPoliciesV1<'_, '_, '_>>(),
        std::mem::size_of::<CrGuardV1>(),
        std::mem::size_of::<std::thread::Result<CsResultV1<T>>>(),
        std::mem::size_of::<std::thread::Result<()>>(),
    ])?)?;
    let guard = CrGuardV1::new(budget);
    let view = ProductionCanonicalPrivateCallPoliciesV1 {
        source,
        lineage: ProductionCanonicalScalarLineageV1 {
            original: source.inventory,
            output,
            rows: lineage,
            guard: &guard,
        },
        assertions: &transport.assertions.rows,
        operations: &transport.operations,
        aliases: &transport.origins.aliases,
        calls: &transport.origins.calls,
        callables,
        native,
        memory,
    };
    let paid = budget.storage();
    let returned =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(&view, budget)));
    let check = if budget.storage() == paid {
        view.query(budget)
    } else {
        Err(ArgumentResourceV1::Accounting.into())
    };
    if let Err(error) = check {
        cpc_discard_v1(returned);
        return Err(error);
    }
    match returned {
        Ok(result) => result,
        Err(payload) => {
            cpc_discard_v1(payload);
            Err(Failure::Panicked.into())
        }
    }
}

impl ProductionCanonicalScalarFixedPointOwnerV1 {
    /// Rederive complete original source obligations, replay every owned adjacent
    /// transition, and join all sites/calls/assertions to fresh real F policies.
    /// Prepay retained_storage_floor_v1() and siblings. Every callback must retain
    /// the same ledger, slot and exact floor; owned returned data must be prepaid.
    /// No original-N report, donor proof, cached grant or fallback is accepted.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionCanonicalScalarFixedPointOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(owner: &ProductionCanonicalScalarFixedPointOwnerV1, budget: &mut Budget<'_>) {
    ///     owner.with_private_call_policy_checks_v1(budget, |view, _| Ok(view));
    /// }
    /// ```
    pub fn with_private_call_policy_checks_v1<'w, T>(
        &self,
        budget: &mut ArgumentBudgetV1<'w>,
        callback: impl for<'s, 'm, 'g> FnOnce(
            &ProductionCanonicalPrivateCallPoliciesV1<'s, 'm, 'g>,
            &mut ArgumentBudgetV1<'w>,
        ) -> CsResultV1<T>,
    ) -> CsResultV1<T> {
        budget.charge_work(1)?;
        if budget.storage() < self.retained {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        cpc_scope_v1(budget, |budget| {
            self.original.with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                Ok(cpc_with_source_v1(view, budget, |source, coverage, callables, budget| {
                    self.history.replay_against(self.original.executable(), budget)?;
                    let origins = CpcOriginsV1::derive(source, budget)?;
                    let mut transport = CpcTransportV1::original(&origins, coverage, budget)?;
                    let lineage = cs_lineage_with_observer_v1(self, source.inventory, &mut transport, budget)?;
                    cs_with_final_view_v1(self, source, &lineage, budget, |output, checked, budget| {
                        fe2o3_pliron::with_canonical_private_memory_policy_checks_v1(
                            checked,
                            fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1 {
                                max_cells: source.owner.limits.max_operations,
                            },
                            budget,
                            |native, budget| {
                                Ok(cpc_scope_v1(budget, |budget| {
                                    let policies = native.trap_policies(budget)?;
                                    csa_final_join_v1(
                                        source, coverage, output, &lineage, &transport.assertions,
                                        policies, budget,
                                    )?;
                                    cpc_final_calls_v1(
                                        &origins, &transport, output, &lineage, coverage, callables, budget,
                                    )?;
                                    let sites = CpcSiteViewV1::derive(source, output, &lineage, &transport, budget)?;
                                    sites.with_private_source_reader_v1(
                                        native.physical_memory(budget)?, budget, |memory, budget| {
                                            Ok(cpc_callback_v1(
                                                self, source, output, &lineage, &transport, callables,
                                                native, memory, budget, callback,
                                            ))
                                        },
                                    ).map_err(ProductionCanonicalScalarSourceErrorV1::SourcePolicy)?
                                }))
                            },
                        ).map_err(ProductionCanonicalScalarSourceErrorV1::FinalPolicy)?
                    })
                }))
            })?
        })
    }
}

#[cfg(test)]
pub(super) fn read_test_private_call_final_subject_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    donor: &ProductionCanonicalScalarFixedPointOwnerV1,
    inventory_foreign: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    cpc_scope_v1(budget, |budget| {
        owner
            .original
            .with_canonical_ranked_metadata_v1(budget, |source, budget| {
                Ok(cpc_scope_v1(budget, |budget| {
                    let graph = if inventory_foreign {
                        donor.output()
                    } else {
                        owner.output()
                    };
                    let report_subject = if inventory_foreign {
                        owner.output()
                    } else {
                        donor.output()
                    };
                    let (output, receipt) = CanonicalKirInventoryV1::derive(graph, budget)?;
                    budget.reserve_storage(receipt.retained_storage())?;
                    cs_check_subjects_v1(owner, source, &output, report_subject, budget)
                }))
            })?
    })
}
