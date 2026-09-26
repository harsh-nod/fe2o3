use std::cell::RefCell;

// A consuming accounting observation, never an allocation or refund permit.
// It cannot borrow the layouts across the existing mutable root entry scope.
pub(super) struct SourceStorageEmissionCreditV29 {
    layouts: usize,
    source: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    entry: usize,
    persistent: usize,
}

impl SourceStorageEmissionCreditV29 {
    pub(super) fn headers() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<Self>(),
            size_of::<Option<Self>>(),
            size_of::<Result<Self, Error>>(),
        ])
    }

    pub(super) fn root_credit(
        &self,
        layouts: &SourceStorageLayoutsV29<'_>,
        budget: &Budget<'_>,
    ) -> Option<usize> {
        if self.layouts != layouts as *const SourceStorageLayoutsV29<'_> as usize
            || self.source != layouts.owner as *const ProductionSemanticSsaOwnerV1 as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || self.slot != budget as *const Budget<'_> as usize
            || layouts.lease.root.get().is_some()
            || layouts.lease.owned.get() != layouts.lease.persistent.get()
            || layouts.lease.custody(budget).is_err()
        {
            return None;
        }
        let growth = layouts
            .lease
            .persistent
            .get()
            .checked_sub(self.persistent)?;
        budget
            .storage()
            .checked_sub(self.entry.checked_add(growth)?)
    }

    pub(super) fn into_root_credit(
        self,
        layouts: &SourceStorageLayoutsV29<'_>,
        budget: &Budget<'_>,
    ) -> Option<usize> {
        self.root_credit(layouts, budget)
    }
}

