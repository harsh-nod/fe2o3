//! Same-owner access derivation over retained CFG, origins and pointer facts.
use super::*;
use crate::formal_memory_obligations::pointer_derivation::{AccessDerivationError, access_engine};

#[path = "body_source_v19.rs"]
pub(in crate::formal_memory_obligations) mod body_source_v19;

type AccessResult = std::result::Result<FormalMemoryAccess, FormalMemoryIncompleteReason>;

#[must_use = "dropping access extraction without release retains its resource charge"]
pub(in crate::formal_memory_obligations) struct ActualOwnerAccessesV18<
    'pointer,
    'borrow,
    'affine,
    'owner,
    'work,
> {
    pointers: &'pointer mut ActualOwnerPointersV18<'borrow, 'affine, 'owner, 'work>,
    // () deliberately does not implement GuardMeter. Facts can only be queried
    // after reattaching the same authenticated live budget, never a private one.
    guarded: Option<GuardedAnalysisV1<'owner, ()>>,
    report_failure: Option<body_source_v19::BodyErrorV19>,
    floor: usize,
    retained: usize,
}

impl<'borrow, 'affine, 'owner, 'work> ActualOwnerPointersV18<'borrow, 'affine, 'owner, 'work> {
    pub(in crate::formal_memory_obligations) fn accesses<'pointer>(
        &'pointer mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        budget: &mut Budget<'_>,
    ) -> Result<ActualOwnerAccessesV18<'pointer, 'borrow, 'affine, 'owner, 'work>> {
        self.check(owner, root_index, budget)?;
        let floor = budget.storage();
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<_> {
            budget.reserve_storage(build_frame_v18())?;
            budget.charge_work(4)?;
            let rank_one = owner
                .module()
                .kernels
                .get(root_index)
                .ok_or(ResourceError::Accounting)?
                .domain
                .rank()
                == 1;
            let affine = &mut self.slots.affine;
            let source = affine.source;
            let (flow, origins) = affine.context.guarded_inputs(source, budget)?;
            let mut meter = meter::LiveGuardMeter::new(budget, usize::MAX, usize::MAX, usize::MAX);
            meter.storage(size_of::<
                GuardedControlCollectionV1<meter::LiveGuardMeter<'_, '_>>,
            >())?;
            let seed = GuardedControlV1::collect_preserving_ledger(source, flow, meter)?;
            match seed {
                GuardedControlCollectionV1::Unselected(_) => Ok(None),
                GuardedControlCollectionV1::Selected(seed) => {
                    let entry = seed.entry;
                    let mut guarded = GuardedAnalysisV1::empty(seed, rank_one);
                    canonical_reads::collect_actual_definitions(&mut guarded, source)?;
                    // Preserve the original formal extractor's exact truth
                    // grammar, not the broader carried-predicate query profile.
                    guarded.collect_parameters_and_truths(source, entry)?;
                    guarded
                        .ledger
                        .reserve(&mut guarded.runtime_reads.origins, origins.len())?;
                    guarded.ledger.charge(origins.len())?;
                    guarded.runtime_reads.origins.extend_from_slice(origins);
                    guarded.collect_runtime_read_guards(source)?;
                    guarded.collect_recipes()?;
                    Ok(Some(guarded.replace_meter(())))
                }
            }
        }));
        match result {
            Ok(Ok(guarded)) => {
                let retained = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ResourceError::Accounting)?;
                Ok(ActualOwnerAccessesV18 {
                    pointers: self,
                    guarded,
                    report_failure: None,
                    floor,
                    retained,
                })
            }
            Ok(Err(error)) => {
                budget.rollback_storage(floor)?;
                self.slots.affine.keep(Err(error))
            }
            Err(payload) => {
                budget.rollback_storage(floor)?;
                resume_unwind(payload)
            }
        }
    }
}

struct AccessQuery<'query, 'pointer, 'borrow, 'affine, 'owner, 'work, 'budget, 'budget_work> {
    accesses: &'query mut ActualOwnerAccessesV18<'pointer, 'borrow, 'affine, 'owner, 'work>,
    budget: &'budget mut Budget<'budget_work>,
}

