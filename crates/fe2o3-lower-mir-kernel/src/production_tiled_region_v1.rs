//! Opt-in PRE-RANKED nominal BF16 emission inspection.
//!
//! The live source/SSA owner and actual emitter supply every row. This is not
//! authentic frontend/HIR custody, a numerical theorem, convergence, helper
//! materialization, ranked/formal success, or artifact/launch authority.
//! No map/frame archive is cloned. The private capture adapter borrows the
//! actual function map before its owning materializer destroys it.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticMfmaAccumulatorDistributionV1;
use fe2o3_mir_model::{SsaEdgeIdV1, SsaResolvedEventV1};
use fe2o3_pliron::{
    ProductionSemanticSsaEventRoleV1 as EventRole,
    ProductionSemanticSsaFunctionOccurrencesV1 as Occurrences,
    ProductionSemanticSsaOccurrenceSiteV1 as Site,
    ProductionSemanticSsaOperandRoleV1 as OperandRole,
};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "production_tiled_region_declaration_v1.rs"]
mod declaration;
#[path = "production_tiled_region_emission_v1.rs"]
mod emission;
#[path = "production_tiled_region_query_v1.rs"]
mod query;
#[path = "production_tiled_region_source_v1.rs"]
mod source;
#[cfg(test)]
#[path = "production_tiled_region_v1_tests.rs"]
mod tests;

const ALIASES: usize = 64;
const USES: usize = 128;
// Whole semantic root CFG only; the frontend's sparse mappings remain16.
// Matches the measured finite P0 frontend profile, not general tiled support.
const SOURCE_BLOCKS: usize = 32;
const OPERATIONS: usize = 1024;

/// Refusal of the opt-in relation, not a relaxation of ordinary admission.
#[derive(Debug)]
pub enum ProductionTiledRegionInspectionErrorV1 {
    /// Original cumulative resource ledger refused.
    Resource(Resource),
    /// The ordinary materializer refused; no substitute owner was made.
    Materialization(ProductionPreRankedKirErrorV1),
    /// Capturing the genuine source/SSA occurrence inventory refused.
    Occurrences(fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1),
    /// Exact finite source or emission relation is unavailable.
    Unavailable(&'static str),
    /// The inspection-only callback unwound; ordinary materialization retains
    /// its existing, separate unwind contract.
    CallbackPanicked,
}
type Error = ProductionTiledRegionInspectionErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pre-ranked BF16 inspection: {self:?}")
    }
}
impl std::error::Error for Error {}
fn unavailable<T>(why: &'static str) -> Result<T> {
    Err(Error::Unavailable(why))
}

