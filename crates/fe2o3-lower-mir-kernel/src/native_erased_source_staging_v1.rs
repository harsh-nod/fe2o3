// Same fixed row copier for both owning source routes; it mints no proof.
fn copy_native_ranked_staging_commitments_v1(
    receipts: &[fe2o3_pliron::ProductionPolicyCheckedRefinementStagingV2],
    budget: &mut Budget<'_>,
) -> Result<
    (
        Vec<NativeRankedStagingCommitmentV1>,
        NativeRankedStagingStorageV1,
    ),
    NativeSourceReplayErrorV1,
> {
    budget.charge_work(3)?;
    let header = std::mem::size_of::<Vec<NativeRankedStagingCommitmentV1>>();
    let requested = receipts
        .len()
        .checked_mul(std::mem::size_of::<NativeRankedStagingCommitmentV1>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(header.checked_add(requested).ok_or(Resource::Arithmetic)?)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(receipts.len())
        .map_err(|_| Resource::Allocation)?;
    let capacity = rows
        .capacity()
        .checked_mul(std::mem::size_of::<NativeRankedStagingCommitmentV1>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        capacity
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    for receipt in receipts {
        budget.charge_work(289)?;
        let toolchain = receipt.toolchain();
        rows.push(NativeRankedStagingCommitmentV1 {
            digests: [
                *receipt.receipt_identity().digest().as_bytes(),
                *receipt
                    .binding()
                    .normalized_obligation_effect_ir_hash()
                    .as_bytes(),
                *receipt.signer_identity().as_bytes(),
                *receipt.execution_identity().as_bytes(),
                *toolchain.verus_executable().as_bytes(),
                *toolchain.verus_configuration().as_bytes(),
                *toolchain.solver_executable().as_bytes(),
                *toolchain.solver_configuration().as_bytes(),
                *toolchain.runtime_closure().as_bytes(),
            ],
        });
    }
    Ok((
        rows,
        NativeRankedStagingStorageV1(header.checked_add(capacity).ok_or(Resource::Arithmetic)?),
    ))
}

impl crate::ProductionUnitLocalErasedSourceOwnerV1 {
    /// Copies one exact original ranked root's ordered staging commitments.
    /// These inert values do not authenticate signer or execution provenance.
    /// The complete original/ranked/E/map floor must already be reserved.
    /// Work is 13 + 289 per row; success/error/unwind restore the incoming floor.
    /// Reserve the returned header/actual-capacity receipt before allocating.
    pub fn ranked_staging_commitments_v1(
        &self,
        ordinal: usize,
        semantic_root: u32,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            Vec<NativeRankedStagingCommitmentV1>,
            NativeRankedStagingStorageV1,
        ),
        NativeSourceReplayErrorV1,
    > {
        budget.charge_work(6)?;
        if budget.storage() < self.retained_storage_floor_v1() {
            return Err(Resource::Accounting.into());
        }
        native_source_transfer_v1(budget, |budget| {
            budget.charge_work(4)?;
            let roots = self
                .original_source()
                .semantic_ssa()
                .source_semantic()
                .roots();
            let retained = self
                .roots
                .get(ordinal)
                .ok_or(NativeSourceReplayErrorV1::Mismatch(
                    "retained erased staging root",
                ))?;
            if self.roots.len() != roots.len()
                || roots.get(ordinal).map(|root| root.index()) != Some(semantic_root)
                || retained.selected_root.index() != semantic_root
            {
                return Err(NativeSourceReplayErrorV1::Mismatch(
                    "retained erased staging root",
                ));
            }
            copy_native_ranked_staging_commitments_v1(
                retained
                    .lowering
                    .retained_policy_checked_refinement_staging(),
                budget,
            )
        })
    }
}
