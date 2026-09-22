//! Shared original-N packet assembly; owning endpoints stay route-specific.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum NativeSourceRefV1<'a> {
    Direct(&'a ProductionSemanticKirOwnerV1),
    Erased(&'a fe2o3_lower_mir_kernel::ProductionUnitLocalErasedSourceOwnerV1),
}

impl<'a> NativeSourceRefV1<'a> {
    fn verify(self, budget: &mut Budget<'_>) -> Result<(), E> {
        match self {
            Self::Direct(source) => source
                .verify_equivalence_with_budget_v1(budget)
                .map_err(E::Source),
            Self::Erased(source) => source.verify_equivalence(budget).map_err(E::Source),
        }
    }

    fn semantic(self) -> &'a fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1 {
        match self {
            Self::Direct(source) => source.semantic().semantic(),
            Self::Erased(source) => source.original_source().semantic_ssa().source_semantic(),
        }
    }

    fn native(self) -> Result<&'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12, E> {
        match self {
            Self::Direct(source) => source
                .pre_ranked_executable()
                .ok_or(E::Mismatch("missing native source owner")),
            Self::Erased(source) => Ok(source.original_source().executable()),
        }
    }

    fn launch(self) -> Result<&'a fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1, E> {
        match self {
            Self::Direct(source) => source
                .source_launch_roster()
                .ok_or(E::Mismatch("missing source launch roster")),
            Self::Erased(source) => Ok(source.original_source().source_launch()),
        }
    }

    pub(super) fn check_roster(
        self,
        roots: &[(u32, &str, &str)],
        budget: &mut Budget<'_>,
    ) -> Result<(), E> {
        let source = match self {
            Self::Direct(source) => {
                return check_native_source_ranked_roster_v1(source, roots, budget)
                    .map_err(E::SourceJoin);
            }
            Self::Erased(source) => source,
        };
        budget.charge_work(4)?;
        if budget.storage() < source.retained_storage_floor_v1() {
            return Err(Resource::Accounting.into());
        }
        let semantic_roots = self.semantic().roots();
        let launches = source.original_source().source_launch().roots();
        if roots.is_empty()
            || roots.len() != source.ranked_root_count()
            || roots.len() != semantic_roots.len()
            || roots.len() != launches.len()
        {
            return Err(E::Mismatch("complete retained erased ranked roster"));
        }
        for (ordinal, &(root, name, text)) in roots.iter().enumerate() {
            let candidate = source
                .ranked_candidate_v1(ordinal, budget)
                .map_err(E::Source)?
                .ok_or(E::Mismatch("complete retained erased ranked roster"))?;
            budget.charge_work(
                6usize
                    .checked_add(name.len())
                    .and_then(|n| n.checked_add(candidate.kernel().function_name().len()))
                    .and_then(|n| n.checked_add(text.len()))
                    .and_then(|n| n.checked_add(candidate.ranked_ir().len()))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if root != semantic_roots[ordinal].index()
                || root != candidate.semantic_root()
                || root != launches[ordinal].selected_root().index()
                || candidate.launch_rank() != launches[ordinal].source_rank()
                || name != candidate.kernel().function_name()
                || text != candidate.ranked_ir()
            {
                return Err(E::Mismatch("exact retained erased ranked root/text"));
            }
        }
        Ok(())
    }

    fn staging(
        self,
        ordinal: usize,
        root: u32,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            Vec<fe2o3_lower_mir_kernel::NativeRankedStagingCommitmentV1>,
            fe2o3_lower_mir_kernel::NativeRankedStagingStorageV1,
        ),
        E,
    > {
        match self {
            Self::Direct(source) => {
                native_source_ranked_staging_commitments_v1(source, ordinal, root, budget)
                    .map_err(E::SourceJoin)
            }
            Self::Erased(source) => source
                .ranked_staging_commitments_v1(ordinal, root, budget)
                .map_err(E::SourceJoin),
        }
    }

    fn candidates(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            Vec<fe2o3_lower_mir_kernel::NativeRankedSourceCandidateV1<'a>>,
            usize,
        ),
        E,
    > {
        let source = match self {
            Self::Direct(source) => {
                let (rows, storage) =
                    native_source_ranked_candidates_v1(source, budget).map_err(E::SourceJoin)?;
                return Ok((rows, storage.retained_storage()));
            }
            Self::Erased(source) => source,
        };
        budget.charge_work(8)?;
        if budget.storage() < source.retained_storage_floor_v1() {
            return Err(Resource::Accounting.into());
        }
        with_native_lineage_transfer_v1(budget, |budget| {
            let header = std::mem::size_of::<
                Vec<fe2o3_lower_mir_kernel::NativeRankedSourceCandidateV1<'_>>,
            >();
            budget.reserve_storage(header)?;
            let count = source.ranked_root_count();
            let mut rows = reserved_vec(count, budget)?;
            for ordinal in 0..count {
                rows.push(
                    source
                        .ranked_candidate_v1(ordinal, budget)
                        .map_err(E::Source)?
                        .ok_or(E::Mismatch("complete retained erased ranked roster"))?,
                );
            }
            let retained = rows
                .capacity()
                .checked_mul(std::mem::size_of::<
                    fe2o3_lower_mir_kernel::NativeRankedSourceCandidateV1<'_>,
                >())
                .and_then(|n| n.checked_add(header))
                .ok_or(Resource::Arithmetic)?;
            Ok((rows, retained))
        })
    }
}