/// Nominal meaning, not a register bank or a hardware sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionBf16MfmaRoleV1 {
    /// Actual compiler matrix-context producer.
    Context,
    /// Actual Wave64 current-lane producer, not width equality alone.
    Lane,
    /// Actual checked row-major BF16 A producer.
    Lhs,
    /// Actual checked row-major BF16 B producer.
    Rhs,
    /// Actual typed zero-accumulator producer.
    Zero,
    /// Actual selected MFMA result producer.
    Result,
}
use ProductionBf16MfmaRoleV1 as Role;
impl Role {
    const ALL: [Self; 6] = [
        Self::Context,
        Self::Lane,
        Self::Lhs,
        Self::Rhs,
        Self::Zero,
        Self::Result,
    ];
    const fn index(self) -> usize {
        match self {
            Self::Context => 0,
            Self::Lane => 1,
            Self::Lhs => 2,
            Self::Rhs => 3,
            Self::Zero => 4,
            Self::Result => 5,
        }
    }
    const fn width(self) -> usize {
        match self {
            Self::Context => 0,
            Self::Lane => 1,
            _ => 4,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Producer {
    block: SemanticBlockIdV1,
    value: SsaValueV1,
    arguments: [Option<SsaValueV1>; 4],
}
#[derive(Clone, Copy, Debug)]
enum AliasKind {
    Producer,
    Copy { from: usize },
    Edge { from: usize, edge: SsaEdgeIdV1 },
}
#[derive(Clone, Copy, Debug)]
struct Alias {
    value: SsaValueV1,
    role: Role,
    kind: AliasKind,
    components: [ValueId; 4],
}

/// Actual outside use of a result component in the same admitted graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionBf16MfmaResultUseV1 {
    block: BlockId,
    operation: Option<u32>,
    operand: u32,
    component: u8,
}
impl ProductionBf16MfmaResultUseV1 {
    /// Actual canonical block ID.
    pub const fn block(self) -> BlockId {
        self.block
    }
    /// None means an actual terminator operand.
    pub const fn operation(self) -> Option<u32> {
        self.operation
    }
    /// Ordered operand position, including repeated uses.
    pub const fn operand(self) -> u32 {
        self.operand
    }
    /// Selected Matrix result component 0..4.
    pub const fn component(self) -> u8 {
        self.component
    }
}

/// Sparse private annotation payload, never a copied frame or graph.
#[derive(Debug)]
pub(super) struct Capture {
    rows: Box<[Recorder]>,
}
#[derive(Debug)]
struct Recorder {
    root: SemanticFunctionIdV1,
    producers: [Option<Producer>; 6],
    aliases: [Option<Alias>; ALIASES],
    alias_count: usize,
    producer_aliases: [usize; 6],
    mfma_arguments: [usize; 4],
    spans: [Option<SemanticKirTerminatorOperationSpanV1>; 6],
    matrix: Option<(usize, u32)>,
    uses: [Option<ProductionBf16MfmaResultUseV1>; USES],
    use_count: usize,
    captured: bool,
}
impl Recorder {
    fn blank(root: SemanticFunctionIdV1) -> Self {
        Self {
            root,
            producers: [None; 6],
            aliases: [None; ALIASES],
            alias_count: 0,
            producer_aliases: [0; 6],
            mfma_arguments: [0; 4],
            spans: [None; 6],
            matrix: None,
            uses: [None; USES],
            use_count: 0,
            captured: false,
        }
    }
    fn producer(&self, role: Role) -> Result<Producer> {
        self.producers[role.index()].ok_or(Error::Unavailable("missing producer"))
    }
    fn alias(&self, index: usize) -> Result<&Alias> {
        self.aliases
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(Error::Unavailable("missing alias"))
    }
    fn producer_alias(&self, role: Role) -> Result<&Alias> {
        let row = self.alias(self.producer_aliases[role.index()])?;
        if row.role != role || !matches!(row.kind, AliasKind::Producer) {
            return unavailable("missing producer binding");
        }
        Ok(row)
    }
}

/// Borrowed emission relation. Its constructor and recorder are private, and
/// neither the real owner nor the temporary sparse rows can be detached.
///
/// A detached DTO cannot construct an emission view:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionBf16MfmaEmissionViewV1;
/// fn forge() { let _ = ProductionBf16MfmaEmissionViewV1 { }; }
/// ```
pub struct ProductionBf16MfmaEmissionViewV1<'a> {
    owner: &'a ProductionPreRankedKirOwnerV1,
    recorder: &'a Recorder,
    operation: &'a Operation,
}
impl<'a> ProductionBf16MfmaEmissionViewV1<'a> {
    /// Same ordinary pre-ranked owner; all mandatory later checks still apply.
    pub const fn original(&self) -> &'a ProductionPreRankedKirOwnerV1 {
        self.owner
    }
    /// Actual admitted Matrix operation, not a reconstructed instruction DTO.
    pub const fn operation(&self) -> &'a Operation {
        self.operation
    }
    /// Actual semantic root in this owner.
    pub const fn semantic_function(&self) -> SemanticFunctionIdV1 {
        self.recorder.root
    }
    /// Borrowed real source function.
    pub fn source_function(&self) -> &'a SemanticFunctionDeclV1 {
        &self.owner.semantic_ssa().source_semantic().functions()
            [self.recorder.root.index() as usize]
    }
    /// Exact actual call that emitted this Matrix operation.
    pub fn source_call(&self) -> &'a SemanticDirectCallV1 {
        let block = self.recorder.producers[Role::Result.index()]
            .expect("sealed producer")
            .block;
        let SemanticTerminatorKindV1::Call(call) = self.source_function().blocks()
            [block.index() as usize]
            .terminator()
            .kind()
        else {
            unreachable!("sealed call")
        };
        call
    }
    /// Source-to-emission span belonging to the same materialization.
    pub fn terminator_span(&self) -> SemanticKirTerminatorOperationSpanV1 {
        self.recorder.spans[Role::Result.index()].expect("sealed span")
    }
    /// Actual source/SSA definition, not a raw MIR local number.
    pub fn producer_ssa(&self, role: Role) -> SsaValueV1 {
        self.recorder.producers[role.index()]
            .expect("sealed producer")
            .value
    }
    /// Actual source call block for this nominal role.
    pub fn producer_block(&self, role: Role) -> SemanticBlockIdV1 {
        self.recorder.producers[role.index()]
            .expect("sealed producer")
            .block
    }
    /// Ordered genuine producer components; context has none, lane one.
    pub fn producer_components(&self, role: Role) -> &[ValueId] {
        let row = self.recorder.producer_alias(role).expect("sealed binding");
        &row.components[..role.width()]
    }
    /// Actual consumed source SSA use after permitted exact aliases.
    pub fn consumed_ssa(&self, argument: usize) -> Option<SsaValueV1> {
        Some(
            self.recorder
                .alias(*self.recorder.mfma_arguments.get(argument)?)
                .ok()?
                .value,
        )
    }
    /// Ordered actual consumed components after permitted exact SSA aliases.
    pub fn consumed_components(&self, argument: usize) -> Option<&[ValueId]> {
        let row = self
            .recorder
            .alias(*self.recorder.mfma_arguments.get(argument)?)
            .ok()?;
        Some(&row.components[..row.role.width()])
    }
    /// Complete finite outside-use roster, retaining each repeated use.
    pub fn result_uses(&self) -> impl ExactSizeIterator<Item = ProductionBf16MfmaResultUseV1> + '_ {
        self.recorder.uses[..self.recorder.use_count]
            .iter()
            .map(|row| row.expect("sealed use"))
    }
    /// This view cannot authorize source publication or execution.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Materializes once using the original supplied phase ledger, invokes a
