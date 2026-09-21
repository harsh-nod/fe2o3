//! Owning decoded backing remains separate from short checked borrowed receipts.
use crate::{
    CanonicalRefinedForwardingHistoryErrorV1 as PrefixError,
    CheckedCanonicalRefinedForwardingHistoryV1 as PrefixChecked,
    DecodedRefinedForwardingHistoryV1 as PrefixDecoded, loop_unroll_history_rows_v1 as rows,
    loop_unroll_history_wire_v1::{
        Error as WireError, InertLoopUnrollHistoryRefV1 as Frame,
        LoopUnrollHistoryWireStorageV1 as WireStorage, MAX_LOOP_UNROLL_HISTORY_STORAGE_V1, add,
        configured,
    },
    materialize_refined_forwarding_history_v1, private_cell_promotion_resources_v1 as resources,
};
use fe2o3_kernel_analysis::{
    CanonicalKirLoopUnrollErrorV1 as UnrollError, CanonicalKirLoopUnrollLimitsV1 as Limits,
    CanonicalKirLoopUnrollOriginV1 as Origin, CanonicalKirLoopUnrollOriginsV1 as Origins,
    CheckedCanonicalKirLoopUnrollPairV1 as Pair, check_canonical_kir_loop_unroll_pair_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as Argument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirOperationCoordinateV1 as Site, VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use std::{fmt, mem::size_of};

#[derive(Debug)]
pub enum LoopUnrollHistoryErrorV1 {
    Resource(Resource),
    Prefix(PrefixError),
    Unroll(UnrollError),
    Mismatch(&'static str),
    Panicked,
}
type Error = LoopUnrollHistoryErrorV1;
impl From<Resource> for Error {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked loop-unroll history: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopUnrollHistoryStorageV1(usize);
impl LoopUnrollHistoryStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Thirteen independently admitted graph roles and complete typed row backing.
/// Equal F/U bytes do not alias owners. No source proof, execution or ABI authority.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedLoopUnrollHistoryV1;
/// fn copy(v: DecodedLoopUnrollHistoryV1<'_, '_>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedLoopUnrollHistoryV1;
/// fn escape<'a, 'w>(v: DecodedLoopUnrollHistoryV1<'a, 'w>) -> DecodedLoopUnrollHistoryV1<'static, 'w> { v }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{DecodedLoopUnrollHistoryV1, CheckedLoopUnrollHistoryV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape<'a>(v: DecodedLoopUnrollHistoryV1<'_, '_>, b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) -> CheckedLoopUnrollHistoryV1<'a> {
///     v.check_semantics(b).unwrap()
/// }
/// ```
pub struct DecodedLoopUnrollHistoryV1<'frame, 'wire> {
    frame: &'frame Frame<'wire>,
    prefix: PrefixDecoded<'frame, 'wire>,
    output: Graph,
    blocks: Vec<Origin<Block>>,
    definitions: Vec<Origin<Definition>>,
    operations: Vec<Origin<Site>>,
    terminators: Vec<Origin<Block>>,
    edges: Vec<Origin<Edge>>,
    arguments: Vec<Origin<Argument>>,
    storage: WireStorage,
}
impl<'frame, 'wire> DecodedLoopUnrollHistoryV1<'frame, 'wire> {
    pub const fn frame(&self) -> &'frame Frame<'wire> {
        self.frame
    }
    pub const fn prefix(&self) -> &PrefixDecoded<'frame, 'wire> {
        &self.prefix
    }
    pub const fn output(&self) -> &Graph {
        &self.output
    }
    pub const fn limits(&self) -> Limits {
        self.frame.limits
    }
    pub const fn storage(&self) -> WireStorage {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub fn origins(&self) -> Origins<'_> {
        Origins {
            selection: self.frame.selection,
            blocks: &self.blocks,
            definitions: &self.definitions,
            operations: &self.operations,
            terminators: &self.terminators,
            edges: &self.edges,
            arguments: &self.arguments,
        }
    }
    /// Caller retains and reserves decoded backing, frame and complete wire.
    /// Both relations are independently checked; no optimizer runs here.
    /// Success returns a new unreserved borrowed receipt. Numeric floors cannot
    /// authenticate a caller's reservations or signed source lineage.
    pub fn check_semantics<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<CheckedLoopUnrollHistoryV1<'a>, Error> {
        let required = self
            .storage
            .0
            .checked_add(self.frame.storage().retained_storage())
            .and_then(|n| n.checked_add(self.frame.canonical_bytes().len()))
            .ok_or(Resource::Arithmetic)?;
        if budget.storage_limit() > MAX_LOOP_UNROLL_HISTORY_STORAGE_V1
            || budget.storage() < required
        {
            return Err(Resource::Accounting.into());
        }
        let inherited = budget.storage();
        resources::scoped(budget, |meter| {
            let mut retained = wrapper()?;
            meter.reserve(retained)?;
            let prefix = meter.derive(|b| self.prefix.check_semantics(b).map_err(Error::Prefix))?;
            let ps = prefix.storage().retained_storage();
            meter.reserve(ps)?;
            retained = retained.checked_add(ps).ok_or(Resource::Arithmetic)?;
            let (continuation, storage) = meter.derive(|b| {
                check_canonical_kir_loop_unroll_pair_v1(
                    prefix.output(),
                    &self.output,
                    self.origins(),
                    self.frame.limits,
                    b,
                )
                .map_err(Error::Unroll)
            })?;
            let us = storage.retained_storage();
            meter.reserve(us)?;
            retained = retained.checked_add(us).ok_or(Resource::Arithmetic)?;
            meter.work(1)?;
            Ok(CheckedLoopUnrollHistoryV1 {
                prefix,
                continuation,
                limits: self.frame.limits,
                inherited,
                storage: LoopUnrollHistoryStorageV1(retained),
            })
        })
    }
}
pub struct CheckedLoopUnrollHistoryV1<'a> {
    prefix: PrefixChecked<'a>,
    continuation: Pair<'a, 'a, 'a>,
    limits: Limits,
    inherited: usize,
    storage: LoopUnrollHistoryStorageV1,
}
fn wrapper() -> Result<usize, Error> {
    size_of::<CheckedLoopUnrollHistoryV1<'_>>()
        .checked_sub(size_of::<PrefixChecked<'_>>())
        .and_then(|n| n.checked_sub(size_of::<Pair<'_, '_, '_>>()))
        .ok_or_else(|| Resource::Arithmetic.into())
}
impl<'a> CheckedLoopUnrollHistoryV1<'a> {
    pub const fn prefix(&self) -> &PrefixChecked<'a> {
        &self.prefix
    }
    pub const fn continuation(&self) -> &Pair<'a, 'a, 'a> {
        &self.continuation
    }
    pub fn output(&self) -> &'a Graph {
        self.continuation.output()
    }
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    pub const fn storage(&self) -> LoopUnrollHistoryStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    /// Fresh complete F replay followed by the independent F-to-U relation.
    /// The retained original receipt and all its external backing remain live.
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        let floor = self
            .inherited
            .checked_add(self.storage.0)
            .ok_or(Resource::Arithmetic)?;
        if budget.storage_limit() > MAX_LOOP_UNROLL_HISTORY_STORAGE_V1 || budget.storage() < floor {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.derive(|b| {
                self.prefix
                    .replay(self.prefix.limits(), b)
                    .map_err(Error::Prefix)
            })?;
            let storage = {
                let (_pair, storage) = meter.derive(|b| {
                    check_canonical_kir_loop_unroll_pair_v1(
                        self.prefix.output(),
                        self.continuation.output(),
                        self.continuation.origins(),
                        self.limits,
                        b,
                    )
                    .map_err(Error::Unroll)
                })?;
                let storage = storage.retained_storage();
                meter.reserve(storage)?;
                meter.work(1)?;
                storage
            };
            meter.release(storage)?;
            Ok(())
        })
    }
}

