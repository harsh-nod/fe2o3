//! Typed V18 successor for the shared lifecycle-only erasure rule.
use super::{
    Budget, ProductionExecutionDischargeStorageV29, ProductionExecutionErasureV29, ResourceError,
    erasure_core,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrReplayAdmissionErrorV18, StorageLayoutLimitsV1,
    VerifiedCanonicalKernelIrIdentityV18, VerifiedCanonicalKernelIrModuleV18,
};
use std::{error::Error, fmt, mem::size_of};

/// Failure to erase lifecycle operations while preserving a V18 storage graph.
#[derive(Debug)]
pub enum ProductionExecutionDischargeErrorV18 {
    /// Bounded decoding or copying of the original input failed.
    Input(CanonicalKernelIrReplayAdmissionErrorV18),
    /// Fresh storage-aware admission of the modified candidate failed.
    Output(CanonicalKernelIrReplayAdmissionErrorV18),
    /// Work, storage, allocation or accounting validation failed.
    Resource(ResourceError),
    /// The input contains no execution-capability operations to erase.
    NoExecution,
    /// An execution operation requires additional semantic lowering.
    UnsupportedExecution,
    /// Independent replay found a change outside the permitted erasures.
    ReplayMismatch,
}
type DischargeError = ProductionExecutionDischargeErrorV18;

impl From<ResourceError> for DischargeError {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}
impl From<erasure_core::RuleError> for DischargeError {
    fn from(error: erasure_core::RuleError) -> Self {
        match error {
            erasure_core::RuleError::Resource(error) => Self::Resource(error),
            erasure_core::RuleError::NoExecution => Self::NoExecution,
            erasure_core::RuleError::UnsupportedExecution => Self::UnsupportedExecution,
            erasure_core::RuleError::ReplayMismatch => Self::ReplayMismatch,
        }
    }
}
impl fmt::Display for DischargeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => write!(formatter, "V18 execution input copy failed: {error}"),
            Self::Output(error) => {
                write!(formatter, "V18 execution output admission failed: {error}")
            }
            Self::Resource(error) => error.fmt(formatter),
            Self::NoExecution => {
                formatter.write_str("execution discharge has no execution subject")
            }
            Self::UnsupportedExecution => formatter
                .write_str("execution discharge requires a separate tile/fragment lowering"),
            Self::ReplayMismatch => formatter.write_str("V18 execution erasure replay mismatch"),
        }
    }
}
impl Error for DischargeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) | Self::Output(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::NoExecution | Self::UnsupportedExecution | Self::ReplayMismatch => None,
        }
    }
}

/// Fresh V18 graph after exact, independently replayed lifecycle-only erasure.
///
/// The complete storage table is preserved. This establishes neither source
/// custody nor initialized bytes, active variants, alias, backend or launch
/// correctness. The input identity is a locator, not an original-source owner.
/// Production must retain its actual source/pending ancestor separately.
///
/// ```no_run
/// use fe2o3_lower_mir_kernel::{ProductionExecutionDischargeV18,
///     ProductionExecutionDischargeErrorV18};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV18, StorageLayoutLimitsV1,
///     CanonicalKernelIrVerificationResourceBudgetV1};
/// fn inspect(input: &VerifiedCanonicalKernelIrModuleV18, limits: StorageLayoutLimitsV1,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> Result<(), ProductionExecutionDischargeErrorV18>
/// {
///     // The caller keeps the actual input owner's reservation live.
///     let (successor, receipt) = ProductionExecutionDischargeV18::try_discharge(input, limits, budget)?;
///     budget.reserve_storage(receipt.retained_storage())?;
///     assert_eq!(successor.input_identity(), input.identity());
///     assert_eq!(successor.output().module().storage_layouts, input.module().storage_layouts);
///     drop(successor);
///     budget.release_storage(receipt.retained_storage())?;
///     Ok(())
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionExecutionDischargeV18;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionExecutionDischargeV18>();
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionExecutionDischargeV18;
/// fn mutate(owner: &mut ProductionExecutionDischargeV18) {
///     owner.output().module().storage_layouts.clear();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionExecutionDischargeV18;
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
/// fn convert(owner: &ProductionExecutionDischargeV18) -> &VerifiedCanonicalKernelIrModuleV12 {
///     owner.output()
/// }
/// ```
#[derive(Debug)]
pub struct ProductionExecutionDischargeV18 {
    input_identity: VerifiedCanonicalKernelIrIdentityV18,
    output: VerifiedCanonicalKernelIrModuleV18,
    erased_operations: Vec<ProductionExecutionErasureV29>,
}

