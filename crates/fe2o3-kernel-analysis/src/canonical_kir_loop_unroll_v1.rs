//! Independent bounded literal-loop cloning relation. No source or execution authority.
use crate::{
    CanonicalKirGuardDistanceV1 as Distance, CanonicalKirGuardedUpdateV1 as Update,
    CanonicalKirInductionErrorV1 as FactsError, CanonicalKirInductionFactsV1 as Facts,
    CanonicalKirInductionOutcomeV1 as Outcome, CanonicalKirInventoryErrorV1 as InventoryError,
    CanonicalKirInventoryV1 as Inventory, CanonicalKirIterationScopeV1 as Completion,
    CanonicalKirLoopErrorV1 as LoopError, CanonicalKirLoopLimitsV1, CanonicalKirLoopsV1 as Loops,
    canonical_kir_private_cell_pair_resources_v1 as resources,
};
use fe2o3_kernel_ir::{
    AddressSpace, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as Argument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirOperationCoordinateV1 as Site, CastKind, OperationKind, ScalarType, Terminator,
    Type, ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

/// New, exact component limits; old loop policies and defaults are unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoopUnrollLimitsV1 {
    pub loops: CanonicalKirLoopLimitsV1,
    /// Valid configuration is 0..=8. Zero still admits a proved zero-trip loop.
    pub max_iterations: u8,
    pub max_output_operand_uses: usize,
    pub max_output_edge_arguments: usize,
    pub max_origin_rows: usize,
}
impl Default for CanonicalKirLoopUnrollLimitsV1 {
    fn default() -> Self {
        Self {
            loops: Default::default(),
            max_iterations: 8,
            max_output_operand_uses: 1_048_576,
            max_output_edge_arguments: 1_048_576,
            max_origin_rows: 4_194_304,
        }
    }
}
type Limits = CanonicalKirLoopUnrollLimitsV1;

/// Inert copy role. OmittedBody exists only for a selected zero-trip body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirLoopUnrollCopyV1 {
    Retained,
    Header(u8),
    Body(u8),
    OmittedBody,
}
type CopyRole = CanonicalKirLoopUnrollCopyV1;

/// Complete occurrence relation, including explicit omitted descendants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoopUnrollOriginV1<C> {
    pub input: C,
    pub output: Option<C>,
    pub copy: CanonicalKirLoopUnrollCopyV1,
}
type Row<C> = CanonicalKirLoopUnrollOriginV1<C>;

/// The selected actual induction-facts ordinal, not a claimed trip-count proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoopUnrollSelectionV1 {
    pub fact: usize,
    pub iterations: u8,
}
type Selection = CanonicalKirLoopUnrollSelectionV1;

