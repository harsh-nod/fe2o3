include!("production_canonical_ranked_private_source_v1.rs");
include!("production_canonical_ranked_private_calls_v1.rs");

/// Fresh private graph policies paired with actual original-source lifetimes
/// and every root-qualified helper/ABI association. No completed-ranked authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCanonicalPrivateSourcePoliciesV1;
/// fn forge() { let _ = ProductionCanonicalPrivateSourcePoliciesV1 {}; }
/// ```
pub struct ProductionCanonicalPrivateSourcePoliciesV1<'s, 'm, 'g> {
    source: &'s ProductionCanonicalRankedMetadataV1<'m>,
    policies: &'s fe2o3_pliron::CheckedCanonicalPrivatePoliciesV1<'s, 'g>,
    memory: [usize; 2],
    calls: usize,
}
impl ProductionCanonicalPrivateSourcePoliciesV1<'_, '_, '_> {
    /// Borrow the original-source metadata after checking both live query guards.
    pub fn metadata(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrPolicyResultV1<&ProductionCanonicalRankedMetadataV1<'_>> {
        self.policies.function_count(budget)?;
        self.source.guard.query(budget)?;
        Ok(self.source)
    }
    /// Borrow the fresh private policy reports for this same unchanged graph.
    pub fn policies(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrPolicyResultV1<&fe2o3_pliron::CheckedCanonicalPrivatePoliciesV1<'_, '_>> {
        self.source.guard.query(budget)?;
        self.policies.function_count(budget)?;
        Ok(self.policies)
    }
    /// Physical allocation/access and root-qualified call counts are diagnostics,
    /// not permission to replace the graph or omit another proof component.
    pub fn census(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CrPolicyResultV1<(usize, usize, usize)> {
        self.source.guard.query(budget)?;
        self.policies.function_count(budget)?;
        Ok((self.memory[0], self.memory[1], self.calls))
    }
    /// Always false: these paired reports do not discharge all compiler obligations.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// Always false: diagnostic source and policy facts do not authorize execution.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn cr_private_protected_v1<'w, T>(
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> CrPolicyResultV1<T>,
) -> CrPolicyResultV1<T> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    budget.charge_work(2)?;
    let floor = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    if slot != std::ptr::from_ref(&*budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        drop(result);
        return Err(ArgumentResourceV1::Accounting.into());
    }
    // Unwind-owned callback values and payloads die before restoring credit.
    let value = match result {
        Ok(value) => value,
        Err(payload) => {
            drop(payload);
            Err(ProductionCanonicalRankedSourceErrorV1::Panicked.into())
        }
    };
    budget.release_storage(budget.storage() - floor)?;
    value
}

impl ProductionCanonicalRankedSourceViewV1<'_, '_, '_, '_, '_> {
    /// Check one unchanged N. The graph pass precedes source-proof scratch so
    /// the existing structural view's exact-floor query is never bypassed.
    pub fn with_private_policy_checks_v1<'w, T>(
        &mut self,
        budget: &mut ArgumentBudgetV1<'w>,
        callback: impl for<'s, 'm, 'g> FnOnce(
            &ProductionCanonicalPrivateSourcePoliciesV1<'s, 'm, 'g>,
            &mut ArgumentBudgetV1<'w>,
        ) -> CrPolicyResultV1<T>,
    ) -> CrPolicyResultV1<T> {
        self.source.guard.query(budget)?;
        let actual = self
            .checked
            .inventory(budget)
            .map_err(ProductionCanonicalRankedSourceErrorV1::from)?
            .owner();
        if !std::ptr::eq(actual, self.source.owner.executable()) {
            return Err(cr_policy_unsupported_v1(
                ProductionCanonicalRankedSourceRequirementV1::GraphIdentity,
                0,
            ));
        }
        let source = self.source;
        fe2o3_pliron::with_canonical_private_policy_checks_v1(
            self.checked,
            budget,
            |policies, budget| {
                Ok(cr_private_protected_v1(budget, |budget| {
                    cr_private_source_profile_v1(source, budget)?;
                    let calls = cr_private_calls_v1(source, budget)?;
                    checked_output_admission_policy3_v1::with_canonical_private_source_reader_v1(
                        source,
                        budget,
                        |memory, budget| {
                            use std::panic::{AssertUnwindSafe, catch_unwind};
                            budget.reserve_storage(argument_sum_v1(&[
                                std::mem::size_of::<
                                    ProductionCanonicalPrivateSourcePoliciesV1<'_, '_, '_>,
                                >(),
                                std::mem::size_of::<std::thread::Result<CrPolicyResultV1<T>>>(),
                            ])?)?;
                            let paid = budget.storage();
                            let slot = std::ptr::from_ref(&*budget) as usize;
                            let ledger = budget.work_ledger_identity_v1();
                            let view = ProductionCanonicalPrivateSourcePoliciesV1 {
                                source,
                                policies,
                                memory,
                                calls,
                            };
                            let returned =
                                catch_unwind(AssertUnwindSafe(|| callback(&view, budget)));
                            let exact = slot == std::ptr::from_ref(&*budget) as usize
                                && ledger == budget.work_ledger_identity_v1()
                                && budget.storage() == paid;
                            let check = if exact {
                                source
                                    .guard
                                    .check(budget)
                                    .map_err(ProductionCanonicalRankedPolicyErrorV1::from)
                                    .and_then(|()| {
                                        policies
                                            .function_count(budget)
                                            .map(|_| ())
                                            .map_err(Into::into)
                                    })
                            } else {
                                Err(ArgumentResourceV1::Accounting.into())
                            };
                            if let Err(error) = check {
                                let rejected = catch_unwind(AssertUnwindSafe(|| drop(returned)));
                                drop(rejected);
                                return Err(error);
                            }
                            match returned {
                                Ok(result) => result,
                                Err(payload) => {
                                    drop(payload);
                                    Err(ProductionCanonicalRankedSourceErrorV1::Panicked.into())
                                }
                            }
                        },
                    )
                }))
            },
        )
        .map_err(ProductionCanonicalRankedPolicyErrorV1::Policy)?
    }
}