impl ProductionExecutionDischargeV18 {
    /// Keep the input owner's reservation live. Every Result and unwind restores
    /// incoming storage after dropping temporary candidates, retaining work,
    /// peak and first-denial history. Success transfers the exact logical
    /// receipt, which must be reserved before another controlled allocation.
    /// Canonical diagnostic payloads retain their canonical API transfer contract.
    /// Limits are caller policy, never inferred from the candidate's table.
    pub fn try_discharge(
        input: &VerifiedCanonicalKernelIrModuleV18,
        layout_limits: StorageLayoutLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, ProductionExecutionDischargeStorageV29), DischargeError> {
        let mut scope = DischargeScope::enter(budget)?;
        let result = discharge(input, layout_limits, scope.budget);
        let released = scope.finish();
        match result {
            Err(error) => Err(error),
            Ok(output) => {
                released?;
                Ok(output)
            }
        }
    }

    /// Identifies the input for replay; identity alone does not grant source custody.
    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV18 {
        &self.input_identity
    }
    /// Borrows the freshly admitted V18 output with the original storage table.
    pub const fn output(&self) -> &VerifiedCanonicalKernelIrModuleV18 {
        &self.output
    }
    /// Returns removed operations' original input coordinates in traversal order.
    pub fn erased_operations(&self) -> &[ProductionExecutionErasureV29] {
        &self.erased_operations
    }
}

struct DischargeScope<'a, 'work> {
    budget: &'a mut Budget<'work>,
    floor: Option<usize>,
}
impl<'a, 'work> DischargeScope<'a, 'work> {
    fn headers() -> Result<usize, ResourceError> {
        type Outcome = Result<
            (
                ProductionExecutionDischargeV18,
                ProductionExecutionDischargeStorageV29,
            ),
            DischargeError,
        >;
        [
            (1, size_of::<Self>()),
            (2, size_of::<Result<Self, ResourceError>>()),
            (3, size_of::<Outcome>()),
            (2, size_of::<Result<(), ResourceError>>()),
        ]
        .into_iter()
        .try_fold(0usize, |sum, (count, size)| {
            size.checked_mul(count)
                .and_then(|bytes| sum.checked_add(bytes))
                .ok_or(ResourceError::Arithmetic)
        })
    }

    fn enter(budget: &'a mut Budget<'work>) -> Result<Self, ResourceError> {
        budget.charge_work(1)?;
        let floor = budget.storage();
        budget.reserve_storage(Self::headers()?)?;
        Ok(Self {
            budget,
            floor: Some(floor),
        })
    }

    fn finish(&mut self) -> Result<(), ResourceError> {
        match self.floor.take() {
            Some(floor) => {
                let released = self
                    .budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ResourceError::Accounting)?;
                self.budget.release_storage(released)
            }
            None => Ok(()),
        }
    }
}
impl Drop for DischargeScope<'_, '_> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn discharge(
    input: &VerifiedCanonicalKernelIrModuleV18,
    layout_limits: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ProductionExecutionDischargeV18,
        ProductionExecutionDischargeStorageV29,
    ),
    DischargeError,
