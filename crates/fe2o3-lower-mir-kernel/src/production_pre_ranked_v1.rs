// This shard shares private correspondence and lowering helpers with its owner.

/// Construction failure before ranked checking starts.
#[derive(Debug)]
pub enum ProductionPreRankedKirErrorV1 {
    /// Existing source, SSA, materialization or correspondence rejection.
    Lowering(ProductionSemanticKirErrorV1),
    /// Connected V12 wire, semantic verification or resource rejection.
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12),
}

impl fmt::Display for ProductionPreRankedKirErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(error) => error.fmt(formatter),
            Self::Canonical(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProductionPreRankedKirErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lowering(error) => Some(error),
            Self::Canonical(error) => Some(error),
        }
    }
}

impl From<ProductionSemanticKirErrorV1> for ProductionPreRankedKirErrorV1 {
    fn from(error: ProductionSemanticKirErrorV1) -> Self {
        Self::Lowering(error)
    }
}

/// One executable mixed-SSA graph constructed before ranked verification.
///
/// The source plans and launch roster are retained for correspondence and
/// existing source-ranked checks. They are not a second executable graph.
/// This owner does not assert semantic equivalence or grant artifact/launch
/// authority. Explicit later replay audits retain their existing meaning.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionPreRankedKirOwnerV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
/// fn mutate(owner: &mut ProductionPreRankedKirOwnerV1) {
///     owner.executable().module().functions.clear();
/// }
/// ```
#[must_use = "dropping the owner abandons pre-ranked executable custody"]
#[derive(Debug)]
pub struct ProductionPreRankedKirOwnerV1 {
    semantic_ssa: ProductionSemanticSsaOwnerV1,
    executable: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    executable_storage: fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV12,
    assert_origins: SealedAssertOriginsV1,
    source_launch: crate::ProductionSourceLaunchRosterV1,
    canonical_kernel_ir: ProductionCanonicalKernelIrV1,
    correspondence: SemanticKirCorrespondenceV1,
    limits: ProductionSemanticKirLimitsV1,
    launch_roots: Box<[RetainedRankedLaunchRootV1]>,
}

impl ProductionPreRankedKirOwnerV1 {
    /// Borrow the source-to-N ordinary Store trace owned by this materialization.
    /// These numeric rows are source-replayed importer correspondence only;
    /// they grant no numerical, optimized-output, or artifact authority.
    pub fn source_store_value_uses_v1(&self) -> &[SemanticKirSourceStoreValueUseV1] {
        &self.correspondence.source_store_value_uses
    }

    /// Replay the existing source-to-N materialization and compare the complete
    /// executable and correspondence, including each Store-use row.
    /// This uses the existing source-phase limits, not a canonical byte receipt.
    pub fn replay_source_store_value_uses_v1(&self) -> Result<(), ProductionSemanticKirErrorV1> {
        source_output_replay_v1(self)
    }

