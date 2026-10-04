//! Source-owned fixed Policy6 prefix followed by one checked local-order tail.
//! Not a numbered policy alias, source-anchor parser, native artifact or proof.
use super::*;
use fe2o3_kernel_ir::{CanonicalKirOperationCoordinateV1, VerifiedCanonicalKernelIrIdentityV12};
use fe2o3_kernel_opt::{
    OwnedU32LocalOrderContinuationV1 as Tail, U32LocalOrderPreferenceV1, U32LocalOrderRegionV1,
    prepare_owned_u32_local_order_continuation_v1 as prepare_tail,
};
use std::mem::size_of;

/// Inert selection in the exact retained original N. Coordinates alone carry no
/// live HIR/Instance, target/xnack, recipe, proof or artifact authentication.
/// The compiler-facing entry independently admits those facts before calling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceU32LocalOrderRequestV1 {
    /// Exact original target-neutral connected identity, not current I or L.
    pub expected_source: VerifiedCanonicalKernelIrIdentityV12,
    /// Source-ordered XOR, OR, AND occurrences in one contiguous u32 diamond.
    pub operations: [CanonicalKirOperationCoordinateV1; 3],
    /// Closed deterministic choice; never an arbitrary pass list.
    pub preference: U32LocalOrderPreferenceV1,
}

/// Refusal of this separately versioned Direct source-owned composition.
#[derive(Debug)]
pub enum ProductionSourceLocalOrderErrorV1 {
    /// Canonical work, storage or accounting refusal.
    Resource(AssertOriginResourceV1),
    /// Complete retained source-to-I prefix refusal.
    Prefix(Box<ProductionCheckedOutputAdmissionErrorPolicy6V1>),
    /// Actual I/L execution or complete independent replay refusal.
    Continuation(fe2o3_kernel_opt::CheckedU32LocalOrderErrorV1),
    /// Source profile/anchor, fresh L safety or report refusal.
    Admission(ProductionCheckedOutputAdmissionErrorPolicy3V1),
    /// Private phase unwound; no partly checked owner is returned.
    Panicked,
}
type CError = ProductionSourceLocalOrderErrorV1;
type CResult<T> = Result<T, CError>;
impl fmt::Display for CError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "source-owned local-order continuation: {self:?}")
    }
}
impl Error for CError {}
impl From<AssertOriginResourceV1> for CError {
    fn from(value: AssertOriginResourceV1) -> Self {
        Self::Resource(value)
    }
}
impl From<E> for CError {
    fn from(value: E) -> Self {
        Self::Admission(value)
    }
}

/// Added L, selection metadata and fresh reports, excluding the retained prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceLocalOrderStorageV1(usize);
impl ProductionSourceLocalOrderStorageV1 {
    /// Reserve the unreserved transfer before another controlled allocation.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
struct Data {
    request: SourceU32LocalOrderRequestV1,
    tail: Tail,
    kernels: Box<[FormalMemoryObligations]>,
    added: usize,
}

/// Actual Direct N/B/fixed-prefix retained once, privately produced actual L,
/// and newly derived L reports. No supplied tail, Clone, mutation, policy cast,
/// snapshot import, descriptor or protected-publication permission is exposed.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOwnedSourceLocalOrderContinuationV1;
/// fn clone(x: ProductionOwnedSourceLocalOrderContinuationV1) { let _ = x.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_lower_mir_kernel::ProductionOwnedSourceLocalOrderContinuationV1;
/// fn replace(x: ProductionOwnedSourceLocalOrderContinuationV1) { let _ = x.data; }
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOwnedSourceLocalOrderContinuationV1 as New, ProductionCheckedOutputOwnerPolicy6V1 as Old};
/// fn relabel(x: New) -> Old { x }
/// ```
pub struct ProductionOwnedSourceLocalOrderContinuationV1 {
    prefix: ProductionCheckedOutputOwnerPolicy6V1,
    data: Data,
}
#[path = "production_checked_output_local_order_scope_v1.rs"]
mod scope;
use scope::{Binding, scoped};
#[path = "production_checked_output_local_order_origins_v1.rs"]
mod origins;
#[path = "production_checked_output_local_order_sites_v1.rs"]
mod source_sites;

#[cfg(test)]
#[path = "production_checked_output_local_order_internal_v1_tests.rs"]
mod tests;

