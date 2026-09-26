mod canonical_assertion_v1 {
    use super::*;
    use fe2o3_kernel_analysis::{CanonicalKirSparseV1, CanonicalKirSparseValueV1};
    use fe2o3_mir_model::{SemanticAssertionLimitsV1, SemanticAssertionOutcomeV1};
    use fe2o3_pliron::CheckedCanonicalTrapPoliciesV1;

    // Only owner-bound C views implement this reader. Shape is not policy success.
    trait AssertionTrapReaderV1<'s, 'g> {
        fn owner(
            &self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<&CsGraphV1, fe2o3_pliron::CanonicalRankedPolicyFailureV1>;
        fn pair_count(
            &self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<usize, fe2o3_pliron::CanonicalRankedPolicyFailureV1>;
        fn pair(
            &self,
            ordinal: usize,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<&fe2o3_pliron::CanonicalTrapPairV1, fe2o3_pliron::CanonicalRankedPolicyFailureV1>;
        fn incoming_edge(
            &self,
            ordinal: usize,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<
            &fe2o3_pliron::CanonicalTrapIncomingEdgeV1<'s, 'g>,
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >;
    }
    macro_rules! assertion_trap_reader_v1 {
        ($view:ident) => {
            impl<'s, 'g> AssertionTrapReaderV1<'s, 'g> for fe2o3_pliron::$view<'s, 'g> {
                fn owner(
                    &self,
                    budget: &mut ArgumentBudgetV1<'_>,
                ) -> Result<&CsGraphV1, fe2o3_pliron::CanonicalRankedPolicyFailureV1> {
                    fe2o3_pliron::$view::owner(self, budget)
                }
                fn pair_count(
                    &self,
                    budget: &mut ArgumentBudgetV1<'_>,
                ) -> Result<usize, fe2o3_pliron::CanonicalRankedPolicyFailureV1> {
                    fe2o3_pliron::$view::pair_count(self, budget)
                }
                fn pair(
                    &self,
                    ordinal: usize,
                    budget: &mut ArgumentBudgetV1<'_>,
                ) -> Result<
                    &fe2o3_pliron::CanonicalTrapPairV1,
                    fe2o3_pliron::CanonicalRankedPolicyFailureV1,
                > {
                    fe2o3_pliron::$view::pair(self, ordinal, budget)
                }
                fn incoming_edge(
                    &self,
                    ordinal: usize,
                    budget: &mut ArgumentBudgetV1<'_>,
                ) -> Result<
                    &fe2o3_pliron::CanonicalTrapIncomingEdgeV1<'s, 'g>,
                    fe2o3_pliron::CanonicalRankedPolicyFailureV1,
                > {
                    fe2o3_pliron::$view::incoming_edge(self, ordinal, budget)
                }
            }
        };
    }
    assertion_trap_reader_v1!(CheckedCanonicalTrapPoliciesV1);
    assertion_trap_reader_v1!(CheckedCanonicalTrapShapeV1);

    /// An actual source, graph, proof or resource refusal, never an admission grant.
    #[derive(Debug)]
    pub enum ProductionCanonicalAssertionFailureV1 {
        /// A graph, source, private-memory or accounting policy rejected the input.
        Policy(ProductionCanonicalRankedPolicyErrorV1),
        /// The live source assertion query failed or exhausted its budget.
        SourceQuery(crate::ProductionSemanticAssertionQueryErrorV1),
        /// Sparse canonical value analysis failed.
        Sparse(fe2o3_kernel_analysis::CanonicalKirSparseErrorV1),
        /// Canonical call-effect analysis failed.
        CallEffects(fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1),
        /// The source callable-summary engine failed.
        Callable(fe2o3_mir_model::SemanticAssertionErrorV1),
        /// No accepted source proof establishes this assertion.
        NotProved {
            /// Original source-span ordinal.
            span: usize,
            /// The proof engine's explicit incomplete or unsupported reason.
            reason: fe2o3_mir_model::SemanticAssertionNotProvedV1,
        },
        /// The source proof engine established the opposite assertion outcome.
        Refuted {
            /// Original source-span ordinal.
            span: usize,
        },
        /// Source, graph or proof subjects do not match exactly.
        Binding {
            /// Original source-span ordinal when the mismatch identifies one.
            span: Option<usize>,
            /// Stable description of the failed subject check.
            detail: &'static str,
        },
        /// Prepaid backing storage could not be allocated.
        Allocation,
        /// A scoped producer or callback panicked.
        Panicked,
    }
    use ProductionCanonicalAssertionFailureV1 as Failure;
    type R<T> = Result<T, Failure>;
    impl std::fmt::Display for Failure {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "canonical assertion admission: {self:?}")
        }
    }
    impl std::error::Error for Failure {}
    impl From<ArgumentResourceV1> for Failure {
        fn from(error: ArgumentResourceV1) -> Self {
            Self::Policy(error.into())
        }
    }
    impl From<ProductionCanonicalRankedSourceErrorV1> for Failure {
        fn from(error: ProductionCanonicalRankedSourceErrorV1) -> Self {
            Self::Policy(error.into())
        }
    }
    impl From<ProductionCanonicalRankedPolicyErrorV1> for Failure {
        fn from(error: ProductionCanonicalRankedPolicyErrorV1) -> Self {
            Self::Policy(error)
        }
    }
    impl From<fe2o3_pliron::CanonicalRankedPolicyFailureV1> for Failure {
        fn from(error: fe2o3_pliron::CanonicalRankedPolicyFailureV1) -> Self {
            Self::Policy(error.into())
        }
    }
    include!("production_canonical_assertion_resources_v1.rs");
    include!("production_canonical_assertion_source_v1.rs");
    include!("production_canonical_assertion_join_v1.rs");
    include!("production_canonical_assertion_calls_v1.rs");
    include!("production_canonical_scalar_assertion_resources_v1.rs");
    include!("production_canonical_scalar_assertion_source_v1.rs");
    include!("production_canonical_scalar_assertion_transport_v1.rs");
    include!("production_canonical_scalar_assertion_checks_v1.rs");
    include!("production_canonical_private_call_resources_v1.rs");
    include!("production_canonical_private_call_source_v1.rs");
    include!("production_canonical_private_call_memory_v1.rs");
    include!("production_canonical_private_call_calls_v1.rs");
    include!("production_canonical_private_call_transport_v1.rs");
    include!("production_canonical_private_call_checks_v1.rs");

    #[cfg(test)]
    pub(super) fn read_test_row(
        source: &ProductionCanonicalRankedMetadataV1<'_>,
        policies: &CheckedCanonicalTrapPoliciesV1<'_, '_>,
        row: &ProductionCanonicalRankedAssertionV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<()> {
        scoped(budget, |budget| {
            budget.reserve_storage(std::mem::size_of::<AssertGraphIndexV1<'_>>())?;
            let graph = AssertGraphIndexV1::build(source.inventory.owner().module(), true, budget)
                .map_err(query_origin)?;
            let (sparse, receipt) = CanonicalKirSparseV1::derive(
                source.inventory,
                fe2o3_kernel_analysis::CanonicalKirSparseLimitsV1::default(),
                budget,
            )
            .map_err(Failure::Sparse)?;
            budget.reserve_storage(receipt.retained_storage())?;
            let mut count = 0usize;
            for pair in 0..policies.pair_count(budget)? {
                count = policies.pair(pair, budget)?.incoming_edges().end;
            }
            budget.reserve_storage(std::mem::size_of::<Vec<usize>>())?;
            let mut incoming = rows(count, budget)?;
            budget.charge_work(count)?;
            incoming.resize(count, 0);
            join(
                source,
                row,
                &graph,
                &sparse,
                policies,
                &mut incoming,
                budget,
            )
        })
    }
    #[cfg(test)]
    pub(super) fn read_test_sparse(
        source: &ProductionCanonicalRankedMetadataV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<()> {
        scoped(budget, |budget| {
            let (sparse, receipt) = CanonicalKirSparseV1::derive(
                source.inventory,
                fe2o3_kernel_analysis::CanonicalKirSparseLimitsV1::default(),
                budget,
            )
            .map_err(Failure::Sparse)?;
            budget.reserve_storage(receipt.retained_storage())?;
            let row = &source.contracts.assertions[0];
            let SemanticKirAssertConditionOutcomeV1::Emitted { condition_use, .. } =
                row.binding.outcome()
            else {
                return Err(binding(None, "test requires an actual retained assertion"));
            };
            let value = sparse
                .value_at_use(condition_use, budget)
                .map_err(Failure::Sparse)?;
            assert!(matches!(value, CanonicalKirSparseValueV1::Constant(value)
                if value.ty() == ScalarType::Bool && value.bits() == u128::from(row.binding.expected())));
            sparse_veto(value, row.binding.expected(), row.span)?;
            assert!(matches!(
                sparse_veto(value, !row.binding.expected(), row.span),
                Err(Failure::Binding {
                    detail: "actual Boolean contradicts proved source polarity",
                    ..
                })
            ));
            Ok(())
        })
    }
    #[cfg(test)]
    pub(super) fn read_test_limits(
        source: &ProductionCanonicalRankedMetadataV1<'_>,
        policies: &CheckedCanonicalTrapPoliciesV1<'_, '_>,
        limits: SemanticAssertionLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<()> {
        scoped(budget, |budget| {
            let _coverage = derive_with_limits(source, policies, limits, budget)?;
            Ok(())
        })
    }
    #[cfg(test)]
    mod resource_tests {
        use super::*;
        include!("production_canonical_assertion_resources_v1_tests.rs");
    }

    /// Independent source-terminal and completed-C callback-prefix observations.
    #[derive(Debug)]
    pub struct ProductionCanonicalAssertionChecksErrorV1 {
        /// The refusal that prevented completion of this scoped invocation.
        pub failure: Failure,
        /// Callback-entry snapshot after all real C reports, before their cleanup.
        /// A nested C failure separately preserves its established terminal prefix.
        pub policies: Option<fe2o3_pliron::CanonicalRankedPolicyResourceObservationV1>,
        /// None if the caller replaced or undercut the original ledger.
        pub source: Option<ProductionCanonicalAssertionResourcesV1>,
    }
    impl std::fmt::Display for ProductionCanonicalAssertionChecksErrorV1 {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.failure.fmt(f)
        }
    }
    impl std::error::Error for ProductionCanonicalAssertionChecksErrorV1 {}

    /// Complete original assertion aliases joined to one unchanged graph.
    /// The view cannot be constructed, cloned or detached from its paid callback.
    /// All full-compiler obligations remain pending.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionCanonicalAssertionSourcePoliciesV1;
    /// fn forge() { let _ = ProductionCanonicalAssertionSourcePoliciesV1 {}; }
    /// ```
    pub struct ProductionCanonicalAssertionSourcePoliciesV1<'s, 'm, 'g> {
        source: &'s ProductionCanonicalRankedMetadataV1<'m>,
        policies: &'s CheckedCanonicalTrapPoliciesV1<'s, 'g>,
        coverage: &'s Coverage<'m>,
        calls: &'s [ProductionCanonicalAssertionCallableV1],
        memory: [usize; 2],
        guard: &'s CrGuardV1,
    }
    impl ProductionCanonicalAssertionSourcePoliciesV1<'_, '_, '_> {
        fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<()> {
            self.guard.query(budget)?;
            self.source.guard.query(budget)?;
            self.policies.owner(budget)?;
            Ok(())
        }
        /// Borrow exact original metadata after checking the live source budget.
        pub fn metadata(
            &self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> R<&ProductionCanonicalRankedMetadataV1<'_>> {
            self.query(budget)?;
            Ok(self.source)
        }
        /// Borrow the completed original-graph trap/private analysis reports.
        pub fn policies(
            &self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> R<&CheckedCanonicalTrapPoliciesV1<'_, '_>> {
            self.query(budget)?;
            Ok(self.policies)
        }
        /// Query the number of completely covered original assertion aliases.
        pub fn assertion_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
            self.query(budget)?;
            Ok(self.source.contracts.assertions.len())
        }
        /// Borrow one proved alias; an invalid ordinal poisons the scoped query.
        pub fn assertion(
            &self,
            ordinal: usize,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> R<&ProductionCanonicalAssertionV1> {
            self.query(budget)?;
            self.source
                .contracts
                .assertions
                .get(ordinal)
                .and_then(|row| self.coverage.rows.get(row.span))
                .and_then(Option::as_ref)
                .ok_or_else(|| self.guard.missing("assertion ordinal").into())
        }
        /// Borrow source/graph call classifications without detaching their owner.
        pub fn callables(
            &self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> R<&[ProductionCanonicalAssertionCallableV1]> {
            self.query(budget)?;
            Ok(self.calls)
        }
        /// Query the independently checked private allocation and access counts.
        pub fn memory_census(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<[usize; 2]> {
            self.query(budget)?;
            Ok(self.memory)
        }
        /// Observe cumulative source-ledger usage after this paid query.
        pub fn source_resources(
            &self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> R<ProductionCanonicalAssertionResourcesV1> {
            self.query(budget)?;
            Ok(observation(budget))
        }
        /// Whether every full-compiler obligation is discharged; currently false.
        pub const fn ranked_verification_is_complete(&self) -> bool {
            false
        }
        /// This scoped analysis view never grants artifact or launch authority.
        pub const fn grants_artifact_or_launch_authority(&self) -> bool {
            false
        }
    }

    impl ProductionCanonicalRankedSourceViewV1<'_, '_, '_, '_, '_> {
        /// Check original N, never a donor graph or an optimized successor.
        /// A/B source facts do not discharge all nineteen compiler obligations.
        ///
        /// ```compile_fail
        /// use fe2o3_lower_mir_kernel::ProductionCanonicalRankedSourceViewV1;
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn escape(view: &mut ProductionCanonicalRankedSourceViewV1<'_, '_, '_, '_, '_>, budget: &mut Budget<'_>) {
        ///     view.with_assertion_policy_checks_v1(budget, |proof, _| Ok(proof));
        /// }
        /// ```
        pub fn with_assertion_policy_checks_v1<'w, T>(
            &mut self,
            budget: &mut ArgumentBudgetV1<'w>,
            callback: impl for<'s, 'm, 'g> FnOnce(
                &ProductionCanonicalAssertionSourcePoliciesV1<'s, 'm, 'g>,
                &mut ArgumentBudgetV1<'w>,
            ) -> R<T>,
        ) -> Result<T, ProductionCanonicalAssertionChecksErrorV1> {
            let ledger = budget.work_ledger_identity_v1();
            let floor = budget.storage();
            let mut policies_observation = None;
            let result = (|| {
                self.source.guard.query(budget)?;
                let actual = self
                    .checked
                    .inventory(budget)
                    .map_err(ProductionCanonicalRankedSourceErrorV1::from)?
                    .owner();
                if !std::ptr::eq(actual, self.source.owner.executable()) {
                    return Err(binding(None, "original graph identity"));
                }
                let source = self.source;
                // C performs its last exact-floor inventory query before B scratch.
                fe2o3_pliron::with_canonical_trap_policy_checks_v1(self.checked, budget, |policies, budget| {
                    policies_observation = Some(policies.observation(budget)?);
                    Ok(scoped(budget, |budget| {
                        let coverage = derive(source, policies, budget)?;
                        cr_private_source_profile_with_assertions_v1(source, Some(&coverage), budget)?;
                        let calls = callable_rows(source, budget)?;
                        cr_private_calls_v1(source, budget)?;
                        checked_output_admission_policy3_v1::with_canonical_private_source_reader_v1(source, budget, |memory, budget| {
                            Ok(scoped(budget, |budget| {
                                budget.reserve_storage(argument_sum_v1(&[
                                    std::mem::size_of::<ProductionCanonicalAssertionSourcePoliciesV1<'_, '_, '_>>(),
                                    std::mem::size_of::<CrGuardV1>(),
                                    std::mem::size_of::<std::thread::Result<R<T>>>(),
                                ])?)?;
                                let guard = CrGuardV1::new(budget);
                                let view = ProductionCanonicalAssertionSourcePoliciesV1 {
                                    source, policies, coverage: &coverage, calls: &calls, memory, guard: &guard,
                                };
                                let paid = budget.storage();
                                let returned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(&view, budget)));
                                let check = if budget.storage() == paid {
                                    view.query(budget)
                                } else { Err(ArgumentResourceV1::Accounting.into()) };
                                if let Err(error) = check { drop(returned); return Err(error); }
                                match returned {
                                    Ok(result) => result,
                                    Err(payload) => { drop(payload); Err(Failure::Panicked) }
                                }
                            }))
                        }).map_err(Failure::from)?
                    }))
                }).map_err(|error| Failure::Policy(ProductionCanonicalRankedPolicyErrorV1::Policy(error)))?
            })();
            result.map_err(|failure| ProductionCanonicalAssertionChecksErrorV1 {
                failure,
                policies: policies_observation,
                source: (ledger == budget.work_ledger_identity_v1() && budget.storage() >= floor)
                    .then(|| observation(budget)),
            })
        }
    }
}
pub use canonical_assertion_v1::{
    ProductionCanonicalAssertionCallKindV1, ProductionCanonicalAssertionCallableV1,
    ProductionCanonicalAssertionChecksErrorV1, ProductionCanonicalAssertionFailureV1,
    ProductionCanonicalAssertionResourcesV1, ProductionCanonicalAssertionSourcePoliciesV1,
    ProductionCanonicalAssertionV1, ProductionCanonicalPrivateCallPoliciesV1,
    ProductionCanonicalPrivateCallSiteV1, ProductionCanonicalPrivateOperationKindV1,
    ProductionCanonicalPrivateOperationV1, ProductionCanonicalPrivateSourceAliasV1,
    ProductionCanonicalScalarAssertionDispositionV1, ProductionCanonicalScalarAssertionPoliciesV1,
    ProductionCanonicalScalarAssertionStepV1, ProductionCanonicalScalarAssertionV1,
};
