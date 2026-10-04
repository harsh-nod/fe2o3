use super::*;

#[path = "production_scalar_ssa_bound_snapshot_query_v1.rs"]
mod bound_snapshot;
use bound_snapshot::CertificateSubject;
pub use bound_snapshot::{
    ProductionU32BoundSnapshotRecurrenceFactV1, ProductionU32BoundSnapshotRecurrenceV1,
};

enum RecurrenceParts {
    Unavailable(ProductionScalarSsaEmissionUnavailableV1),
    Joined(Recurrence),
}

/// Inert fixed-coordinate result borrowing only the actual source owner, not
/// the temporary inventory, CFG, recurrence report or query resources.
#[derive(Clone, Copy, Debug)]
pub struct ProductionU32RecurrenceConsistencyFactV1<'source> {
    source: &'source ProductionPreRankedKirOwnerV1,
    root: SemanticFunctionIdV1,
    certificate: Certificate,
    recurrence: Recurrence,
}
impl ProductionU32RecurrenceConsistencyFactV1<'_> {
    /// Borrows the exact genuine owner whose source and N coordinates were joined.
    pub fn source(&self) -> &ProductionPreRankedKirOwnerV1 {
        self.source
    }
    /// Returns the actual source root bound by the retained emission alias.
    pub fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    /// Returns the separately admitted source certificate used as a prerequisite.
    pub fn certificate(&self) -> Certificate {
        self.certificate
    }
    /// Returns the independently replayed exact N recurrence joined to the source.
    pub fn recurrence(&self) -> Recurrence {
        self.recurrence
    }
    /// Always false: this additional consistency fact grants no rewrite authority.
    pub const fn authorizes_compiler_transform(&self) -> bool {
        false
    }
}

/// A supported inert join or an explicit unsupported mapping, never admission.
#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "Copyable borrowed facts keep scoped queries allocation-free"
)]
pub enum ProductionU32RecurrenceConsistencyV1<'source> {
    /// The source or actual N shape has no supported exact join.
    Unavailable(ProductionScalarSsaEmissionUnavailableV1),
    /// The source prerequisite, genuine emission rows, and exact N facts agree.
    Joined(ProductionU32RecurrenceConsistencyFactV1<'source>),
}

/// Nonescaping view of reserved canonical analysis resources. The source
/// certificate remains a separately established, privately constructed input.
pub struct ProductionScalarSsaEmissionQueryV1<'scope, 'source> {
    owner: &'source ProductionScalarSsaEmissionOwnerV1,
    inventory: &'scope Inventory<'source>,
    loops: &'scope Loops<'scope, 'source>,
    headers: &'scope [(Block, usize)],
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    corrupted: &'scope mut bool,
    failure: Option<Error>,
}