impl SourceStorageLayoutsV29<'_> {
    #[cfg(test)]
    pub(super) fn persistent_storage_for_test(&self) -> usize {
        assert!(self.lease.root.get().is_none());
        assert_eq!(self.lease.owned.get(), self.lease.persistent.get());
        self.lease.persistent.get()
    }

    pub(super) fn capture_emission_credit(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageEmissionCreditV29, Error> {
        self.check_owner(owner, budget)?;
        // Capture and consuming cleanup comparisons are prepaid together.
        self.lease.work(12, budget)?;
        if self.lease.root.get().is_some() || self.lease.owned.get() != self.lease.persistent.get()
        {
            self.lease.record(&ArgumentResourceV1::Accounting.into());
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let entry = budget.storage();
        let headers = SourceStorageEmissionCreditV29::headers()
            .map_err(Error::from)
            .inspect_err(|error| self.lease.record(error))?;
        budget
            .reserve_storage(headers)
            .map_err(Error::from)
            .inspect_err(|error| self.lease.record(error))?;
        Ok(SourceStorageEmissionCreditV29 {
            layouts: self as *const Self as usize,
            source: owner as *const ProductionSemanticSsaOwnerV1 as usize,
            ledger: budget.work_ledger_identity_v1(),
            slot: budget as *const Budget<'_> as usize,
            entry,
            persistent: self.lease.persistent.get(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum SourceStorageFailureObservationV29 {
    Resource(ArgumentResourceV1),
    Unsupported {
        function: u32,
        block: Option<u32>,
        statement: Option<u32>,
        detail: &'static str,
    },
    OwnedDiagnostic,
}

impl SourceStorageFailureObservationV29 {
    fn from_error(error: &Error) -> Self {
        match error {
            Error::ArgumentCorrespondenceResource(resource) => Self::Resource(*resource),
            Error::Unsupported {
                function,
                block,
                statement,
                detail,
            } => Self::Unsupported {
                function: *function,
                block: *block,
                statement: *statement,
                detail: *detail,
            },
            _ => Self::OwnedDiagnostic,
        }
    }

    fn error(self) -> Error {
        match self {
            Self::Resource(resource) => resource.into(),
            Self::Unsupported {
                function,
                block,
                statement,
                detail,
            } => Error::Unsupported {
                function,
                block,
                statement,
                detail,
            },
            Self::OwnedDiagnostic => {
                error("source storage root is poisoned by an earlier owned diagnostic")
            }
        }
    }
}

#[derive(Debug)]
enum SourceStorageFirstErrorV29 {
    Clear,
    Owned(Error),
    Returned(SourceStorageFailureObservationV29),
}

#[derive(Debug)]
struct SourceStorageFailureCellV29(RefCell<SourceStorageFirstErrorV29>);

impl SourceStorageFailureCellV29 {
    fn new() -> Self {
        Self(RefCell::new(SourceStorageFirstErrorV29::Clear))
    }

    fn observation(&self) -> Option<SourceStorageFailureObservationV29> {
        match &*self.0.borrow() {
            SourceStorageFirstErrorV29::Clear => None,
            SourceStorageFirstErrorV29::Owned(error) => {
                Some(SourceStorageFailureObservationV29::from_error(error))
            }
            SourceStorageFirstErrorV29::Returned(observation) => Some(*observation),
        }
    }

    fn first_error(&self) -> Option<Error> {
        self.observation()
            .map(SourceStorageFailureObservationV29::error)
    }

    fn resource(&self) -> Option<ArgumentResourceV1> {
        match self.observation() {
            Some(SourceStorageFailureObservationV29::Resource(resource)) => Some(resource),
            _ => None,
        }
    }

    fn record(&self, error: Error) {
        let mut first = self.0.borrow_mut();
        if matches!(*first, SourceStorageFirstErrorV29::Clear) {
            *first = SourceStorageFirstErrorV29::Owned(error);
        }
    }

    fn into_first(self) -> Option<Error> {
        match self.0.into_inner() {
            SourceStorageFirstErrorV29::Clear => None,
            SourceStorageFirstErrorV29::Owned(error) => Some(error),
            SourceStorageFirstErrorV29::Returned(observation) => Some(observation.error()),
        }
    }

    // Called only by consuming root settlement, never by a query or a handle.
    // The original diagnostic moves once; the permanent poison cannot be reset.
    fn return_first_from_consuming_scope(&self) -> Option<Error> {
        let mut first = self.0.borrow_mut();
        let observation = match &*first {
            SourceStorageFirstErrorV29::Clear => return None,
            SourceStorageFirstErrorV29::Owned(error) => {
                SourceStorageFailureObservationV29::from_error(error)
            }
            SourceStorageFirstErrorV29::Returned(observation) => {
                return Some(observation.error());
            }
        };
        match std::mem::replace(
            &mut *first,
            SourceStorageFirstErrorV29::Returned(observation),
        ) {
            SourceStorageFirstErrorV29::Owned(error) => Some(error),
            SourceStorageFirstErrorV29::Clear => None,
            SourceStorageFirstErrorV29::Returned(prior) => Some(prior.error()),
        }
    }
}

enum SourceReferenceFailureBindingV29<'root> {
    Owned(SourceStorageFailureCellV29),
    Borrowed(&'root SourceStorageFailureCellV29),
}

pub(super) struct SourceReferenceFailureV29<'root> {
    binding: SourceReferenceFailureBindingV29<'root>,
}

impl<'root> SourceReferenceFailureV29<'root> {
    pub(super) fn owned() -> Self {
        Self {
            binding: SourceReferenceFailureBindingV29::Owned(SourceStorageFailureCellV29::new()),
        }
    }

    fn borrowed(cell: &'root SourceStorageFailureCellV29) -> Self {
        Self {
            binding: SourceReferenceFailureBindingV29::Borrowed(cell),
        }
    }

    fn cell(&self) -> &SourceStorageFailureCellV29 {
        match &self.binding {
            SourceReferenceFailureBindingV29::Owned(cell) => cell,
            SourceReferenceFailureBindingV29::Borrowed(cell) => cell,
        }
    }

    pub(super) fn get(&self) -> Option<ArgumentResourceV1> {
        self.cell().resource()
    }

    pub(super) fn first_error(&self) -> Option<Error> {
        self.cell().first_error()
    }

    pub(super) fn record_resource(&self, resource: ArgumentResourceV1) {
        self.cell().record(resource.into());
    }

    pub(super) fn into_owned_first(self) -> Option<Error> {
        match self.binding {
            SourceReferenceFailureBindingV29::Owned(cell) => cell.into_first(),
            SourceReferenceFailureBindingV29::Borrowed(_) => None,
        }
    }

    fn borrows(&self, cell: &SourceStorageFailureCellV29) -> bool {
        matches!(self.binding, SourceReferenceFailureBindingV29::Borrowed(found) if std::ptr::eq(found, cell))
    }

    pub(super) fn matches_recorded(&self, recorded: &RecordedStorageFailureV29<'_>) -> bool {
        std::ptr::eq(self.cell(), recorded.cell)
    }
}

#[derive(Debug)]
pub(super) struct RecordedStorageFailureV29<'view> {
    cell: &'view SourceStorageFailureCellV29,
}

#[derive(Debug)]
pub(super) enum SourceStorageRootCallbackErrorV29<'view> {
    Recorded(RecordedStorageFailureV29<'view>),
    Owned(Error),
}

impl<'view> From<RecordedStorageFailureV29<'view>> for SourceStorageRootCallbackErrorV29<'view> {
    fn from(recorded: RecordedStorageFailureV29<'view>) -> Self {
        Self::Recorded(recorded)
    }
}

impl From<Error> for SourceStorageRootCallbackErrorV29<'_> {
    fn from(error: Error) -> Self {
        Self::Owned(error)
    }
}

impl From<ArgumentResourceV1> for SourceStorageRootCallbackErrorV29<'_> {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Owned(error.into())
    }
}