    /// Materializes the retained SSA plans once, using the full source launch
    /// roster. No ranked verification receipt is accepted at this stage.
    ///
    /// The ledger covers connected V12 admission and assertion-origin emission,
    /// sealing scratch, and retained payload, including their coexistence.
    /// Retained source MIR, SSA plans, launch rows, legacy correspondence/bytes
    /// and other lowering scratch keep their existing limits and are excluded.
    /// This includes Store-use rows and definition scratch: their module-shared
    /// source-phase work and actual-capacity row caps are not receipt bytes.
    /// The incoming live floor is restored after failure drops or success transfer.
    /// Before another allocation, reserve BOTH `executable_storage()` and
    /// `assert_origin_storage()` while this owner or its attached successor lives.
    /// This phase-local logical ledger is not an allocator/RSS meter or a claim
    /// that excluded host allocation failures are recoverable.
    /// This legacy constructor restores the floor on ordinary `Result` return;
    /// it does not provide an unwind-cleanup guarantee. An optional existing SSA
    /// occurrence-capture receipt stays separately caller-reserved and is not
    /// included in either transferred receipt. No new capture is performed.
    pub fn try_materialize_with_budget(
        semantic_ssa: ProductionSemanticSsaOwnerV1,
        source_launch: crate::ProductionSourceLaunchRosterV1,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Self, ProductionPreRankedKirErrorV1> {
        let floor = budget.storage();
        let result =
            Self::try_materialize_origins_inner_v1(semantic_ssa, source_launch, limits, budget);
        // The inner call dropped all failed payloads. On success this is the
        // explicit transfer of graph+origin ownership and their two receipts.
        let release =
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(SemanticKirAssertOriginErrorV1::Resource(
                    AssertOriginResourceV1::Accounting,
                ))?;
        budget
            .release_storage(release)
            .map_err(SemanticKirAssertOriginErrorV1::from)?;
        result
    }

    fn try_materialize_origins_inner_v1(
        semantic_ssa: ProductionSemanticSsaOwnerV1,
        source_launch: crate::ProductionSourceLaunchRosterV1,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Self, ProductionPreRankedKirErrorV1> {
        let launch_roots = materialization_launch_roots_v1(&semantic_ssa, &source_launch)?;
        let mut emitted_origins = AssertOriginEmissionV1::new(budget);
        let (module, correspondence) = lower_module_with_assert_origins_v1(
            &semantic_ssa,
            limits,
            Some(&launch_roots),
            Some(&mut emitted_origins),
        )?;
        // Keep the frozen legacy bytes for existing publication/replay consumers.
        // Their transient inverse validation is not another retained graph.
        let canonical_kernel_ir = ProductionCanonicalKernelIrV1::from_module_ref(&module)?;
        let (executable, executable_storage) =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::
                from_module_ref_with_verification_budget_v12(&module, emitted_origins.budget)
                .map_err(ProductionPreRankedKirErrorV1::Canonical)?;
        emitted_origins
            .budget
            .reserve_storage(executable_storage.retained_storage())
            .map_err(SemanticKirAssertOriginErrorV1::from)?;
        drop(module);
        let assert_origins = emitted_origins.seal(&semantic_ssa, &correspondence, &executable)?;
        Ok(Self {
            semantic_ssa,
            executable,
            executable_storage,
            assert_origins,
            source_launch,
            canonical_kernel_ir,
            correspondence,
            limits,
            launch_roots,
        })
    }

    /// Borrows the exact source and SSA plans consumed by materialization.
    pub const fn semantic_ssa(&self) -> &ProductionSemanticSsaOwnerV1 {
        &self.semantic_ssa
    }

    /// Borrows the only retained executable graph and its canonical V12 bytes.
    pub const fn executable(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        &self.executable
    }

    /// Returns the connected graph payload to reserve when continuing its ledger.
    pub const fn executable_storage(&self) -> fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV12 {
        self.executable_storage
    }

    /// Borrows source assertion origins sealed against this exact executable.
    pub const fn assert_origins(&self) -> SemanticKirAssertOriginsV1<'_> {
        SemanticKirAssertOriginsV1 {
            executable: &self.executable,
            semantic_ssa: &self.semantic_ssa,
            origins: &self.assert_origins,
        }
    }

    /// Returns the separate retained assertion-origin payload transfer.
    pub const fn assert_origin_storage(&self) -> SemanticKirAssertOriginStorageV1 {
        self.assert_origins.storage
    }

    /// Borrows the complete source-only launch roster used during materialization.
    pub const fn source_launch(&self) -> &crate::ProductionSourceLaunchRosterV1 {
        &self.source_launch
    }

    /// Compiler custody grants no artifact, verification-policy or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Scoped source/ranked correspondence reports for one exact borrowed N owner.
///
/// The existing reports do not establish indexed-address or complete operational
/// equivalence, termination, independent external-reference proof, or artifact
/// authority. This borrow does not attach ranked custody or admit any O graph.
/// No constructor, Clone, serialization or final-receipt conversion exists.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionBorrowedRankedCorrespondenceV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionBorrowedRankedCorrespondenceV1<'static>>();
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionBorrowedRankedCorrespondenceV1,
///     ProductionPreRankedKirOwnerV1, ProductionRankedSemanticProjectionRootV1,
///     ProductionSemanticKirErrorV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape<'a>(source: &'a ProductionPreRankedKirOwnerV1,
///     roots: &'a [ProductionRankedSemanticProjectionRootV1],
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> Result<&'a ProductionBorrowedRankedCorrespondenceV1<'a>, ProductionSemanticKirErrorV1>
/// {
///     source.with_borrowed_ranked_correspondence_v1(roots, budget, |checked, _| Ok(checked))
/// }
/// ```
pub struct ProductionBorrowedRankedCorrespondenceV1<'scope> {
    materialized: &'scope ProductionPreRankedKirOwnerV1,
    roots: &'scope [ProductionRankedSemanticProjectionRootV1],
    reports: &'scope [ProductionMirPlironTranslationValidationV1],
}

impl ProductionBorrowedRankedCorrespondenceV1<'_> {
    /// Exact source/N owner checked by this scope, never a rematerialized copy.
    pub const fn materialized(&self) -> &ProductionPreRankedKirOwnerV1 {
        self.materialized
    }

