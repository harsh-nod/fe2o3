/// Which exact owner in the current paired-report batch supplies a path view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionFormalPathSideV20 {
    /// The genuine original source-owned canonical module.
    Original,
    /// The actual checked optimizer successor, not a rebuilt module.
    Optimized,
}

/// Typed failure while consuming genuine paired reports and path observations.
/// This error and successful observation are not final memory authority.
#[derive(Debug)]
pub enum ProductionOptimizedSourcePathsErrorV20<E> {
    /// Original owner, correspondence, or shared source custody refused.
    Source(ProductionSourceOwnedViewErrorV18),
    /// Full report construction refused without discarding its typed cause.
    Formal {
        /// Exact formal engine failure.
        error: fe2o3_kernel_ir::CanonicalFormalReportErrorV19,
        /// First source-side refusal retained on the original account.
        source_refusal: ProductionSourceOwnedViewErrorV18,
    },
    /// The path consumer refused one exact side of the current pair.
    Path {
        /// Original or actual optimized report that was being consumed.
        side: ProductionFormalPathSideV20,
        /// Exact source, relation, solver or resource failure.
        error: fe2o3_kernel_analysis::FormalPaidPathErrorV20,
        /// Separately retained first source-side refusal.
        source_refusal: ProductionSourceOwnedViewErrorV18,
    },
    /// The single cumulative solver session refused entry or settlement.
    Query {
        /// Exact cumulative solver failure.
        error: fe2o3_kernel_analysis::PresburgerQueryErrorV2,
        /// Separately retained first source-side refusal.
        source_refusal: ProductionSourceOwnedViewErrorV18,
    },
    /// The lexical consumer rejected the unchanged reports and observations.
    Consumer(E),
}
impl<E> From<ProductionSourceOwnedViewErrorV18> for ProductionOptimizedSourcePathsErrorV20<E> {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl<E: fmt::Display> fmt::Display for ProductionOptimizedSourcePathsErrorV20<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::Formal {
                error,
                source_refusal,
            } => {
                write!(out, "{error}; retained source refusal: {source_refusal}")
            }
            Self::Path {
                side,
                error,
                source_refusal,
            } => {
                write!(
                    out,
                    "{side:?} path: {error}; retained source refusal: {source_refusal}"
                )
            }
            Self::Query {
                error,
                source_refusal,
            } => {
                write!(out, "{error}; retained source refusal: {source_refusal}")
            }
            Self::Consumer(error) => error.fmt(out),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProductionOptimizedSourcePathsErrorV20<E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Formal { error, .. } => Some(error),
            Self::Path { error, .. } => Some(error),
            Self::Query { error, .. } => Some(error),
            Self::Consumer(error) => Some(error),
        }
    }
}

fn optimized_source_path_query_refusal_v20(
    original: &ProductionSourceCorrespondenceV18<'_>,
    error: &fe2o3_kernel_analysis::PresburgerQueryErrorV2,
    budget: &ArgumentBudgetV1<'_>,
) -> ProductionSourceOwnedViewErrorV18 {
    use fe2o3_kernel_analysis::PresburgerQueryErrorV2 as Query;
    if let Err(first) = original.paired_report_prior_denial_v19(budget) {
        return first;
    }
    let refusal = match error {
        Query::Resource(resource) => {
            if matches!(resource, ArgumentResourceV1::Accounting) {
                original.source.cleanup.deny_refund();
            }
            (*resource).into()
        }
        Query::Panicked => {
            ProductionSourceOwnedViewErrorV18::Binding("paired path solver callback panicked")
        }
        _ => ProductionSourceOwnedViewErrorV18::Binding("paired path cumulative solver refused"),
    };
    original.retain_query(Err::<(), _>(refusal)).unwrap_err()
}

