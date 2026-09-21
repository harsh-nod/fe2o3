impl<P: PlironStructuralIdentityProviderV1> PlironPassContractSessionV1<P> {
    // Only the contiguous path accepts observation. Its epoch-triggered
    // recapture cannot succeed, so no successful revalidation charge is lost.
    fn begin_contiguous_pass_with_observation_v1(
        &mut self,
        pass: KernelCheckPassKindV1,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: Option<&InvocationObserverV1<'_, '_>>,
    ) -> Result<(), PlironPassPreservationErrorV1> {
        match observer {
            None => self.begin_pass_with_resource_limits_v1(pass, false, limits),
            Some(observer) => observer.with_projection(&Ok, |nested| {
                self.begin_pass_observed_inner_v1(pass, false, limits, Some(nested))
            }),
        }
    }

    fn begin_pass_observed_inner_v1(
        &mut self,
        pass: KernelCheckPassKindV1,
        revalidate_input: bool,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: Option<&InvocationObserverV1<'_, '_>>,
    ) -> Result<(), PlironPassPreservationErrorV1> {
        self.require_pass_can_begin(pass)?;
        let mutation_epoch = self.observe_mutation_epoch(Some(pass))?;
        let expected = self.take_lineage()?;
        let before = if revalidate_input || mutation_epoch != self.lineage_mutation_epoch {
            let before = match observer {
                None => self.capture(limits)?,
                Some(observer) => {
                    let phase = ProductionAnalysisResourcePhaseV1::PassPreservation;
                    let old_storage = self
                        .lineage_resource_upper_bound
                        .retained_storage_upper_bound();
                    let resource_error =
                        |error| preservation_resource_error_v1(observer.deny(error));
                    let old = ProductionAnalysisResourceUpperBoundV1::checked_phase(
                        phase,
                        0,
                        old_storage,
                        0,
                    )
                    .map_err(resource_error)?;
                    // The caller's end-style projection removes the old owner.
                    // Begin recapture must restore it until both snapshots die.
                    let overlap = |capture| old.checked_then_retain(capture, phase);
                    observer.with_projection(&overlap, |nested| {
                        let capture = self
                            .provider
                            .capture_with_resource_observation_v1(limits, Some(nested))
                            .map_err(|error| {
                                if let IdentityCaptureFailureV1::ResourceLimit(error) = &error {
                                    observer.deny(*error);
                                }
                                identity_error(error)
                            })?;
                        let bound = capture.resource_upper_bound;
                        let comparison = old_storage
                            .checked_add(bound.retained_storage_upper_bound())
                            .and_then(|work| work.checked_add(1))
                            .and_then(|work| work.checked_add(bound.work_upper_bound()))
                            .ok_or_else(|| {
                                resource_error(ProductionAnalysisResourceLimitV1 {
                                    phase,
                                    resource: "begin identity comparison work upper bound",
                                })
                            })?;
                        let compared = ProductionAnalysisResourceUpperBoundV1::checked_phase(
                            phase,
                            comparison,
                            bound.retained_storage_upper_bound(),
                            bound.peak_storage_upper_bound() - bound.retained_storage_upper_bound(),
                        )
                        .map_err(resource_error)?;
                        // Input limits already exclude the old retained owner;
                        // only the projection, not this local check, restores it.
                        nested
                            .require(limits, phase, Ok(compared))
                            .map_err(resource_error)?;
                        Ok(capture)
                    })?
                }
            };
            let after_capture = self.observe_mutation_epoch(Some(pass))?;
            if let Err(mismatch) = self
                .provider
                .require_exact_identity(&expected, &before.snapshot)
            {
                return Err(PlironPassPreservationErrorV1::StaleInputIdentity {
                    pass,
                    source_code: mismatch.code,
                    detail: mismatch.detail,
                });
            }
            if after_capture != mutation_epoch {
                return Err(PlironPassPreservationErrorV1::MutationAttempted {
                    pass: Some(pass),
                    before: mutation_epoch,
                    after: after_capture,
                });
            }
            if mutation_epoch != self.lineage_mutation_epoch {
                return Err(PlironPassPreservationErrorV1::StaleMutationEpoch {
                    pass,
                    expected: self.lineage_mutation_epoch,
                    observed: mutation_epoch,
                });
            }
            self.lineage_resource_upper_bound = before.resource_upper_bound;
            before.snapshot
        } else {
            expected
        };
        self.pending = Some(PendingPassV1 {
            pass,
            before,
            before_resource_upper_bound: self.lineage_resource_upper_bound,
            mutation_epoch,
        });
        Ok(())
    }

    pub(crate) fn run_contiguous_pass_with_observation_v1<T, E>(
        &mut self,
        pass: KernelCheckPassKindV1,
        limits: impl Into<ProductionAnalysisReplacementLimitsV1>,
        execute: impl FnOnce() -> Result<T, E>,
        observer: Option<&InvocationObserverV1<'_, '_>>,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1> {
        let run = || {
            let limits = limits.into();
            self.begin_contiguous_pass_with_observation_v1(pass, limits.input, observer)?;
            let result = catch_unwind(AssertUnwindSafe(|| match observer {
                None => execute(),
                Some(observer) => observer.with_projection(&Ok, |_| execute()),
            }));
            match result {
                Err(_) => Err(PlironPassPreservationErrorV1::AnalysisPanicked { pass }),
                Ok(result) => {
                    self.end_pass_with_observation_v1(pass, limits.output, observer)?;
                    Ok(result)
                }
            }
        };
        match observer {
            None => run(),
            Some(observer) => observer.with_projection(&Ok, |_| run()),
        }
    }

    pub(crate) fn finish_with_observation_v1(
        self,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: Option<&InvocationObserverV1<'_, '_>>,
    ) -> Result<PlironPassPreservationReportV1, PlironPassPreservationErrorV1> {
        match observer {
            None => self.finish_observed_inner_v1(limits, None),
            Some(observer) => observer.with_projection(&Ok, |nested| {
                self.finish_observed_inner_v1(limits, Some(nested))
            }),
        }
    }

    fn finish_observed_inner_v1(
        mut self,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: Option<&InvocationObserverV1<'_, '_>>,
    ) -> Result<PlironPassPreservationReportV1, PlironPassPreservationErrorV1> {
        if self.pending.is_some() {
            return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "the final pass has not completed",
            });
        }
        if self.next != MAX_PLIRON_PASS_CONTRACTS_V1 {
            let contract = PRODUCTION_PLIRON_PASS_CONTRACTS_V1.get(self.next).ok_or(
                PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "completed pass count exceeds the fixed sequence",
                },
            )?;
            return Err(PlironPassPreservationErrorV1::OmittedPassDeclaration {
                position: self.next,
                pass: contract.pass(),
            });
        }
        let preadmitted = observer
            .map(|observer| {
                observer.require(
                    limits,
                    ProductionAnalysisResourcePhaseV1::PassPreservation,
                    self.finish_resource_upper_bound_v1(),
                )
            })
            .transpose()
            .map_err(preservation_resource_error_v1)?;
        let output_mutation_epoch = self.observe_mutation_epoch(None)?;
        if output_mutation_epoch != self.lineage_mutation_epoch {
            return Err(PlironPassPreservationErrorV1::MutationAttempted {
                pass: None,
                before: self.lineage_mutation_epoch,
                after: output_mutation_epoch,
            });
        }
        let output =
            self.lineage
                .take()
                .ok_or(PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "the final identity snapshot is absent",
                })?;
        let resource_upper_bound = match preadmitted {
            Some(bound) => bound,
            None => self
                .finish_resource_upper_bound_v1()
                .and_then(|bound| {
                    limits.require(ProductionAnalysisResourcePhaseV1::PassPreservation, bound)
                })
                .map_err(preservation_resource_error_v1)?,
        };
        let exact_output_identity = self.provider.retain_exact_identity(output);
        if exact_output_identity.len() != self.lineage_identity.canonical_len() {
            return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "the retained identity length differs from its compact label",
            });
        }
        Ok(PlironPassPreservationReportV1 {
            input_identity: self.input_identity,
            output_identity: self.lineage_identity,
            exact_output_identity,
            custody: self.custody,
            certificates: self.certificates,
            input_mutation_epoch: self.input_mutation_epoch,
            output_mutation_epoch,
            resource_upper_bound,
        })
    }

    fn finish_resource_upper_bound_v1(
        &self,
    ) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
        let canonical_bytes = self.lineage_identity.canonical_len();
        let certificate_count = self.certificates.len();
        ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::PassPreservation,
            canonical_bytes
                .checked_add(certificate_count)
                .and_then(|work| work.checked_add(4))
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                    resource: "preservation report work upper bound",
                })?,
            canonical_bytes
                .checked_add(certificate_count)
                .and_then(|storage| storage.checked_add(2))
                .ok_or(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                    resource: "preservation report retained storage upper bound",
                })?,
            self.lineage_resource_upper_bound
                .retained_storage_upper_bound(),
        )
    }

    pub(crate) fn end_pass_with_observation_v1(
        &mut self,
        pass: KernelCheckPassKindV1,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: Option<&InvocationObserverV1<'_, '_>>,
    ) -> Result<(), PlironPassPreservationErrorV1> {
        match observer {
            None => self.end_pass_observed_inner_v1(pass, limits, None),
            Some(observer) => observer.with_projection(&Ok, |nested| {
                self.end_pass_observed_inner_v1(pass, limits, Some(nested))
            }),
        }
    }

    fn end_pass_observed_inner_v1(
        &mut self,
        pass: KernelCheckPassKindV1,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: Option<&InvocationObserverV1<'_, '_>>,
    ) -> Result<(), PlironPassPreservationErrorV1> {
        let resource_error = |error| {
            if let Some(observer) = observer {
                observer.deny(error);
            }
            preservation_resource_error_v1(error)
        };
        let pending =
            self.pending
                .take()
                .ok_or(PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "no pass is active",
                })?;
        if pending.pass != pass {
            return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "the completed pass differs from the active pass",
            });
        }
        let before_storage = pending
            .before_resource_upper_bound
            .retained_storage_upper_bound();
        let reserved_work = before_storage.checked_add(1).ok_or_else(|| {
            resource_error(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                resource: "identity comparison work reservation",
            })
        })?;
        let capture_limits = ProductionAnalysisResourceLimitsV1::new(
            limits
                .max_work()
                .checked_sub(reserved_work)
                .ok_or_else(|| {
                    resource_error(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                        resource: "remaining identity capture work upper bound",
                    })
                })?,
            limits
                .max_peak_storage()
                .checked_sub(reserved_work)
                .ok_or_else(|| {
                    resource_error(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                        resource: "remaining identity capture storage upper bound",
                    })
                })?,
        );
        // Replacement excludes the old owner from the frozen anchor, so each
        // observed capture prefix must include its still-live overlap here.
        let capture_prefix = |capture: ProductionAnalysisResourceUpperBoundV1| {
            ProductionAnalysisResourceUpperBoundV1::checked_phase(
                ProductionAnalysisResourcePhaseV1::PassPreservation,
                capture
                    .work_upper_bound()
                    .checked_add(reserved_work)
                    .ok_or(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                        resource: "preservation capture prefix work upper bound",
                    })?,
                capture
                    .retained_storage_upper_bound()
                    .checked_add(1)
                    .ok_or(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                        resource: "preservation capture prefix retained storage upper bound",
                    })?,
                capture
                    .peak_storage_upper_bound()
                    .checked_sub(capture.retained_storage_upper_bound())
                    .and_then(|temporary| temporary.checked_add(before_storage))
                    .ok_or(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                        resource: "preservation capture prefix temporary storage upper bound",
                    })?,
            )
        };
        let after = match observer {
            None => self
                .provider
                .capture_with_resource_limits_v1(capture_limits),
            Some(observer) => {
                observer
                    .require(
                        limits,
                        ProductionAnalysisResourcePhaseV1::PassPreservation,
                        capture_prefix(ProductionAnalysisResourceUpperBoundV1::default()),
                    )
                    .map_err(resource_error)?;
                observer.with_projection(&capture_prefix, |nested| {
                    self.provider
                        .capture_with_resource_observation_v1(capture_limits, Some(nested))
                })
            }
        }
        .map_err(|error| match error {
            IdentityCaptureFailureV1::Unavailable {
                source_code,
                detail,
            } => PlironPassPreservationErrorV1::StructuralIdentityChanged {
                pass,
                source_code,
                detail: format!("post-pass structural identity is unavailable: {detail}"),
            },
            IdentityCaptureFailureV1::ResourceLimit(error) => resource_error(error),
        })?;
        // Capture work and retained size need not be equal. Admit the actual
        // comparison cost before observing the epoch or comparing snapshots.
        let comparison_work = pending
            .before_resource_upper_bound
            .retained_storage_upper_bound()
            .checked_add(after.resource_upper_bound.retained_storage_upper_bound())
            .and_then(|work| work.checked_add(1))
            .ok_or_else(|| {
                resource_error(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                    resource: "identity comparison work upper bound",
                })
            })?;
        let retained_storage = after
            .resource_upper_bound
            .retained_storage_upper_bound()
            .checked_add(1)
            .ok_or_else(|| {
                resource_error(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                    resource: "preservation certificate storage upper bound",
                })
            })?;
        let capture_temporary_storage = after
            .resource_upper_bound
            .peak_storage_upper_bound()
            .checked_sub(after.resource_upper_bound.retained_storage_upper_bound())
            .ok_or_else(|| {
                resource_error(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                    resource: "identity capture storage upper bound",
                })
            })?;
        let temporary_storage = pending
            .before_resource_upper_bound
            .retained_storage_upper_bound()
            .checked_add(capture_temporary_storage)
            .ok_or_else(|| {
                resource_error(ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                    resource: "preservation temporary storage upper bound",
                })
            })?;
        let stage_bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::PassPreservation,
            after
                .resource_upper_bound
                .work_upper_bound()
                .checked_add(comparison_work)
                .ok_or_else(|| {
                    resource_error(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::PassPreservation,
                        resource: "preservation work upper bound",
                    })
                })?,
            retained_storage,
            temporary_storage,
        )
        .map_err(resource_error)?;
        require_observed_v1(
            limits,
            ProductionAnalysisResourcePhaseV1::PassPreservation,
            Ok(stage_bound),
            observer,
        )
        .map_err(resource_error)?;
        let after_mutation_epoch = self.observe_mutation_epoch(Some(pass))?;
        if let Err(mismatch) = self
            .provider
            .require_exact_identity(&pending.before, &after.snapshot)
        {
            return Err(PlironPassPreservationErrorV1::StructuralIdentityChanged {
                pass,
                source_code: mismatch.code,
                detail: mismatch.detail,
            });
        }
        if after_mutation_epoch != pending.mutation_epoch {
            return Err(PlironPassPreservationErrorV1::MutationAttempted {
                pass: Some(pass),
                before: pending.mutation_epoch,
                after: after_mutation_epoch,
            });
        }
        let identity = self.provider.label(&after.snapshot);
        if observer.is_some() {
            drop(pending);
        }
        self.certificates.push(PlironPassPreservationCertificateV1 {
            pass,
            identity,
            mutation_epoch: after_mutation_epoch,
        });
        self.lineage = Some(after.snapshot);
        self.lineage_resource_upper_bound = after.resource_upper_bound;
        self.last_checkpoint_resource_upper_bound = Some(stage_bound);
        self.lineage_identity = identity;
        self.lineage_mutation_epoch = after_mutation_epoch;
        self.next =
            self.next
                .checked_add(1)
                .ok_or(PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "completed pass position overflowed",
                })?;
        Ok(())
    }

    fn new_with_observation_v1(
        provider: P,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: Option<&InvocationObserverV1<'_, '_>>,
    ) -> Result<Self, PlironPassPreservationErrorV1> {
        match observer {
            None => Self::new_observed_inner_v1(provider, limits, None),
            Some(observer) => observer.with_projection(&Ok, |nested| {
                Self::new_observed_inner_v1(provider, limits, Some(nested))
            }),
        }
    }

    fn new_observed_inner_v1(
        mut provider: P,
        limits: ProductionAnalysisResourceLimitsV1,
        observer: Option<&InvocationObserverV1<'_, '_>>,
    ) -> Result<Self, PlironPassPreservationErrorV1> {
        let resource_error = |error| {
            if let Some(observer) = observer {
                observer.deny(error);
            }
            preservation_resource_error_v1(error)
        };
        let session_setup = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::PassPreservation,
            1,
            MAX_PLIRON_PASS_CONTRACTS_V1 + 1,
            0,
        )
        .map_err(resource_error)?;
        if let Some(observer) = observer {
            observer
                .require(
                    limits,
                    ProductionAnalysisResourcePhaseV1::PassPreservation,
                    Ok(session_setup),
                )
                .map_err(resource_error)?;
        }
        let capture_limits = limits
            .remaining_after_retained(
                ProductionAnalysisResourcePhaseV1::PassPreservation,
                session_setup,
            )
            .map_err(resource_error)?;
        let input_mutation_epoch = provider.mutation_epoch().map_err(|error| {
            PlironPassPreservationErrorV1::MutationEpochUnavailable {
                pass: None,
                detail: error.detail,
            }
        })?;
        let input = match observer {
            None => provider.capture_with_resource_limits_v1(capture_limits),
            Some(observer) => observer.with_projection(
                &|capture| {
                    session_setup.checked_then_retain(
                        capture,
                        ProductionAnalysisResourcePhaseV1::PassPreservation,
                    )
                },
                |nested| {
                    provider.capture_with_resource_observation_v1(capture_limits, Some(nested))
                },
            ),
        }
        .map_err(|error| {
            if let (Some(observer), IdentityCaptureFailureV1::ResourceLimit(limit)) =
                (observer, &error)
            {
                observer.deny(*limit);
            }
            identity_error(error)
        })?;
        let after_capture = provider.mutation_epoch().map_err(|error| {
            PlironPassPreservationErrorV1::MutationEpochUnavailable {
                pass: None,
                detail: error.detail,
            }
        })?;
        if after_capture != input_mutation_epoch {
            return Err(PlironPassPreservationErrorV1::MutationAttempted {
                pass: None,
                before: input_mutation_epoch,
                after: after_capture,
            });
        }
        let input_identity = provider.label(&input.snapshot);
        let initial_identity_resource_upper_bound = session_setup
            .checked_then_retain(
                input.resource_upper_bound,
                ProductionAnalysisResourcePhaseV1::PassPreservation,
            )
            .map_err(resource_error)?;
        require_observed_v1(
            limits,
            ProductionAnalysisResourcePhaseV1::PassPreservation,
            Ok(initial_identity_resource_upper_bound),
            observer,
        )
        .map_err(resource_error)?;
        Ok(Self {
            provider,
            custody: Arc::new(()),
            input_identity,
            lineage: Some(input.snapshot),
            input_census: input.input_census,
            initial_identity_resource_upper_bound,
            lineage_resource_upper_bound: input.resource_upper_bound,
            last_checkpoint_resource_upper_bound: None,
            lineage_identity: input_identity,
            next: 0,
            pending: None,
            certificates: Vec::with_capacity(MAX_PLIRON_PASS_CONTRACTS_V1),
            input_mutation_epoch,
            lineage_mutation_epoch: input_mutation_epoch,
        })
    }
}

pub(crate) fn begin_production_pliron_pass_contract_session_with_observation_v1<P>(
    provider: P,
    limits: ProductionAnalysisResourceLimitsV1,
    observer: Option<&InvocationObserverV1<'_, '_>>,
) -> Result<PlironPassContractSessionV1<P>, PlironPassPreservationErrorV1>
where
    P: PlironStructuralIdentityProviderV1,
{
    PlironPassContractSessionV1::new_with_observation_v1(provider, limits, observer)
}
