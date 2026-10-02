/// Caller-selected distribution for a scoped tile diagnostic candidate.
///
/// Selection grants no execution authority or equivalence between observable
/// per-invocation fragment values under different distributions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionScopedTileObservationOrderV29 {
    /// Invocation `lane` receives logical offsets `lane * E + component`.
    Blocked,
    /// Invocation `lane` receives logical offsets `component * L + lane`.
    Striped,
}

/// A borrowed scalar candidate reconstructed from retained scoped source.
///
/// Its source, schedule and graph identities are diagnostic associations, not
/// producer authentication or discharge of collective, memory or launch proofs.
/// The underlying prepared and materialized owners never leave the continuation.
pub struct ProductionScopedTileCandidateViewV29<'a> {
    candidate: &'a ScopedTileScalarCandidateV29,
}

impl ProductionScopedTileCandidateViewV29<'_> {
    /// Borrows the structurally verified candidate, without source-proof or launch authority.
    pub fn canonical(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18 {
        &self.candidate.output
    }

    /// Returns the retained source semantic owner's digest.
    pub fn source_semantic_sha256(&self) -> &[u8; 32] {
        self.candidate.input.pending.source_semantic_sha256()
    }

    /// Returns the exact pending canonical identity from which this candidate was prepared.
    pub fn pending_identity(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV18 {
        self.candidate.input.pending.pending_identity()
    }

    /// Returns the source-bound schedule preparation identity used during replay.
    pub fn schedule_identity(&self) -> &[u8; 32] {
        &self.candidate.input.identity
    }

    /// Always returns false: diagnostic observation cannot authorize an artifact or launch.
    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

/// Bounded diagnostic failure. A callback's error and allocations remain its
/// caller's responsibility; this wrapper never refunds their logical storage.
#[derive(Debug)]
pub enum ProductionScopedTileObservationErrorV29<E> {
    /// The original work/storage account refused an operation or lost valid custody.
    Resource(ArgumentResourceV1),
    /// Candidate preparation or replay could not establish a required invariant.
    Unavailable {
        /// The bounded continuation stage at which the invariant was unavailable.
        phase: &'static str,
        /// The invariant category, without retaining source or candidate allocations.
        reason: &'static str,
    },
    /// The caller's observation failed; its error and storage remain caller-owned.
    Callback(E),
}

impl<E: std::fmt::Display> std::fmt::Display for ProductionScopedTileObservationErrorV29<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => write!(f, "scoped tile observation: {error}"),
            Self::Unavailable { phase, reason } => {
                write!(f, "scoped tile observation {phase}: {reason}")
            }
            Self::Callback(error) => write!(f, "scoped tile observation callback: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ProductionScopedTileObservationErrorV29<E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Callback(error) => Some(error),
            Self::Unavailable { .. } => None,
        }
    }
}

fn scoped_tile_observation_error_v29<E>(
    phase: &'static str,
    kind: ScopedTileFailureKindV29,
) -> ProductionScopedTileObservationErrorV29<E> {
    use ProductionScopedTileObservationErrorV29 as Error;
    let reason = match kind {
        ScopedTileFailureKindV29::Resource(error) => return Error::Resource(error),
        ScopedTileFailureKindV29::MissingDonor => "missing donor",
        ScopedTileFailureKindV29::Source => "source correspondence",
        ScopedTileFailureKindV29::SemanticSsa => "semantic SSA",
        ScopedTileFailureKindV29::SourceLaunch => "source launch",
        ScopedTileFailureKindV29::Canonical => "canonical graph",
        ScopedTileFailureKindV29::Occurrences => "source occurrences",
        ScopedTileFailureKindV29::Census => "tile occurrence census",
        ScopedTileFailureKindV29::Geometry => "tile geometry",
        ScopedTileFailureKindV29::NoTileOccurrences => "no tile occurrences",
        ScopedTileFailureKindV29::ReplayMismatch => "candidate replay",
    };
    Error::Unavailable { phase, reason }
}

