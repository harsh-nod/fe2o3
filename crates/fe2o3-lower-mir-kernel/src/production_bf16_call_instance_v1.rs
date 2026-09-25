//! Borrowed nominal call transport only; no emitted/normal/CPU authority.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1;
use fe2o3_pliron::ProductionSemanticSsaEntryOriginV1 as EntryOrigin;
#[path = "production_bf16_call_instance_source_v1.rs"]
mod source;
#[path = "production_bf16_call_instance_ssa_v1.rs"]
mod ssa;
#[cfg(test)]
#[path = "production_bf16_call_instance_v1_tests.rs"]
mod tests;

/// Exact closed nominal roles, not physical registers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bf16CallInstanceRoleV1 {
    /// Compiler matrix context.
    Context,
    /// Compiler Wave64 lane.
    Lane,
    /// Checked row-major BF16 A fragment.
    Lhs,
    /// Checked row-major BF16 B fragment.
    Rhs,
    /// Typed zero accumulator.
    Zero,
    /// Actual helper MFMA result.
    Result,
    /// Actual helper accumulator-to-four-values conversion.
    Values,
}
use Bf16CallInstanceRoleV1 as CallRole;
impl CallRole {
    const ALL: [Self; 7] = [
        Self::Context,
        Self::Lane,
        Self::Lhs,
        Self::Rhs,
        Self::Zero,
        Self::Result,
        Self::Values,
    ];
    const fn index(self) -> usize {
        match self {
            Self::Context => 0,
            Self::Lane => 1,
            Self::Lhs => 2,
            Self::Rhs => 3,
            Self::Zero => 4,
            Self::Result => 5,
            Self::Values => 6,
        }
    }
}
/// Refusals do not alter the ordinary materializer's admission policy.
#[derive(Debug)]
pub enum Bf16CallInstanceErrorV1 {
    /// Original cumulative resource ledger refused.
    Resource(Resource),
    /// Exact source/SSA relation is unavailable.
    Unavailable(&'static str),
    /// Inspection callback unwound.
    CallbackPanicked,
}
type CallError = Bf16CallInstanceErrorV1;
type CallResult<T> = std::result::Result<T, CallError>;
impl From<Resource> for CallError {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BF16 nominal call transport: {self:?}")
    }
}
impl std::error::Error for CallError {}
fn refuse<T>(why: &'static str) -> CallResult<T> {
    Err(CallError::Unavailable(why))
}
fn one<T: Copy>(slot: &mut Option<T>, value: T) -> CallResult<()> {
    if slot.replace(value).is_some() {
        return refuse("duplicate source relation");
    }
    Ok(())
}

/// Actual function/block/result-SSA coordinate; inert without the borrowed owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bf16CallInstanceProducerV1 {
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    value: SsaValueV1,
}
impl Bf16CallInstanceProducerV1 {
    /// Actual source function.
    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    /// Actual source call block.
    pub const fn block(self) -> SemanticBlockIdV1 {
        self.block
    }
    /// Actual captured source result definition.
    pub const fn value(self) -> SsaValueV1 {
        self.value
    }
}
struct Relation {
    root: SemanticFunctionIdV1,
    helper: SemanticFunctionIdV1,
    call: SemanticBlockIdV1,
    producers: [Bf16CallInstanceProducerV1; 7],
    arguments: [SsaValueV1; 4],
    formals: [SsaValueV1; 4],
    permutation: [u8; 4],
}
/// Borrowed SAME-SSA-owner relation. Not source-file authenticity, collective
/// convergence, emitted KIR, normal/ranked readiness or CPU execution authority.
/// Complete actual helper FnABI remains available through helper_declaration().
/// No physical ABI mode is inferred from these source-language joins.
///
/// No construction from detached coordinates:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CheckedBf16CallInstanceV1;
/// fn forge() { let _ = CheckedBf16CallInstanceV1 {}; }
/// ```
/// No detached Clone:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CheckedBf16CallInstanceV1;
/// fn clone<'a>(v: &CheckedBf16CallInstanceV1<'a>) -> CheckedBf16CallInstanceV1<'a> {
///     Clone::clone(v)
/// }
/// ```
pub struct CheckedBf16CallInstanceV1<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    relation: &'a Relation,
}
impl<'a> CheckedBf16CallInstanceV1<'a> {
    /// Same original immutable semantic SSA owner.
    pub const fn owner(&self) -> &'a ProductionSemanticSsaOwnerV1 {
        self.owner
    }
    /// Actual sole root, not an assumed ordinal zero.
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.relation.root
    }
    /// Actual sole Defined-call target.
    pub const fn helper(&self) -> SemanticFunctionIdV1 {
        self.relation.helper
    }
    /// Actual root call block.
    pub const fn call_block(&self) -> SemanticBlockIdV1 {
        self.relation.call
    }
    /// Exact source producer. This does not claim emitted components exist.
    pub fn producer(&self, role: CallRole) -> Bf16CallInstanceProducerV1 {
        self.relation.producers[role.index()]
    }
    /// Ordered actual source call operand definition.
    pub fn call_argument_ssa(&self, index: usize) -> Option<SsaValueV1> {
        self.relation.arguments.get(index).copied()
    }
    /// Ordered actual helper entry definition.
    pub fn formal_ssa(&self, index: usize) -> Option<SsaValueV1> {
        self.relation.formals.get(index).copied()
    }
    /// Actual return array ancestry: Identity or Swap01 only.
    pub const fn return_permutation(&self) -> [u8; 4] {
        self.relation.permutation
    }
    /// Same owner's actual helper declaration including complete source/FnABI.
    pub fn helper_declaration(&self) -> &'a SemanticFunctionDeclV1 {
        &self.owner.source_semantic().functions()[self.helper().index() as usize]
    }
    /// Same owner's actual sole Defined call.
    pub fn source_call(&self) -> &'a SemanticDirectCallV1 {
        source::call_at(self.owner, self.root(), self.call_block())
            .expect("immutable checked call coordinate")
    }
}

