/// The complete original report refusal and sticky source refusal are distinct.
/// This error is diagnostic; it never authorizes a publication or memory effect.
#[derive(Debug)]
pub enum ProductionOptimizedSourceReportsErrorV19<E> {
    /// Original source/output custody or correspondence was refused.
    Source(ProductionSourceOwnedViewErrorV18),
    /// Exact formal extraction failure observed on the original live ledger.
    Formal {
        /// Original KIR source, CFG, effects, invocation or resource error.
        error: fe2o3_kernel_ir::CanonicalFormalReportErrorV19,
        /// Separately retained first source-side refusal.
        source_refusal: ProductionSourceOwnedViewErrorV18,
    },
    /// The paired-report consumer rejected its inputs without a new authority.
    Consumer(E),
}
impl<E> From<ProductionSourceOwnedViewErrorV18> for ProductionOptimizedSourceReportsErrorV19<E> {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for ProductionOptimizedSourceReportsErrorV19<E> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::Formal {
                error,
                source_refusal,
            } => write!(out, "{error}; retained source refusal: {source_refusal}"),
            Self::Consumer(error) => error.fmt(out),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ProductionOptimizedSourceReportsErrorV19<E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Formal { error, .. } => Some(error),
            Self::Consumer(error) => Some(error),
        }
    }
}

fn optimized_source_report_refusal_v19(
    original: &ProductionSourceCorrespondenceV18<'_>,
    error: &fe2o3_kernel_ir::CanonicalFormalReportErrorV19,
) -> ProductionSourceOwnedViewErrorV18 {
    use fe2o3_kernel_ir::{
        CanonicalEffectErrorV19 as Effect, CanonicalFormalReportErrorV19 as Formal,
    };
    let refusal = match error {
        Formal::Resource(error) => {
            if matches!(
                error,
                fe2o3_kernel_ir::FormalGuardedMemoryResourceErrorV1::Accounting
            ) {
                original.source.cleanup.deny_refund();
            }
            optimized_source_formal_resource_v18(*error).into()
        }
        Formal::Effects(Effect::Resource(error)) => {
            if matches!(error, ArgumentResourceV1::Accounting) {
                original.source.cleanup.deny_refund();
            }
            (*error).into()
        }
        Formal::ControlFlow(_) => {
            ProductionSourceOwnedViewErrorV18::Binding("actual formal report source CFG refused")
        }
        Formal::Invocation(_) => ProductionSourceOwnedViewErrorV18::Binding(
            "actual formal report invocation range refused",
        ),
        Formal::Effects(_) => ProductionSourceOwnedViewErrorV18::Binding(
            "actual formal report effects custody refused",
        ),
        Formal::ConsumerRejected => {
            ProductionSourceOwnedViewErrorV18::Binding("actual formal report consumer refused")
        }
        Formal::Panicked => {
            ProductionSourceOwnedViewErrorV18::Binding("actual formal report callback panicked")
        }
    };
    original.retain_query(Err::<(), _>(refusal)).unwrap_err()
}