impl ProductionScalarSsaEmissionOwnerV1 {
    /// Replays source SSA, complete emission coverage and independent A+B once.
    /// Source SSA replay keeps its pre-existing local-limit accounting domain;
    /// the new inventory/index/report payload and all C queries use `budget`.
    /// The existing U32 no-overflow analyzer is deliberately not run here.
    ///
    /// Callback-owned retained allocations must be prepaid outside this scope.
    /// An extra callback reservation is rejected and is not silently released.
    /// A replacement ledger or undercut resource floor is never charged/released.
    /// Failed setup drops its local values before restoring its accepted storage
    /// delta to the entry floor. Once the callback starts, cleanup releases only
    /// the scope's retained reports and index, preserving callback-owned extras.
    /// Facts may escape with the original source borrow; the query cannot.
    ///
    /// ```
    /// use fe2o3_lower_mir_kernel::{ProductionScalarSsaEmissionOwnerV1,
    ///     ProductionScalarSsaEmissionErrorV1, ProductionU32RecurrenceConsistencyV1};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::SemanticU32InductionNoOverflowCertificateV1;
    /// use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
    /// fn fact<'s>(owner: &'s ProductionScalarSsaEmissionOwnerV1,
    ///     certificate: &SemanticU32InductionNoOverflowCertificateV1,
    ///     budget: &mut Budget<'_>)
    ///     -> Result<ProductionU32RecurrenceConsistencyV1<'s>, ProductionScalarSsaEmissionErrorV1> {
    ///     owner.with_u32_recurrences_v1(Default::default(), budget,
    ///         |query, budget| query.check_u32_certificate_v1(
    ///             SemanticFunctionIdV1::from_index(0), certificate, budget))
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionScalarSsaEmissionOwnerV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape(owner: &ProductionScalarSsaEmissionOwnerV1, budget: &mut Budget<'_>) {
    ///     let _ = owner.with_u32_recurrences_v1(Default::default(), budget, |query, _| Ok(query));
    /// }
    /// ```
    pub fn with_u32_recurrences_v1<'source, 'work, T>(
        &'source self,
        limits: CanonicalKirLoopLimitsV1,
        budget: &mut Budget<'work>,
        run: impl for<'scope> FnOnce(
            &mut ProductionScalarSsaEmissionQueryV1<'scope, 'source>,
            &mut Budget<'work>,
        ) -> Result<T>,
    ) -> Result<T> {
        let entry = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget as *mut _ as usize;
        let mut owned = 0usize;
        let mut callback_floor = None;
        let mut invalid_slot = false;
        let mut panic_payload = None;
        let mut rejected_payload = None;
        let mut outcome = match catch_unwind(AssertUnwindSafe(|| -> Result<T> {
            let incoming_capture = if self.captured_occurrences.is_none() {
                self.original
                    .semantic_ssa()
                    .occurrence_storage()
                    .ok_or(Error::Mismatch("source occurrence receipt"))?
                    .retained_storage()
            } else {
                0
            };
            if entry
                < self
                    .retained
                    .checked_add(incoming_capture)
                    .ok_or(Resource::Arithmetic)?
            {
                return Err(Resource::Accounting.into());
            }
            self.original
                .semantic_ssa()
                .verify_replay()
                .map_err(Error::Source)?;
            let (inventory, receipt) = Inventory::derive(self.original.executable(), budget)?;
            budget.reserve_storage(receipt.retained_storage())?;
            owned = receipt.retained_storage();
            self.emission.replay(&self.original, &inventory, budget)?;
            let (loops, receipt) = Loops::derive(&inventory, limits, budget)?;
            budget.reserve_storage(receipt.retained_storage())?;
            owned = owned
                .checked_add(receipt.retained_storage())
                .ok_or(Resource::Arithmetic)?;
            loops.replay(&inventory, limits, budget)?;
            let mut headers = Vec::new();
            // Fixed capacity is reconciled before initialization. Updating owned
            // immediately keeps partial setup cleanup exact if a later query fails.
            reserve(&mut headers, loops.loop_count(), budget)?;
            owned = owned
                .checked_add(table_bytes::<(Block, usize)>(headers.capacity())?)
                .ok_or(Resource::Arithmetic)?;
            for index in 0..loops.loop_count() {
                budget.charge_work(1)?;
                headers.push((loops.natural_loop(index, budget)?.header(), index));
            }
            resources::sort_work(headers.len(), budget)?;
            headers.sort_unstable_by_key(|row| row.0);
            let floor = entry.checked_add(owned).ok_or(Resource::Arithmetic)?;
            if budget.storage() != floor {
                return Err(Resource::Accounting.into());
            }
            callback_floor = Some(floor);
            let mut query = ProductionScalarSsaEmissionQueryV1 {
                owner: self,
                inventory: &inventory,
                loops: &loops,
                headers: &headers,
                slot,
                ledger,
                floor,
                failure: None,
                corrupted: &mut invalid_slot,
            };
            let result = run(&mut query, budget);
            *query.corrupted |= query.slot != budget as *mut _ as usize
                || budget.work_ledger_identity_v1() != query.ledger
                || budget.storage() < floor;
            let failure = query.failure.take();
            if *query.corrupted || budget.storage() != floor || failure.is_some() {
                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(result))) {
                    rejected_payload = Some(payload);
                }
                return Err(failure.unwrap_or(Error::Resource(Resource::Accounting)));
            }
            result
        })) {
            Ok(result) => Some(result),
            Err(payload) => {
                panic_payload = Some(payload);
                None
            }
        };
        let required =
            callback_floor.unwrap_or(entry.checked_add(owned).ok_or(Resource::Arithmetic)?);
        if invalid_slot
            || budget as *mut _ as usize != slot
            || budget.work_ledger_identity_v1() != ledger
            || budget.storage() < required
        {
            if let Some(value) = outcome.take()
                && let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(value)))
            {
                // A rejected result was already dropped inside the protected
                // callback path, so this slot is otherwise empty.
                rejected_payload = Some(payload);
            }
            drop((panic_payload, rejected_payload));
            return Err(Resource::Accounting.into());
        }
        // Setup has no caller allocation callback: after its locals drop, even
        // an interrupted temporary reservation belongs to this scope. Once the
        // callback starts, preserve its extras and release only our known receipt.
        let release = if callback_floor.is_some() {
            owned
        } else {
            budget
                .storage()
                .checked_sub(entry)
                .ok_or(Resource::Accounting)?
        };
        budget.release_storage(release)?;
        drop((panic_payload, rejected_payload));
        outcome.unwrap_or(Err(Error::Panicked))
    }
}