fn scoped_tile_observation_frame_v29<E, F>() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<F>(),
        size_of::<
            Option<(
                ProductionPendingScopedSourceOwnerV29,
                ScopedTileScheduleInputV29,
            )>,
        >(),
        size_of::<Result<PreparedScopedTileSourceV29, ScopedTileFailureSummaryV29>>(),
        size_of::<Result<ScopedTileScalarCandidateV29, ScopedTileMaterializationFailureV29>>(),
        argument_product_v1(
            2,
            size_of::<Result<(), ProductionScopedTileObservationErrorV29<E>>>(),
        )?,
        size_of::<ProductionScopedTileCandidateViewV29<'_>>(),
        size_of::<ProductionScopedTileObservationOrderV29>(),
        size_of::<Option<(usize, usize)>>(),
        size_of::<std::thread::Result<Result<(), ProductionScopedTileObservationErrorV29<E>>>>(),
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        argument_product_v1(5, size_of::<usize>())?,
    ])
}

impl ProductionPendingScopedSourceOwnerV29 {
    /// Takes an authenticated-on-entry donor into a fixed blocked scalar
    /// candidate, replays it, and lends it to one diagnostic continuation.
    ///
    /// An entry or wrapper-frame refusal leaves the donor in place. After the
    /// take it is consumed on every outcome. Source/SSA/launch reservations and
    /// preexisting occurrence captures stay in their original upstream domain.
    /// The exact incoming account funds preparation, materialization and replay;
    /// no account or safety limit is replaced. All candidate data drops before
    /// its reservation is released. Work and prior denials are never refunded.
    ///
    /// The callback owns its errors and any retained/captured allocations. It
    /// must account for those separately, including their headers. Their debit
    /// is not part of this method's temporary wrapper frame and is not refunded.
    /// A changed ledger or stolen live floor prevents cleanup. An original panic
    /// payload is resumed unchanged. This is not an executable conversion.
    /// A successful result means the callback returned success and candidate
    /// replay/account custody held, not that every callback resource probe
    /// succeeded. Handled refusals remain in the unchanged ledger history.
    ///
    /// Candidate borrows cannot escape through callback state:
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPendingScopedSourceOwnerV29 as Pending;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(donor: &mut Option<Pending>, budget: &mut Budget<'_>) {
    ///     let mut escaped = None;
    ///     let _ = Pending::with_scalar_candidate_observation_v29(donor, budget, |view, _| {
    ///         escaped = Some(view.canonical());
    ///         Ok::<(), ()>(())
    ///     });
    ///     drop(escaped);
    /// }
    /// ```
    /// Nor can an error carry the borrowed candidate:
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPendingScopedSourceOwnerV29 as Pending;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(donor: &mut Option<Pending>, budget: &mut Budget<'_>) {
    ///     let _ = Pending::with_scalar_candidate_observation_v29(donor, budget, |view, _| {
    ///         Err::<(), _>(view.canonical())
    ///     });
    /// }
    /// ```
    pub fn with_scalar_candidate_observation_v29<'work, E, F>(
        donor: &mut Option<Self>,
        budget: &mut ArgumentBudgetV1<'work>,
        observe: F,
    ) -> Result<(), ProductionScopedTileObservationErrorV29<E>>
    where
        F: for<'view> FnOnce(
            ProductionScopedTileCandidateViewV29<'view>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
    {
        Self::with_scalar_candidate_observation_in_order_v29(
            donor,
            budget,
            ProductionScopedTileObservationOrderV29::Blocked,
            observe,
        )
    }

    /// Observes one candidate using an explicitly selected diagnostic distribution.
    ///
    /// This has the same donor, original-account, callback-storage, cleanup and
    /// panic contracts as [`Self::with_scalar_candidate_observation_v29`]. The
    /// selected order is included in paid preparation and candidate replay; it
    /// is not a caller-supplied proof or an executable scheduling decision.
    pub fn with_scalar_candidate_observation_in_order_v29<'work, E, F>(
        donor: &mut Option<Self>,
        budget: &mut ArgumentBudgetV1<'work>,
        order: ProductionScopedTileObservationOrderV29,
        observe: F,
    ) -> Result<(), ProductionScopedTileObservationErrorV29<E>>
    where
        F: for<'view> FnOnce(
            ProductionScopedTileCandidateViewV29<'view>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), E>,
    {
        use ProductionScopedTileObservationErrorV29 as Error;
        let pending = donor.as_ref().ok_or(Error::Unavailable {
            phase: "entry",
            reason: "missing donor",
        })?;
        scoped_tile_floor_v29(pending, pending.adopted_storage(), budget)
            .map_err(|kind| scoped_tile_observation_error_v29("entry", kind))?;
        let floor = budget.storage();
        let target = floor
            .checked_sub(pending.adopted_storage())
            .ok_or(Error::Resource(ArgumentResourceV1::Accounting))?;
        let frame = scoped_tile_observation_frame_v29::<E, F>().map_err(Error::Resource)?;
        budget.charge_work(frame).map_err(Error::Resource)?;
        budget.reserve_storage(frame).map_err(Error::Resource)?;
        let ledger = budget.work_ledger_identity_v1();
        let mut callback_cleanup = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut schedule_donor = Some((
                donor.take().expect("validated scoped tile donor"),
                ScopedTileScheduleInputV29 {
                    order: match order {
                        ProductionScopedTileObservationOrderV29::Blocked => {
                            ScopedTileOrderV29::Blocked
                        }
                        ProductionScopedTileObservationOrderV29::Striped => {
                            ScopedTileOrderV29::Striped
                        }
                    },
                },
            ));
            let prepared = prepare_scoped_tile_source_v29(&mut schedule_donor, budget)
                .map_err(|error| scoped_tile_observation_error_v29("preparation", error.kind))?;
            let candidate =
                materialize_scoped_tile_source_v29(prepared, budget).map_err(|error| {
                    let summary = error.summary;
                    drop(error);
                    scoped_tile_observation_error_v29("materialization", summary.kind)
                })?;
            candidate
                .replay_with_budget(budget)
                .map_err(|kind| scoped_tile_observation_error_v29("replay", kind))?;
            let owned =
                argument_sum_v1(&[candidate.adopted_storage(), frame]).map_err(Error::Resource)?;
            let ready = budget.storage();
            callback_cleanup = Some((ready, owned));
            let outcome = observe(
                ProductionScopedTileCandidateViewV29 {
                    candidate: &candidate,
                },
                budget,
            )
            .map_err(Error::Callback);
            if budget.work_ledger_identity_v1() != ledger || budget.storage() < ready {
                drop(outcome);
                return Err(Error::Resource(ArgumentResourceV1::Accounting));
            }
            if outcome.is_ok() {
                candidate
                    .replay_with_budget(budget)
                    .map_err(|kind| scoped_tile_observation_error_v29("replay", kind))?;
            }
            drop(candidate);
            outcome
        }));
        // Materialization already cleans up its consumed input on panic. Before
        // callback entry use the owned delta, never a stale stage reservation.
        let cleanup = if budget.work_ledger_identity_v1() != ledger {
            Err(ArgumentResourceV1::Accounting)
        } else if let Some((ready, owned)) = callback_cleanup {
            if budget.storage() < ready {
                Err(ArgumentResourceV1::Accounting)
            } else {
                budget.release_storage(owned)
            }
        } else {
            budget
                .storage()
                .checked_sub(target)
                .ok_or(ArgumentResourceV1::Accounting)
                .and_then(|owned| budget.release_storage(owned))
        };
        match result {
            Ok(outcome) => {
                cleanup.map_err(Error::Resource)?;
                outcome
            }
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}
