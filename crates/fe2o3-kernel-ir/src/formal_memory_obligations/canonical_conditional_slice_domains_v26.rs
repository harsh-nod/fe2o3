//! Complete global slice families, conditional on explicit runtime premises.
use super::*;
use crate::{
    CanonicalGuardedGlobalStoreDomainV24, CanonicalGuardedGlobalStoreOutcomeV24,
    CheckedCanonicalGuardedGlobalStoresV24, ScalarType,
};
use std::ops::Range;

/// Read and Store facts stay distinct; there is no Store-to-read conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalConditionalSliceDomainV26 {
    Read(FormalRuntimeSliceReadDomainV1),
    Store(CanonicalGuardedGlobalStoreDomainV24),
}

impl CanonicalConditionalSliceDomainV26 {
    pub const fn allocation(self) -> FormalAllocationIdentity {
        match self {
            Self::Read(d) => d.allocation(),
            Self::Store(d) => d.allocation(),
        }
    }
    pub const fn slice(self) -> ValueId {
        match self {
            Self::Read(d) => d.slice(),
            Self::Store(d) => d.slice(),
        }
    }
    pub const fn pointer(self) -> ValueId {
        match self {
            Self::Read(d) => d.pointer(),
            Self::Store(d) => d.pointer(),
        }
    }
    pub const fn index(self) -> ValueId {
        match self {
            Self::Read(d) => d.index(),
            Self::Store(d) => d.index(),
        }
    }
    pub const fn element_bytes(self) -> u64 {
        match self {
            Self::Read(d) => d.element_bytes(),
            Self::Store(d) => d.element_bytes(),
        }
    }
    pub const fn writing(self) -> bool {
        matches!(self, Self::Store(_))
    }
}

/// A descriptive occurrence, accessible only through its checked batch. Copying
/// the coordinate/domain out does not copy the batch's conditional proof.
pub struct CanonicalConditionalSliceAccessV26 {
    operation: Coordinate,
    domain: CanonicalConditionalSliceDomainV26,
    projection: Option<(Axis, ValueId)>,
}
impl CanonicalConditionalSliceAccessV26 {
    pub const fn operation(&self) -> Coordinate {
        self.operation
    }
    pub const fn domain(&self) -> CanonicalConditionalSliceDomainV26 {
        self.domain
    }
    pub const fn invocation_projection(&self) -> Option<(Axis, ValueId)> {
        self.projection
    }
}

/// Undischarged runtime conditions for one actual physical slice parameter.
/// This is neither an allocation binding nor a source ownership certificate.
pub struct CanonicalConditionalSliceParameterV26 {
    function: FunctionCoordinate,
    parameter: u32,
    value: ValueId,
    scalar: ScalarType,
    access: AccessMode,
    reads: usize,
    writes: usize,
    axis: Option<Axis>,
    different_projection: bool,
}
impl CanonicalConditionalSliceParameterV26 {
    pub const fn function(&self) -> FunctionCoordinate {
        self.function
    }
    pub const fn parameter(&self) -> u32 {
        self.parameter
    }
    pub const fn value(&self) -> ValueId {
        self.value
    }
    pub const fn scalar(&self) -> ScalarType {
        self.scalar
    }
    pub const fn access(&self) -> AccessMode {
        self.access
    }
    pub const fn reads(&self) -> usize {
        self.reads
    }
    pub const fn writes(&self) -> usize {
        self.writes
    }
    pub const fn requires_valid_aligned_extent(&self) -> bool {
        self.reads != 0 || self.writes != 0
    }
    pub const fn requires_initialized_extent(&self) -> bool {
        self.reads != 0
    }
    /// The runtime extent must not overlap any other accessed parameter's
    /// extent, including a read-only parameter. Formal parameter inequality
    /// alone never proves this premise.
    pub const fn requires_exclusive_runtime_binding(&self) -> bool {
        self.writes != 0
    }
    pub const fn requires_exact_launch_binding(&self) -> bool {
        self.writes != 0
    }
    pub const fn invocation_axis(&self) -> Option<Axis> {
        self.axis
    }