// Logical scratch envelope, NOT a native-stack/RSS claim. At most64 recursive
// resolver frames, each bounded by the source shape checks below. There is no
// heap table or copied graph. Relation constructor/coexisting callback view paid.
const DEPTH: usize = 64;
const FRAME_BYTES: usize = 4096;
const SCRATCH: usize = DEPTH * FRAME_BYTES
    + 2 * size_of::<Relation>()
    + size_of::<CheckedBf16CallInstanceV1<'static>>();
const _: () = assert!(size_of::<Relation>() <= FRAME_BYTES);

fn scoped<'w, R: Copy + 'static>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> CallResult<R>,
) -> CallResult<R> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    let identity = budget.work_ledger_identity_v1();
    let slot = budget as *mut _ as usize;
    budget.reserve_storage(SCRATCH)?;
    let floor = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(DEPTH + 32 + 7 + 8)?;
        run(budget)
    }));
    let valid = slot == budget as *mut _ as usize
        && identity == budget.work_ledger_identity_v1()
        && budget.storage() >= floor;
    // Replaced/undercut ledger cannot be repaired by refunding someone else's
    // storage. The enclosing original scope owns that invalid outcome.
    if !valid {
        return Err(Resource::Accounting.into());
    }
    let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
    budget.release_storage(SCRATCH)?;
    match outcome {
        Ok(Ok(_)) if denied => Err(Resource::Accounting.into()),
        Ok(value) => value,
        Err(payload) => {
            drop(payload);
            Err(CallError::CallbackPanicked)
        }
    }
}
/// Inspect exact nominal transport while borrowing the original owner. The
/// caller must first capture actual occurrences and reserve their receipt on
/// this SAME ledger. This function never captures/reserves them a second time.
/// Only fixed private scratch is refunded; extra caller storage stays live.
/// No emitted view or materialization permission is returned.
///
/// Borrowed relation cannot escape the callback:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::with_checked_bf16_call_instance_v1;
/// fn escape(o: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
/// b: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = with_checked_bf16_call_instance_v1(o, b, |v, _| Ok(v));
/// }
/// ```
pub fn with_checked_bf16_call_instance_v1<'w, R: Copy + 'static>(
    owner: &ProductionSemanticSsaOwnerV1,
    budget: &mut Budget<'w>,
    inspect: impl FnOnce(&CheckedBf16CallInstanceV1<'_>, &mut Budget<'w>) -> CallResult<R>,
) -> CallResult<R> {
    scoped(budget, |budget| {
        let receipt = owner.occurrence_storage().ok_or(CallError::Unavailable(
            "captured source occurrences required",
        ))?;
        if budget
            .storage()
            .checked_sub(SCRATCH)
            .ok_or(Resource::Accounting)?
            < receipt.retained_storage()
        {
            return Err(Resource::Accounting.into());
        }
        let relation = source::derive(owner, budget)?;
        inspect(
            &CheckedBf16CallInstanceV1 {
                owner,
                relation: &relation,
            },
            budget,
        )
    })
}