/// Fresh admission of F's complete decoded history and the separate U owner.
/// The complete new owning header (including six Vec handles) is prepaid before
/// child construction. Actual child receipts/backing coexist conservatively;
/// embedded header overlap is charged, never converted into a storage credit.
/// All local owners drop before same-ledger scope cleanup. Result is unreserved.
pub fn materialize_loop_unroll_history_v1<'frame, 'wire>(
    frame: &'frame Frame<'wire>,
    budget: &mut Budget<'_>,
) -> Result<DecodedLoopUnrollHistoryV1<'frame, 'wire>, WireError> {
    configured(budget)?;
    if budget.storage()
        < add(
            frame.storage().retained_storage(),
            frame.canonical_bytes().len(),
        )?
    {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        let mut retained = size_of::<DecodedLoopUnrollHistoryV1<'_, '_>>();
        meter.reserve(add(retained, rows::SCRATCH)?)?;
        let prefix = meter.derive(|b| {
            materialize_refined_forwarding_history_v1(frame.prefix(), b).map_err(WireError::Prefix)
        })?;
        let ps = prefix.storage().retained_storage();
        meter.reserve(ps)?;
        retained = add(retained, ps)?;
        let (output, storage) = meter.derive(|b| {
            Graph::from_canonical_bytes_with_verification_budget_v12(frame.output_bytes(), b)
                .map_err(WireError::Admission)
        })?;
        let us = storage.retained_storage();
        meter.reserve(us)?;
        retained = add(retained, us)?;
        let (blocks, bs) = rows::decode(frame.fields[2], 0, meter)?;
        retained = add(retained, bs)?;
        let (definitions, ds) = rows::decode(frame.fields[3], 1, meter)?;
        retained = add(retained, ds)?;
        let (operations, os) = rows::decode(frame.fields[4], 2, meter)?;
        retained = add(retained, os)?;
        let (terminators, ts) = rows::decode(frame.fields[5], 3, meter)?;
        retained = add(retained, ts)?;
        let (edges, es) = rows::decode(frame.fields[6], 4, meter)?;
        retained = add(retained, es)?;
        let (arguments, ars) = rows::decode(frame.fields[7], 5, meter)?;
        retained = add(retained, ars)?;
        meter.work(1)?;
        Ok(DecodedLoopUnrollHistoryV1 {
            frame,
            prefix,
            output,
            blocks,
            definitions,
            operations,
            terminators,
            edges,
            arguments,
            storage: WireStorage(retained),
        })
    })
}