    fn add(&mut self, writing: bool, projection: Option<Axis>) -> Result<bool> {
        if self.reads == 0 && self.writes == 0 {
            self.axis = projection;
        }
        self.different_projection |= projection.is_none() || self.axis != projection;
        if writing {
            if !matches!(self.access, AccessMode::WriteOnly | AccessMode::ReadWrite)
                || self.different_projection
            {
                return Ok(false);
            }
            self.writes = self
                .writes
                .checked_add(1)
                .ok_or(ResourceError::Arithmetic)?;
        } else {
            if !matches!(self.access, AccessMode::ReadOnly | AccessMode::ReadWrite)
                || (self.writes != 0 && self.different_projection)
            {
                return Ok(false);
            }
            self.reads = self.reads.checked_add(1).ok_or(ResourceError::Arithmetic)?;
        }
        Ok(true)
    }
}

struct SliceFunctionV26 {
    parameters: Range<usize>,
    launch: ExplicitLaunchExtent,
    reads: usize,
    writes: usize,
}

/// Conditional whole-module global family proof. Every global effect must be
/// one ordinary guarded scalar slice Load or Store in this batch. Unsupported
/// effects, raw pointers and unresolved calls prevent construction. Private
/// effects are deliberately unclaimed and require an independent private proof.
///
/// All reads/writes of a parameter with any Store have the same *direct Global
/// invocation axis*, equal scalar width and checked local bounds. Thus each
/// pair is non-conflicting across distinct invocations under the retained exact
/// launch/width premise. General affine injectivity is not substituted here.
/// Distinct parameters remain conditional on runtime exclusivity; their numeric
/// identities are never treated as an alias proof.
pub struct CheckedCanonicalConditionalSliceDomainsV26<'s, 'g> {
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    functions: &'s [SliceFunctionV26],
    parameters: &'s [Option<CanonicalConditionalSliceParameterV26>],
    accesses: &'s [CanonicalConditionalSliceAccessV26],
    width: FormalIndexWidth,
    accounting: &'s Accounting,
    reads: &'s CheckedCanonicalGuardedGlobalReadsV18<'s, 'g>,
    stores: &'s CheckedCanonicalGuardedGlobalStoresV24<'s, 'g>,
}