> {
    let count = erasure_core::preflight(input.module(), input.canonical_bytes().len(), budget)?;
    let wrapper_bytes = size_of::<ProductionExecutionDischargeV18>()
        .checked_sub(size_of::<VerifiedCanonicalKernelIrModuleV18>())
        .ok_or(ResourceError::Accounting)?;
    let rows_bytes = count
        .checked_mul(size_of::<ProductionExecutionErasureV29>())
        .ok_or(ResourceError::Arithmetic)?;
    let extra = wrapper_bytes
        .checked_add(rows_bytes)
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(extra)?;
    let mut erased_operations = Vec::new();
    erased_operations
        .try_reserve_exact(count)
        .map_err(|_| ResourceError::Allocation)?;
    if erased_operations.capacity() != count {
        return Err(ResourceError::Accounting.into());
    }
    let (mut candidate, candidate_storage) = input
        .copy_module_for_transformation_v18(budget)
        .map_err(DischargeError::Input)?;
    let candidate_bytes = candidate_storage.retained_storage();
    #[cfg(test)]
    let candidate_bytes = faults::transfer(faults::Point::InputTransfer, candidate_bytes, budget);
    budget.reserve_storage(candidate_bytes)?;
    #[cfg(test)]
    faults::checkpoint(faults::Point::InputAdopted, budget);
    erasure_core::erase(
        &mut candidate,
        input.canonical_bytes().len(),
        count,
        &mut erased_operations,
        budget,
    )?;
    #[cfg(test)]
    faults::checkpoint(faults::Point::Erased, budget);
    let (output, output_storage) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            &candidate,
            layout_limits,
            budget,
        )
        .map_err(DischargeError::Output)?;
    let output_bytes = output_storage.retained_storage();
    #[cfg(test)]
    let output_bytes = faults::transfer(faults::Point::OutputTransfer, output_bytes, budget);
    budget.reserve_storage(output_bytes)?;
    #[cfg(test)]
    faults::checkpoint(faults::Point::OutputAdopted, budget);
    drop(candidate);
    budget.release_storage(candidate_bytes)?;
    replay(input, &output, &erased_operations, budget)?;
    #[cfg(test)]
    faults::checkpoint(faults::Point::Replayed, budget);
    let retained = extra
        .checked_add(output_bytes)
        .ok_or(ResourceError::Arithmetic)?;
    Ok((
        ProductionExecutionDischargeV18 {
            input_identity: *input.identity(),
            output,
            erased_operations,
        },
        ProductionExecutionDischargeStorageV29 { retained },
    ))
}

fn replay(
    input: &VerifiedCanonicalKernelIrModuleV18,
    output: &VerifiedCanonicalKernelIrModuleV18,
    rows: &[ProductionExecutionErasureV29],
    budget: &mut Budget<'_>,
) -> Result<(), DischargeError> {
    erasure_core::replay(
        input.module(),
        input.canonical_bytes().len(),
        output.module(),
        output.canonical_bytes().len(),
        rows,
        erasure_core::TableProfile::Complete,
        budget,
    )
    .map_err(DischargeError::from)
}

#[cfg(test)]
mod faults {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum Point {
        InputTransfer,
        InputAdopted,
        Erased,
        OutputTransfer,
        OutputAdopted,
        Replayed,
    }
    pub(super) enum Fault {
        Observe,
        Deny,
        Panic(Box<dyn std::any::Any + Send>),
    }
    pub(super) type History = (usize, usize, usize, Option<usize>);
    thread_local! {
        pub(super) static ARMED: RefCell<Option<(Point, Fault)>> = const { RefCell::new(None) };
        pub(super) static SEEN: Cell<Option<History>> = const { Cell::new(None) };
    }
    fn take(point: Point, budget: &Budget<'_>) -> Option<Fault> {
        let fault = ARMED.with(|armed| {
            let mut armed = armed.borrow_mut();
            if armed
                .as_ref()
                .is_some_and(|(expected, _)| *expected == point)
            {
                armed.take().map(|(_, fault)| fault)
            } else {
                None
            }
        });
        if fault.is_some() {
            SEEN.set(Some((
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage(),
            )));
        }
        fault
    }
    pub(super) fn transfer(point: Point, actual: usize, budget: &Budget<'_>) -> usize {
        match take(point, budget) {
            None | Some(Fault::Observe) => actual,
            Some(Fault::Panic(payload)) => std::panic::resume_unwind(payload),
            Some(Fault::Deny) => budget
                .storage_limit()
                .checked_add(1)
                .unwrap()
                .checked_sub(budget.storage())
                .unwrap(),
        }
    }
    pub(super) fn checkpoint(point: Point, budget: &Budget<'_>) {
        if let Some(fault) = take(point, budget) {
            match fault {
                Fault::Observe => {}
                Fault::Panic(payload) => std::panic::resume_unwind(payload),
                Fault::Deny => panic!("denial fault requires a transfer point"),
            }
        }
    }
}

#[cfg(test)]
#[path = "production_execution_discharge_v18_tests.rs"]
mod tests;
