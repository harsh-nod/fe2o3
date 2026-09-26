/// The original formal refusal and the source scope's first refusal are retained
/// separately. Merely constructing or converting an error grants no authority.
#[derive(Debug)]
pub enum ProductionOptimizedSourceFormalErrorV18<E> {
    /// Source custody or endpoint association failed independently of a formal
    /// query. No facts from a substituted or underfunded owner may be consumed.
    Source(ProductionSourceOwnedViewErrorV18),
    /// An observed formal-query refusal, retaining both its exact diagnostic
    /// and the source scope's separately selected first refusal.
    Formal {
        /// Original formal diagnostic, including its own work/storage counts.
        error: fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        /// Sticky source refusal recorded when the formal query was observed.
        /// Inert error conversion alone does not mutate either resource meter.
        source_refusal: ProductionSourceOwnedViewErrorV18,
    },
    /// Caller-provided failure from the scoped facts consumer, without erasing
    /// its payload or replacing it with a generic formal error.
    Consumer(E),
}

impl<E> From<ProductionSourceOwnedViewErrorV18> for ProductionOptimizedSourceFormalErrorV18<E> {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

impl<E: std::fmt::Display> std::fmt::Display for ProductionOptimizedSourceFormalErrorV18<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Formal {
                error,
                source_refusal,
            } => {
                write!(
                    formatter,
                    "{error}; retained source refusal: {source_refusal}"
                )
            }
            Self::Consumer(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ProductionOptimizedSourceFormalErrorV18<E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Formal { error, .. } => Some(error),
            Self::Consumer(error) => Some(error),
        }
    }
}

fn optimized_source_observed_formal_error_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    error: &fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    use fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1 as Formal;
    let refusal = match error {
        Formal::Resource(error) => {
            if matches!(
                error,
                fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Accounting
            ) {
                original.source.cleanup.deny_refund();
            }
            ArgumentResourceV1::from(*error).into()
        }
        Formal::ControlFlow(_) => ProductionSourceOwnedViewErrorV18::Binding(
            "optimized formal control-flow analysis refused the actual output",
        ),
        Formal::FunctionLimit { .. } => ProductionSourceOwnedViewErrorV18::Binding(
            "optimized formal function roster exceeded its local limit",
        ),
        Formal::Coordinate(_) => ProductionSourceOwnedViewErrorV18::Binding(
            "optimized formal query used a foreign output coordinate",
        ),
        Formal::Panicked => {
            ProductionSourceOwnedViewErrorV18::Binding("optimized formal callback panicked")
        }
    };
    original.retain_query(Err::<(), _>(refusal)).unwrap_err()
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Borrows fresh local guarded-read facts from the actual checked output.
    /// These facts do not discharge allocation, source equivalence, or pending
    /// reference proofs; those remain separate final-relation obligations.
    pub fn with_optimized_guarded_reads_v18<'work, T, E>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        limits: fe2o3_kernel_ir::CanonicalGuardedGlobalReadLimitsV1,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope, 'graph> FnOnce(
            &fe2o3_kernel_ir::CheckedCanonicalGuardedGlobalReadsV18<'scope, 'graph>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, ProductionOptimizedSourceFormalErrorV18<E>> {
        use ProductionOptimizedSourceFormalErrorV18 as Failure;
        optimized_source_endpoints_v18(self, optimized, budget)?;
        let output = optimized.output_inventory(budget)?;
        let floor = budget.storage();
        scoped_source_attempt_v29(self.source.cleanup, budget, floor, |budget| {
            let headers = std::mem::size_of::<
                Result<Result<T, E>, fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>,
            >()
            .checked_add(std::mem::size_of::<Result<T, Failure<E>>>())
            .and_then(|bytes| {
                bytes.checked_add(std::mem::size_of::<
                    fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
                >())
            })
            .and_then(|bytes| bytes.checked_add(source_reference_cleanup_headers_v29().ok()?))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
            self.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
            self.retain_query(
                budget
                    .charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)
                    .map_err(Into::into),
            )?;
            let result = fe2o3_kernel_ir::with_canonical_guarded_global_reads_v18(
                output.owner(),
                limits,
                budget,
                |facts, budget| Ok(consume(facts, budget)),
            );
            let value = match result {
                Err(error) => {
                    let source_refusal = optimized_source_observed_formal_error_v18(self, &error);
                    return Err(SourceConsumerErrorV18(Failure::Formal {
                        error,
                        source_refusal,
                    }));
                }
                Ok(Err(error)) => {
                    if let Some(first) = self.source.guard.first.get() {
                        source_reference_discard_v29(Err::<T, E>(error));
                        return Err(SourceConsumerErrorV18(Failure::Source(first.error())));
                    }
                    return Err(SourceConsumerErrorV18(Failure::Consumer(error)));
                }
                Ok(Ok(value)) => value,
            };
            let postflight =
                optimized_source_endpoints_v18(self, optimized, budget).and_then(|()| {
                    self.retain_query((|| {
                        budget.charge_work(1)?;
                        if !std::ptr::eq(output, optimized.output_inventory(budget)?) {
                            return self
                                .source
                                .missing("optimized formal output inventory changed");
                        }
                        Ok(())
                    })())
                });
            if let Err(error) = postflight {
                source_reference_discard_v29(Ok::<T, E>(value));
                return Err(SourceConsumerErrorV18(Failure::Source(error)));
            }
            if let Err(error) =
                self.retain_query(budget.release_storage(headers).map_err(Into::into))
            {
                source_reference_discard_v29(Ok::<T, E>(value));
                return Err(SourceConsumerErrorV18(Failure::Source(error)));
            }
            Ok(value)
        })
        .map_err(|error: SourceConsumerErrorV18<Failure<E>>| error.0)
    }
}