impl<'source> ProductionScalarSsaEmissionQueryV1<'_, 'source> {
    pub(super) fn inventory_for_guard(&self) -> &Inventory<'source> {
        self.inventory
    }

    /// Checks an admitted U32 certificate against the actual source SSA, emission
    /// inventory, and independently replayed exact N recurrence. Unsupported
    /// mapping is distinct from malformed claimed rows. This neither reruns the
    /// no-overflow analyzer nor replaces mandatory production proof replay.
    pub fn check_u32_certificate_v1(
        &mut self,
        root: SemanticFunctionIdV1,
        certificate: &Certificate,
        budget: &mut Budget<'_>,
    ) -> Result<ProductionU32RecurrenceConsistencyV1<'source>> {
        let result = (|| {
            if let Some(error) = &self.failure {
                return Err(copy_query_error(error));
            }
            if budget as *mut _ as usize != self.slot
                || budget.work_ledger_identity_v1() != self.ledger
                || budget.storage() < self.floor
            {
                *self.corrupted = true;
                return Err(Resource::Accounting.into());
            }
            if budget.storage() != self.floor {
                return Err(Resource::Accounting.into());
            }
            Ok(
                match self.check(root, CertificateSubject::Legacy(*certificate), budget)? {
                    RecurrenceParts::Unavailable(reason) => {
                        ProductionU32RecurrenceConsistencyV1::Unavailable(reason)
                    }
                    RecurrenceParts::Joined(recurrence) => {
                        ProductionU32RecurrenceConsistencyV1::Joined(
                            ProductionU32RecurrenceConsistencyFactV1 {
                                source: &self.owner.original,
                                root,
                                certificate: *certificate,
                                recurrence,
                            },
                        )
                    }
                },
            )
        })();
        if let Err(error) = &result {
            self.failure = Some(copy_query_error(error));
        }
        result
    }

    fn check(
        &self,
        root: SemanticFunctionIdV1,
        certificate: CertificateSubject,
        budget: &mut Budget<'_>,
    ) -> Result<RecurrenceParts> {
        use ProductionScalarSsaEmissionUnavailableV1 as Unavailable;
        use RecurrenceParts as Outcome;
        let original = &self.owner.original;
        let source = original.semantic_ssa().source_semantic();
        check_certificate_binding(original, certificate, budget)?;
        let sealed = &self.owner.emission;
        charge_lookup(sealed.aliases.len(), budget)?;
        let alias = sealed
            .aliases
            .binary_search_by_key(&(root.index(), certificate.function().index()), |row| {
                row.key()
            })
            .map_err(|_| Error::Mismatch("certificate actual root/body alias"))?;
        let function = &sealed.capture.functions[sealed.aliases[alias].emitted];
        if function.unsupported_placement {
            return Ok(Outcome::Unavailable(Unavailable::ExpandedPlacement));
        }
        let coordinate = function
            .coordinate
            .ok_or(Error::Mismatch("sealed physical function"))?;
        let header_block = certificate.header().block();
        let variable = SsaVariableIdV1::new(certificate.induction().local().index());
        let header_value = SsaValueV1::BlockArgument {
            block: SsaBlockIdV1::new(header_block.index()),
            variable,
        };
        let header = sealed.definition(function, header_value, budget)?;
        let header_coordinate =
            header.definitions[0].ok_or(Error::Mismatch("header definition"))?;
        let Definition::BlockArgument { block, argument } = header_coordinate else {
            return Err(Error::Mismatch("actual induction header parameter"));
        };
        // The first source fragment has only scalar transport before this
        // parameter. Generic flattened/frame component numbering is not inferred.
        let plan = original
            .semantic_ssa()
            .plan_for_function(certificate.function())
            .ok_or(Error::Mismatch("certificate SSA function"))?
            .plan();
        let variables = plan
            .transport_variables(SsaBlockIdV1::new(header_block.index()))
            .ok_or(Error::Mismatch("certificate header transport"))?;
        let mut expected_argument = None;
        let declaration = &source.functions()[certificate.function().index() as usize];
        for (ordinal, &local) in variables.iter().enumerate() {
            budget.charge_work(3)?;
            let ty = declaration
                .locals()
                .get(local.get() as usize)
                .ok_or(Error::Mismatch("header source local"))?
                .ty();
            if fixed_scalar(source.types(), ty).is_none()
                && !matches!(
                    source.types()[ty.index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                )
            {
                return Ok(Outcome::Unavailable(Unavailable::UnsupportedFragment));
            }
            if local == variable {
                expected_argument = Some(ordinal);
            }
        }
        if expected_argument != Some(argument as usize) || block.function != coordinate {
            return Err(Error::Mismatch("exact source header parameter ordinal"));
        }
        charge_lookup(self.headers.len(), budget)?;
        let Ok(index) = self.headers.binary_search_by_key(&block, |row| row.0) else {
            return Ok(Outcome::Unavailable(Unavailable::NoExactRecurrence));
        };
        let recurrences = self.loops.recurrences(self.headers[index].1, budget)?;
        charge_lookup(recurrences.len(), budget)?;
        let Ok(index) = recurrences.binary_search_by_key(&header_coordinate, |row| row.parameter())
        else {
            return Ok(Outcome::Unavailable(Unavailable::NoExactRecurrence));
        };
        let recurrence = recurrences[index];
        let initial = site_definition(
            sealed,
            function,
            certificate.initialization(),
            variable,
            budget,
        )?;
        let update = site_definition(sealed, function, certificate.update(), variable, budget)?;
        let checked = site_definition(
            sealed,
            function,
            certificate.checked_addition(),
            SsaVariableIdV1::new(certificate.checked_result().local().index()),
            budget,
        )?;
        let source_input = sealed.first_operand(
            original,
            certificate.function(),
            certificate.checked_addition(),
            variable,
            budget,
        )?;
        let source_update = sealed.first_operand(
            original,
            certificate.function(),
            certificate.update(),
            SsaVariableIdV1::new(certificate.checked_result().local().index()),
            budget,
        )?;
        if source_input != header_value
            || source_update != sealed.capture.expected[checked.expected].value
        {
            return Err(Error::Mismatch(
                "source SSA checked-input and update relation",
            ));
        }
        if recurrence.scalar() != ScalarType::U32
            || recurrence.step_bits() != 1
            || recurrence.parameter_operand() != 0
            || Some(recurrence.initial()) != initial.definitions[0]
            || Some(recurrence.update()) != update.definitions[0]
            || checked.definitions[0] != update.definitions[0]
            || recurrence.overflow() != checked.definitions[1]
        {
            return Err(Error::Mismatch(
                "source certificate and actual recurrence fields",
            ));
        }
        let entry_edge = source_edge(
            sealed,
            function,
            certificate.preheader().block().index(),
            variable,
            budget,
        )?;
        let backedge = source_edge(
            sealed,
            function,
            certificate.update().block().block().index(),
            variable,
            budget,
        )?;
        let initial_key = sealed.capture.expected[initial.expected].value;
        let update_key = sealed.capture.expected[update.expected].value;
        if entry_edge.edge != Some(recurrence.initial_edge())
            || entry_edge.incoming != initial_key
            || backedge.edge != Some(recurrence.backedge())
            || backedge.incoming != update_key
            || entry_edge.target != header_block.index()
            || backedge.target != header_block.index()
        {
            return Err(Error::Mismatch(
                "exact source recurrence incoming occurrences",
            ));
        }
        require_constant_in_source_span(
            sealed,
            original,
            function,
            self.inventory,
            recurrence.initial(),
            certificate.initialization(),
            0,
            budget,
        )?;
        require_constant_in_source_span(
            sealed,
            original,
            function,
            self.inventory,
            recurrence.step(),
            certificate.checked_addition(),
            1,
            budget,
        )?;
        let assertion = original
            .assert_origins()
            .assert_condition(
                root,
                certificate.function(),
                certificate.checked_addition().block().block(),
                budget,
            )
            .map_err(|error| match error {
                SemanticKirAssertOriginErrorV1::Resource(error) => Error::Resource(error),
                _ => Error::Mismatch("source checked-overflow assertion origin"),
            })?;
        if assertion.expected()
            || assertion.semantic_success() != certificate.update().block().block()
        {
            return Err(Error::Mismatch("source checked-overflow success edge"));
        }
        if let SemanticKirAssertConditionOutcomeV1::Emitted { definition, .. } = assertion.outcome()
            && Some(definition) != recurrence.overflow()
        {
            return Err(Error::Mismatch(
                "actual checked-overflow assertion definition",
            ));
        }
        if let CertificateSubject::Snapshot(certificate) = certificate {
            bound_snapshot::check_transport(self.owner, function, certificate, budget)?;
        }
        Ok(Outcome::Joined(recurrence))
    }
}

