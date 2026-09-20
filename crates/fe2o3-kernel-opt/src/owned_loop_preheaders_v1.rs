//! Closed neutral preheader transaction, with no numbered/source/native policy.
use crate::private_cell_promotion_resources_v1 as resources;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirLoopErrorV1 as LoopError, CanonicalKirLoopLimitsV1 as Limits,
    CanonicalKirLoopPreheaderV1 as Row, CanonicalKirLoopPreheadersErrorV1 as PairError,
    CanonicalKirLoopPreheadersStorageV1 as PairStorage, CanonicalKirLoopsV1 as Loops,
    CheckedCanonicalKirLoopPreheadersV1 as Pair, check_canonical_kir_loop_preheaders_v1,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrReplayAdmissionErrorV12 as AdmissionError,
    CanonicalKernelIrReplayStorageV12 as OutputStorage,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirEdgeCoordinateV1 as Edge, Module,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrIdentityV12 as Identity,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "loop_preheaders_build_v1.rs"]
mod build;

#[derive(Debug)]
pub enum OwnedLoopPreheadersErrorV1 {
    Resource(Resource),
    Inventory(InventoryError),
    Loops(LoopError),
    Admission(AdmissionError),
    Pair(PairError),
    Recipe(&'static str),
    ForeignInput,
    Panicked,
}
type Error = OwnedLoopPreheadersErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
fn scoped<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Meter<'_, 'w>) -> Result<T>,
) -> Result<T> {
    resources::scoped(budget, run)
}
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<InventoryError> for Error {
    fn from(value: InventoryError) -> Self {
        Self::Inventory(value)
    }
}
impl From<LoopError> for Error {
    fn from(value: LoopError) -> Self {
        Self::Loops(value)
    }
}
impl From<AdmissionError> for Error {
    fn from(value: AdmissionError) -> Self {
        Self::Admission(value)
    }
}
impl From<PairError> for Error {
    fn from(value: PairError) -> Self {
        Self::Pair(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owning neutral preheaders: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Actual freshly admitted output and inert complete appended-block lineage.
/// There is no raw attachment, mutable output or cloning constructor. Retain
/// genuine source/prefix separately: this canonical relation establishes no
/// source bounds, lifetime, progress, LICM safety, native or launch authority.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedLoopPreheadersContinuationV1;
/// fn duplicate(value: &OwnedLoopPreheadersContinuationV1) -> OwnedLoopPreheadersContinuationV1 { value.clone() }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_kernel_opt::OwnedLoopPreheadersContinuationV1;
/// fn detach(value: OwnedLoopPreheadersContinuationV1) { let _ = value.output; }
/// ```
/// ```
/// use fe2o3_kernel_opt::{prepare_owned_loop_preheaders_v1,
///     OwnedLoopPreheadersContinuationV1, OwnedLoopPreheadersErrorV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn independent(input: Owner, budget: &mut Budget<'_>)
///     -> Result<OwnedLoopPreheadersContinuationV1, OwnedLoopPreheadersErrorV1> {
///     let result = prepare_owned_loop_preheaders_v1(&input, budget)?;
///     drop(input);
///     Ok(result)
/// }
/// ```
pub struct OwnedLoopPreheadersContinuationV1 {
    output: Owner,
    output_storage: OutputStorage,
    input_identity: Identity,
    rows: Vec<Row>,
    retained: usize,
}
impl OwnedLoopPreheadersContinuationV1 {
    pub const fn output(&self) -> &Owner {
        &self.output
    }
    pub const fn input_identity(&self) -> &Identity {
        &self.input_identity
    }
    pub fn preheaders(&self) -> &[Row] {
        &self.rows
    }
    /// Unreserved completed owner/header/row-capacity receipt. Reserve before use.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    /// Requires this owning receipt reserved; full identity AND independent
    /// actual-pair replay are mandatory. Witness receipt is unreserved.
    pub fn replay_against<'a>(
        &'a self,
        input: &'a Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(Pair<'a>, PairStorage)> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |meter| {
            meter.work(
                size_of::<Identity>()
                    .checked_add(3)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if self.retained != retained(self.output_storage, &self.rows)? {
                return Err(Resource::Accounting.into());
            }
            if input.canonical().identity() != &self.input_identity {
                return Err(Error::ForeignInput);
            }
            meter.derive(|b| {
                Ok(check_canonical_kir_loop_preheaders_v1(
                    input,
                    &self.output,
                    &self.rows,
                    Limits::default(),
                    b,
                )?)
            })
        })
    }
}

/// Selects all eligible original natural-loop headers once, appends neutral
/// typed forwarding blocks and redirects exact external edge occurrences.
/// Entry headers, existing preheaders and ineligible/no-external loops are
/// unchanged. Operations, conditions, case constants/order and edge arguments
/// are never rewritten. Nested loops use original edge coordinates, followed by
/// fresh output admission and independent input/output loop/pair replay.
///
/// One private candidate copy; no caller callback, selector, claimed recipe or
/// raw output. New vectors pay old/new coexistence and actual backing, and each
/// copied Type box is prepaid. Conservative candidate receipt is held without
/// early old-backing credits. Inherited owner/inventory logical accounting is
/// not allocator/RSS accounting. Success restores entry storage and transfers
/// the complete owner receipt unreserved; errors/panics drop partial objects
/// before valid-ledger cleanup. Work/peak/first-denial history persists.
pub fn prepare_owned_loop_preheaders_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<OwnedLoopPreheadersContinuationV1> {
    scoped(budget, |meter| {
        meter.reserve(header()?)?;
        let (inventory, is) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
        meter.reserve(is.retained_storage())?;
        let (loops, ls) = meter.derive(|b| Ok(Loops::derive(&inventory, Limits::default(), b)?))?;
        meter.reserve(ls.retained_storage())?;
        meter.derive(|b| Ok(loops.replay(&inventory, Limits::default(), b)?))?;
        let (mut candidate, cs) =
            meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
        meter.reserve(cs.retained_storage())?;
        let (rows, extra_candidate) = build::apply(&inventory, &loops, &mut candidate, meter)?;
        let (output, output_storage) = meter.derive(|b| {
            Ok(Owner::from_module_ref_with_verification_budget_v12(
                &candidate, b,
            )?)
        })?;
        meter.reserve(output_storage.retained_storage())?;
        let ps = {
            let (_pair, ps) = meter.derive(|b| {
                Ok(check_canonical_kir_loop_preheaders_v1(
                    input,
                    &output,
                    &rows,
                    Limits::default(),
                    b,
                )?)
            })?;
            meter.reserve(ps.retained_storage())?;
            ps
        };
        meter.release(ps.retained_storage())?;
        let retained = retained(output_storage, &rows)?;
        drop(candidate);
        meter.release(
            cs.retained_storage()
                .checked_add(extra_candidate)
                .ok_or(Resource::Arithmetic)?,
        )?;
        drop(loops);
        meter.release(ls.retained_storage())?;
        drop(inventory);
        meter.release(is.retained_storage())?;
        Ok(OwnedLoopPreheadersContinuationV1 {
            output,
            output_storage,
            input_identity: *input.canonical().identity(),
            rows,
            retained,
        })
    })
}
fn header() -> Result<usize> {
    size_of::<OwnedLoopPreheadersContinuationV1>()
        .checked_sub(size_of::<Owner>())
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn retained(output: OutputStorage, rows: &Vec<Row>) -> Result<usize> {
    header()?
        .checked_add(output.retained_storage())
        .and_then(|n| n.checked_add(rows.capacity().checked_mul(size_of::<Row>())?))
        .ok_or_else(|| Resource::Arithmetic.into())
}

#[cfg(test)]
#[path = "owned_loop_preheaders_v1_tests.rs"]
mod tests;
