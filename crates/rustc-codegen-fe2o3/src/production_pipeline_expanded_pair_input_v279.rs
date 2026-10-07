//! Borrowed inputs to a future expanded whole-pair obligation, never a proof.
//! Only the private source stage supplies the retained compiler binding owner.
use super::*;
use fe2o3_artifacts::{BlockSize, LaunchContract};
use fe2o3_kernel_ir::{
    CanonicalFormalLaunchInputV19 as Launch, CanonicalKirFunctionCoordinateV1 as Function,
    EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth,
    VerifiedCanonicalKernelIrIdentityV18 as Identity,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceExecutionLayoutV1 as SourceLayout,
    ProductionSourceLaunchInputV1 as SourceLaunch,
};
use sha2::{Digest, Sha256};

use super::super::mixed_worker_v28::publication::original_mir_v30 as runtime;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RootInputV279 {
    pub(crate) source_root: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    pub(crate) original_function: usize,
    pub(crate) target_function: Function,
    pub(crate) source_launch: SourceLaunch,
    pub(crate) source_layout: SourceLayout,
    pub(crate) launch: ExplicitLaunchExtent,
    pub(crate) tile: Option<(ExecutionTileLayoutV1, u16)>,
    pub(crate) instances: usize,
}

/// This identity names unproved inputs. It has no generated-program or receipt
/// conversion, and does not name a nominal optimizer execution transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SubjectV279 {
    pub(crate) semantic: [u8; 32],
    pub(crate) ssa: [u8; 32],
    pub(crate) graphs: [Identity; 3],
    pub(crate) runtime_and_instances: [u8; 32],
    pub(crate) references: [u8; 32],
}

pub(crate) struct ExpandedPairInputV279<'borrow, 'source, 'original, 'scope> {
    source: &'borrow Source<'source>,
    original: &'original Original<'scope>,
    tile: &'borrow Expanded<'original, 'scope>,
    bindings: &'borrow AuthenticatedProductionBindings,
    roots: Vec<RootInputV279>,
    width: FormalIndexWidth,
    endian: EndiannessV2,
    subject: SubjectV279,
    retained: usize,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

type Input<'b, 's, 'o, 'scope> = ExpandedPairInputV279<'b, 's, 'o, 'scope>;

fn binding() -> Error {
    Error::Unsupported("expanded pair input owner or runtime differs")
}

fn retained_bytes(capacity: usize) -> Result<usize, Resource> {
    capacity
        .checked_mul(size_of::<RootInputV279>())
        .and_then(|bytes| bytes.checked_add(size_of::<Input<'_, '_, '_, '_>>()))
        .and_then(|bytes| bytes.checked_add(query_headers()))
        .ok_or(Resource::Arithmetic)
}

fn query_headers() -> usize {
    type Frame<'a> = (
        &'a Input<'a, 'a, 'a, 'a>,
        &'a Source<'a>,
        &'a Original<'a>,
        &'a Expanded<'a, 'a>,
        &'a mut Budget<'a>,
    );
    2 * size_of::<Frame<'_>>()
        + align_of::<Frame<'_>>()
        + size_of::<SubjectV279>()
        + size_of::<Result<SubjectV279, Error>>()
        + size_of::<Result<&[RootInputV279], Error>>()
        + size_of::<Result<(FormalIndexWidth, EndiannessV2), Error>>()
        + size_of::<Result<usize, Error>>()
        + 4 * size_of::<Result<(), Error>>()
        + 3 * size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
        + 3 * size_of::<usize>()
        + 4 * size_of::<&()>()
}

pub(super) fn headers() -> Result<usize, Resource> {
    [
        runtime::runtime_headers()?,
        query_headers(),
        2 * size_of::<Input<'_, '_, '_, '_>>(),
        2 * size_of::<Result<Input<'_, '_, '_, '_>, Error>>(),
        size_of::<std::thread::Result<Result<Input<'_, '_, '_, '_>, Error>>>(),
        2 * size_of::<SubjectV279>(),
        3 * size_of::<RootInputV279>(),
        3 * size_of::<SourceLaunch>(),
        2 * size_of::<SourceLayout>(),
        3 * size_of::<[u64; 3]>(),
        3 * size_of::<[u32; 3]>(),
        2 * size_of::<&LaunchContract>(),
        2 * size_of::<Sha256>(),
        size_of::<Vec<RootInputV279>>(),
        size_of::<Vec<Launch>>(),
        size_of::<fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>>(),
        size_of::<
            Result<
                fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>,
                ProductionSourceOwnedViewErrorV18,
            >,
        >(),
        24 * size_of::<usize>(),
        24 * size_of::<&()>(),
        6 * size_of::<&[u8]>(),
        4 * size_of::<Result<(), Error>>(),
        4 * size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, n| {
        sum.checked_add(n).ok_or(Resource::Arithmetic)
    })
}