fn optimized_source_path_refusal_v20(
    original: &ProductionSourceCorrespondenceV18<'_>,
    error: &fe2o3_kernel_analysis::FormalPaidPathErrorV20,
    budget: &ArgumentBudgetV1<'_>,
) -> ProductionSourceOwnedViewErrorV18 {
    use fe2o3_kernel_analysis::FormalPaidPathErrorV20 as Path;
    match error {
        Path::Source(error) => optimized_source_report_refusal_v19(original, error),
        Path::Query(error) => optimized_source_path_query_refusal_v20(original, error, budget),
        Path::Resource(resource) => {
            if matches!(resource, ArgumentResourceV1::Accounting) {
                original.source.cleanup.deny_refund();
            }
            original
                .retain_query(Err::<(), _>((*resource).into()))
                .unwrap_err()
        }
        Path::InconsistentOriginalReport => original
            .source
            .missing::<()>("paired path original report association refused")
            .unwrap_err(),
        Path::ConsumerRejected => original
            .source
            .missing::<()>("paired path consumer refused without typed error")
            .unwrap_err(),
        Path::Panicked => original
            .source
            .missing::<()>("paired path consumer panicked")
            .unwrap_err(),
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Consumes original then optimized path observations for every genuine
    /// paired root, on one cumulative Presburger session and original ledger.
    ///
    /// Both full reports remain live during each side's callback. Every reason,
    /// access, conflict and other obligation remains unchanged. A proved path
    /// row cannot supply missing full-domain pairs, prove memory equivalence,
    /// or authorize default compilation, target output or publication.
    pub fn with_optimized_formal_paths_v20<'work, E, F>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        launches: &[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        limits: fe2o3_kernel_ir::ControlFlowLimits,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: F,
    ) -> Result<(), ProductionOptimizedSourcePathsErrorV20<E>>
    where
        F: for<'a, 'b, 'c, 'd, 'path> FnMut(
            ProductionFormalPathSideV20,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'a, 'b>,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'c, 'd>,
            &fe2o3_kernel_analysis::FormalPaidPathViewV20<'path>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
    {
        self.with_optimized_formal_paths_limits_v20(
            optimized,
            launches,
            width,
            limits,
            fe2o3_kernel_analysis::PresburgerQueryLimitsV2::default(),
            budget,
            consume,
        )
    }

    fn with_optimized_formal_paths_limits_v20<'work, E, F>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        launches: &[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        limits: fe2o3_kernel_ir::ControlFlowLimits,
        query_limits: fe2o3_kernel_analysis::PresburgerQueryLimitsV2,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: F,
    ) -> Result<(), ProductionOptimizedSourcePathsErrorV20<E>>
    where
        F: for<'a, 'b, 'c, 'd, 'path> FnMut(
            ProductionFormalPathSideV20,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'a, 'b>,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'c, 'd>,
            &fe2o3_kernel_analysis::FormalPaidPathViewV20<'path>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
    {
        use ProductionOptimizedSourcePathsErrorV20 as Failure;
        let floor = budget.storage();
        let slot = std::ptr::from_ref(&*budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let mut pending = Some(consume);
        let mut entry = |budget: &mut ArgumentBudgetV1<'work>| {
            self.paired_formal_paths_inner_v20(
                optimized,
                launches,
                width,
                limits,
                query_limits,
                budget,
                pending.as_mut().expect("single paired path consumer"),
            )
        };
        let entry_bytes = std::mem::size_of_val(&entry);
        let mut credit = 0;
        let attempted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<(), Failure<E>> {
                self.paired_report_prior_denial_v19(budget)?;
                self.query(budget)?;
                let headers = argument_sum_v1(&[
                    entry_bytes,
                    size_of::<Option<F>>(),
                    size_of::<F>(),
                    2 * size_of::<Result<(), Failure<E>>>(),
                    size_of::<std::thread::Result<Result<(), Failure<E>>>>(),
                    size_of::<Option<Failure<E>>>(),
                    size_of::<Option<E>>(),
                    size_of::<Result<(), E>>(),
                    size_of::<Result<(), ProductionOptimizedSourceReportsErrorV19<Failure<E>>>>(),
                    size_of::<Result<(), fe2o3_kernel_analysis::PresburgerQueryErrorV2>>(),
                    size_of::<Result<(), fe2o3_kernel_analysis::FormalPaidPathErrorV20>>(),
                    size_of::<ProductionFormalPathSideV20>(),
                    size_of::<fe2o3_kernel_analysis::PresburgerQueryLimitsV2>(),
                    size_of::<(
                        &ProductionSourceCorrespondenceV18<'_>,
                        &mut ArgumentBudgetV1<'_>,
                        &mut (),
                        &mut usize,
                        &usize,
                    )>(),
                    size_of::<(
                        usize,
                        usize,
                        usize,
                        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
                    )>(),
                    size_of::<(
                        usize,
                        usize,
                        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
                    )>(),
                    size_of::<(
                        &ProductionSourceCorrespondenceV18<'_>,
                        &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
                        &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
                        &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
                        &mut fe2o3_kernel_analysis::PresburgerQueryScopeV4<'_>,
                        &mut ArgumentBudgetV1<'_>,
                        &mut F,
                    )>(),
                    source_reference_cleanup_headers_v29()
                        .map_err(ProductionSourceOwnedViewErrorV18::from)?,
                ])
                .map_err(ProductionSourceOwnedViewErrorV18::from)?;
                self.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
                credit = headers;
                entry(budget)
            },
        ));
        drop(entry);
        let result = match attempted {
            Ok(result) => result,
            Err(payload) => {
                let first = self.source.guard.first.get().map(|first| first.error());
                source_reference_discard_v29(payload);
                Err(Failure::Source(first.unwrap_or(
                    ProductionSourceOwnedViewErrorV18::Binding("paired path entry panicked"),
                )))
            }
        };
        let mut result = match result {
            Err(Failure::Source(error)) => Err(Failure::Source(
                self.retain_query(Err::<(), _>(error)).unwrap_err(),
            )),
            result => result,
        };
        let dropped = source_reference_discard_v29(pending);
        let same = slot == std::ptr::from_ref(&*budget) as usize
            && ledger == budget.work_ledger_identity_v1()
            && self.source.guard.slot == slot
            && self.source.guard.ledger == ledger;
        if same {
            if let Err(error) = self.paired_report_prior_denial_v19(budget) {
                if result.is_ok() {
                    result = Err(Failure::Source(error));
                }
            }
        }
        if !same
            || self.source.cleanup.is_denied()
            || floor
                .checked_add(credit)
                .is_none_or(|required| budget.storage() < required)
        {
            self.source.cleanup.deny_refund();
            if result.is_ok() {
                result = Err(Failure::Source(ArgumentResourceV1::Accounting.into()));
            }
        } else {
            if result.is_ok() {
                if let Err(error) = self.query(budget) {
                    result = Err(Failure::Source(error));
                } else if dropped {
                    result = Err(Failure::Source(ProductionSourceOwnedViewErrorV18::Binding(
                        "paired path capture panicked",
                    )));
                }
            }
            if let Err(error) = budget.release_storage(credit) {
                self.source.cleanup.deny_refund();
                if result.is_ok() {
                    result = Err(Failure::Source(error.into()));
                }
            }
        }
        match result {
            Err(Failure::Source(error)) => Err(Failure::Source(
                self.retain_query(Err::<(), _>(error)).unwrap_err(),
            )),
            result => result,
        }
    }

    fn paired_formal_paths_inner_v20<'work, E, F>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        launches: &[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        limits: fe2o3_kernel_ir::ControlFlowLimits,
        query_limits: fe2o3_kernel_analysis::PresburgerQueryLimitsV2,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: &mut F,
    ) -> Result<(), ProductionOptimizedSourcePathsErrorV20<E>>
    where
        F: for<'a, 'b, 'c, 'd, 'path> FnMut(
            ProductionFormalPathSideV20,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'a, 'b>,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'c, 'd>,
            &fe2o3_kernel_analysis::FormalPaidPathViewV20<'path>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
    {
        use ProductionOptimizedSourcePathsErrorV20 as Failure;
        use ProductionOptimizedSourceReportsErrorV19 as Reports;
        use fe2o3_kernel_analysis::{PresburgerQueryErrorV2 as Query, with_presburger_queries_v4};
        let session_floor = budget.storage();
        let session_slot = std::ptr::from_ref(&*budget) as usize;
        let session_ledger = budget.work_ledger_identity_v1();
        let mut selected = None;
        let session = with_presburger_queries_v4(query_limits, budget, |queries, budget| {
            let result = self.with_optimized_formal_reports_v19(
                optimized,
                launches,
                width,
                limits,
                budget,
                |before, after, budget| {
                    self.paired_formal_path_side_v20(
                        ProductionFormalPathSideV20::Original,
                        before,
                        before,
                        after,
                        queries,
                        budget,
                        consume,
                    )?;
                    self.paired_formal_path_side_v20(
                        ProductionFormalPathSideV20::Optimized,
                        after,
                        before,
                        after,
                        queries,
                        budget,
                        consume,
                    )
                },
            );
            match result {
                Ok(()) => Ok(()),
                Err(error) => {
                    selected = Some(match error {
                        Reports::Source(error) => Failure::Source(error),
                        Reports::Formal {
                            error,
                            source_refusal,
                        } => Failure::Formal {
                            error,
                            source_refusal,
                        },
                        Reports::Consumer(error) => error,
                    });
                    Err(Query::CallbackRejected)
                }
            }
        });
        if session_slot != std::ptr::from_ref(&*budget) as usize
            || session_ledger != budget.work_ledger_identity_v1()
            || budget.storage() != session_floor
        {
            // No report/query owner may escape this unit callback. In
            // particular, an earlier Work refusal must not hide solver credit
            // intentionally retained after an observed custody violation.
            self.source.cleanup.deny_refund();
            let refusal = self
                .retain_query(Err::<(), _>(ArgumentResourceV1::Accounting.into()))
                .unwrap_err();
            if selected.is_none() && session.is_ok() {
                selected = Some(Failure::Source(refusal));
            }
        }
        if let Some(error) = selected {
            return Err(error);
        }
        session.map_err(|error| Failure::Query {
            source_refusal: optimized_source_path_query_refusal_v20(self, &error, budget),
            error,
        })
    }

    fn paired_formal_path_side_v20<'work, E, F>(
        &self,
        side: ProductionFormalPathSideV20,
        report: &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
        before: &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
        after: &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
        queries: &mut fe2o3_kernel_analysis::PresburgerQueryScopeV4<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: &mut F,
    ) -> Result<(), ProductionOptimizedSourcePathsErrorV20<E>>
    where
        F: for<'a, 'b, 'c, 'd, 'path> FnMut(
            ProductionFormalPathSideV20,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'a, 'b>,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'c, 'd>,
            &fe2o3_kernel_analysis::FormalPaidPathViewV20<'path>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
    {
        use ProductionOptimizedSourcePathsErrorV20 as Failure;
        use fe2o3_kernel_analysis::{
            FormalPaidPathErrorV20 as Path, with_formal_path_observations_v21,
        };
        let source = report
            .source_scope_v20(budget)
            .map_err(|error| Failure::Formal {
                source_refusal: optimized_source_report_refusal_v19(self, &error),
                error,
            })?;
        let mut consumer_error = None;
        let result =
            with_formal_path_observations_v21(
                source,
                queries,
                budget,
                |path, budget| match consume(side, before, after, path, budget) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        consumer_error = Some(error);
                        Err(Path::ConsumerRejected)
                    }
                },
            );
        match (result, consumer_error) {
            (Ok(()), None) => Ok(()),
            (Err(Path::ConsumerRejected), Some(error)) | (Ok(()), Some(error)) => {
                Err(Failure::Consumer(error))
            }
            (Err(error), rejected) => {
                let source_refusal = optimized_source_path_refusal_v20(self, &error, budget);
                source_reference_discard_v29(rejected);
                Err(Failure::Path {
                    side,
                    error,
                    source_refusal,
                })
            }
        }
    }
}