// Kernel ordinals are checked against both immutable inventories; function
// ordinals are deliberately resolved through the existing output name index.
fn optimized_source_report_root_v19(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.retain_query((|| {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let input = original.inventory;
        let output = optimized.output_inventory(budget)?;
        let owner = original.source.canonical(budget)?;
        let count = original.source.root_count(budget)?;
        budget.charge_work(6)?;
        if !std::ptr::eq(owner, input.owner())
            || input.kernels().len() != count
            || output.kernels().len() != count
        {
            return original
                .source
                .missing("paired formal report original kernel roster");
        }
        let (_, ordinal) = original.source.root(root, budget)?;
        let input_root =
            input
                .kernels()
                .get(root)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "paired formal report original root",
                ))?;
        let output_root =
            output
                .kernels()
                .get(root)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "paired formal report output root",
                ))?;
        let function = optimized_source_root_function_v18(original, optimized, root, budget)?;
        let bytes = input_root
            .kernel
            .id
            .as_str()
            .len()
            .checked_add(output_root.kernel.id.as_str().len())
            .and_then(|n| n.checked_add(input_root.kernel.entry.as_str().len()))
            .and_then(|n| n.checked_add(output_root.kernel.entry.as_str().len()))
            .and_then(|n| n.checked_add(12))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.charge_work(bytes)?;
        if input_root.ordinal as usize != root
            || output_root.ordinal as usize != root
            || input_root.entry.0 as usize != ordinal
            || output_root.entry != function.coordinate
            || input_root.kernel.id != output_root.kernel.id
            || input_root.kernel.entry != output_root.kernel.entry
            || input_root.kernel.domain != output_root.kernel.domain
            || !owner
                .module()
                .kernels
                .get(root)
                .is_some_and(|value| std::ptr::eq(value, input_root.kernel))
            || !output
                .owner()
                .module()
                .kernels
                .get(root)
                .is_some_and(|value| std::ptr::eq(value, output_root.kernel))
        {
            return original
                .source
                .missing("paired formal report actual source/output root association");
        }
        Ok(())
    })())
}

