//! Storage-aware live Pliron transport. No legacy proof-owner conversion.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV18,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
    CanonicalKernelIrWorkLedgerIdentityV1, CanonicalStorageTableIdentityV18, StorageLayoutLimitsV1,
    VerifiedCanonicalKernelIrIdentityV18, VerifiedCanonicalKernelIrModuleV18,
};

#[path = "kir_bridge_v18_resources.rs"]
mod resources;
pub(crate) use resources::{
    CLEANUP_ATTEMPTS as BOUNDED_PAYLOAD_CLEANUP_ATTEMPTS_V1,
    discard_caught_payload as discard_bounded_payload_v1,
};

#[derive(Debug)]
pub enum KirBridgeErrorV18 {
    Bridge(KirBridgeErrorV1),
    Resource(ResourceError),
    Canonical(CanonicalKernelIrReplayAdmissionErrorV18),
    SessionSetup,
    Allocation,
}
impl fmt::Display for KirBridgeErrorV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::Canonical(error) => error.fmt(f),
            Self::SessionSetup => f.write_str("V18 bridge session setup failed"),
            Self::Allocation => f.write_str("V18 bridge exact allocation refused"),
        }
    }
}
impl Error for KirBridgeErrorV18 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Bridge(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::Canonical(error) => Some(error),
            Self::SessionSetup | Self::Allocation => None,
        }
    }
}
impl From<KirBridgeErrorV1> for KirBridgeErrorV18 {
    fn from(value: KirBridgeErrorV1) -> Self {
        Self::Bridge(value)
    }
}
impl From<OperationHandleError> for KirBridgeErrorV18 {
    fn from(value: OperationHandleError) -> Self {
        Self::Bridge(value.into())
    }
}
impl From<ResourceError> for KirBridgeErrorV18 {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}
impl From<CanonicalKernelIrReplayAdmissionErrorV18> for KirBridgeErrorV18 {
    fn from(value: CanonicalKernelIrReplayAdmissionErrorV18) -> Self {
        Self::Canonical(value)
    }
}

/// Transfer reservation, not exact upstream heap bytes. The graph/session
/// envelope is versioned logical storage; explicit typed rows/headers add bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KirBridgeStorageV18 {
    retained: usize,
}
impl KirBridgeStorageV18 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Inert report. This does not certify memory safety, provenance or optimization.
#[derive(Debug)]
pub struct KirBridgeReportV18 {
    pub input: VerifiedCanonicalKernelIrIdentityV18,
    pub output: VerifiedCanonicalKernelIrIdentityV18,
    pub table: CanonicalStorageTableIdentityV18,
    pub correspondence: Vec<KirBridgeCorrespondenceV1>,
}

/// Move-only private session borrowing the exact immutable canonical input.
/// Equal table keys do not authorize another owner, session, graph or epoch.
/// No raw context, arbitrary callback or legacy/native certificate conversion.
///
/// ```compile_fail
/// use fe2o3_pliron::KirPlironGraphV18;
/// fn clone_required<T: Clone>() {}
/// clone_required::<KirPlironGraphV18<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_pliron::KirPlironGraphV18;
/// fn escape(graph: &mut KirPlironGraphV18<'_>) { let _ = &mut graph.session; }
/// ```
pub struct KirPlironGraphV18<'input> {
    session: PlironSession,
    root: OperationHandle,
    epoch: crate::OperationGraphEpochV1,
    profile: storage_v18::ProfileV18<'input>,
    correspondence: Vec<KirBridgeCorrespondenceV1>,
    origins: KirBridgeOriginsV1,
    coordinates: HashMap<Ptr<Operation>, KirBridgeCoordinateV1>,
    retained_storage: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    caller_floor: usize,
}

impl fmt::Debug for KirPlironGraphV18<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KirPlironGraphV18")
            .field("input", self.profile.owner().identity())
            .field("table", &self.profile.key())
            .field("epoch", &self.epoch)
            .field("retained_storage", &self.retained_storage)
            .finish_non_exhaustive()
    }
}