const SOURCE_STORAGE_ROOT_FIXED_WORK_V29: usize = 3;

#[derive(Clone, Copy)]
struct SourceStorageRootRetainedV29 {
    floor: usize,
    delta: usize,
}

#[derive(Clone, Copy)]
struct SourceStorageRootCustodyV29 {
    instances: usize,
    root: ProductionCallInstanceIdV1,
    table_owned: usize,
    floor: usize,
    retained: Option<SourceStorageRootRetainedV29>,
    poisoned: bool,
}

impl SourceStorageRootCustodyV29 {
    fn required(self, owned: usize) -> Option<usize> {
        if self.poisoned {
            return None;
        }
        let delta = owned.checked_sub(self.table_owned)?;
        match self.retained {
            Some(retained) => retained
                .floor
                .checked_add(delta.checked_sub(retained.delta)?),
            None => self.floor.checked_add(delta),
        }
    }
}

pub(super) struct SourceStorageRootCheckpointV29<'scope, 'source> {
    layouts: &'scope mut SourceStorageLayoutsV29<'source>,
    instances: &'scope ExecutionInstancesV29<'source>,
    table_owned: usize,
    floor: usize,
}

pub(super) struct SourceStorageRootRefundV29<'scope, 'source> {
    layouts: &'scope mut SourceStorageLayoutsV29<'source>,
    table_owned: usize,
    scratch: usize,
    plan: usize,
    required: usize,
}

impl<'scope, 'source> SourceStorageRootCheckpointV29<'scope, 'source> {
    pub(super) fn begin(
        layouts: &'scope mut SourceStorageLayoutsV29<'source>,
        instances: &'scope ExecutionInstancesV29<'source>,
        budget: &Budget<'_>,
    ) -> Result<Self, Error> {
        layouts.lease.check(budget)?;
        if layouts.lease.root.get().is_some() || !std::ptr::eq(layouts.owner, instances.owner()) {
            let error = error("source storage root changed its module table or active scope");
            layouts.lease.failure.record(error);
            return Err(layouts
                .lease
                .failure
                .first_error()
                .unwrap_or_else(|| ArgumentResourceV1::Accounting.into()));
        }
        let table_owned = layouts.lease.owned.get();
        if table_owned != layouts.lease.persistent.get() {
            layouts
                .lease
                .failure
                .record(ArgumentResourceV1::Accounting.into());
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget.storage();
        layouts.lease.root.set(Some(SourceStorageRootCustodyV29 {
            instances: instances as *const ExecutionInstancesV29<'_> as usize,
            root: instances.root(),
            table_owned,
            floor,
            retained: None,
            poisoned: false,
        }));
        Ok(Self {
            layouts,
            instances,
            table_owned,
            floor,
        })
    }

    pub(super) fn additional_work() -> usize {
        SOURCE_STORAGE_ROOT_FIXED_WORK_V29
    }

    pub(super) fn arena<'root>(
        &'root self,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageRootArenaV29<'root, 'source>, Error> {
        SourceStorageRootArenaV29::new(self.layouts, self.instances, budget)
    }