fn copy_query_error(error: &Error) -> Error {
    match error {
        Error::Resource(error) => Error::Resource(*error),
        Error::Inventory(error) => Error::Inventory(*error),
        Error::Loops(error) => Error::Loops(error.clone()),
        Error::Mismatch(message) => Error::Mismatch(message),
        _ => Error::Mismatch("query failure cannot be ignored"),
    }
}

fn check_certificate_binding(
    original: &ProductionPreRankedKirOwnerV1,
    certificate: CertificateSubject,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(5)?;
    let owner = original.semantic_ssa();
    let source = owner.source_semantic();
    let function = source
        .functions()
        .get(certificate.function().index() as usize)
        .ok_or(Error::Mismatch("certificate function"))?;
    if certificate.semantic_mir_sha256().as_bytes() != owner.source_semantic_sha256()
        || certificate.function_identity() != function.identity()
        || certificate.grants_authority()
        || certificate.authorizes_compiler_transform()
    {
        return Err(Error::Mismatch("certificate actual source identity"));
    }
    for place in [
        certificate.induction(),
        certificate.guard_induction(),
        certificate.bound(),
        certificate.predicate(),
        certificate.checked_result(),
    ] {
        budget.charge_work(4)?;
        let local = function
            .locals()
            .get(place.local().index() as usize)
            .ok_or(Error::Mismatch("certificate local"))?;
        let ty = source
            .types()
            .get(place.ty().index() as usize)
            .ok_or(Error::Mismatch("certificate type"))?;
        if local.identity() != place.local_identity()
            || local.ty() != place.ty()
            || ty.identity() != place.type_identity()
        {
            return Err(Error::Mismatch("certificate typed local identity"));
        }
    }
    for site in [
        certificate.preheader(),
        certificate.header(),
        certificate.body_entry(),
        certificate.exit(),
        certificate.initialization().block(),
        certificate.guard().block(),
        certificate.checked_addition().block(),
        certificate.update().block(),
    ] {
        budget.charge_work(2)?;
        if function
            .blocks()
            .get(site.block().index() as usize)
            .map(|block| block.identity())
            != Some(site.identity())
        {
            return Err(Error::Mismatch("certificate block identity"));
        }
    }
    for site in [
        Some(certificate.initialization()),
        Some(certificate.guard()),
        Some(certificate.checked_addition()),
        Some(certificate.update()),
        certificate.guard_induction_snapshot(),
    ] {
        let Some(site) = site else { continue };
        budget.charge_work(2)?;
        if function
            .blocks()
            .get(site.block().block().index() as usize)
            .filter(|block| block.identity() == site.block().identity())
            .and_then(|block| block.statements().get(site.statement() as usize))
            .is_none()
        {
            return Err(Error::Mismatch("certificate statement site"));
        }
    }
    if let CertificateSubject::Snapshot(certificate) = certificate {
        bound_snapshot::check_extra_binding(original, certificate, budget)?;
    }
    Ok(())
}

