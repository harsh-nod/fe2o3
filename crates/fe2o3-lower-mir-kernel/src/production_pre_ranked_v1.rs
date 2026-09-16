// This shard shares private correspondence and lowering helpers with its owner.

include!("production_pre_ranked_local_frames_v1.rs");

/// Construction failure before ranked checking starts.
#[derive(Debug)]
pub enum ProductionPreRankedKirErrorV1 {
    /// Existing source, SSA, materialization or correspondence rejection.
    Lowering(ProductionSemanticKirErrorV1),
    /// Connected V12 wire, semantic verification or resource rejection.
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12),
    /// Exact checked local-frame classification or resource rejection.
    LocalFrame(fe2o3_kernel_ir::LocalFrameErrorV1),
}

impl fmt::Display for ProductionPreRankedKirErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(error) => error.fmt(formatter),
            Self::Canonical(error) => error.fmt(formatter),
            Self::LocalFrame(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProductionPreRankedKirErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lowering(error) => Some(error),
            Self::Canonical(error) => Some(error),
            Self::LocalFrame(error) => Some(error),
        }
    }
}

impl From<ProductionSemanticKirErrorV1> for ProductionPreRankedKirErrorV1 {
    fn from(error: ProductionSemanticKirErrorV1) -> Self {
        Self::Lowering(error)
    }
}

impl From<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>
    for ProductionPreRankedKirErrorV1
{
    fn from(error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Lowering(error.into())
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
    helper_memory: SealedHelperMemoryV1,
}

/// Borrowed helper facts from one immutable materialization and its validated
/// source/KIR correspondence. Construction requires every helper's combined
/// interprocedural effects to be complete and pure, including compiler ordering.
/// This does not establish determinism, termination, or value equivalence.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionEmptyEffectHelpersV1, ProductionPreRankedKirOwnerV1};
/// fn forge(owner: &ProductionPreRankedKirOwnerV1) {
///     let _ = ProductionEmptyEffectHelpersV1 { owner };
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ProductionEmptyEffectHelpersV1<'a> {
    owner: &'a ProductionPreRankedKirOwnerV1,
}

impl<'a> ProductionEmptyEffectHelpersV1<'a> {
    /// Work needed to scan the entire function roster, including root rows and
    /// repeated shared-helper associations. The view allocates no payload.
    pub fn scanned_function_count(self) -> usize {
        self.owner.correspondence.lowered_functions().len()
    }

    /// Preserves root owner, semantic function and physical function identity.
    /// Shared helpers can occur more than once, once for each owning root.
    pub fn iter(self) -> impl Iterator<Item = &'a SemanticKirFunctionCorrespondenceV1> {
        self.owner
            .correspondence
            .lowered_functions()
            .iter()
            .enumerate()
            .filter(|(index, row)| {
                row.role() == SemanticKirFunctionRoleV1::InternalHelper
                    && self.owner.helper_memory.is_raw_empty(*index)
            })
            .map(|(_, row)| row)
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Materializes the retained SSA plans once, using the full source launch
    /// roster. No ranked verification receipt is accepted at this stage.
    ///
    /// The ledger covers connected V12 admission and assertion-origin emission,
    /// sealing scratch, and retained payload, including their coexistence.
    /// Retained source MIR, SSA plans, launch rows, legacy correspondence/bytes
    /// and other lowering scratch keep their existing limits and are excluded.
    /// Entry-argument replay uses the independent limits in `limits`; its
    /// transient logical payload is not transferred to this canonical ledger.
    /// The incoming live floor is restored after failure drops or success transfer.
    /// Before another allocation, reserve `retained_analysis_storage_v1()` while
    /// this owner or its attached successor lives. Its separate graph, origin
    /// and helper receipts describe those same payloads, not additional copies.
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
        let mut helper_memory = SealedHelperMemoryV1::derive(
            CanonicalCallSubjectV1 {
                semantic_ssa: &semantic_ssa,
                executable: &executable,
                correspondence: &correspondence,
            },
            budget,
        )?;
        budget
            .charge_work(2)
            .map_err(ProductionSemanticKirErrorV1::from)?;
        helper_memory.analysis_storage = helper_memory_live_storage_v1(
            executable_storage.retained_storage(),
            assert_origins.storage.payload_storage(),
            helper_memory.storage.retained_storage(),
        )?;
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
            helper_memory,
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

    /// Borrows only facts established during this owner's exact materialization.
    pub const fn empty_effect_helpers(&self) -> ProductionEmptyEffectHelpersV1<'_> {
        ProductionEmptyEffectHelpersV1 { owner: self }
    }

    /// Returns the connected graph payload to reserve when continuing its ledger.
    pub const fn executable_storage(&self) -> fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV12 {
        self.executable_storage
    }

    /// Retained helper payload, to reserve with the graph and assertion origins.
    pub const fn helper_memory_storage_v1(&self) -> ProductionHelperMemoryStorageV1 {
        self.helper_memory.storage
    }

    /// Complete retained graph, assertion-origin and helper payload. Source SSA
    /// capture retains its existing separate caller reservation.
    pub const fn retained_analysis_storage_v1(&self) -> usize {
        self.helper_memory.analysis_storage
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
        helper_memory: SealedHelperMemoryV1,
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
            helper_memory,
        } = materialized;
        let semantic = semantic_ssa.source_semantic();
        let mut generic_checks = Vec::with_capacity(roots.len());
        for root in roots.into_vec() {
            let function_name = root.function_name().to_owned();
            let translation_validation = validate_mir_pliron_translation_with_semantic_v1(
                Some(semantic),
                executable.module(),
                &correspondence,
                &function_name,
                &root.lowering,
                &root.access_sources,
                &root.executable_effect_sources,
                limits.max_operations,
            )
            .map_err(ProductionSemanticKirErrorV1::MirPlironTranslation)?;
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
                helper_memory,
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

    /// Retained helper obligations move with connected graph custody. Legacy
    /// owners do not synthesize this receipt.
    pub const fn pre_ranked_helper_memory_storage_v1(
        &self,
    ) -> Option<ProductionHelperMemoryStorageV1> {
        match &self.module {
            RetainedProductionKirModuleV1::Connected { helper_memory, .. } => {
                Some(helper_memory.storage)
            }
            RetainedProductionKirModuleV1::Legacy(_) => None,
        }
    }

    /// Complete checked retained subtotal transferred with a connected owner.
    /// Source SSA occurrence capture remains a separate caller reservation.
    pub const fn pre_ranked_retained_analysis_storage_v1(&self) -> Option<usize> {
        match &self.module {
            RetainedProductionKirModuleV1::Connected { helper_memory, .. } => {
                Some(helper_memory.analysis_storage)
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