pub(super) fn with_native_lineage_transfer_v1<T>(
    budget: &mut Budget<'_>,
    next: impl FnOnce(&mut Budget<'_>) -> Result<T, E>,
) -> Result<T, E> {
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| next(budget)));
    if token != budget.work_ledger_identity_v1() || slot != budget as *const Budget<'_> as usize {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    let Some(release) = budget.storage().checked_sub(floor) else {
        drop(result);
        return Err(Resource::Accounting.into());
    };
    if let Err(error) = budget.release_storage(release) {
        drop(result);
        return Err(error.into());
    }
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

pub(super) struct NativeSourcePacketPartsV1<T> {
    pub(super) proof: T,
    pub(super) native_module: Vec<u8>,
    pub(super) retained: usize,
}

pub(super) fn prepare_native_source_packet_v1<'w, T>(
    source: NativeSourceRefV1<'_>,
    ranked: &AuthenticatedRankedVerificationRosterV1,
    wrapper_header: fn() -> Result<usize, E>,
    budget: &mut Budget<'w>,
    replay: impl for<'p> FnOnce(
        NativeCompilerRankedRecipeSourceProofInputsV1<'p>,
        &mut Budget<'w>,
    ) -> Result<(T, usize), E>,
) -> Result<NativeSourcePacketPartsV1<T>, E> {
    source.verify(budget)?;
    let native = source.native()?;
    let launch = source.launch()?;
    let semantic = source.semantic();
    let count = semantic.roots().len();
    budget.charge_work(3)?;
    if ranked.root_count() != count
        || launch.roots().len() != count
        || !ranked.every_functional_verification_is_coherent()
    {
        return Err(E::Mismatch("complete coherent source/ranked roster"));
    }
    let mut joins = reserved_vec(count, budget)?;
    for root in ranked.roots() {
        budget.charge_work(2 + root.export_symbol().len())?;
        let export = std::str::from_utf8(root.export_symbol())
            .map_err(|_| E::Mismatch("ranked export encoding"))?;
        joins.push((
            root.semantic_root().index(),
            export,
            root.verification().middle_end_evidence().ranked_ir(),
        ));
    }
    source.check_roster(&joins, budget)?;
    // Check required signed custody before allocating any native envelopes.
    for root in ranked.roots() {
        budget.charge_work(1)?;
        if root.verification().aggregate_verus_execution().is_none() {
            return Err(E::MissingSignedRankedReceipt {
                root: root.semantic_root().index(),
            });
        }
    }
    let mut payloads = reserved_vec(count, budget)?;
    let mut launches = reserved_vec(count, budget)?;
    let mut order = reserved_vec(count, budget)?;
    for &index in ranked.canonical_kernel_order() {
        budget.charge_work(1)?;
        order.push(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
    }
    for (ordinal, root) in ranked.roots().iter().enumerate() {
        budget.charge_work(80)?;
        let launch_root = launch.roots()[ordinal];
        if root.semantic_root() != semantic.roots()[ordinal]
            || root.semantic_root() != launch_root.selected_root()
            || root.semantic_root_identity() != launch_root.semantic_root_identity()
            || *root.kernel_binding() != launch_root.kernel_binding()
            || root.source_rank() != launch_root.source_rank()
        {
            return Err(E::Mismatch("source launch/ranked root"));
        }
        let induction = Induction::from_report(root.verification().semantic_u32_induction())
            .map_err(E::Induction)?;
        let execution = root.verification().aggregate_verus_execution().ok_or(
            E::MissingSignedRankedReceipt {
                root: root.semantic_root().index(),
            },
        )?;
        let signed = Signed::from_execution(execution).map_err(E::Signed)?;
        budget.reserve_storage(
            induction
                .canonical_bytes()
                .len()
                .checked_add(signed.canonical_bytes().len())
                .and_then(|n| n.checked_mul(2))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let (source_rows, staging_receipt) =
            source.staging(ordinal, root.semantic_root().index(), budget)?;
        budget.reserve_storage(staging_receipt.retained_storage())?;
        let mut staging = reserved_vec(source_rows.len(), budget)?;
        for row in source_rows {
            budget.charge_work(289)?;
            staging.push(NativeCompilerStagingCommitmentV1 {
                receipt: row.receipt(),
                effect: row.effect(),
                signer: row.signer(),
                execution: row.execution(),
                toolchain: row.toolchain(),
            });
        }
        budget.release_storage(staging_receipt.retained_storage())?;
        payloads.push((induction, signed, staging));
        launches.push(ProductionSourceLaunchRootInputV1::new(
            root.logical_name(),
            *root.kernel_binding(),
            launch_root.source_launch(),
        ));
    }
    let semantic_identity = *semantic.semantic_sha256().as_bytes();
    let (catalog, catalog_storage) =
        Catalog::from_rows_with_budget(semantic_identity, &[], &[], budget).map_err(E::Catalog)?;
    budget.reserve_storage(catalog_storage.retained_storage())?;
    let graph_bytes = native.canonical().canonical_bytes();
    let catalog_bytes = catalog.canonical_bytes();
    let subject = InertNativeNeutralSubjectV1::new(
        *native.canonical().identity().digest(),
        u64::try_from(graph_bytes.len()).map_err(|_| Resource::Arithmetic)?,
        *catalog.digest(),
        u64::try_from(catalog_bytes.len()).map_err(|_| Resource::Arithmetic)?,
    )
    .map_err(E::Subject)?;
    let native_len = 112usize
        .checked_add(graph_bytes.len())
        .and_then(|n| n.checked_add(catalog_bytes.len()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(native_len)?;
    budget.charge_work(native_len)?;
    let native_module =
        encode_native_neutral_module_v1(&subject, graph_bytes, catalog_bytes).map_err(E::Native)?;
    budget.reserve_storage(
        native_module
            .capacity()
            .checked_sub(native_len)
            .ok_or(Resource::Accounting)?,
    )?;
    let mut rows = reserved_vec(count, budget)?;
    for (ordinal, root) in ranked.roots().iter().enumerate() {
        budget.charge_work(4)?;
        let workgroup = launch.roots()[ordinal]
            .source_launch()
            .exact_workgroup()
            .ok_or(E::Mismatch("missing exact source workgroup"))?;
        rows.push(RootInput {
            semantic_root: root.semantic_root().index(),
            semantic_root_identity: *root.semantic_root_identity().as_bytes(),
            kernel_binding: *root.kernel_binding(),
            source_rank: root.source_rank(),
            workgroup,
            logical_name: root.logical_name(),
            export_symbol: joins[ordinal].1,
            kernel_id: joins[ordinal].1,
            payload: root.verification().middle_end_evidence().canonical_bytes(),
        });
    }
    let identity = *ranked.canonical_roster_identity().as_bytes();
    let middle = encode_roster(
        Kind::MiddleEnd,
        semantic_identity,
        subject,
        identity,
        &order,
        &rows,
        budget,
    )?;
    for (row, (induction, _, _)) in rows.iter_mut().zip(&payloads) {
        budget.charge_work(1)?;
        row.payload = induction.canonical_bytes();
    }
    let correspondence = encode_roster(
        Kind::Correspondence,
        semantic_identity,
        subject,
        identity,
        &order,
        &rows,
        budget,
    )?;
    for (row, (_, signed, _)) in rows.iter_mut().zip(&payloads) {
        budget.charge_work(1)?;
        row.payload = signed.canonical_bytes();
    }
    let verus = encode_roster(
        Kind::VerusExecution,
        semantic_identity,
        subject,
        identity,
        &order,
        &rows,
        budget,
    )?;
    let mut staging_roots = reserved_vec(count, budget)?;
    for (root, (_, _, staging)) in ranked.roots().iter().zip(&payloads) {
        budget.charge_work(1)?;
        staging_roots.push(NativeCompilerRootStagingV1 {
            semantic_root: root.semantic_root().index(),
            commitments: staging,
        });
    }
    let (candidates, candidate_storage) = source.candidates(budget)?;
    budget.reserve_storage(candidate_storage)?;
    if candidates.len() != count {
        return Err(E::Mismatch("complete typed ranked candidate roster"));
    }
    budget.reserve_storage(std::mem::size_of::<Vec<(Vec<u8>, Vec<u8>)>>())?;
    let mut encoded = reserved_vec(count, budget)?;
    for candidate in &candidates {
        let (recipe, storage) = fe2o3_pliron::encode_production_ranked_recipe_v1(
            candidate.kernel(),
            budget,
        )
        .map_err(|error| {
            E::Replay(fe2o3_verifier::NativeCompilerSourceProofErrorV1::RankedRecipeWire(error))
        })?;
        budget.reserve_storage(storage.retained_storage())?;
        let (rows, storage) = fe2o3_lower_mir_kernel::encode_production_ranked_source_rows_v1(
            candidate.access_sources(),
            candidate.executable_effect_sources(),
            budget,
        )
        .map_err(|error| {
            E::Replay(fe2o3_verifier::NativeCompilerSourceProofErrorV1::RankedSourceRowsWire(error))
        })?;
        budget.reserve_storage(storage.retained_storage())?;
        encoded.push((recipe, rows));
    }
    let mut ranked_roots = reserved_vec(count, budget)?;
    for ((candidate, root), (recipe, rows)) in candidates.iter().zip(ranked.roots()).zip(&encoded) {
        budget.charge_work(2)?;
        ranked_roots.push(NativeCompilerRankedRecipeRootV1 {
            semantic_root: candidate.semantic_root(),
            launch_rank: candidate.launch_rank(),
            recipe_bytes: recipe,
            source_rows_bytes: rows,
            ranked_ir: candidate.ranked_ir(),
            effect_receipts: root.verification().effect_receipts(),
        });
    }
    let (proof, proof_storage) = replay(
        NativeCompilerRankedRecipeSourceProofInputsV1 {
            source: NativeCompilerSourceProofInputsV1 {
                semantic_mir: semantic.canonical_encoding(),
                native_module: &native_module,
                middle_end_roster: middle.canonical_bytes(),
                correspondence_roster: correspondence.canonical_bytes(),
                verus_roster: verus.canonical_bytes(),
                launch_inputs: &launches,
                staging_roots: &staging_roots,
            },
            ranked_roots: &ranked_roots,
        },
        budget,
    )?;
    budget.reserve_storage(proof_storage)?;
    let header = wrapper_header()?;
    budget.reserve_storage(header)?;
    let retained = header
        .checked_add(native_module.capacity())
        .and_then(|n| n.checked_add(proof_storage))
        .ok_or(Resource::Arithmetic)?;
    drop(rows);
    drop(staging_roots);
    drop(launches);
    drop(joins);
    drop(ranked_roots);
    drop(encoded);
    drop(candidates);
    Ok(NativeSourcePacketPartsV1 {
        proof,
        native_module,
        retained,
    })
}