fn site_definition<'a>(
    sealed: &'a Sealed,
    function: &EmittedFunction,
    site: fe2o3_mir_model::SemanticU32InductionStatementSiteV1,
    variable: SsaVariableIdV1,
    budget: &mut Budget<'_>,
) -> Result<&'a EmittedDefinition> {
    charge_lookup(sealed.sites.len(), budget)?;
    let key = (
        function.source.index(),
        2,
        site.block().block().index(),
        site.statement(),
        variable.get(),
    );
    let index = sealed
        .sites
        .binary_search_by_key(&key, |row| row.key)
        .map_err(|_| Error::Mismatch("certificate source definition occurrence"))?;
    sealed.definition(
        function,
        sealed.capture.expected[sealed.sites[index].expected].value,
        budget,
    )
}

fn source_edge<'a>(
    sealed: &'a Sealed,
    function: &EmittedFunction,
    block: u32,
    variable: SsaVariableIdV1,
    budget: &mut Budget<'_>,
) -> Result<&'a EmittedEdge> {
    let rows = &sealed.capture.edges[function.edges.clone()];
    charge_lookup(rows.len(), budget)?;
    let index = rows
        .binary_search_by_key(&(block, variable.get()), |row| {
            (row.source.source().get(), row.variable.get())
        })
        .map_err(|_| Error::Mismatch("certificate source Goto occurrence"))?;
    Ok(&rows[index])
}