type ImportResult<'input> =
    Result<(KirPlironGraphV18<'input>, KirBridgeStorageV18), KirBridgeErrorV18>;
type ExportResult = Result<
    (
        VerifiedCanonicalKernelIrModuleV18,
        KirBridgeReportV18,
        KirBridgeStorageV18,
    ),
    KirBridgeErrorV18,
>;

#[path = "kir_bridge_v18_optimization.rs"]
mod optimization;
pub(crate) use optimization::{ExecutedV18Parts, optimize_v18_graph};
#[path = "kir_bridge_canonical_ranked_v18.rs"]
mod ranked_policy;
#[path = "kir_bridge_private_memory_v18.rs"]
mod private_policy;
pub(crate) use private_policy::NativeCanonicalPrivateAdmissionV18;

#[path = "kir_bridge_global_pending_v18.rs"]
mod global_pending;

impl<'input> KirPlironGraphV18<'input> {
    /// The returned reservation must be held on the same work ledger while the
    /// session lives. The input's canonical reservation is the caller's custody.
    pub fn import(
        input: &'input VerifiedCanonicalKernelIrModuleV18,
        budget: &mut Budget<'_>,
    ) -> ImportResult<'input> {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let mut scope = resources::Scope::enter(budget)?;
        let result = import_inner(input, scope.budget, floor, ledger);
        let release = scope.finish();
        match result {
            Err(error) => Err(error),
            Ok(value) => {
                release?;
                Ok(value)
            }
        }
    }

    pub fn input(&self) -> &VerifiedCanonicalKernelIrIdentityV18 {
        self.profile.owner().identity()
    }
    pub fn table_identity(&self) -> CanonicalStorageTableIdentityV18 {
        self.profile.key()
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained_storage
    }
    pub fn correspondence(&self) -> &[KirBridgeCorrespondenceV1] {
        &self.correspondence
    }

    fn validate_custody(&self, budget: &Budget<'_>) -> Result<(), KirBridgeErrorV18> {
        self.session.validate_identity()?;
        if budget.work_ledger_identity_v1() != self.ledger
            || budget.storage() < resources::add(self.caller_floor, self.retained_storage)?
            || self.root.owner != self.session.identity
            || !self.session.operations.contains_key(&self.root.identity)
            || !self
                .session
                .owned_tree_work
                .contains_key(&self.root.identity)
            || self.session.operation_graph_epochs.get(&self.root.identity) != Some(&self.epoch)
        {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        self.profile
            .validate_module(self.profile.owner().module())?;
        Ok(())
    }

    /// Extracts the actual live SSA graph and re-admits a fresh immutable V18
    /// owner with the same table. This is not source/init/runtime qualification.
    pub fn extract_canonical_v18(
        &mut self,
        limits: StorageLayoutLimitsV1,
        budget: &mut Budget<'_>,
    ) -> ExportResult {
        self.extract(limits, budget, false)
    }

    /// Additionally requires exact source coordinates, origins and bytes.
    pub fn extract_canonical_v18_o0(
        &mut self,
        limits: StorageLayoutLimitsV1,
        budget: &mut Budget<'_>,
    ) -> ExportResult {
        self.extract(limits, budget, true)
    }

    fn extract(
        &mut self,
        limits: StorageLayoutLimitsV1,
        budget: &mut Budget<'_>,
        exact: bool,
    ) -> ExportResult {
        // A foreign ledger must not acquire a cleanup guard for this owner.
        self.validate_custody(budget)?;
        let mut scope = resources::Scope::enter(budget)?;
        let result = self.extract_inner(limits, scope.budget, exact);
        let release = scope.finish();
        match result {
            Err(error) => Err(error),
            Ok(value) => {
                release?;
                Ok(value)
            }
        }
    }

    fn extract_inner(
        &mut self,
        limits: StorageLayoutLimitsV1,
        budget: &mut Budget<'_>,
        exact: bool,
    ) -> ExportResult {
        let charged_tree = self.session.owned_tree_work[&self.root.identity];
        let source = self.profile.owner();
        budget.charge_work(resources::add(
            source.canonical_bytes().len(),
            charged_tree,
        )?)?;
        let root = self.session.operations[&self.root.identity];
        let (tree, slots) = census_live_graph_v12(&self.session.context, root)?;
        if tree != charged_tree {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        let envelope =
            resources::envelope(source.canonical_bytes().len(), resources::add(tree, slots)?)?;
        budget.charge_work(envelope.work)?;
        budget.reserve_storage(envelope.storage)?;
        let table_bytes = resources::table_bytes(source.module(), budget)?;
        budget.reserve_storage(table_bytes)?;
        let extracted = catch_unwind(AssertUnwindSafe(|| -> Result<_, KirBridgeErrorV18> {
            // Paid by the V18 upstream envelope, never by an inert table key.
            self.session.operation_graph_snapshot_v1(&self.root)?;
            for (live, kind) in &self.origins.preserved_operations {
                let Some(KirBridgeCoordinateV1::Operation {
                    function,
                    block,
                    operation,
                }) = self.coordinates.get(live)
                else {
                    return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
                };
                let original = source
                    .module()
                    .functions
                    .get(*function as usize)
                    .and_then(|f| f.body.as_ref())
                    .and_then(|body| body.blocks.get(*block as usize))
                    .and_then(|block| block.operations.get(*operation as usize));
                if original.map(|operation| &operation.kind) != Some(kind) {
                    return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
                }
            }
            let table = resources::copy_table(source.module())?;
            let mut metadata = module_metadata_v12(source.module());
            metadata.storage_layouts = table;
            if exact {
                validate_exact_graph_origins_v12(
                    &self.session.context,
                    root,
                    source.module(),
                    &self.origins,
                )?;
            }
            #[cfg(test)]
            tests::panic_at(tests::PanicAt::Extract);
            let (module, correspondence) = extract_optimized_module_graph_prepared(
                &self.session.context,
                root,
                source.module(),
                &self.origins,
                KirBridgeTypeProfileV12::V18(self.profile),
                Some(metadata),
            )?;
            if exact && correspondence != self.correspondence {
                return Err(KirBridgeErrorV1::NonExactRoundTrip.into());
            }
            Ok((module, correspondence))
        }));
        let (module, correspondence) = match extracted {
            Ok(result) => result?,
            Err(payload) => {
                self.session.poisoned = true;
                let error = KirBridgeErrorV18::from(KirBridgeErrorV1::UpstreamPanicked);
                #[cfg(test)]
                tests::cleanup_start(budget.storage(), Some(self.session.poisoned));
                resources::discard_caught_payload(payload);
                return Err(error);
            }
        };
        let (owner, canonical_storage) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module, limits, budget,
            )?;
        budget.reserve_storage(canonical_storage.retained_storage())?;
        let table = owner.storage_table_identity_with_budget_v18(budget)?;
        if table != self.profile.key() {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        if exact {
            budget.charge_work(source.canonical_bytes().len())?;
            if owner.canonical_bytes() != source.canonical_bytes() {
                return Err(KirBridgeErrorV1::NonExactRoundTrip.into());
            }
        }
        let report_storage = resources::add(
            resources::mul(
                correspondence.capacity(),
                std::mem::size_of::<KirBridgeCorrespondenceV1>(),
            )?,
            std::mem::size_of::<KirBridgeReportV18>(),
        )?;
        budget.reserve_storage(report_storage)?;
        let retained = resources::add(canonical_storage.retained_storage(), report_storage)?;
        let report = KirBridgeReportV18 {
            input: *source.identity(),
            output: *owner.identity(),
            table,
            correspondence,
        };
        drop(module);
        budget.release_storage(table_bytes)?;
        budget.release_storage(envelope.storage)?;
        Ok((owner, report, KirBridgeStorageV18 { retained }))
    }
}

fn import_inner<'input>(
    input: &'input VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'_>,
    floor: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
) -> ImportResult<'input> {
    let profile = storage_v18::ProfileV18::new(input, budget)?;
    budget.charge_work(input.canonical_bytes().len())?;
    let tree = source_tree_work_v12(input.module())?;
    let envelope = resources::envelope(input.canonical_bytes().len(), tree)?;
    let coordinate_envelope = resources::coordinate_envelope(tree)?;
    let import_work = resources::add(envelope.work, coordinate_envelope.work)?;
    let import_storage = resources::add(envelope.storage, coordinate_envelope.storage)?;
    budget.charge_work(import_work)?;
    budget.reserve_storage(import_storage)?;
    let retained = resources::add(
        import_storage,
        std::mem::size_of::<KirPlironGraphV18<'_>>(),
    )?;
    budget.reserve_storage(std::mem::size_of::<KirPlironGraphV18<'_>>())?;
    let result = catch_unwind(AssertUnwindSafe(|| -> ImportResult<'input> {
        let (actual_tree, correspondence) =
            preflight_with_profile_v12(input.module(), KirBridgeTypeProfileV12::V18(profile))?;
        if actual_tree != tree {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        let registration =
            dialect_gpu::dialect_registration().map_err(|_| KirBridgeErrorV18::SessionSetup)?;
        let limits =
            crate::ShellLimits::new(32, 64, 512).map_err(|_| KirBridgeErrorV18::SessionSetup)?;
        let mut session = PlironSession::new(limits, [registration])
            .map_err(|_| KirBridgeErrorV18::SessionSetup)?;
        session.require_internal_tree_capacity(tree)?;
        let root = session.create_module("kir_bridge_v18")?;
        let transaction = session.begin_checked_operation_graph_mutation_v1(&root)?;
        let pointer = session.operations[&root.identity];
        let mut coordinates = HashMap::new();
        coordinates.try_reserve(tree).map_err(|_| KirBridgeErrorV18::Allocation)?;
        if coordinates.capacity() > resources::mul(tree, 2)? {
            return Err(KirBridgeErrorV18::Allocation);
        }
        let origins = build_module_graph_with_coordinates(
            &mut session.context,
            pointer,
            input.module(),
            KirBridgeTypeProfileV12::V18(profile),
            Some(&mut coordinates),
        )?;
        if coordinates.len() > tree {
            return Err(KirBridgeErrorV1::GraphIdentityMismatch.into());
        }
        session.finish_internal_root_construction(&root, transaction)?;
        let epoch = *session
            .operation_graph_epochs
            .get(&root.identity)
            .ok_or(KirBridgeErrorV1::GraphIdentityMismatch)?;
        #[cfg(test)]
        tests::panic_at(tests::PanicAt::Import);
        Ok((
            KirPlironGraphV18 {
                session,
                root,
                epoch,
                profile,
                correspondence,
                origins,
                coordinates,
                retained_storage: retained,
                ledger,
                caller_floor: floor,
            },
            KirBridgeStorageV18 { retained },
        ))
    }));
    match result {
        Ok(result) => result,
        Err(payload) => {
            let error = KirBridgeErrorV18::from(KirBridgeErrorV1::UpstreamPanicked);
            #[cfg(test)]
            tests::cleanup_start(budget.storage(), None);
            resources::discard_caught_payload(payload);
            Err(error)
        }
    }
}

#[cfg(test)]
#[path = "kir_pointer_to_generic_v18_tests.rs"]
mod pointer_to_generic_tests;

#[cfg(test)]
#[path = "kir_slice_to_generic_v18_tests.rs"]
mod slice_to_generic_tests;

#[cfg(test)]
#[path = "kir_bridge_v18_resource_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "kir_bridge_v18_tests.rs"]
mod tests;
