// Source-bound schedule preparation only. Original collective and memory
// obligations stay live; this owner has no scalar or executable conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
enum ScopedTileOrderV29 {
    Blocked,
    Striped,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
struct ScopedTileScheduleInputV29 {
    order: ScopedTileOrderV29,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
struct ScopedTileSelectionV29 {
    root: usize,
    semantic_root: SemanticFunctionIdV1,
    insertion: usize,
    witness: LifecycleInsertionV29,
    tile: DeferredTileEventV29,
    order: ScopedTileOrderV29,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
struct PreparedScopedTileSourceV29 {
    selections: Vec<ScopedTileSelectionV29>,
    identity: [u8; 32],
    request: ScopedTileScheduleInputV29,
    pending: ProductionPendingScopedSourceOwnerV29,
    retained_storage: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
enum ScopedTileFailureKindV29 {
    MissingDonor,
    Resource(ArgumentResourceV1),
    Source,
    SemanticSsa,
    SourceLaunch,
    Canonical,
    Occurrences,
    Census,
    Geometry,
    NoTileOccurrences,
    ReplayMismatch,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
impl From<ArgumentResourceV1> for ScopedTileFailureKindV29 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Resource(error)
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
impl From<ProductionSemanticKirErrorV1> for ScopedTileFailureKindV29 {
    fn from(error: ProductionSemanticKirErrorV1) -> Self {
        let summary = match &error {
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(resource)
            | ProductionSemanticKirErrorV1::AssertOrigin(
                SemanticKirAssertOriginErrorV1::Resource(resource),
            ) => Self::Resource(*resource),
            ProductionSemanticKirErrorV1::SemanticSsa(_) => Self::SemanticSsa,
            _ => Self::Source,
        };
        // Raw errors may own strings, type trees or provenance after rollback.
        drop(error);
        summary
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
impl From<ScopedModuleErrorV29> for ScopedTileFailureKindV29 {
    fn from(error: ScopedModuleErrorV29) -> Self {
        use fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18 as Canonical;
        match error {
            ScopedModuleErrorV29::Source(error) => error.into(),
            error => {
                let summary = match &error {
                    ScopedModuleErrorV29::Canonical(Canonical::Encode(
                        fe2o3_kernel_ir::KernelIrEncodeError::WorkLimit(limit),
                    ))
                    | ScopedModuleErrorV29::Canonical(Canonical::Decode(
                        fe2o3_kernel_ir::KernelIrDecodeError::WorkLimit(limit),
                    ))
                    | ScopedModuleErrorV29::Canonical(Canonical::Decode(
                        fe2o3_kernel_ir::KernelIrDecodeError::Encode(
                            fe2o3_kernel_ir::KernelIrEncodeError::WorkLimit(limit),
                        ),
                    )) => Self::Resource(ArgumentResourceV1::Work(*limit)),
                    ScopedModuleErrorV29::Canonical(Canonical::Resource(resource))
                    | ScopedModuleErrorV29::Canonical(Canonical::Layout(
                        fe2o3_kernel_ir::StorageLayoutErrorV1::Resource(resource),
                    ))
                    | ScopedModuleErrorV29::Canonical(Canonical::Verification(
                        fe2o3_kernel_ir::BorrowedKernelIrVerificationErrorV1::Resource(resource),
                    ))
                    | ScopedModuleErrorV29::Canonical(Canonical::Decode(
                        fe2o3_kernel_ir::KernelIrDecodeError::Resource(resource),
                    ))
                    | ScopedModuleErrorV29::Occurrences(
                        fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(resource),
                    ) => Self::Resource(*resource),
                    ScopedModuleErrorV29::Canonical(Canonical::Encode(
                        fe2o3_kernel_ir::KernelIrEncodeError::Allocation,
                    ))
                    | ScopedModuleErrorV29::Canonical(Canonical::Decode(
                        fe2o3_kernel_ir::KernelIrDecodeError::Encode(
                            fe2o3_kernel_ir::KernelIrEncodeError::Allocation,
                        ),
                    )) => Self::Resource(ArgumentResourceV1::Allocation),
                    ScopedModuleErrorV29::Canonical(Canonical::Encode(
                        fe2o3_kernel_ir::KernelIrEncodeError::Overflow { .. },
                    ))
                    | ScopedModuleErrorV29::Canonical(Canonical::Decode(
                        fe2o3_kernel_ir::KernelIrDecodeError::Encode(
                            fe2o3_kernel_ir::KernelIrEncodeError::Overflow { .. },
                        ),
                    )) => Self::Resource(ArgumentResourceV1::Arithmetic),
                    ScopedModuleErrorV29::Canonical(_) => Self::Canonical,
                    ScopedModuleErrorV29::Occurrences(_) => Self::Occurrences,
                    ScopedModuleErrorV29::Source(_) => unreachable!(),
                };
                drop(error);
                summary
            }
        }
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn scoped_tile_attempt_v29<'work, T>(
    budget: &mut ArgumentBudgetV1<'work>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'work>) -> Result<T, ScopedTileFailureKindV29>,
) -> Result<T, ScopedTileFailureKindV29> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(budget)));
    if budget.work_ledger_identity_v1() != ledger {
        match result {
            Err(payload) => std::panic::resume_unwind(payload),
            other => {
                drop(other);
                return Err(ArgumentResourceV1::Accounting.into());
            }
        }
    }
    match result {
        Ok(Ok(value)) => Ok(value),
        other => {
            let cleanup = budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)
                .and_then(|extra| budget.release_storage(extra));
            match other {
                Ok(Err(error)) => {
                    cleanup?;
                    Err(error)
                }
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(Ok(_)) => unreachable!(),
            }
        }
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn scoped_tile_header_v29() -> Result<usize, ArgumentResourceV1> {
    // Pending is already paid; V18 admission pays its own retained header.
    // Reserve the largest owner envelope before taking Prepared custody.
    let candidate = size_of::<ScopedTileScalarCandidateV29>()
        .checked_sub(size_of::<fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>())
        .ok_or(ArgumentResourceV1::Accounting)?;
    size_of::<PreparedScopedTileSourceV29>()
        .max(size_of::<ScopedTileMaterializationFailureV29>())
        .max(candidate)
        .checked_sub(size_of::<ProductionPendingScopedSourceOwnerV29>())
        .ok_or(ArgumentResourceV1::Accounting)
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn scoped_tile_floor_v29(
    pending: &ProductionPendingScopedSourceOwnerV29,
    retained: usize,
    budget: &ArgumentBudgetV1<'_>,
) -> Result<(), ScopedTileFailureKindV29> {
    let inner = &pending.inner;
    if inner.source.input.ledger != budget.work_ledger_identity_v1()
        || inner.pending.ledger != budget.work_ledger_identity_v1()
        || budget.storage() < argument_sum_v1(&[retained, inner.capture.preexisting_storage()])?
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(())
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn scoped_tile_census_v29(
    pending: &ProductionPendingScopedSourceOwnerV29,
    order: ScopedTileOrderV29,
    budget: &mut ArgumentBudgetV1<'_>,
    mut visit: impl FnMut(
        ScopedTileSelectionV29,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ScopedTileFailureKindV29>,
) -> Result<usize, ScopedTileFailureKindV29> {
    use fe2o3_kernel_ir::ExecutionOperationV15 as Op;
    let inner = &pending.inner;
    let graph = inner.pending.graph.module();
    budget.charge_work(3)?;
    if inner.pending.roots.len() != inner.source.launch.roots().len()
        || inner.pending.roots.len() != graph.kernels.len()
    {
        return Err(ScopedTileFailureKindV29::SourceLaunch);
    }
    let mut count = 0;
    for (root_ordinal, root) in inner.pending.roots.iter().enumerate() {
        budget.charge_work(4)?;
        let launch = inner.source.launch.roots()[root_ordinal];
        let function = graph
            .functions
            .get(root.function_ordinal)
            .ok_or(ScopedTileFailureKindV29::Census)?;
        let body = function
            .body
            .as_ref()
            .ok_or(ScopedTileFailureKindV29::Census)?;
        budget.charge_work(argument_sum_v1(&[
            graph.kernels[root_ordinal].entry.as_str().len(),
            function.id.as_str().len(),
        ])?)?;
        if root.function_ordinal != root_ordinal
            || root.coordinates.root != launch.selected_root()
            || graph.kernels[root_ordinal].entry != function.id
        {
            return Err(ScopedTileFailureKindV29::SourceLaunch);
        }
        let before = count;
        for sidecar in &root.sidecars.rows {
            budget.charge_work(1)?;
            let events = sidecar
                .lifecycle_events
                .as_ref()
                .ok_or(ScopedTileFailureKindV29::Census)?;
            let source = lifecycle_source_row_v29(&root.coordinates, events, budget)?;
            for (event_ordinal, event) in events.rows.iter().enumerate() {
                budget.charge_work(1)?;
                let DeferredLifecycleKindV29::Tile(tile) = event.kind else {
                    continue;
                };
                let DeferredTileInputV29::Load {
                    workgroup,
                    input,
                    base,
                } = tile.input
                else {
                    continue;
                };
                budget.charge_work(argument_sum_v1(&[
                    root.insertions.len(),
                    body.blocks.len(),
                    16,
                ])?)?;
                tile.result_range()?;
                if tile.producer
                    != (ProductionCallOccurrenceV1 {
                        caller: source.instance,
                        block: event.block,
                    })
                {
                    return Err(ScopedTileFailureKindV29::Census);
                }
                if launch.source_launch().exact_workgroup() != Some([u32::from(tile.lanes), 1, 1])
                    || launch.layout().workgroup_extents() != [u64::from(tile.lanes), 1, 1]
                {
                    return Err(ScopedTileFailureKindV29::Geometry);
                }
                let mut matching = root.insertions.iter().enumerate().filter(|(_, row)| {
                    row.instance == source.instance && row.event == event_ordinal
                });
                let (insertion, witness) =
                    matching.next().ok_or(ScopedTileFailureKindV29::Census)?;
                if matching.next().is_some() || witness.after.count != 1 {
                    return Err(ScopedTileFailureKindV29::Census);
                }
                let span = root
                    .coordinates
                    .spans
                    .rows
                    .get(witness.source_span)
                    .ok_or(ScopedTileFailureKindV29::Census)?;
                if span.instance != source.instance
                    || !matches!(span.source, InstanceSpanSourceV1::Terminator(row)
                        if row.semantic_block == event.block && row.semantic_function == source.function)
                {
                    return Err(ScopedTileFailureKindV29::Census);
                }
                let mut blocks = body
                    .blocks
                    .iter()
                    .filter(|block| block.id == witness.after.block);
                let block = blocks.next().ok_or(ScopedTileFailureKindV29::Census)?;
                let operation = block
                    .operations
                    .get(witness.after.first as usize)
                    .ok_or(ScopedTileFailureKindV29::Census)?;
                if blocks.next().is_some()
                    || operation.kind
                        != OperationKind::Execution(Op::MaskedTileLoadU32 {
                            workgroup: workgroup.value,
                            input,
                            base,
                            lanes: tile.lanes,
                            elements: tile.elements,
                        })
                    || !matches!(operation.results.as_slice(), [result]
                        if result.id == tile.first_result && result.ty == tile.result_type(0))
                {
                    return Err(ScopedTileFailureKindV29::Census);
                }
                visit(
                    ScopedTileSelectionV29 {
                        root: root_ordinal,
                        semantic_root: launch.selected_root(),
                        insertion,
                        witness: *witness,
                        tile,
                        order,
                    },
                    budget,
                )?;
                count = argument_sum_v1(&[count, 1])?;
            }
        }
        let mut physical = 0;
        for block in &body.blocks {
            budget.charge_work(argument_sum_v1(&[block.operations.len(), 1])?)?;
            for operation in &block.operations {
                if matches!(
                    operation.kind,
                    OperationKind::Execution(Op::MaskedTileLoadU32 { .. })
                ) {
                    physical = argument_sum_v1(&[physical, 1])?;
                }
            }
        }
        if physical != count - before {
            return Err(ScopedTileFailureKindV29::Census);
        }
    }
    if count == 0 {
        return Err(ScopedTileFailureKindV29::NoTileOccurrences);
    }
    Ok(count)
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn scoped_tile_hash_bytes_v29(
    digest: &mut sha2::Sha256,
    bytes: &[u8],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ScopedTileFailureKindV29> {
    use sha2::Digest;
    budget.charge_work(argument_sum_v1(&[bytes.len(), 1])?)?;
    digest.update(bytes);
    Ok(())
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn scoped_tile_hash_words_v29(
    digest: &mut sha2::Sha256,
    words: &[u64],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ScopedTileFailureKindV29> {
    for word in words {
        scoped_tile_hash_bytes_v29(digest, &word.to_le_bytes(), budget)?;
    }
    Ok(())
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn scoped_tile_index_v29(value: usize) -> Result<u64, ScopedTileFailureKindV29> {
    u64::try_from(value).map_err(|_| ArgumentResourceV1::Arithmetic.into())
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn scoped_tile_identity_v29(
    pending: &ProductionPendingScopedSourceOwnerV29,
    request: &ScopedTileScheduleInputV29,
    selections: &[ScopedTileSelectionV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<[u8; 32], ScopedTileFailureKindV29> {
    use sha2::Digest;
    budget.reserve_storage(size_of::<sha2::Sha256>())?;
    let mut digest = sha2::Sha256::new();
    let inner = &pending.inner;
    scoped_tile_hash_bytes_v29(&mut digest, b"FE2O3/PENDING-TILE-SCHEDULE/V29\0", budget)?;
    scoped_tile_hash_bytes_v29(
        &mut digest,
        inner.source.owner.source_semantic_sha256(),
        budget,
    )?;
    scoped_tile_hash_bytes_v29(
        &mut digest,
        inner.source.owner.identity().as_bytes(),
        budget,
    )?;
    scoped_tile_hash_bytes_v29(&mut digest, inner.pending.graph.identity().digest(), budget)?;
    let target = inner.source.owner.source_semantic().target();
    scoped_tile_hash_bytes_v29(&mut digest, target.identity().as_bytes(), budget)?;
    let architecture = match target.architecture() {
        fe2o3_mir_model::semantic_mir_v1::SemanticTargetArchitectureV1::AmdGpuGfx942 => 1,
    };
    let order = |order| match order {
        ScopedTileOrderV29::Blocked => 1,
        ScopedTileOrderV29::Striped => 2,
    };
    scoped_tile_hash_words_v29(
        &mut digest,
        &[
            1, // Private schedule identity policy.
            inner.pending.graph.identity().canonical_length(),
            architecture,
            target.object_size_bound_bytes(),
            order(request.order),
            scoped_tile_index_v29(inner.source.launch.roots().len())?,
        ],
        budget,
    )?;
    for root in inner.source.launch.roots() {
        scoped_tile_hash_bytes_v29(
            &mut digest,
            root.semantic_root_identity().as_bytes(),
            budget,
        )?;
        scoped_tile_hash_bytes_v29(&mut digest, &root.kernel_binding(), budget)?;
        let input = root.source_launch();
        let layout = root.layout();
        scoped_tile_hash_words_v29(
            &mut digest,
            &[
                u64::from(root.selected_root().index()),
                u64::from(input.rank()),
                u64::from(input.exact_workgroup().is_some()),
            ],
            budget,
        )?;
        if let Some(extents) = input.exact_workgroup() {
            scoped_tile_hash_words_v29(&mut digest, &extents.map(u64::from), budget)?;
        }
        scoped_tile_hash_words_v29(&mut digest, &input.max_grid().map(u64::from), budget)?;
        scoped_tile_hash_words_v29(&mut digest, &layout.workgroup_extents(), budget)?;
        scoped_tile_hash_words_v29(&mut digest, &layout.global_extents(), budget)?;
        scoped_tile_hash_words_v29(
            &mut digest,
            &[
                layout.grid_identity(),
                layout.subgroup_size(),
                u64::from(layout.full_physical_workgroups()),
            ],
            budget,
        )?;
    }
    scoped_tile_hash_words_v29(
        &mut digest,
        &[scoped_tile_index_v29(selections.len())?],
        budget,
    )?;
    for row in selections {
        let DeferredTileInputV29::Load {
            workgroup,
            input,
            base,
        } = row.tile.input
        else {
            return Err(ScopedTileFailureKindV29::Census);
        };
        scoped_tile_hash_words_v29(
            &mut digest,
            &[
                scoped_tile_index_v29(row.root)?,
                u64::from(row.semantic_root.index()),
                scoped_tile_index_v29(row.insertion)?,
                scoped_tile_index_v29(row.witness.instance.index())?,
                scoped_tile_index_v29(row.witness.event)?,
                scoped_tile_index_v29(row.witness.source_span)?,
                u64::from(row.witness.before.block.0),
                u64::from(row.witness.before.first),
                u64::from(row.witness.before.count),
                u64::from(row.witness.after.block.0),
                u64::from(row.witness.after.first),
                u64::from(row.witness.after.count),
                scoped_tile_index_v29(row.tile.producer.caller.index())?,
                u64::from(row.tile.producer.block.index()),
                u64::from(row.tile.result_type.index()),
                u64::from(row.tile.first_result.0),
                u64::from(row.tile.lanes),
                u64::from(row.tile.elements),
                u64::from(workgroup.semantic_type.index()),
                scoped_tile_index_v29(workgroup.producer.caller.index())?,
                u64::from(workgroup.producer.block.index()),
                u64::from(workgroup.value.0),
                u64::from(input.0),
                u64::from(base.0),
                order(row.order),
            ],
            budget,
        )?;
        scoped_tile_hash_bytes_v29(&mut digest, workgroup.type_identity.as_bytes(), budget)?;
    }
    budget.charge_work(33)?;
    let identity = digest.finalize().into();
    budget.release_storage(size_of::<sha2::Sha256>())?;
    Ok(identity)
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn prepare_scoped_tile_source_inner_v29(
    donor: &mut Option<(
        ProductionPendingScopedSourceOwnerV29,
        ScopedTileScheduleInputV29,
    )>,
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PreparedScopedTileSourceV29, ScopedTileFailureKindV29> {
    let (pending, request) = donor
        .as_ref()
        .ok_or(ScopedTileFailureKindV29::MissingDonor)?;
    scoped_tile_floor_v29(pending, pending.adopted_storage(), budget)?;
    let floor = budget.storage();
    let (selections, identity, retained_storage) =
        scoped_source_attempt_v29(cleanup, budget, floor, |budget| {
            pending
                .inner
                .replay_with_cleanup(cleanup, budget)
                .map_err(ScopedTileFailureKindV29::from)?;
            budget.reserve_storage(scoped_tile_header_v29()?)?;
            let mut selections = Vec::new();
            scoped_tile_census_v29(pending, request.order, budget, |row, budget| {
                emission_push_v1(&mut selections, row, budget)?;
                Ok(())
            })?;
            let identity = scoped_tile_identity_v29(pending, request, &selections, budget)?;
            let extra = budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let retained_storage = argument_sum_v1(&[pending.adopted_storage(), extra])?;
            Ok::<_, ScopedTileFailureKindV29>((selections, identity, retained_storage))
        })?;
    // All fallible work finishes while the exact original donor is still borrowed.
    let (pending, request) = donor.take().expect("validated tile donor");
    Ok(PreparedScopedTileSourceV29 {
        selections,
        identity,
        request,
        pending,
        retained_storage,
    })
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
impl PreparedScopedTileSourceV29 {
    fn adopted_storage(&self) -> usize {
        self.retained_storage
    }

    fn replay_inner_v29(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ScopedTileFailureKindV29> {
        scoped_tile_floor_v29(&self.pending, self.retained_storage, budget)?;
        let floor = budget.storage();
        with_scoped_source_cleanup_v29(budget, floor, |cleanup, budget| {
            self.replay_inner_with_cleanup_v29(cleanup, budget)
        })
    }

    fn replay_inner_with_cleanup_v29(
        &self,
        cleanup: &ScopedSourceCleanupV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ScopedTileFailureKindV29> {
        let expected = argument_sum_v1(&[
            self.pending.adopted_storage(),
            scoped_tile_header_v29()?,
            argument_product_v1(
                self.selections.capacity(),
                size_of::<ScopedTileSelectionV29>(),
            )?,
        ])?;
        if expected != self.retained_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        scoped_tile_floor_v29(&self.pending, expected, budget)?;
        let floor = budget.storage();
        scoped_source_attempt_v29(cleanup, budget, floor, |budget| {
            self.pending
                .inner
                .replay_with_cleanup(cleanup, budget)
                .map_err(ScopedTileFailureKindV29::from)?;
            let mut next = 0;
            let count = scoped_tile_census_v29(
                &self.pending,
                self.request.order,
                budget,
                |row, budget| {
                    budget.charge_work(1)?;
                    if self.selections.get(next) != Some(&row) {
                        return Err(ScopedTileFailureKindV29::ReplayMismatch);
                    }
                    next += 1;
                    Ok(())
                },
            )?;
            budget.charge_work(33)?;
            if count != self.selections.len()
                || scoped_tile_identity_v29(&self.pending, &self.request, &self.selections, budget)?
                    != self.identity
            {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            Ok(())
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
enum ScopedTileFailurePhaseV29 {
    Preparation,
    Replay,
    Materialization,
    Correspondence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
struct ScopedTileFailureSummaryV29 {
    phase: ScopedTileFailurePhaseV29,
    kind: ScopedTileFailureKindV29,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
fn prepare_scoped_tile_source_v29(
    donor: &mut Option<(
        ProductionPendingScopedSourceOwnerV29,
        ScopedTileScheduleInputV29,
    )>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PreparedScopedTileSourceV29, ScopedTileFailureSummaryV29> {
    let entry = donor
        .as_ref()
        .ok_or(ScopedTileFailureKindV29::MissingDonor)
        .and_then(|(pending, _)| scoped_tile_floor_v29(pending, pending.adopted_storage(), budget));
    entry.map_err(|kind| ScopedTileFailureSummaryV29 {
        phase: ScopedTileFailurePhaseV29::Preparation,
        kind,
    })?;
    let floor = budget.storage();
    with_scoped_source_cleanup_v29(budget, floor, |cleanup, budget| {
        prepare_scoped_tile_source_inner_v29(donor, cleanup, budget)
    })
    .map_err(|kind| ScopedTileFailureSummaryV29 {
        phase: ScopedTileFailurePhaseV29::Preparation,
        kind,
    })
}

fn prepare_scoped_tile_source_with_cleanup_v29(
    donor: &mut Option<(
        ProductionPendingScopedSourceOwnerV29,
        ScopedTileScheduleInputV29,
    )>,
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PreparedScopedTileSourceV29, ScopedTileFailureSummaryV29> {
    prepare_scoped_tile_source_inner_v29(donor, cleanup, budget).map_err(|kind| {
        ScopedTileFailureSummaryV29 {
            phase: ScopedTileFailurePhaseV29::Preparation,
            kind,
        }
    })
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped tile schedule admission remains gated")
)]
impl PreparedScopedTileSourceV29 {
    fn replay_with_budget(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ScopedTileFailureSummaryV29> {
        self.replay_inner_v29(budget)
            .map_err(|kind| ScopedTileFailureSummaryV29 {
                phase: ScopedTileFailurePhaseV29::Replay,
                kind,
            })
    }
}