impl CheckedCanonicalConditionalSliceDomainsV26<'_, '_> {
    fn retained_custody_is_intact(&self, budget: &Budget<'_>) -> bool {
        let local = self.accounting.valid(budget);
        let reads = self.reads.accounting.valid(budget);
        let stores = self.stores.retained_custody_is_intact(budget);
        local && reads && stores
    }
    /// A composing child may report lost retained custody. This irreversibly
    /// refuses this batch and both fact owners; it cannot create authority.
    pub fn refuse_retained_custody(&self) -> Failure {
        let failure = self.accounting.refuse_retained_custody();
        self.reads
            .accounting
            .refuse_retained_failure(failure.clone());
        self.stores.refuse_retained_failure(failure.clone());
        failure
    }
    fn enter(&self, budget: &mut Budget<'_>) -> Result<()> {
        if !self.retained_custody_is_intact(budget) {
            return Err(
                if self.accounting.refund_denied.get()
                    || self.reads.accounting.refund_denied.get()
                    || self.stores.retained_refund_is_denied()
                {
                    self.refuse_retained_custody()
                } else {
                    self.accounting.accounting_failure()
                },
            );
        }
        self.accounting.enter(budget)
    }
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&VerifiedCanonicalKernelIrModuleV18> {
        self.enter(budget)?;
        Ok(self.owner)
    }
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize> {
        self.enter(budget)?;
        Ok(self.functions.len())
    }
    pub fn access_count(&self, budget: &mut Budget<'_>) -> Result<usize> {
        self.enter(budget)?;
        Ok(self.accesses.len())
    }
    pub fn access_at(
        &self,
        at: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&CanonicalConditionalSliceAccessV26>> {
        self.enter(budget)?;
        self.accounting.save((|| {
            let row = verification_find_last_by_v1(self.accesses, 3, budget, |row| {
                row.operation.cmp(&at)
            })?;
            Ok(row.map(|i| &self.accesses[i]))
        })())
    }
    pub fn parameter(
        &self,
        function: FunctionCoordinate,
        parameter: u32,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&CanonicalConditionalSliceParameterV26>> {
        self.enter(budget)?;
        self.accounting.save((|| {
            budget.charge_work(8)?;
            let Some(function) = self.functions.get(function.0 as usize) else {
                return Ok(None);
            };
            let Some(index) = function
                .parameters
                .start
                .checked_add(parameter as usize)
                .filter(|index| *index < function.parameters.end)
            else {
                return Ok(None);
            };
            Ok(self.parameters[index].as_ref())
        })())
    }
    pub fn function_conditions(
        &self,
        function: FunctionCoordinate,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(ExplicitLaunchExtent, FormalIndexWidth, usize, usize)>> {
        self.enter(budget)?;
        self.accounting.save((|| {
            budget.charge_work(4)?;
            Ok(self
                .functions
                .get(function.0 as usize)
                .map(|row| (row.launch, self.width, row.reads, row.writes)))
        })())
    }
    pub const fn runtime_requirements_are_discharged(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Joins existing owner-bound local analyses, without rerunning them. `None`
/// means some family or complete global effect census was not proved. Neither
/// callback success nor a descriptive launch supplies runtime/source authority.
pub fn with_canonical_conditional_slice_domains_v26<'g, 'w, T>(
    reads: &CheckedCanonicalGuardedGlobalReadsV18<'_, 'g>,
    stores: &CheckedCanonicalGuardedGlobalStoresV24<'_, 'g>,
    launches: &[ExplicitLaunchExtent],
    width: FormalIndexWidth,
    budget: &mut Budget<'w>,
    consume: impl for<'s> FnOnce(
        &CheckedCanonicalConditionalSliceDomainsV26<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T>,
) -> Result<Option<T>> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let refund_denied = Cell::new(false);
    let mut consume = Some(consume);
    let mut returned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let owner = reads.owner(budget)?;
        if !std::ptr::eq(owner, stores.owner(budget)?) {
            return Err(ResourceError::Accounting.into());
        }
        budget.reserve_storage(slice_batch_headers_v26::<T>(
            std::mem::size_of_val(&consume),
            std::mem::align_of_val(&consume),
        )?)?;
        let Some((functions, parameters, accesses)) =
            build_slice_batch_v26(owner, reads, stores, launches, width, budget)?
        else {
            return Ok(None);
        };
        let accounting = Accounting {
            slot,
            ledger,
            floor: budget.storage(),
            failure: RefCell::new(None),
            refund_denied: Cell::new(false),
        };
        let view = CheckedCanonicalConditionalSliceDomainsV26 {
            owner,
            functions: &functions,
            parameters: &parameters,
            accesses: &accesses,
            width,
            accounting: &accounting,
            reads,
            stores,
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            consume.take().ok_or(ResourceError::Accounting)?(&view, budget)
        }));
        let result = match caught {
            Ok(mut result) => {
                let local = if !view.retained_custody_is_intact(budget) {
                    Err(view.refuse_retained_custody())
                } else if budget.storage() != accounting.floor {
                    Err(ResourceError::Accounting.into())
                } else {
                    accounting
                        .enter(budget)
                        .and_then(|()| reads.owner(budget).map(|_| ()))
                        .and_then(|()| stores.owner(budget).map(|_| ()))
                };
                if let Err(error) = local {
                    drain(std::mem::replace(&mut result, Err(error)));
                }
                result.map(Some)
            }
            Err(payload) => {
                drain(payload);
                Err(if !view.retained_custody_is_intact(budget) {
                    view.refuse_retained_custody()
                } else if let Some(error) = accounting.failure.borrow().clone() {
                    error
                } else {
                    reads
                        .owner(budget)
                        .and_then(|_| stores.owner(budget))
                        .err()
                        .unwrap_or(Failure::Panicked)
                })
            }
        };
        let result = if !view.retained_custody_is_intact(budget) {
            let failure = view.refuse_retained_custody();
            drain(result);
            Err(failure)
        } else {
            result
        };
        refund_denied.set(accounting.refund_denied.get());
        drop(view);
        drop(accesses);
        drop(parameters);
        drop(functions);
        result
    }));
    // An uncalled capture may itself panic in Drop. Destroy it while the frame
    // remains paid, without replacing an earlier typed construction refusal.
    drain(consume.take());
    if refund_denied.get()
        || slot != std::ptr::from_ref(&*budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        reads.refuse_retained_custody();
        stores.refuse_retained_custody();
        return refused_retained_result(returned, ResourceError::Accounting.into());
    }
    if returned.is_err() {
        drain(std::mem::replace(&mut returned, Ok(Err(Failure::Panicked))));
    }
    let result = match returned {
        Ok(result) => result,
        Err(_) => unreachable!("drained panic replaced"),
    };
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        drain(result);
        return Err(error.into());
    }
    result
}

type SliceBatchRowsV26 = (
    Vec<SliceFunctionV26>,
    Vec<Option<CanonicalConditionalSliceParameterV26>>,
    Vec<CanonicalConditionalSliceAccessV26>,
);