/// Untrusted borrowed rows. Each roster is in block-copy order (function
/// arguments precede that function's block definitions). Header edge omissions
/// retain their original occurrence ordinal; duplicate targets never coalesce.
#[derive(Clone, Copy, Debug)]
pub struct CanonicalKirLoopUnrollOriginsV1<'r> {
    pub selection: Option<CanonicalKirLoopUnrollSelectionV1>,
    pub blocks: &'r [CanonicalKirLoopUnrollOriginV1<Block>],
    pub definitions: &'r [CanonicalKirLoopUnrollOriginV1<Definition>],
    pub operations: &'r [CanonicalKirLoopUnrollOriginV1<Site>],
    pub terminators: &'r [CanonicalKirLoopUnrollOriginV1<Block>],
    pub edges: &'r [CanonicalKirLoopUnrollOriginV1<Edge>],
    pub arguments: &'r [CanonicalKirLoopUnrollOriginV1<Argument>],
}
type Origins<'r> = CanonicalKirLoopUnrollOriginsV1<'r>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirLoopUnrollErrorV1 {
    Resource(Resource),
    Inventory(InventoryError),
    Loops(LoopError),
    Facts(FactsError),
    InvalidIterationLimit(u8),
    OutputLimit {
        kind: &'static str,
        actual: usize,
        limit: usize,
    },
    Mismatch(&'static str),
    Panicked,
}
type Error = CanonicalKirLoopUnrollErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl From<Resource> for Error {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl From<InventoryError> for Error {
    fn from(v: InventoryError) -> Self {
        Self::Inventory(v)
    }
}
impl From<LoopError> for Error {
    fn from(v: LoopError) -> Self {
        Self::Loops(v)
    }
}
impl From<FactsError> for Error {
    fn from(v: FactsError) -> Self {
        Self::Facts(v)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bounded loop unroll: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoopUnrollStorageV1(usize);
impl CanonicalKirLoopUnrollStorageV1 {
    pub fn retained_storage(self) -> usize {
        self.0
    }
}

/// Both actual owners and every complete row remain borrowed. This relation
/// grants no source, progress, memory-safety, native or runtime authority.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalKirLoopUnrollPairV1;
/// fn cannot_clone(pair: CheckedCanonicalKirLoopUnrollPairV1<'_, '_, '_>) { let _ = pair.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalKirLoopUnrollPairV1;
/// fn cannot_escape<'a>(pair: CheckedCanonicalKirLoopUnrollPairV1<'a, 'a, 'a>)
///     -> CheckedCanonicalKirLoopUnrollPairV1<'static, 'static, 'static> { pair }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::{check_canonical_kir_loop_unroll_pair_v1, CanonicalKirLoopUnrollOriginsV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12, CanonicalKernelIrVerificationResourceBudgetV1};
/// fn input_stays_live(input: VerifiedCanonicalKernelIrModuleV12, output: &VerifiedCanonicalKernelIrModuleV12,
///     rows: CanonicalKirLoopUnrollOriginsV1<'_>, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let (pair, _) = check_canonical_kir_loop_unroll_pair_v1(&input, output, rows, Default::default(), budget).unwrap();
///     drop(input);
///     let _ = pair.input();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::{check_canonical_kir_loop_unroll_pair_v1, CanonicalKirLoopUnrollOriginsV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12, CanonicalKernelIrVerificationResourceBudgetV1};
/// fn output_stays_live(input: &VerifiedCanonicalKernelIrModuleV12, output: VerifiedCanonicalKernelIrModuleV12,
///     rows: CanonicalKirLoopUnrollOriginsV1<'_>, budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let (pair, _) = check_canonical_kir_loop_unroll_pair_v1(input, &output, rows, Default::default(), budget).unwrap();
///     drop(output);
///     let _ = pair.output();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::{check_canonical_kir_loop_unroll_pair_v1, CanonicalKirLoopUnrollOriginsV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12, CanonicalKernelIrVerificationResourceBudgetV1};
/// fn rows_stay_live(input: &VerifiedCanonicalKernelIrModuleV12, output: &VerifiedCanonicalKernelIrModuleV12,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let blocks = Vec::new();
///     let rows = CanonicalKirLoopUnrollOriginsV1 { selection: None, blocks: &blocks,
///         definitions: &[], operations: &[], terminators: &[], edges: &[], arguments: &[] };
///     let (pair, _) = check_canonical_kir_loop_unroll_pair_v1(input, output, rows, Default::default(), budget).unwrap();
///     drop(blocks);
///     let _ = pair.origins();
/// }
/// ```
#[derive(Debug)]
pub struct CheckedCanonicalKirLoopUnrollPairV1<'i, 'o, 'r> {
    input: &'i Owner,
    output: &'o Owner,
    origins: Origins<'r>,
    limits: Limits,
}
impl<'i, 'o, 'r> CheckedCanonicalKirLoopUnrollPairV1<'i, 'o, 'r> {
    pub fn input(&self) -> &'i Owner {
        self.input
    }
    pub fn output(&self) -> &'o Owner {
        self.output
    }
    pub fn origins(&self) -> Origins<'r> {
        self.origins
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    pub fn grants_authority(&self) -> bool {
        false
    }
}

/// Derives actual input inventory/loops/facts afresh, then independently checks
/// the actual output and all six complete occurrence rosters. Selection is
/// O(H*(F+B+D+O+U+E+A)); pair checking is bounded linear graph/row traversal plus
/// the inherited metered sparse lookups. No producer is invoked.
///
/// All new vector headers and actual capacities are paid before use. The
/// borrowed owner/row backing remains the caller's responsibility. The returned
/// pair header receipt is UNRESERVED; every exit restores the same-ledger floor.
pub fn check_canonical_kir_loop_unroll_pair_v1<'i, 'o, 'r>(
    input: &'i Owner,
    output: &'o Owner,
    origins: Origins<'r>,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirLoopUnrollPairV1<'i, 'o, 'r>,
    CanonicalKirLoopUnrollStorageV1,
)> {
    resources::scoped(budget, |meter| {
        meter.work(11)?;
        if limits.max_iterations > 8 {
            return Err(Error::InvalidIterationLimit(limits.max_iterations));
        }
        let header = size_of::<CheckedCanonicalKirLoopUnrollPairV1<'_, '_, '_>>();
        meter.reserve(header)?;
        let (inventory, is) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
        meter.reserve(is.retained_storage())?;
        let (loops, ls) = meter.derive(|b| Ok(Loops::derive(&inventory, limits.loops, b)?))?;
        meter.reserve(ls.retained_storage())?;
        let (facts, fs) = meter.derive(|b| Ok(Facts::derive(&loops, limits.loops, b)?))?;
        meter.reserve(fs.retained_storage())?;
        meter.derive(|b| Ok(facts.replay(&loops, limits.loops, b)?))?;
        let (final_inventory, os) = meter.derive(|b| Ok(Inventory::derive(output, b)?))?;
        meter.reserve(os.retained_storage())?;
        check::verify(&inventory, &final_inventory, &facts, origins, limits, meter)?;
        drop(final_inventory);
        meter.release(os.retained_storage())?;
        drop(facts);
        meter.release(fs.retained_storage())?;
        drop(loops);
        meter.release(ls.retained_storage())?;
        drop(inventory);
        meter.release(is.retained_storage())?;
        meter.work(1)?;
        Ok((
            CheckedCanonicalKirLoopUnrollPairV1 {
                input,
                output,
                origins,
                limits,
            },
            CanonicalKirLoopUnrollStorageV1(header),
        ))
    })
}

#[path = "canonical_kir_loop_unroll_check_v1.rs"]
mod check;
#[cfg(test)]
#[path = "canonical_kir_loop_unroll_resources_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "canonical_kir_loop_unroll_v1_tests.rs"]
mod tests;