fn prefix_error(error: ProductionCheckedOutputAdmissionErrorPolicy6V1) -> CError {
    CError::Prefix(Box::new(error))
}
fn header() -> CResult<usize> {
    size_of::<ProductionOwnedSourceLocalOrderContinuationV1>()
        .checked_sub(size_of::<ProductionCheckedOutputOwnerPolicy6V1>())
        .and_then(|n| n.checked_sub(size_of::<Tail>()))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn added(data: &Data) -> CResult<usize> {
    data.tail
        .retained_storage()
        .checked_add(header()?)
        .and_then(|n| n.checked_add(std::mem::size_of_val(data.kernels.as_ref())))
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn required(prefix: &ProductionCheckedOutputOwnerPolicy6V1, data: &Data) -> CResult<usize> {
    if added(data)? != data.added {
        return Err(AssertOriginResourceV1::Accounting.into());
    }
    prefix
        .retained_input_storage_floor_v1()
        .map_err(prefix_error)?
        .checked_add(data.added)
        .ok_or_else(|| AssertOriginResourceV1::Arithmetic.into())
}
fn check_output(
    prefix: &ProductionCheckedOutputOwnerPolicy6V1,
    request: SourceU32LocalOrderRequestV1,
    tail: &Tail,
    budget: &mut AssertOriginBudgetV1<'_>,
    binding: &Binding,
) -> CResult<Box<[FormalMemoryObligations]>> {
    prefix.verify_equivalence(budget).map_err(prefix_error)?;
    binding.check(budget)?;
    let region = origins::resolve(prefix, request, budget, binding)?;
    if tail.region() != region || tail.preference() != request.preference {
        return Err(refused(
            "local-order L",
            "exact current source region and preference",
        )
        .into());
    }
    tail.replay(prefix.output(), budget)
        .map_err(CError::Continuation)?;
    binding.check(budget)?;
    source_sites::check(prefix, tail, budget, binding)
}

impl ProductionCheckedOutputOwnerPolicy6V1 {
    /// Consumes actual N/B/I once and privately runs the checked local scheduler.
    /// Direct/RawEmpty only; there is no UnitLocal or diagnostic fallback.
    /// All inherited reservations remain owned by the caller on every exit.
    /// Success transfers only the returned added-storage receipt unreserved.
    pub fn continue_source_local_order_v1(
        self,
        request: SourceU32LocalOrderRequestV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> CResult<(
        ProductionOwnedSourceLocalOrderContinuationV1,
        ProductionSourceLocalOrderStorageV1,
    )> {
        let minimum = self
            .retained_input_storage_floor_v1()
            .map_err(prefix_error)?;
        scoped(minimum, budget, |budget, binding| {
            self.verify_equivalence(budget).map_err(prefix_error)?;
            binding.check(budget)?;
            let region = origins::resolve(&self, request, budget, binding)?;
            let tail = prepare_tail(self.output(), region, request.preference, budget)
                .map_err(CError::Continuation)?;
            binding.check(budget)?;
            budget.reserve_storage(tail.retained_storage())?;
            let kernels = check_output(&self, request, &tail, budget, binding)?;
            binding.check(budget)?;
            budget.reserve_storage(header()?)?;
            let mut data = Data {
                request,
                tail,
                kernels,
                added: 0,
            };
            data.added = added(&data)?;
            let storage = ProductionSourceLocalOrderStorageV1(data.added);
            Ok((
                ProductionOwnedSourceLocalOrderContinuationV1 { prefix: self, data },
                storage,
            ))
        })
    }
}
impl ProductionOwnedSourceLocalOrderContinuationV1 {
    /// Unchanged original Direct source-to-I prefix.
    pub const fn prefix(&self) -> &ProductionCheckedOutputOwnerPolicy6V1 {
        &self.prefix
    }
    /// Exact privately produced I/L local-order execution and inert observations.
    pub const fn continuation(&self) -> &Tail {
        &self.data.tail
    }
    /// Actual L, not the historical I, B or N.
    pub fn output(&self) -> &StoreOwner {
        self.data.tail.output()
    }
    /// Fresh formal obligations derived from actual L in kernel order.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.data.kernels
    }
    /// Original inert source selection, independently rechecked on replay.
    pub const fn request(&self) -> SourceU32LocalOrderRequestV1 {
        self.data.request
    }
    /// Additional storage excluding all caller-inherited reservations.
    pub const fn additional_retained_storage_v1(&self) -> usize {
        self.data.added
    }
    /// No descriptor, protected publication, proof or execution authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Prefix minimum plus exact added storage; B remains separately prepaid.
    pub fn retained_input_storage_floor_v1(&self) -> CResult<usize> {
        required(&self.prefix, &self.data)
    }
    /// Replays all source/prefix/anchor/tail checks and compares fresh L reports.
    /// Historical formal-engine metering exclusions remain unchanged.
    pub fn verify_equivalence(&self, budget: &mut AssertOriginBudgetV1<'_>) -> CResult<()> {
        scoped(
            self.retained_input_storage_floor_v1()?,
            budget,
            |budget, binding| {
                let fresh = check_output(
                    &self.prefix,
                    self.data.request,
                    &self.data.tail,
                    budget,
                    binding,
                )?;
                binding.check(budget)?;
                if fresh != self.data.kernels {
                    return Err(E::Formal(
                        crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
                    )
                    .into());
                }
                Ok(())
            },
        )
    }
}