impl access_engine::State for AccessQuery<'_, '_, '_, '_, '_, '_, '_, '_> {
    type Error = Failure;
    fn step(&mut self) -> Result<()> {
        // Fixed decisions and construction in the shared non-iterating engine.
        Ok(self.budget.charge_work(32)?)
    }
    fn guarded(
        &mut self,
        location: FunctionOperationLocation,
        pointer: ValueId,
        kind: FormalMemoryAccessKind,
        access: MemoryAccess,
        invocations: InvocationRange1d,
        predicate: Option<ValueId>,
        runtime: bool,
    ) -> std::result::Result<Option<FormalMemoryAccess>, AccessDerivationError<Failure>> {
        let Some(facts) = self.accesses.guarded.take() else {
            return Ok(None);
        };
        let mut guarded = facts.replace_meter(meter::LiveGuardMeter::new(
            self.budget,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ));
        let result = if runtime {
            guarded
                .runtime_slice_read(location, pointer, kind, access, invocations, predicate)
                .map_err(AccessDerivationError::Resource)
        } else {
            guarded.access(location, pointer, kind, access, invocations, predicate)
        };
        self.accesses.guarded = Some(guarded.replace_meter(()));
        result.map_err(|error| match error {
            AccessDerivationError::Incomplete(reason) => AccessDerivationError::Incomplete(reason),
            AccessDerivationError::Resource(error) => AccessDerivationError::Resource(error.into()),
        })
    }
    fn width(&mut self, pointer: ValueId) -> Result<Option<u64>> {
        let affine = &mut self.accesses.pointers.slots.affine;
        Ok(affine
            .context
            .value_type(affine.source, pointer, self.budget)?
            .and_then(pointer_byte_width))
    }
    fn expression(
        &mut self,
        pointer: ValueId,
        location: FunctionOperationLocation,
    ) -> Result<std::result::Result<PointerExpression, FormalMemoryIncompleteReason>> {
        let pointers = &mut self.accesses.pointers;
        pointers
            .expression(
                pointers.slots.affine.owner,
                pointers.slots.root_index,
                pointer,
                self.budget,
            )
            .map(|value| value.map_err(|error| error.materialize(location)))
    }
    fn allocation(
        &mut self,
        pointer: ValueId,
        location: FunctionOperationLocation,
    ) -> Result<std::result::Result<FormalAllocationIdentity, FormalMemoryIncompleteReason>> {
        let pointers = &mut self.accesses.pointers;
        pointers
            .allocation(
                pointers.slots.affine.owner,
                pointers.slots.root_index,
                pointer,
                self.budget,
            )
            .map(|value| value.map_err(|error| error.materialize(location)))
    }
    fn space(
        &mut self,
        pointer: ValueId,
        access: MemoryAccess,
        allocation: FormalAllocationIdentity,
        location: FunctionOperationLocation,
    ) -> Result<std::result::Result<AddressSpace, FormalMemoryIncompleteReason>> {
        let affine = &mut self.accesses.pointers.slots.affine;
        let unsupported =
            || FormalMemoryIncompleteReason::UnsupportedPointerDerivation { location, pointer };
        let Some(Type::Pointer(ty)) =
            affine
                .context
                .value_type(affine.source, pointer, self.budget)?
        else {
            return Ok(Err(unsupported()));
        };
        self.budget.charge_work(8)?;
        if ty.address_space != access.address_space {
            return Ok(Err(unsupported()));
        }
        let ordinal = allocation.parameter_index as usize;
        let row = affine
            .source
            .body
            .as_ref()
            .and_then(|body| body.parameters.get(ordinal))
            .zip(affine.source.signature.parameters.get(ordinal))
            .and_then(|(&value, ty)| formal_allocation_parameter(ordinal, value, ty));
        let Some(row) = row else {
            return Ok(Err(unsupported()));
        };
        if row.identity != allocation
            || (row.address_space != access.address_space
                && access.address_space != AddressSpace::Generic)
        {
            return Ok(Err(unsupported()));
        }
        Ok(Ok(row.address_space))
    }
}