    /// Exact original executable N, not a bound or optimized replacement.
    pub const fn executable(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.materialized.executable()
    }

    /// Exact ordered caller roster; its borrow retains identity beyond names.
    pub const fn roots(&self) -> &[ProductionRankedSemanticProjectionRootV1] {
        self.roots
    }

    /// Number of source roots checked before this callback was entered.
    pub const fn root_count(&self) -> usize {
        self.roots.len()
    }

    /// Existing inert translation report for the same root ordinal.
    pub fn report(&self, ordinal: usize) -> Option<&ProductionMirPlironTranslationValidationV1> {
        self.reports.get(ordinal)
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Validates a full borrowed ranked roster against this exact source and N.
    /// No source lowering, executable copy, O substitution or consuming ranked
    /// attachment occurs. The callback receives the same caller budget.
    ///
    /// New wrapper/header and actual report-Vec capacity are paid on that ledger;
    /// reports contain only fixed-size numeric fields. Existing source/SSA replay,
    /// roster and translation internals keep their separately bounded source-phase
    /// resource domains, not a newly claimed canonical whole-analysis meter.
    /// Reports drop before incoming-floor restoration on Ok, Err and unwind;
    /// work and first-failure history remain sticky. Caller-owned history and
    /// retained owner reservations must not be replaced in place.
    /// Before entry the caller must reserve BOTH this owner's executable-storage
    /// and assertion-origin payload receipts, plus any separate SSA receipt.
    /// This method charges only its new scope; it does not supply that entry
    /// reservation. Cleanup/accounting failure takes precedence over callback
    /// errors or panic; a panic payload resumes unchanged after valid cleanup.
    pub fn with_borrowed_ranked_correspondence_v1<T>(
        &self,
        roots: &[ProductionRankedSemanticProjectionRootV1],
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        body: impl for<'scope> FnOnce(
            &ProductionBorrowedRankedCorrespondenceV1<'scope>,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        validate_source_ranked_roster_v1(&self.semantic_ssa, &self.source_launch, roots)?;
        let floor = budget.storage();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            budget
                .charge_work(2)
                .map_err(SemanticKirAssertOriginErrorV1::from)?;
            let header = std::mem::size_of::<ProductionBorrowedRankedCorrespondenceV1<'_>>()
                .checked_add(std::mem::size_of::<
                    Vec<ProductionMirPlironTranslationValidationV1>,
                >())
                .ok_or(SemanticKirAssertOriginErrorV1::Resource(
                    AssertOriginResourceV1::Arithmetic,
                ))?;
            budget
                .reserve_storage(header)
                .map_err(SemanticKirAssertOriginErrorV1::from)?;
            let mut reports = Vec::new();
            assert_origin_reserve_v1(&mut reports, roots.len(), budget)?;
            for root in roots {
                budget
                    .charge_work(1)
                    .map_err(SemanticKirAssertOriginErrorV1::from)?;
                let report = validate_materialized_ranked_root_v1(
                    self.semantic_ssa.source_semantic(),
                    self.executable.module(),
                    &self.correspondence,
                    root.function_name(),
                    root,
                    self.limits.max_operations,
                )?;
                assert_origin_push_v1(&mut reports, report, budget)?;
            }
            let checked = ProductionBorrowedRankedCorrespondenceV1 {
                materialized: self,
                roots,
                reports: &reports,
            };
            body(&checked, budget)
        }));
        let cleanup = budget
            .storage()
            .checked_sub(floor)
            .ok_or(SemanticKirAssertOriginErrorV1::Resource(
                AssertOriginResourceV1::Accounting,
            ))
            .and_then(|retained| {
                budget
                    .release_storage(retained)
                    .map_err(SemanticKirAssertOriginErrorV1::from)
            });
        cleanup?;
        match outcome {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

fn validate_materialized_ranked_root_v1(
    semantic: &AdmittedInertSemanticMirV1,
    executable: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    function_name: &str,
    root: &ProductionRankedSemanticProjectionRootV1,
    max_operations: usize,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionSemanticKirErrorV1> {
    validate_mir_pliron_translation_with_semantic_v1(
        Some(semantic),
        executable,
        correspondence,
        function_name,
        &root.lowering,
        &root.access_sources,
        &root.executable_effect_sources,
        max_operations,
    )
    .map_err(ProductionSemanticKirErrorV1::MirPlironTranslation)
}

/// Ranked checks attached to the exact previously materialized graph.
/// Unlike the legacy receipt this cannot cause source re-materialization.
#[must_use = "dropping the receipt abandons ranked and executable custody"]
pub struct ProductionMaterializedRankedModuleReceiptV1 {
    materialized: ProductionPreRankedKirOwnerV1,
    roots: Box<[ProductionRankedSemanticProjectionRootV1]>,
}

impl ProductionMaterializedRankedModuleReceiptV1 {
    /// Checks the complete root bijection and exact source execution layout,
    /// then validates existing borrowed ranked correspondence against this owner.
    pub fn from_unvalidated_projection_roster_candidate(
        materialized: ProductionPreRankedKirOwnerV1,
        roots: Vec<ProductionRankedSemanticProjectionRootV1>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        validate_source_ranked_roster_v1(
            &materialized.semantic_ssa,
            &materialized.source_launch,
            &roots,
        )?;
        Ok(Self {
            materialized,
            roots: roots.into_boxed_slice(),
        })
    }

    /// Returns the retained complete ranked root count.
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }
    /// Compiler custody grants no artifact, verification-policy or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
enum RetainedProductionKirModuleV1 {
    Legacy(Module),
    Connected {
        executable: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        storage: fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV12,
        assert_origins: SealedAssertOriginsV1,
        source_launch: crate::ProductionSourceLaunchRosterV1,
    },
}

impl RetainedProductionKirModuleV1 {
    const fn module(&self) -> &Module {
        match self {
            Self::Legacy(module) => module,
            Self::Connected { executable, .. } => executable.module(),
        }
    }
}

impl std::ops::Deref for RetainedProductionKirModuleV1 {
    type Target = Module;
    fn deref(&self) -> &Module {
        self.module()
    }
}

impl ProductionSemanticKirOwnerV1 {
    /// Attaches checked ranked custody without invoking executable lowering.
    /// Existing `verify_equivalence` remains an explicit reconstruction audit.
    /// Graph and origin receipts remain caller-reserved across this consuming
    /// attachment. Any preexisting SSA occurrence-capture receipt is separate
    /// and remains reserved until its source owner is dropped, including when
    /// receipt validation or attachment fails.
    pub fn try_attach_materialized_ranked_checks(
        receipt: ProductionMaterializedRankedModuleReceiptV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let ProductionMaterializedRankedModuleReceiptV1 {
            materialized,
            roots,
        } = receipt;
        let ProductionPreRankedKirOwnerV1 {
            semantic_ssa,
            executable,
            executable_storage,
            assert_origins,
            source_launch,
            canonical_kernel_ir,
            correspondence,
            limits,
            launch_roots,
        } = materialized;
        let semantic = semantic_ssa.source_semantic();
        let mut generic_checks = Vec::with_capacity(roots.len());
        for root in roots.into_vec() {
            let function_name = root.function_name().to_owned();
            let translation_validation = validate_materialized_ranked_root_v1(
                semantic,
                executable.module(),
                &correspondence,
                &function_name,
                &root,
                limits.max_operations,
            )?;
            generic_checks.push(RetainedGenericKernelChecksV1 {
                selected_root: root.selected_root,
                launch_rank: root.launch_rank,
                semantic_sha256: *semantic.semantic_sha256().as_bytes(),
                function_name,
                ranked_ir: root.ranked_ir.into_boxed_str(),
                lowering: root.lowering,
                access_sources: root.access_sources,
                executable_effect_sources: root.executable_effect_sources,
                translation_validation,
            });
        }
        Ok(Self {
            semantic_ssa,
            module: RetainedProductionKirModuleV1::Connected {
                executable,
                storage: executable_storage,
                assert_origins,
                source_launch,
            },
            canonical_kernel_ir,
            correspondence,
            limits,
            launch_roots: Some(launch_roots),
            generic_checks: generic_checks.into_boxed_slice(),
        })
    }

    /// The exact pre-ranked graph, when construction used the new production
    /// stage. Legacy standalone constructors do not synthesize this custody.
    pub const fn pre_ranked_executable(
        &self,
    ) -> Option<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12> {
        match &self.module {
            RetainedProductionKirModuleV1::Connected { executable, .. } => Some(executable),
            RetainedProductionKirModuleV1::Legacy(_) => None,
        }
    }

    /// Returns the retained connected graph's original allocation receipt.
    pub const fn pre_ranked_executable_storage(
        &self,
    ) -> Option<fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV12> {
        match &self.module {
            RetainedProductionKirModuleV1::Connected { storage, .. } => Some(*storage),
            RetainedProductionKirModuleV1::Legacy(_) => None,
        }
    }

    /// Borrows retained assertion origins only on the connected production path.
    pub const fn pre_ranked_assert_origins(&self) -> Option<SemanticKirAssertOriginsV1<'_>> {
        match &self.module {
            RetainedProductionKirModuleV1::Connected {
                executable,
                assert_origins,
                ..
            } => Some(SemanticKirAssertOriginsV1 {
                executable,
                semantic_ssa: &self.semantic_ssa,
                origins: assert_origins,
            }),
            RetainedProductionKirModuleV1::Legacy(_) => None,
        }
    }

    /// Returns the separate retained-origin floor alongside the graph receipt.
    pub const fn pre_ranked_assert_origin_storage(
        &self,
    ) -> Option<SemanticKirAssertOriginStorageV1> {
        match &self.module {
            RetainedProductionKirModuleV1::Connected { assert_origins, .. } => {
                Some(assert_origins.storage)
            }
            RetainedProductionKirModuleV1::Legacy(_) => None,
        }
    }

    /// Borrows the source-only launch roster retained by the new production path.
    pub const fn source_launch_roster(&self) -> Option<&crate::ProductionSourceLaunchRosterV1> {
        match &self.module {
            RetainedProductionKirModuleV1::Connected { source_launch, .. } => Some(source_launch),
            RetainedProductionKirModuleV1::Legacy(_) => None,
        }
    }
}

impl ProductionCanonicalKernelIrV1 {
    fn from_module_ref(module: &Module) -> Result<Self, ProductionSemanticKirErrorV1> {
        macro_rules! canonicalize {
            ($encode:path, $owner:ty, $variant:ident, $error:ident, $canonical_error:ident) => {{
                let bytes = $encode(module).map_err(|error| {
                    ProductionSemanticKirErrorV1::$error($canonical_error::Encode(error))
                })?;
                let (owner, inverse) = <$owner>::from_canonical_bytes_with_module(bytes)
                    .map_err(ProductionSemanticKirErrorV1::$error)?;
                if &inverse != module {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                Ok(Self::$variant(owner))
            }};
        }
        if module_requires_kernel_ir_v11_v1(module) {
            canonicalize!(
                fe2o3_kernel_ir::encode_module_v11,
                VerifiedCanonicalKernelIrV11,
                V11,
                CanonicalKernelIrV11,
                VerifiedCanonicalKernelIrErrorV11
            )
        } else if module_requires_kernel_ir_v9_v1(module) {
            canonicalize!(
                fe2o3_kernel_ir::encode_module_v9,
                VerifiedCanonicalKernelIrV9,
                V9,
                CanonicalKernelIrV9,
                VerifiedCanonicalKernelIrErrorV9
            )
        } else {
            canonicalize!(
                fe2o3_kernel_ir::encode_module_v8,
                VerifiedCanonicalKernelIrV8,
                V8,
                CanonicalKernelIrV8,
                VerifiedCanonicalKernelIrErrorV8
            )
        }
    }
}

fn materialization_launch_roots_v1(
    semantic_ssa: &ProductionSemanticSsaOwnerV1,
    source_launch: &crate::ProductionSourceLaunchRosterV1,
) -> Result<Box<[RetainedRankedLaunchRootV1]>, ProductionSemanticKirErrorV1> {
    semantic_ssa
        .verify_replay()
        .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
    let semantic = semantic_ssa.source_semantic();
    if source_launch.semantic_sha256() != semantic.semantic_sha256().as_bytes()
        || source_launch.roots().len() != semantic.roots().len()
        || source_launch
            .roots()
            .iter()
            .zip(semantic.roots())
            .any(|(row, root)| row.selected_root() != *root)
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(source_launch
        .roots()
        .iter()
        .map(|root| {
            let layout = root.layout();
            RetainedRankedLaunchRootV1 {
                selected_root: root.selected_root(),
                launch_rank: root.source_rank(),
                global_extents: layout.global_extents(),
                workgroup_extents: layout.workgroup_extents(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn validate_source_ranked_roster_v1(
    semantic_ssa: &ProductionSemanticSsaOwnerV1,
    source_launch: &crate::ProductionSourceLaunchRosterV1,
    roots: &[ProductionRankedSemanticProjectionRootV1],
) -> Result<(), ProductionSemanticKirErrorV1> {
    semantic_ssa
        .verify_replay()
        .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
    if roots.is_empty() || roots.len() != source_launch.roots().len() {
        return Err(unsupported(
            0,
            None,
            None,
            "ranked projection roster is not a complete semantic root bijection",
        ));
    }
    let mut names = BTreeSet::new();
    for (root, source) in roots.iter().zip(source_launch.roots()) {
        if root.selected_root != source.selected_root()
            || root.launch_rank != source.source_rank()
            || !names.insert(root.function_name())
        {
            return Err(unsupported(
                root.selected_root.index(),
                None,
                None,
                "ranked projection roster has a duplicate, missing, or invalid root identity",
            ));
        }
        let layout = source.layout();
        let expected = ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: layout.grid_identity(),
            global_extents: layout.global_extents(),
            workgroup_extents: layout.workgroup_extents(),
            subgroup_size: layout.subgroup_size(),
            full_physical_workgroups: layout.full_physical_workgroups(),
        };
        if root
            .lowering
            .kernel()
            .blocks()
            .first()
            .and_then(|block| block.operations().first())
            != Some(&expected)
        {
            return Err(unsupported(
                root.selected_root.index(),
                None,
                None,
                "ranked execution layout changed after executable materialization",
            ));
        }
        validate_borrowed_ranked_semantic_projection_candidate_with_generated_effects_v1(
            semantic_ssa.source_owner(),
            root.selected_root,
            &root.lowering,
            &root.ranked_ir,
            &root.access_sources,
            &root.executable_effect_sources,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_pre_ranked_legacy_tests_v1.rs"]
mod pre_ranked_legacy_tests_v1;
