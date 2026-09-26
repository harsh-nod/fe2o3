impl ProductionTileScalarTransportOwnerV29 {
    /// Builds a pre-verification candidate, never a shipping optimizer selection.
    /// Ordinary rejection restores the donor. Existing producer panic unwinding consumes it.
    pub fn try_from_pending_with_budget_v29(
        donor: &mut Option<ProductionPendingScopedSourceOwnerV29>,
        order: ProductionTileScalarOrderV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<Self> {
        let original = donor
            .as_ref()
            .ok_or(ProductionTileScalarTransportErrorV29::MissingDonor)?;
        scoped_tile_floor_v29(original, original.adopted_storage(), budget)
            .map_err(|error| candidate_error(ScopedTileFailurePhaseV29::Preparation, error))?;
        let rollback_floor = budget
            .storage()
            .checked_sub(original.adopted_storage())
            .ok_or(ArgumentResourceV1::Accounting)?;
        with_scoped_source_cleanup_v29(budget, rollback_floor, |cleanup, budget| {
            Self::try_from_pending_with_cleanup_v29(donor, order, cleanup, budget)
        })
    }

    pub(super) fn try_from_pending_with_cleanup_v29(
        donor: &mut Option<ProductionPendingScopedSourceOwnerV29>,
        order: ProductionTileScalarOrderV29,
        cleanup: &ScopedSourceCleanupV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<Self> {
        let original = donor
            .as_ref()
            .ok_or(ProductionTileScalarTransportErrorV29::MissingDonor)?;
        scoped_tile_floor_v29(original, original.adopted_storage(), budget)
            .map_err(|error| candidate_error(ScopedTileFailurePhaseV29::Preparation, error))?;
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let incremental_header = size_of::<Self>()
            .checked_sub(size_of::<ScopedTileScalarCandidateV29>())
            .ok_or(ArgumentResourceV1::Accounting)?;
        let temporary_header = sum(&[
            size_of::<std::thread::Result<R<(Tables, usize)>>>(),
            size_of::<std::thread::Result<()>>(),
        ])?;
        let request = ScopedTileScheduleInputV29 {
            order: match order {
                ProductionTileScalarOrderV29::Blocked => ScopedTileOrderV29::Blocked,
                ProductionTileScalarOrderV29::Striped => ScopedTileOrderV29::Striped,
            },
        };
        let mut prepared_donor = donor.take().map(|pending| (pending, request));
        let prepared =
            match prepare_scoped_tile_source_with_cleanup_v29(&mut prepared_donor, cleanup, budget)
            {
                Ok(prepared) => prepared,
                Err(error) => {
                    *donor = prepared_donor.map(|(pending, _)| pending);
                    return Err(candidate_error(error.phase, error.kind));
                }
            };
        let candidate =
            match materialize_scoped_tile_source_with_cleanup_v29(prepared, cleanup, budget) {
                Ok(candidate) => candidate,
                Err(failure) => {
                    let error = candidate_error(failure.summary.phase, failure.summary.kind);
                    *donor = Some(recover_prepared(failure.into_input()));
                    let _ = refund_to_if_permitted(cleanup, floor, ledger, budget);
                    return Err(error);
                }
            };
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.reserve_storage(sum(&[incremental_header, temporary_header])?)?;
            candidate
                .replay_with_cleanup_v29(cleanup, budget)
                .map_err(|error| candidate_error(ScopedTileFailurePhaseV29::Replay, error))?;
            let (inventory, receipt) = Inventory::derive_v18(&candidate.output, budget)?;
            budget.reserve_storage(receipt.retained_storage())?;
            let tables = build_tables(&candidate, &inventory, budget)?;
            drop(inventory);
            budget.release_storage(receipt.retained_storage())?;
            let retained = sum(&[
                candidate.adopted_storage(),
                incremental_header,
                tables.storage()?,
            ])?;
            let extra = retained
                .checked_sub(donor_retained(&candidate))
                .ok_or(ArgumentResourceV1::Accounting)?;
            if budget.work_ledger_identity_v1() != ledger
                || budget.storage() != sum(&[floor, extra, temporary_header])?
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.release_storage(temporary_header)?;
            Ok((tables, retained))
        }));
        let (tables, retained) = match result {
            Ok(Ok(value)) => value,
            other => {
                let error = match other {
                    Ok(Err(error)) => error,
                    Err(payload) => {
                        discard(payload);
                        ProductionTileScalarTransportErrorV29::Panicked
                    }
                    Ok(Ok(_)) => unreachable!(),
                };
                *donor = Some(recover_candidate(candidate));
                let _ = refund_to_if_permitted(cleanup, floor, ledger, budget);
                return Err(error);
            }
        };
        Ok(Self {
            candidate,
            tables,
            retained,
        })
    }
    /// Returns the retained adopted storage.
    pub const fn adopted_storage(&self) -> usize {
        self.retained
    }

    /// Replays the exact donor/candidate and every typed row before borrowing it.
    /// Retained ancestors are immutable provenance, not separately editable programs.
    pub fn with_checked_transport_v29<'w, T>(
        &self,
        budget: &mut ArgumentBudgetV1<'w>,
        consume: impl for<'scope> FnOnce(
            &ProductionCheckedTileScalarTransportV29<'scope>,
            &mut ArgumentBudgetV1<'w>,
        ) -> R<T>,
    ) -> R<T> {
        scoped_tile_floor_v29(&self.candidate.input.pending, self.retained, budget)
            .map_err(|error| candidate_error(ScopedTileFailurePhaseV29::Replay, error))?;
        let floor = budget.storage();
        with_scoped_source_cleanup_v29(budget, floor, |cleanup, budget| {
            self.with_checked_transport_with_cleanup_v29(cleanup, budget, consume)
        })
    }

    pub(super) fn with_checked_transport_with_cleanup_v29<'w, T>(
        &self,
        cleanup: &ScopedSourceCleanupV29,
        budget: &mut ArgumentBudgetV1<'w>,
        consume: impl for<'scope> FnOnce(
            &ProductionCheckedTileScalarTransportV29<'scope>,
            &mut ArgumentBudgetV1<'w>,
        ) -> R<T>,
    ) -> R<T> {
        scoped_tile_floor_v29(&self.candidate.input.pending, self.retained, budget)
            .map_err(|error| candidate_error(ScopedTileFailurePhaseV29::Replay, error))?;
        let expected = sum(&[
            self.candidate.adopted_storage(),
            size_of::<Self>()
                .checked_sub(size_of::<ScopedTileScalarCandidateV29>())
                .ok_or(ArgumentResourceV1::Accounting)?,
            self.tables.storage()?,
        ])?;
        if expected != self.retained {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        scope_with_cleanup(cleanup, budget, |budget| {
            self.candidate
                .replay_with_cleanup_v29(cleanup, budget)
                .map_err(|error| candidate_error(ScopedTileFailurePhaseV29::Replay, error))?;
            let (inventory, receipt) = Inventory::derive_v18(&self.candidate.output, budget)?;
            budget.reserve_storage(receipt.retained_storage())?;
            budget.reserve_storage(size_of::<Tables>())?;
            let reconstructed = build_tables(&self.candidate, &inventory, budget)?;
            // Every row has bounded scalar/coordinate fields; no type tree/string traversal is hidden.
            budget.charge_work(sum(&[self.tables.rows()?, reconstructed.rows()?, 1])?)?;
            let agrees = reconstructed == self.tables;
            let transient = sum(&[size_of::<Tables>(), reconstructed.storage()?])?;
            drop(reconstructed);
            budget.release_storage(transient)?;
            if !agrees {
                return Err(invalid("transport replay"));
            }
            let guard = QueryGuard::new(budget);
            let checked = ProductionCheckedTileScalarTransportV29 {
                owner: self,
                inventory: &inventory,
                guard: &guard,
            };
            let caught = catch_unwind(AssertUnwindSafe(|| consume(&checked, budget)));
            let source_denied = cleanup.is_denied();
            let postflight = if source_denied {
                Err(ArgumentResourceV1::Accounting.into())
            } else {
                guard.query(budget).and_then(|()| {
                    if budget.storage() != guard.floor {
                        Err(ArgumentResourceV1::Accounting.into())
                    } else {
                        Ok(())
                    }
                })
            };
            let result = match caught {
                Ok(result) => result,
                Err(payload) => {
                    discard(payload);
                    Err(ProductionTileScalarTransportErrorV29::Panicked)
                }
            };
            let result = match postflight {
                Ok(()) => result,
                Err(_) if source_denied && result.is_err() => result,
                Err(error) => {
                    discard(result);
                    Err(error)
                }
            };
            drop(inventory);
            // The enclosing scope owns the receipt and refunds only its original ledger.
            result
        })
    }
}
fn donor_retained(candidate: &ScopedTileScalarCandidateV29) -> usize {
    candidate.input.pending.adopted_storage()
}
fn recover_prepared(
    prepared: PreparedScopedTileSourceV29,
) -> ProductionPendingScopedSourceOwnerV29 {
    let PreparedScopedTileSourceV29 {
        pending,
        selections,
        ..
    } = prepared;
    drop(selections);
    pending
}
fn recover_candidate(
    candidate: ScopedTileScalarCandidateV29,
) -> ProductionPendingScopedSourceOwnerV29 {
    let pending = recover_prepared(candidate.input);
    drop(candidate.output);
    drop(candidate.relations);
    drop(candidate.projections);
    pending
}
fn refund_to_if_permitted(
    cleanup: &ScopedSourceCleanupV29,
    floor: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    if cleanup.is_denied() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let result = (|| {
        if budget.work_ledger_identity_v1() != ledger {
            return Err(ArgumentResourceV1::Accounting);
        }
        let extra = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.release_storage(extra)
    })();
    result.map_err(|error| {
        cleanup.deny_refund();
        error.into()
    })
}