    pub(super) fn retained(&self, floor: usize, budget: &Budget<'_>) -> Result<(), Error> {
        self.layouts.lease.custody(budget)?;
        let mut state = self
            .layouts
            .lease
            .root
            .get()
            .ok_or_else(|| Error::from(ArgumentResourceV1::Accounting))?;
        let delta = self
            .layouts
            .lease
            .owned
            .get()
            .checked_sub(self.table_owned)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if state.retained.is_some() || floor != budget.storage() {
            self.layouts
                .lease
                .failure
                .record(ArgumentResourceV1::Accounting.into());
            return Err(ArgumentResourceV1::Accounting.into());
        }
        floor
            .checked_sub(self.floor)
            .and_then(|value| value.checked_sub(delta))
            .ok_or(ArgumentResourceV1::Accounting)?;
        state.retained = Some(SourceStorageRootRetainedV29 { floor, delta });
        self.layouts.lease.root.set(Some(state));
        Ok(())
    }

    pub(super) fn record(&self, error: Error) {
        self.layouts.lease.failure.record(error);
    }

    pub(super) fn observation(&self) -> Option<Error> {
        self.layouts.lease.failure.first_error()
    }

    // Plan, arena and protected construction locals must already be dropped.
    // A rejected callback result remains paid until bounded cleanup; release
    // rechecks custody afterward. This consumes the only settlement token.
    pub(super) fn prepare_refund(
        self,
        budget: &Budget<'_>,
    ) -> (
        Option<Error>,
        Result<SourceStorageRootRefundV29<'scope, 'source>, Error>,
    ) {
        let checked = (|| {
            self.layouts.lease.custody(budget)?;
            let state = self
                .layouts
                .lease
                .root
                .get()
                .ok_or(ArgumentResourceV1::Accounting)?;
            if state.instances != self.instances as *const ExecutionInstancesV29<'_> as usize
                || state.root != self.instances.root()
                || state.table_owned != self.table_owned
                || state.floor != self.floor
                || !std::ptr::eq(self.layouts.owner, self.instances.owner())
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let delta = self
                .layouts
                .lease
                .owned
                .get()
                .checked_sub(self.table_owned)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let persistent = self.layouts.lease.persistent.get();
            let growth = persistent
                .checked_sub(self.table_owned)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let scratch = delta
                .checked_sub(growth)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let (plan, required) = match state.retained {
                Some(retained) => (
                    retained
                        .floor
                        .checked_sub(self.floor)
                        .and_then(|value| value.checked_sub(retained.delta))
                        .ok_or(ArgumentResourceV1::Accounting)?,
                    retained
                        .floor
                        .checked_add(
                            delta
                                .checked_sub(retained.delta)
                                .ok_or(ArgumentResourceV1::Accounting)?,
                        )
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                ),
                None => (
                    budget
                        .storage()
                        .checked_sub(self.floor)
                        .and_then(|value| value.checked_sub(delta))
                        .ok_or(ArgumentResourceV1::Accounting)?,
                    budget.storage(),
                ),
            };
            if budget.storage() < required {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok((scratch, plan, required, persistent))
        })();
        if let Err(error) = &checked {
            if let Error::ArgumentCorrespondenceResource(resource) = error {
                self.layouts.lease.failure.record((*resource).into());
            }
            if let Some(mut state) = self.layouts.lease.root.get() {
                state.poisoned = true;
                self.layouts.lease.root.set(Some(state));
            }
        }
        let first = self
            .layouts
            .lease
            .failure
            .return_first_from_consuming_scope();
        let refund =
            checked.map(
                |(scratch, plan, required, persistent)| SourceStorageRootRefundV29 {
                    layouts: self.layouts,
                    table_owned: persistent,
                    scratch,
                    plan,
                    required,
                },
            );
        (first, refund)
    }
}

pub(super) fn with_source_storage_root_v29<'source, 'work, R>(
    layouts: &mut SourceStorageLayoutsV29<'source>,
    instances: &ExecutionInstancesV29<'source>,
    budget: &mut Budget<'work>,
    consume: impl for<'owner, 'root, 'view> FnOnce(
        &'view SourceReferencePlanV29<'owner, 'source>,
        SourceStorageRootV29<'view, 'root, 'source>,
        &mut Budget<'work>,
    ) -> Result<
        R,
        SourceStorageRootCallbackErrorV29<'view>,
    >,
) -> Result<R, Error> {
    with_source_storage_descriptor_root_v29(layouts, instances, None, budget, consume)
}