fn append(hash: &mut Sha256, bytes: &[u8], budget: &mut Budget<'_>) -> Result<(), Error> {
    budget.charge_work(bytes.len().checked_add(8).ok_or(Resource::Arithmetic)?)?;
    hash.update(
        u64::try_from(bytes.len())
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    hash.update(bytes);
    Ok(())
}

fn number(hash: &mut Sha256, value: usize, budget: &mut Budget<'_>) -> Result<(), Error> {
    append(
        hash,
        &u64::try_from(value)
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
        budget,
    )
}

fn same_owner<T: ?Sized>(actual: &T, expected: &T) -> Result<(), Error> {
    if std::ptr::eq(actual, expected) {
        Ok(())
    } else {
        Err(binding())
    }
}

fn check_account(
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    required: usize,
    budget: &Budget<'_>,
) -> Result<(), Resource> {
    if slot != std::ptr::from_ref(budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < required
    {
        return Err(Resource::Accounting);
    }
    Ok(())
}

fn join_launch(
    checked: Launch,
    retained: SourceLaunch,
    authentic: &LaunchContract,
) -> Result<ExplicitLaunchExtent, Error> {
    let BlockSize::Exact(block) = authentic.block_size() else {
        return Err(binding());
    };
    let workgroup = [block.x(), block.y(), block.z()];
    let grid = authentic.max_grid();
    let grid = [grid.x(), grid.y(), grid.z()];
    let rank = authentic.rank();
    if retained != SourceLaunch::new(rank, Some(workgroup), grid) || !(1..=3).contains(&rank) {
        return Err(binding());
    }
    // The source layout's zero sentinel is not a physical coordinate ceiling.
    // Reconstruct the ceiling only from the same retained authentic contract.
    let mut extents = [1; 3];
    for axis in 0..3 {
        extents[axis] = u64::from(workgroup[axis])
            .checked_mul(u64::from(grid[axis]))
            .ok_or(Resource::Arithmetic)?;
        if extents[axis] == 0 || (axis >= usize::from(rank) && extents[axis] != 1) {
            return Err(binding());
        }
    }
    let actual = ExplicitLaunchExtent::Exact { rank, extents };
    if checked != Launch::PhysicalEnvelope(actual) {
        return Err(binding());
    }
    Ok(actual)
}

fn join_root(
    expected: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    retained: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
) -> Result<(), Error> {
    if expected != retained {
        return Err(binding());
    }
    Ok(())
}

fn preparation_scope<T>(
    budget: &mut Budget<'_>,
    operation: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let captures = std::mem::size_of_val(&operation)
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(std::mem::align_of_val(&operation)))
        .ok_or(Resource::Arithmetic)?;
    let scratch = headers()?
        .checked_add(captures)
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(budget.storage(), 1, 1, scratch, operation)
}

/// Returns unreserved storage, adopted immediately by the private stage. No
/// caller supplies identities, policy rows, launches or reference-input hashes.
pub(super) fn prepare<'b, 's, 'o, 'scope>(
    source: &'b Source<'s>,
    original: &'o Original<'scope>,
    tile: &'b Expanded<'o, 'scope>,
    context: &'b SourceBindingContextV29<'_>,
    budget: &mut Budget<'_>,
) -> Result<Input<'b, 's, 'o, 'scope>, Error> {
    let floor = budget.storage();
    let slot = std::ptr::from_ref(budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    preparation_scope(budget, |budget| {
        same_owner(original.source(budget)?, source)?;
        same_owner(tile.original_source_v162(budget)?, original)?;
        let neutral = tile.neutral_source_v162(budget)?;
        same_owner(neutral.original_source(budget)?, source)?;
        same_owner(
            neutral.input_inventory(budget)?.owner(),
            source.canonical(budget)?,
        )?;
        let graphs = [
            *source.canonical(budget)?.identity(),
            *neutral.output_inventory(budget)?.owner().identity(),
            *tile.output(budget)?.identity(),
        ];
        let (launches, width) = context.launches(source, budget)?;
        let endian = runtime::target_byte_order(
            source.source_semantic(budget)?.target_layout_identity(),
            context.bindings.rustc_target.rustc_layout(),
            budget,
        )?;
        let references = reference_obligations_v69::with_inputs(
            source.source_ssa(budget)?,
            context.bindings,
            budget,
            |rows, budget| {
                budget.charge_work(2)?;
                if !rows.is_empty()
                    || !context
                        .bindings
                        .reference_effect_bindings
                        .as_slice()
                        .is_empty()
                {
                    return Err(Error::Unsupported(
                        "expanded pair nonempty reference inputs",
                    ));
                }
                let mut hash = Sha256::new();
                append(
                    &mut hash,
                    b"FE2O3/EXPANDED-PAIR/REFERENCE-INPUTS/V279\0",
                    budget,
                )?;
                number(&mut hash, rows.len(), budget)?;
                Ok(hash.finalize().into())
            },
        )?;
        let count = source.root_count(budget)?;
        let retained_launches = source.source_launch(budget)?;
        budget.charge_work(32)?;
        if retained_launches.semantic_sha256()
            != source.source_ssa(budget)?.source_semantic_sha256()
        {
            return Err(binding());
        }
        if count != launches.len() || count != retained_launches.roots().len() {
            return Err(binding());
        }
        let mut roots = paid_vec(count, budget)?;
        let mut hash = Sha256::new();
        append(
            &mut hash,
            b"FE2O3/EXPANDED-PAIR/RUNTIME-AND-INSTANCES/V279\0",
            budget,
        )?;
        append(&mut hash, &[width as u8, endian as u8], budget)?;
        number(&mut hash, count, budget)?;
        for (ordinal, checked) in launches.iter().enumerate() {
            budget.charge_work(4 + 3 * 8)?;
            let (source_root, original_function) = source.root(ordinal, budget)?;
            let retained_launch = retained_launches.roots().get(ordinal).ok_or_else(binding)?;
            join_root(source_root, retained_launch.selected_root())?;
            let authentic = context
                .bindings
                .typed_descriptor_roots
                .get(ordinal)
                .and_then(|descriptor| descriptor.source_launch())
                .ok_or_else(binding)?;
            let source_launch = retained_launch.source_launch();
            let source_layout = retained_launch.layout();
            let launch = join_launch(*checked, source_launch, authentic)?;
            let cfg = neutral.output_root_cfg_v18(ordinal, budget)?;
            let target_function = cfg.function().coordinate;
            let output = tile.output(budget)?;
            let target = output
                .module()
                .functions
                .get(target_function.0 as usize)
                .ok_or_else(binding)?;
            budget.charge_work(
                target
                    .id
                    .as_str()
                    .len()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if target.id != cfg.function().function.id {
                return Err(binding());
            }
            let tile_policy = tile.root_policy_v162(ordinal, budget)?;
            let tile_policy = match tile_policy {
                Some((function, layout, lanes)) if function == target_function => {
                    Some((layout, lanes))
                }
                Some(_) => return Err(binding()),
                None => None,
            };
            let instances = source.instance_count(ordinal, budget)?;
            number(&mut hash, ordinal, budget)?;
            number(&mut hash, source_root.index() as usize, budget)?;
            number(&mut hash, original_function, budget)?;
            number(&mut hash, target_function.0 as usize, budget)?;
            append(&mut hash, &[retained_launch.source_rank()], budget)?;
            let ExplicitLaunchExtent::Exact { extents, .. } = launch else {
                return Err(binding());
            };
            for extent in extents {
                append(&mut hash, &extent.to_le_bytes(), budget)?;
            }
            for extent in source_launch.exact_workgroup().ok_or_else(binding)? {
                append(&mut hash, &extent.to_le_bytes(), budget)?;
            }
            for extent in source_launch.max_grid() {
                append(&mut hash, &extent.to_le_bytes(), budget)?;
            }
            for extent in source_layout.global_extents() {
                append(&mut hash, &extent.to_le_bytes(), budget)?;
            }
            for extent in source_layout.workgroup_extents() {
                append(&mut hash, &extent.to_le_bytes(), budget)?;
            }
            append(
                &mut hash,
                &source_layout.subgroup_size().to_le_bytes(),
                budget,
            )?;
            append(
                &mut hash,
                &[source_layout.full_physical_workgroups() as u8],
                budget,
            )?;
            match tile_policy {
                None => append(&mut hash, &[0], budget)?,
                Some((layout, lanes)) => {
                    append(&mut hash, &[1, layout as u8], budget)?;
                    append(&mut hash, &lanes.to_le_bytes(), budget)?;
                }
            }
            number(&mut hash, instances, budget)?;
            for instance in 0..instances {
                let (function, incoming) = source.instance(ordinal, instance, budget)?;
                number(&mut hash, instance, budget)?;
                number(&mut hash, function.index() as usize, budget)?;
                append(
                    &mut hash,
                    &[source.instance_active(ordinal, instance, budget)? as u8],
                    budget,
                )?;
                match incoming {
                    None => append(&mut hash, &[0], budget)?,
                    Some((caller, block)) => {
                        append(&mut hash, &[1], budget)?;
                        number(&mut hash, caller, budget)?;
                        number(&mut hash, block.index() as usize, budget)?;
                    }
                }
            }
            roots.push(RootInputV279 {
                source_root,
                original_function,
                target_function,
                source_launch,
                source_layout,
                launch,
                tile: tile_policy,
                instances,
            });
        }
        let ssa = source.source_ssa(budget)?;
        let subject = SubjectV279 {
            semantic: *ssa.source_semantic_sha256(),
            ssa: *ssa.identity().as_bytes(),
            graphs,
            runtime_and_instances: hash.finalize().into(),
            references,
        };
        let retained = retained_bytes(roots.capacity())?;
        Ok(ExpandedPairInputV279 {
            source,
            original,
            tile,
            bindings: context.bindings,
            roots,
            width,
            endian,
            subject,
            retained,
            required: floor.checked_add(retained).ok_or(Resource::Arithmetic)?,
            slot,
            ledger,
        })
    })
}

impl Input<'_, '_, '_, '_> {
    pub(super) fn retained_storage(&self) -> usize {
        self.retained
    }

    pub(crate) fn check(
        &self,
        source: &Source<'_>,
        original: &Original<'_>,
        tile: &Expanded<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        check_account(self.slot, self.ledger, self.required, budget)
            .map_err(|error| self.source.retain_query_resource_error_v18(error))?;
        budget.check_prior_denials_v1()?;
        budget.charge_work(5)?;
        same_owner(self.source, source)?;
        same_owner(self.original, original)?;
        same_owner(self.tile, tile)?;
        same_owner(self.tile.original_source_v162(budget)?, self.original)?;
        if !self
            .bindings
            .reference_effect_bindings
            .as_slice()
            .is_empty()
        {
            return Err(binding());
        }
        Ok(())
    }

    pub(crate) fn subject(&self, budget: &mut Budget<'_>) -> Result<SubjectV279, Error> {
        self.check(self.source, self.original, self.tile, budget)?;
        Ok(self.subject)
    }

    pub(crate) fn roots(&self, budget: &mut Budget<'_>) -> Result<&[RootInputV279], Error> {
        self.check(self.source, self.original, self.tile, budget)?;
        Ok(&self.roots)
    }

    pub(crate) fn runtime(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(FormalIndexWidth, EndiannessV2), Error> {
        self.check(self.source, self.original, self.tile, budget)?;
        Ok((self.width, self.endian))
    }

    pub(crate) fn reference_count(&self, budget: &mut Budget<'_>) -> Result<usize, Error> {
        self.check(self.source, self.original, self.tile, budget)?;
        Ok(self.bindings.reference_effect_bindings.as_slice().len())
    }

    pub(super) fn discard(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<(), ProductionSourceOwnedViewErrorV18> {
        let checked = check_account(self.slot, self.ledger, self.required, budget)
            .map_err(|error| self.source.retain_query_resource_error_v18(error));
        let bytes = self.retained;
        drop(self);
        checked?;
        budget.release_storage(bytes).map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "production_pipeline_expanded_pair_input_v279_tests.rs"]
mod tests;