impl ProductionSourceCorrespondenceV18<'_> {
    fn paired_report_prior_denial_v19(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        if self.source.guard.slot != std::ptr::from_ref(budget) as usize
            || self.source.guard.ledger != budget.work_ledger_identity_v1()
        {
            self.source.cleanup.deny_refund();
            return self.retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.retain_query(budget.check_prior_denials_v1().map_err(Into::into))
    }

    /// Borrows both fresh full reports while the actual source, optimized owner,
    /// full correspondence, original ledger and report backing remain live.
    /// The complete launch roster is used on both immutable root rosters. Both
    /// module effect scopes are built once, outside the complete root traversal.
    /// Every incomplete reason/conflict remains in its original report: this is
    /// not source/output memory equivalence, target authority or final admission.
    pub fn with_optimized_formal_reports_v19<'work, E, F>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        launches: &[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        limits: fe2o3_kernel_ir::ControlFlowLimits,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: F,
    ) -> Result<(), ProductionOptimizedSourceReportsErrorV19<E>>
    where
        F: for<'input_report, 'input_owner, 'output_report, 'output_owner> FnMut(
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'input_report, 'input_owner>,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'output_report, 'output_owner>,
            &mut ArgumentBudgetV1<'work>,
        )
            -> Result<(), E>,
    {
        use ProductionOptimizedSourceReportsErrorV19 as Failure;
        let floor = budget.storage();
        let slot = std::ptr::from_ref(&*budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let mut pending = Some(consume);
        let mut entry = |budget: &mut ArgumentBudgetV1<'work>| {
            self.paired_formal_reports_inner_v19(
                optimized,
                launches,
                width,
                limits,
                budget,
                &mut pending,
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
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "paired formal report entry panicked",
                    ),
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
                        "paired formal report capture panicked",
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

    fn paired_formal_reports_inner_v19<'work, E, F>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        launches: &[fe2o3_kernel_ir::CanonicalFormalLaunchInputV19],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        limits: fe2o3_kernel_ir::ControlFlowLimits,
        budget: &mut ArgumentBudgetV1<'work>,
        pending: &mut Option<F>,
    ) -> Result<(), ProductionOptimizedSourceReportsErrorV19<E>>
    where
        F: for<'input_report, 'input_owner, 'output_report, 'output_owner> FnMut(
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'input_report, 'input_owner>,
            &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'output_report, 'output_owner>,
            &mut ArgumentBudgetV1<'work>,
        )
            -> Result<(), E>,
    {
        use ProductionOptimizedSourceReportsErrorV19 as Failure;
        use fe2o3_kernel_ir::{
            CanonicalEffectErrorV19 as Effect, CanonicalFormalReportErrorV19 as Formal,
            with_canonical_effects_v19, with_canonical_owner_formal_report_v19,
        };
        source_output_correspondence_checks_v18(self, optimized, budget)?;
        self.retain_query((|| {
            budget.charge_work(1)?;
            if launches.is_empty() || launches.len() != self.source.root_count(budget)? {
                return self
                    .source
                    .missing("paired formal report complete launch roster");
            }
            Ok(())
        })())?;
        for root in 0..launches.len() {
            optimized_source_report_root_v19(self, optimized, root, budget)?;
        }
        let input = self.source.canonical(budget)?;
        let output = optimized.output_inventory(budget)?;
        let floor = budget.storage();
        scoped_source_attempt_v29(self.source.cleanup, budget, floor, |budget| {
            let mut consumer_error = None;
            let mut reports_result = None;
            let mut construction = |budget: &mut ArgumentBudgetV1<'work>| {
                with_canonical_effects_v19(input, budget, |input_effects, budget| {
                    with_canonical_effects_v19(output.owner(), budget, |output_effects, budget| {
                        for (root, &launch) in launches.iter().enumerate() {
                            reports_result = Some(with_canonical_owner_formal_report_v19(
                                input,
                                root,
                                input_effects,
                                launch,
                                width,
                                limits,
                                budget,
                                |input_report, budget| {
                                    with_canonical_owner_formal_report_v19(
                                        output.owner(),
                                        root,
                                        output_effects,
                                        launch,
                                        width,
                                        limits,
                                        budget,
                                        |output_report, budget| {
                                            let callback =
                                                pending.as_mut().ok_or(Formal::ConsumerRejected)?;
                                            match callback(input_report, output_report, budget) {
                                                Ok(()) => Ok(()),
                                                Err(error) => {
                                                    consumer_error = Some(error);
                                                    Err(Formal::ConsumerRejected)
                                                }
                                            }
                                        },
                                    )
                                },
                            ));
                            if reports_result
                                .as_ref()
                                .is_some_and(|result| result.is_err())
                            {
                                break;
                            }
                        }
                        Ok(())
                    })
                })
            };
            let headers = [
                std::mem::size_of_val(&construction),
                size_of::<Option<E>>(),
                size_of::<Option<Result<(), Formal>>>(),
                size_of::<Result<(), Effect>>(),
                size_of::<Result<(), Failure<E>>>(),
                source_reference_cleanup_headers_v29()?,
            ]
            .into_iter()
            .try_fold(0_usize, |sum, bytes| sum.checked_add(bytes))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
            self.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
            self.retain_query(
                budget
                    .charge_work(2 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)
                    .map_err(Into::into),
            )?;
            let effects_result = construction(budget);
            drop(construction);
            let formal = match effects_result {
                Err(error) => Err(Formal::Effects(error)),
                Ok(()) => reports_result.unwrap_or(Err(Formal::ConsumerRejected)),
            };
            if formal == Err(Formal::ConsumerRejected) {
                if let Some(error) = consumer_error.take() {
                    if let Some(first) = self.source.guard.first.get() {
                        source_reference_discard_v29(error);
                        return Err(SourceConsumerErrorV18(Failure::Source(first.error())));
                    }
                    return Err(SourceConsumerErrorV18(Failure::Consumer(error)));
                }
            }
            source_reference_discard_v29(consumer_error);
            if let Err(error) = formal {
                let source_refusal = optimized_source_report_refusal_v19(self, &error);
                return Err(SourceConsumerErrorV18(Failure::Formal {
                    error,
                    source_refusal,
                }));
            }
            for root in 0..launches.len() {
                optimized_source_report_root_v19(self, optimized, root, budget)?;
            }
            self.check_optimized_source_currentness_v18(optimized, budget)?;
            self.retain_query(budget.release_storage(headers).map_err(Into::into))?;
            Ok(())
        })
        .map_err(|error: SourceConsumerErrorV18<Failure<E>>| error.0)
    }
}