/// Sealed borrowed transport. No public constructor, Clone, serde or proof mutator.
pub struct ProductionCheckedTileScalarTransportV29<'scope> {
    owner: &'scope ProductionTileScalarTransportOwnerV29,
    inventory: &'scope Inventory<'scope>,
    guard: &'scope QueryGuard,
}
impl ProductionCheckedTileScalarTransportV29<'_> {
    /// Pays one work unit and returns the exact piece alias count view.
    pub fn piece_alias_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.piece_aliases.len())
    }
    /// Pays one work unit and returns the exact piece alias view.
    pub fn piece_alias(&self, ordinal: usize, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        match self.owner.tables.piece_aliases.get(ordinal) {
            Some(piece) => Ok(*piece),
            None => self.guard.reject("piece alias ordinal"),
        }
    }
    /// Pays one work unit and returns the exact source semantic view.
    pub fn source_semantic(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&AdmittedInertSemanticMirV1> {
        self.guard.query(budget)?;
        Ok(self
            .owner
            .candidate
            .input
            .pending
            .inner
            .source
            .owner
            .source_semantic())
    }
    /// Pays one work unit and returns the exact source ssa view.
    pub fn source_ssa(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionSemanticSsaOwnerV1> {
        self.guard.query(budget)?;
        Ok(&self.owner.candidate.input.pending.inner.source.owner)
    }
    /// Pays one work unit and returns the exact pending ancestor view.
    pub fn pending_ancestor(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionPendingScopedSourceOwnerV29> {
        self.guard.query(budget)?;
        Ok(&self.owner.candidate.input.pending)
    }
    /// Pays one work unit and returns the exact current inventory view.
    pub fn current_inventory(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<&Inventory<'_>> {
        self.guard.query(budget)?;
        if !self.inventory.belongs_to(&self.owner.candidate.output) {
            return self.guard.reject("foreign inventory");
        }
        Ok(self.inventory)
    }
    /// Pays one work unit and returns the exact source identity view.
    pub fn source_identity(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<&[u8; 32]> {
        self.guard.query(budget)?;
        Ok(self.owner.candidate.input.pending.source_semantic_sha256())
    }
    /// Pays one work unit and returns the exact pending identity view.
    pub fn pending_identity(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV18> {
        self.guard.query(budget)?;
        Ok(self.owner.candidate.input.pending.pending_identity())
    }
    /// Pays one work unit and returns the exact current identity view.
    pub fn current_identity(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV18> {
        self.guard.query(budget)?;
        Ok(self.inventory.identity_v18())
    }
    /// Pays one work unit and returns the exact root count view.
    pub fn root_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.roots.len())
    }
    /// Pays one work unit and returns the exact root view.
    pub fn root(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileRootV29> {
        self.guard.query(budget)?;
        match self.owner.tables.roots.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("root ordinal"),
        }
    }
    /// Pays one work unit and returns the exact instance count view.
    pub fn instance_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.instances.len())
    }
    /// Pays one work unit and returns the exact instance view.
    pub fn instance(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileInstanceV29> {
        self.guard.query(budget)?;
        match self.owner.tables.instances.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("instance ordinal"),
        }
    }
    /// Pays one work unit and returns the exact source alias count view.
    pub fn source_alias_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.source_aliases.len())
    }
    /// Pays one work unit and returns the exact source alias view.
    pub fn source_alias(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileSourceAliasV29> {
        self.guard.query(budget)?;
        match self.owner.tables.source_aliases.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("source_alias ordinal"),
        }
    }
    /// Pays one work unit and returns the exact origin count view.
    pub fn origin_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.origins.len())
    }
    /// Pays one work unit and returns the exact origin view.
    pub fn origin(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileOriginV29> {
        self.guard.query(budget)?;
        match self.owner.tables.origins.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("origin ordinal"),
        }
    }
    /// Pays one work unit and returns the exact piece count view.
    pub fn piece_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.pieces.len())
    }
    /// Pays one work unit and returns the exact piece view.
    pub fn piece(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTilePieceV29> {
        self.guard.query(budget)?;
        match self.owner.tables.pieces.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("piece ordinal"),
        }
    }
    /// Pays one work unit and returns the exact operation count view.
    pub fn operation_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.operations.len())
    }
    /// Pays one work unit and returns the exact operation view.
    pub fn operation(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileOccurrenceV29> {
        self.guard.query(budget)?;
        match self.owner.tables.operations.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("operation ordinal"),
        }
    }
    /// Pays one work unit and returns the exact definition count view.
    pub fn definition_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.definitions.len())
    }
    /// Pays one work unit and returns the exact definition view.
    pub fn definition(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileOccurrenceV29> {
        self.guard.query(budget)?;
        match self.owner.tables.definitions.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("definition ordinal"),
        }
    }
    /// Pays one work unit and returns the exact use site count view.
    pub fn use_site_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.uses.len())
    }
    /// Pays one work unit and returns the exact use site view.
    pub fn use_site(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileOccurrenceV29> {
        self.guard.query(budget)?;
        match self.owner.tables.uses.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("use_site ordinal"),
        }
    }
    /// Pays one work unit and returns the exact block count view.
    pub fn block_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.blocks.len())
    }
    /// Pays one work unit and returns the exact block view.
    pub fn block(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileOccurrenceV29> {
        self.guard.query(budget)?;
        match self.owner.tables.blocks.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("block ordinal"),
        }
    }
    /// Pays one work unit and returns the exact terminator count view.
    pub fn terminator_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.terminators.len())
    }
    /// Pays one work unit and returns the exact terminator view.
    pub fn terminator(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileOccurrenceV29> {
        self.guard.query(budget)?;
        match self.owner.tables.terminators.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("terminator ordinal"),
        }
    }
    /// Pays one work unit and returns the exact edge count view.
    pub fn edge_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.edges.len())
    }
    /// Pays one work unit and returns the exact edge view.
    pub fn edge(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileOccurrenceV29> {
        self.guard.query(budget)?;
        match self.owner.tables.edges.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("edge ordinal"),
        }
    }
    /// Pays one work unit and returns the exact edge argument count view.
    pub fn edge_argument_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.edge_arguments.len())
    }
    /// Pays one work unit and returns the exact edge argument view.
    pub fn edge_argument(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileOccurrenceV29> {
        self.guard.query(budget)?;
        match self.owner.tables.edge_arguments.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("edge_argument ordinal"),
        }
    }
    /// Pays one work unit and returns the exact attachment count view.
    pub fn attachment_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.attachments.len())
    }
    /// Pays one work unit and returns the exact attachment view.
    pub fn attachment(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTileAttachmentV29> {
        self.guard.query(budget)?;
        match self.owner.tables.attachments.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("attachment ordinal"),
        }
    }
    /// Pays one work unit and returns the exact pending obligation count view.
    pub fn pending_obligation_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<usize> {
        self.guard.query(budget)?;
        Ok(self.owner.tables.obligations.len())
    }
    /// Pays one work unit and returns the exact pending obligation view.
    pub fn pending_obligation(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> R<&ProductionTilePendingObligationV29> {
        self.guard.query(budget)?;
        match self.owner.tables.obligations.get(ordinal) {
            Some(row) => Ok(row),
            None => self.guard.reject("pending_obligation ordinal"),
        }
    }
}