pub(super) fn with_source_storage_descriptor_root_v29<'source, 'work, R>(
    layouts: &mut SourceStorageLayoutsV29<'source>,
    instances: &ExecutionInstancesV29<'source>,
    descriptor_root: Option<super::kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'_>>,
    budget: &mut Budget<'work>,
    consume: impl for<'owner, 'root, 'view> FnOnce(
        &'view SourceReferencePlanV29<'owner, 'source>,
        SourceStorageRootV29<'view, 'root, 'source>,
        &mut Budget<'work>,
    ) -> Result<
        R,
        SourceStorageRootCallbackErrorV29<'view>,
    >,
) -> Result<R, Error> {
    super::with_source_reference_descriptor_scope_v29(
        instances,
        super::SourceReferenceStorageV29::ScalarCells,
        Some(layouts),
        descriptor_root,
        budget,
        |plan, root, budget| {
            let root = root.ok_or_else(|| Error::from(ArgumentResourceV1::Accounting))?;
            consume(plan, root, budget)
        },
    )
}

impl SourceStorageRootRefundV29<'_, '_> {
    pub(super) fn release(self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.layouts.lease.custody(budget)?;
        if budget.storage() < self.required
            || self.layouts.lease.owned.get().checked_sub(self.scratch) != Some(self.table_owned)
        {
            self.layouts
                .lease
                .failure
                .record(ArgumentResourceV1::Accounting.into());
            if let Some(mut state) = self.layouts.lease.root.get() {
                state.poisoned = true;
                self.layouts.lease.root.set(Some(state));
            }
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.layouts.lease.refund(self.scratch, budget)?;
        budget.release_storage(self.plan)?;
        self.layouts.lease.root.set(None);
        Ok(())
    }
}

pub(super) fn with_source_storage_descriptor_demands_root_v29<'source, 'work, R>(
    layouts: &mut SourceStorageLayoutsV29<'source>,
    instances: &ExecutionInstancesV29<'source>,
    descriptor_root: Option<super::kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'_>>,
    demands: super::source_storage_demands_v29::RootStorageDemandsV29<'_, 'source>,
    budget: &mut Budget<'work>,
    consume: impl for<'owner, 'root, 'view> FnOnce(
        &'view SourceReferencePlanV29<'owner, 'source>,
        SourceStorageRootV29<'view, 'root, 'source>,
        &mut Budget<'work>,
    ) -> Result<
        R,
        SourceStorageRootCallbackErrorV29<'view>,
    >,
) -> Result<R, Error> {
    super::with_source_reference_descriptor_demands_scope_v29(
        instances,
        super::SourceReferenceStorageV29::ScalarCells,
        Some(layouts),
        descriptor_root,
        Some(demands),
        budget,
        |plan, root, budget| {
            let root = root.ok_or_else(|| Error::from(ArgumentResourceV1::Accounting))?;
            consume(plan, root, budget)
        },
    )
}