/// nonescaping pre-ranked inspection, then returns the SAME ordinary owner.
///
/// Incoming occurrence storage must already be reserved. A newly attached
/// occurrence receipt remains reserved on success; the returned ordinary owner
/// receipt is UNRESERVED and must immediately be reserved by its normal caller.
/// The callback's extra storage remains charged, including on callback refusal.
/// No new work ledger or second materialization is created.
///
/// The borrowed operation cannot escape the callback in its result:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{materialize_with_bf16_mfma_inspection_v1, ProductionSourceLaunchRosterV1};
/// use fe2o3_pliron::ProductionSemanticSsaOwnerV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape(source: ProductionSemanticSsaOwnerV1, launch: ProductionSourceLaunchRosterV1,
///           budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = materialize_with_bf16_mfma_inspection_v1(source, launch, Default::default(),
///         budget, |_, view, _| Ok(view.operation()));
/// }
/// ```
pub fn materialize_with_bf16_mfma_inspection_v1<'work, R>(
    mut source: ProductionSemanticSsaOwnerV1,
    launch: crate::ProductionSourceLaunchRosterV1,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut Budget<'work>,
    inspect: impl for<'a> FnOnce(
        &'a ProductionPreRankedKirOwnerV1,
        &'a ProductionBf16MfmaEmissionViewV1<'a>,
        &mut Budget<'work>,
    ) -> Result<R>,
) -> Result<(ProductionPreRankedKirOwnerV1, R)> {
    let floor = budget.storage();
    let incoming = source
        .occurrence_storage()
        .map_or(0, |r| r.retained_storage());
    if floor < incoming {
        return Err(Resource::Accounting.into());
    }
    let mut added_occurrence = 0;
    let setup = (|| {
        if source.occurrences_v1().is_none() {
            let receipt = source
                .try_capture_occurrences_with_budget_v1(budget)
                .map_err(Error::Occurrences)?;
            budget.reserve_storage(receipt.retained_storage())?;
            added_occurrence = receipt.retained_storage();
        }
        // Prepaid view and fallible sparse recorder, never a hidden frame clone.
        budget.reserve_storage(size_of::<ProductionBf16MfmaEmissionViewV1<'_>>())?;
        let capture = Capture::prepare(&source, budget)?;
        let (owner, scalar, capture) =
            ProductionPreRankedKirOwnerV1::try_materialize_origins_with_captures_v1(
                source,
                launch,
                limits,
                None,
                Some(capture),
                budget,
            )
            .map_err(Error::Materialization)?;
        if scalar.is_some() {
            return unavailable("foreign scalar capture completion");
        }
        let mut capture = capture.ok_or(Error::Unavailable("missing typed capture completion"))?;
        capture.recorder_mut().seal(&owner, budget)?;
        Ok((owner, capture))
    })();
    let (owner, capture) = match setup {
        Ok(result) => result,
        Err(error) => {
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(Resource::Accounting)?,
            )?;
            return Err(error);
        }
    };
    let callback_floor = budget.storage();
    let operation = capture
        .recorder()
        .operation(&owner)
        .expect("sealed Matrix coordinate");
    let view = ProductionBf16MfmaEmissionViewV1 {
        owner: &owner,
        recorder: capture.recorder(),
        operation,
    };
    let outcome = query::callback(budget, |budget| inspect(&owner, &view, budget));
    drop(capture);
    if !outcome.valid {
        drop((owner, outcome.result, outcome.panic));
        return Err(Resource::Accounting.into());
    }
    match outcome.result {
        Ok(value) => {
            let owned = callback_floor
                .checked_sub(floor)
                .and_then(|v| v.checked_sub(added_occurrence))
                .ok_or(Resource::Accounting)?;
            budget.release_storage(owned)?;
            drop(outcome.panic);
            Ok((owner, value))
        }
        Err(error) => {
            drop(owner);
            budget.release_storage(
                callback_floor
                    .checked_sub(floor)
                    .ok_or(Resource::Accounting)?,
            )?;
            drop(outcome.panic);
            Err(error)
        }
    }
}