fn slice_batch_vector_v26<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    budget.charge_work(1)?;
    budget.reserve_storage(
        count
            .checked_mul(size_of::<T>())
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ResourceError::Allocation)?;
    budget.reserve_storage(
        rows.capacity()
            .checked_sub(count)
            .and_then(|n| n.checked_mul(size_of::<T>()))
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    Ok(rows)
}

fn build_slice_batch_v26(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    reads: &CheckedCanonicalGuardedGlobalReadsV18<'_, '_>,
    stores: &CheckedCanonicalGuardedGlobalStoresV24<'_, '_>,
    launches: &[ExplicitLaunchExtent],
    width: FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<Option<SliceBatchRowsV26>> {
    budget.charge_work(4)?;
    if launches.len() != owner.module().functions.len() {
        return Ok(None);
    }
    let mut parameter_count = 0usize;
    let mut operation_count = 0usize;
    for function in &owner.module().functions {
        budget.charge_work(2)?;
        parameter_count = parameter_count
            .checked_add(function.signature.parameters.len())
            .ok_or(ResourceError::Arithmetic)?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1)?;
                operation_count = operation_count
                    .checked_add(block.operations.len())
                    .ok_or(ResourceError::Arithmetic)?;
            }
        }
    }
    let mut functions = slice_batch_vector_v26(owner.module().functions.len(), budget)?;
    let mut parameters = slice_batch_vector_v26(parameter_count, budget)?;
    let mut accesses = slice_batch_vector_v26(operation_count, budget)?;
    for (ordinal, function) in owner.module().functions.iter().enumerate() {
        budget.charge_work(8)?;
        let coordinate =
            FunctionCoordinate(u32::try_from(ordinal).map_err(|_| ResourceError::Arithmetic)?);
        let start = parameters.len();
        for (parameter, ty) in function.signature.parameters.iter().enumerate() {
            budget.charge_work(8)?;
            let row = if let (Some(body), Type::Slice(slice)) = (&function.body, ty)
                && slice.address_space == AddressSpace::Global
                && let Type::Scalar(scalar) = slice.element.as_ref()
            {
                let value = *body
                    .parameters
                    .get(parameter)
                    .ok_or(ResourceError::Accounting)?;
                Some(CanonicalConditionalSliceParameterV26 {
                    function: coordinate,
                    parameter: u32::try_from(parameter).map_err(|_| ResourceError::Arithmetic)?,
                    value,
                    scalar: *scalar,
                    access: slice.access,
                    reads: 0,
                    writes: 0,
                    axis: None,
                    different_projection: false,
                })
            } else {
                None
            };
            parameters.push(row);
        }
        let end = parameters.len();
        let mut observed = [0usize; 2];
        if let Some(body) = &function.body {
            for (block, contents) in body.blocks.iter().enumerate() {
                budget.charge_work(1)?;
                for (operation, actual) in contents.operations.iter().enumerate() {
                    budget.charge_work(4)?;
                    let at = operation_coordinate(coordinate, block, operation)?;
                    let candidate = match actual.kind {
                        OperationKind::Load { access, .. }
                            if matches!(
                                access.address_space,
                                AddressSpace::Global | AddressSpace::Generic
                            ) =>
                        {
                            match reads.read_at(at, budget)? {
                                CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(
                                    fact,
                                ) => Some((
                                    CanonicalConditionalSliceDomainV26::Read(*fact.domain()),
                                    stores.read_invocation_projection(&fact, budget)?,
                                )),
                                _ => None,
                            }
                        }
                        OperationKind::Store { access, .. }
                            if matches!(
                                access.address_space,
                                AddressSpace::Global | AddressSpace::Generic
                            ) =>
                        {
                            match stores.store_at(at, budget)? {
                                CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(
                                    fact,
                                ) => {
                                    let Some(proof) = fact.distinct_invocations(
                                        launches[ordinal],
                                        width,
                                        budget,
                                    )?
                                    else {
                                        return Ok(None);
                                    };
                                    Some((
                                        CanonicalConditionalSliceDomainV26::Store(fact.domain()),
                                        Some(proof.projection()),
                                    ))
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    };
                    let Some((domain, projection)) = candidate else {
                        continue;
                    };
                    budget.charge_work(18)?;
                    let parameter = domain.allocation().parameter_index() as usize;
                    let Some(index) = start.checked_add(parameter).filter(|index| *index < end)
                    else {
                        return Ok(None);
                    };
                    let Some(row) = parameters[index].as_mut() else {
                        return Ok(None);
                    };
                    let Some(bits) = row.scalar.bit_width() else {
                        return Ok(None);
                    };
                    if row.value != domain.slice()
                        || bits % 8 != 0
                        || u64::from(bits / 8) != domain.element_bytes()
                        || !row.add(domain.writing(), projection.map(|(axis, _)| axis))?
                    {
                        return Ok(None);
                    }
                    let count = &mut observed[usize::from(domain.writing())];
                    *count = count.checked_add(1).ok_or(ResourceError::Arithmetic)?;
                    accesses.push(CanonicalConditionalSliceAccessV26 {
                        operation: at,
                        domain,
                        projection,
                    });
                }
            }
        }
        if reads.function_effects(coordinate, budget)? != (observed[0], observed[1], 0)
            || stores.function_effects(coordinate, budget)? != (observed[1], observed[0], 0)
        {
            return Ok(None);
        }
        functions.push(SliceFunctionV26 {
            parameters: start..end,
            launch: launches[ordinal],
            reads: observed[0],
            writes: observed[1],
        });
    }
    Ok(Some((functions, parameters, accesses)))
}

fn slice_batch_headers_v26<T>(capture: usize, alignment: usize) -> Result<usize> {
    type Frame<'a> = (
        &'a VerifiedCanonicalKernelIrModuleV18,
        &'a Module,
        &'a Function,
        &'a CheckedCanonicalGuardedGlobalReadsV18<'a, 'a>,
        &'a CheckedCanonicalGuardedGlobalStoresV24<'a, 'a>,
        CheckedCanonicalConditionalSliceDomainsV26<'a, 'a>,
        Accounting,
        Cell<bool>,
        SliceBatchRowsV26,
        Option<SliceBatchRowsV26>,
        SliceFunctionV26,
        CanonicalConditionalSliceParameterV26,
        Option<CanonicalConditionalSliceParameterV26>,
        CanonicalConditionalSliceAccessV26,
        CanonicalConditionalSliceDomainV26,
        Option<(CanonicalConditionalSliceDomainV26, Option<(Axis, ValueId)>)>,
        CanonicalGuardedGlobalReadOutcomeV1<'a, 'a, VerifiedCanonicalKernelIrModuleV18>,
        CanonicalGuardedGlobalStoreOutcomeV24<'a, 'a>,
        Option<crate::CanonicalGuardedStoreInjectivityV24<'a, 'a, 'a>>,
        CanonicalGuardedGlobalReadFactV18<'a, 'a>,
        &'a mut CanonicalConditionalSliceParameterV26,
        Option<&'a mut CanonicalConditionalSliceParameterV26>,
        &'a [ExplicitLaunchExtent],
        ExplicitLaunchExtent,
        FormalIndexWidth,
        std::iter::Enumerate<std::slice::Iter<'a, Function>>,
        std::iter::Enumerate<std::slice::Iter<'a, crate::BasicBlock>>,
        std::iter::Enumerate<std::slice::Iter<'a, crate::Operation>>,
        std::iter::Enumerate<std::slice::Iter<'a, Type>>,
        &'a crate::FunctionBody,
        &'a crate::BasicBlock,
        &'a crate::Operation,
        &'a Type,
        &'a crate::SliceType,
        &'a ScalarType,
        Coordinate,
        FunctionCoordinate,
        [usize; 12],
        Option<usize>,
        Option<u16>,
        Option<(Axis, ValueId)>,
        Option<Axis>,
        Option<&'a CanonicalConditionalSliceAccessV26>,
        &'a mut Budget<'a>,
    );
    [
        size_of::<Frame<'_>>(),
        size_of::<Result<Frame<'_>>>(),
        size_of::<Result<Frame<'_>>>(),
        size_of::<std::thread::Result<Result<Option<T>>>>(),
        size_of::<std::thread::Result<Result<Option<T>>>>(),
        size_of::<std::thread::Result<Result<T>>>(),
        size_of::<Result<T>>(),
        size_of::<std::thread::Result<()>>(),
        capture.checked_mul(4).ok_or(ResourceError::Arithmetic)?,
        alignment.checked_mul(4).ok_or(ResourceError::Arithmetic)?,
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| {
        total
            .checked_add(bytes)
            .ok_or(ResourceError::Arithmetic.into())
    })
}