#[allow(
    clippy::too_many_arguments,
    reason = "Independent source, emission and canonical subjects are checked together"
)]
fn require_constant_in_source_span(
    sealed: &Sealed,
    original: &ProductionPreRankedKirOwnerV1,
    function: &EmittedFunction,
    inventory: &Inventory<'_>,
    definition: Definition,
    source: fe2o3_mir_model::SemanticU32InductionStatementSiteV1,
    value: u32,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(5)?;
    let Definition::Result {
        operation,
        result: 0,
    } = definition
    else {
        return Err(Error::Mismatch("recurrence literal definition"));
    };
    let row = inventory
        .functions()
        .get(operation.block.function.0 as usize)
        .and_then(|function| {
            inventory
                .blocks()
                .get(function.blocks.start + operation.block.block as usize)
        })
        .ok_or(Error::Mismatch("recurrence literal block"))?;
    let actual = inventory
        .operations()
        .get(row.operations.start + operation.operation as usize)
        .filter(|row| row.coordinate == operation)
        .ok_or(Error::Mismatch("recurrence literal operation"))?;
    if actual.operation.kind != OperationKind::Constant(Constant::U32(value)) {
        return Err(Error::Mismatch("recurrence exact U32 literal"));
    }
    let span = sealed.statement(
        original,
        function,
        source.block().block().index(),
        source.statement(),
        budget,
    )?;
    if span.kernel_ir_block() != row.block.id
        || operation.operation < span.first_operation_ordinal()
        || operation.operation
            >= span
                .first_operation_ordinal()
                .checked_add(span.operation_count())
                .ok_or(Resource::Arithmetic)?
    {
        return Err(Error::Mismatch("recurrence literal exact source span"));
    }
    Ok(())
}