impl ActualOwnerAccessesV18<'_, '_, '_, '_, '_> {
    fn check(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        budget: &Budget<'_>,
    ) -> Result<()> {
        self.pointers.check(owner, root_index, budget)?;
        let result = if budget.storage()
            < self
                .floor
                .checked_add(self.retained)
                .ok_or(ResourceError::Arithmetic)?
        {
            Err(ResourceError::Accounting.into())
        } else {
            Ok(())
        };
        self.pointers.slots.affine.keep(result)
    }

    fn query(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        location: FunctionOperationLocation,
        invocations: InvocationRange1d,
        budget: &mut Budget<'_>,
        conservative: bool,
    ) -> Result<AccessResult> {
        self.check(owner, root_index, budget)?;
        let floor = budget.storage();
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<AccessResult> {
            budget.reserve_storage(query_frame_v18())?;
            budget.charge_work(8)?;
            let affine = &mut self.pointers.slots.affine;
            if !affine
                .context
                .reachable(affine.source, location.block, budget)?
            {
                return Err(ResourceError::Accounting.into());
            }
            let operation = affine
                .context
                .block(affine.source, location.block, budget)?
                .and_then(|block| block.operations.get(location.operation_index))
                .ok_or(ResourceError::Accounting)?;
            let (pointer, kind, access, predicate) = match operation.kind {
                OperationKind::Load { pointer, access } if !conservative => {
                    (pointer, FormalMemoryAccessKind::Read, access, None)
                }
                OperationKind::GuardedLoad {
                    pointer,
                    access,
                    predicate,
                    ..
                } => (
                    pointer,
                    FormalMemoryAccessKind::Read,
                    access,
                    Some(predicate),
                ),
                OperationKind::Store {
                    pointer, access, ..
                } if !conservative => (pointer, FormalMemoryAccessKind::Write, access, None),
                OperationKind::GuardedStore {
                    pointer,
                    access,
                    predicate,
                    ..
                } if !conservative => (
                    pointer,
                    FormalMemoryAccessKind::Write,
                    access,
                    Some(predicate),
                ),
                OperationKind::Atomic(ref atomic) if !conservative => (
                    atomic.pointer,
                    FormalMemoryAccessKind::Atomic,
                    atomic.access,
                    None,
                ),
                _ => return Err(ResourceError::Accounting.into()),
            };
            let mut query = AccessQuery {
                accesses: self,
                budget,
            };
            let result = if conservative {
                access_engine::conservative_read(location, pointer, access, invocations, &mut query)
            } else {
                access_engine::derive(
                    location,
                    pointer,
                    kind,
                    access,
                    invocations,
                    predicate,
                    &mut query,
                )
            };
            match result {
                Ok(access) => Ok(Ok(access)),
                Err(AccessDerivationError::Incomplete(reason)) => Ok(Err(reason)),
                Err(AccessDerivationError::Resource(error)) => Err(error),
            }
        }));
        budget.rollback_storage(floor)?;
        match result {
            Ok(result) => self.pointers.slots.affine.keep(result),
            Err(payload) => {
                // A panic can interrupt detachment of guarded facts. Never
                // allow a later retry to silently skip that original policy.
                let _ = self
                    .pointers
                    .slots
                    .affine
                    .keep::<()>(Err(ResourceError::Accounting.into()));
                resume_unwind(payload)
            }
        }
    }

    pub(in crate::formal_memory_obligations) fn access(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        location: FunctionOperationLocation,
        invocations: InvocationRange1d,
        budget: &mut Budget<'_>,
    ) -> Result<AccessResult> {
        self.query(owner, root_index, location, invocations, budget, false)
    }

    pub(in crate::formal_memory_obligations) fn conservative_guarded_read(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        location: FunctionOperationLocation,
        invocations: InvocationRange1d,
        budget: &mut Budget<'_>,
    ) -> Result<AccessResult> {
        self.query(owner, root_index, location, invocations, budget, true)
    }

    pub(in crate::formal_memory_obligations) fn release(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.pointers.slots.affine.identity(budget)?;
        if budget.storage()
            < self
                .floor
                .checked_add(self.retained)
                .ok_or(ResourceError::Arithmetic)?
        {
            return Err(ResourceError::Accounting.into());
        }
        let retained = self.retained;
        drop(self);
        Ok(budget.release_storage(retained)?)
    }
}

fn build_frame_v18() -> usize {
    size_of::<ActualOwnerAccessesV18<'static, 'static, 'static, 'static, 'static>>()
        + size_of::<Result<ActualOwnerAccessesV18<'static, 'static, 'static, 'static, 'static>>>()
        + size_of::<std::thread::Result<Result<Option<GuardedAnalysisV1<'static, ()>>>>>()
        + size_of::<(
            &mut ActualOwnerPointersV18<'static, 'static, 'static, 'static>,
            &mut Budget<'static>,
            &VerifiedCanonicalKernelIrModuleV18,
            usize,
        )>()
}

fn query_frame_v18() -> usize {
    size_of::<AccessQuery<'static, 'static, 'static, 'static, 'static, 'static, 'static, 'static>>()
        + size_of::<GuardedAnalysisV1<'static, meter::LiveGuardMeter<'static, 'static>>>()
        + size_of::<Result<AccessResult>>()
        + size_of::<std::thread::Result<Result<AccessResult>>>()
        + size_of::<(
            &mut ActualOwnerAccessesV18<'static, 'static, 'static, 'static, 'static>,
            &mut Budget<'static>,
            FunctionOperationLocation,
            InvocationRange1d,
            bool,
        )>()
}
