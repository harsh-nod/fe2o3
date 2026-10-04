//! Read-only exact physical-entry obligations. No serialized authority carrier.
use super::*;

/// Exact logical meaning of one compiler-owned explicit kernarg slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalEntryKernargSlotV20 {
    /// Native slot0, eight-byte output pointer.
    OutputPointer,
    /// Native slot1, eight-byte output slice element count.
    OutputLength,
    /// One actual logical u32 argument, index1..4.
    ScalarArgument(u8),
}
/// One actual authored kernarg read and its explicit readiness boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalEntryKernargReadV20 {
    pub(super) location: FunctionOperationLocation,
    pub(super) site: Site,
    pub(super) offset: u32,
    pub(super) width: u32,
    pub(super) alignment: u32,
    pub(super) slot: PhysicalEntryKernargSlotV20,
    pub(super) base: [ValueId; 2],
    pub(super) results: [Option<ValueId>; 2],
    pub(super) ready_at: FunctionOperationLocation,
    pub(super) ready_site: Site,
}
impl PhysicalEntryKernargReadV20 {
    /// Actual canonical load operation.
    pub const fn location(self) -> FunctionOperationLocation {
        self.location
    }
    /// Actual admitted source occurrence for that load.
    pub const fn source_site(self) -> Site {
        self.site
    }
    /// Byte offset inside the compiler-owned explicit ABI prefix.
    pub const fn byte_offset(self) -> u32 {
        self.offset
    }
    /// Exact read width, four or eight bytes.
    pub const fn byte_width(self) -> u32 {
        self.width
    }
    /// Required read alignment.
    pub const fn alignment(self) -> u32 {
        self.alignment
    }
    /// Exact source/native ABI slot relation.
    pub const fn slot(self) -> PhysicalEntryKernargSlotV20 {
        self.slot
    }
    /// Actual readonly kernarg s0:s1 SSA identities, not numerical addresses.
    pub const fn base(self) -> [ValueId; 2] {
        self.base
    }
    /// Fresh pending scalar register result identities.
    pub const fn results(self) -> [Option<ValueId>; 2] {
        self.results
    }
    /// First actual following lgkmcnt0 operation in the same native block.
    pub const fn ready_at(self) -> FunctionOperationLocation {
        self.ready_at
    }
    /// Retained source occurrence of the explicit readiness operation.
    pub const fn ready_source_site(self) -> Site {
        self.ready_site
    }
}
/// Actual output store SSA attribution behind the ordinary affine obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalEntryStoreV20 {
    pub(super) location: FunctionOperationLocation,
    pub(super) site: Site,
    pub(super) allocation: FormalAllocationIdentity,
    pub(super) output: ValueId,
    pub(super) address: [ValueId; 2],
    pub(super) value: ValueId,
    pub(super) exec: ValueId,
    pub(super) mask_at: FunctionOperationLocation,
    pub(super) mask_site: Site,
    pub(super) comparison_at: FunctionOperationLocation,
    pub(super) comparison_site: Site,
    pub(super) index: [ValueId; 2],
    pub(super) length: [ValueId; 2],
}
impl PhysicalEntryStoreV20 {
    /// Actual store operation coordinate.
    pub const fn location(self) -> FunctionOperationLocation {
        self.location
    }
    /// Actual source occurrence of that store.
    pub const fn source_site(self) -> Site {
        self.site
    }
    /// Real logical output allocation, never the kernarg pack.
    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.allocation
    }
    /// Actual logical slice parameter SSA identity.
    pub const fn output(self) -> ValueId {
        self.output
    }
    /// Actual symbolic low/high address SSA identities.
    pub const fn address(self) -> [ValueId; 2] {
        self.address
    }
    /// Actual data SSA identity, including an authored merge parameter.
    pub const fn value(self) -> ValueId {
        self.value
    }
    /// Actual bounded EXEC SSA identity consumed by the store.
    pub const fn exec(self) -> ValueId {
        self.exec
    }
    /// Actual save-and-mask producer of the store EXEC.
    pub const fn mask_at(self) -> FunctionOperationLocation {
        self.mask_at
    }
    /// Actual source occurrence of save-and-mask.
    pub const fn mask_source_site(self) -> Site {
        self.mask_site
    }
    /// Actual length/index comparison producing the mask's VCC.
    pub const fn comparison_at(self) -> FunctionOperationLocation {
        self.comparison_at
    }
    /// Actual source occurrence of the length/index comparison.
    pub const fn comparison_source_site(self) -> Site {
        self.comparison_site
    }
    /// Actual low/high invocation-index SSA consumed by that comparison.
    pub const fn index(self) -> [ValueId; 2] {
        self.index
    }
    /// Actual low/high output-length SSA consumed by that comparison.
    pub const fn length(self) -> [ValueId; 2] {
        self.length
    }
}
/// ABI memory preconditions still required from a real compiler/runtime binding.
///
/// No numerical kernarg address or runtime allocation identity is invented here.
/// A descriptor/native consumer must preserve these conditions alongside output
/// bounds; a complete output-only affine result cannot discharge this record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalEntryKernargAbiRequirementV20 {
    pub(super) minimum_bytes: u32,
    pub(super) alignment: u32,
    pub(super) disjoint_output: FormalAllocationIdentity,
}
impl PhysicalEntryKernargAbiRequirementV20 {
    /// Required explicit kernarg prefix bytes; hidden ABI fields are separate.
    pub const fn minimum_bytes(self) -> u32 {
        self.minimum_bytes
    }
    /// Required kernarg base alignment.
    pub const fn alignment(self) -> u32 {
        self.alignment
    }
    /// Actual output parameter whose writes must not overlap the kernarg pack.
    pub const fn disjoint_output(self) -> FormalAllocationIdentity {
        self.disjoint_output
    }
    /// Kernarg values must remain immutable for the entire kernel execution.
    pub const fn requires_immutable_kernarg(self) -> bool {
        true
    }
}
/// Exact combined output-memory and compiler-ABI read obligations for KIR20.
/// No public constructor, raw-byte decoder or mutable access is available.
#[derive(Debug, Eq, PartialEq)]
pub struct PhysicalEntryMemoryObligationsV20 {
    pub(super) canonical_identity: [u8; 32],
    pub(super) output: FormalMemoryObligations,
    pub(super) reads: Vec<PhysicalEntryKernargReadV20>,
    pub(super) store: PhysicalEntryStoreV20,
    pub(super) abi: PhysicalEntryKernargAbiRequirementV20,
}
impl PhysicalEntryMemoryObligationsV20 {
    /// Actual canonical content identity; source custody stays in its owner.
    pub const fn canonical_identity(&self) -> &[u8; 32] {
        &self.canonical_identity
    }
    /// Ordinary global-output obligations, not the entire physical memory report.
    pub const fn output(&self) -> &FormalMemoryObligations {
        &self.output
    }
    /// Every authored kernarg read in actual canonical block/operation order.
    pub fn kernarg_reads(&self) -> &[PhysicalEntryKernargReadV20] {
        &self.reads
    }
    /// Actual single output store and its physical SSA attribution.
    pub const fn store(&self) -> PhysicalEntryStoreV20 {
        self.store
    }
    /// Unresolved compiler-owned kernarg ABI memory preconditions.
    pub const fn kernarg_abi(&self) -> PhysicalEntryKernargAbiRequirementV20 {
        self.abi
    }
    /// These descriptive obligations do not authenticate a runtime binding.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
/// Unreserved logical payload allowance returned on the same canonical ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalEntryMemoryStorageV20 {
    pub(super) retained: usize,
}
impl PhysicalEntryMemoryStorageV20 {
    /// Reserve while the added report is retained; not allocator/RSS accounting.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
/// Exact profile, resource or existing formal-analysis refusal.
#[derive(Debug)]
pub enum PhysicalEntryMemoryErrorV20 {
    /// Closed physical profile or ABI join differs.
    Profile(&'static str),
    /// Same canonical ledger refused work/storage/allocation accounting.
    Resource(Resource),
    /// Existing affine bounds/alias/race analysis failed.
    Formal(FormalMemoryObligationError),
    /// A required ordinary analysis input is incomplete.
    Incomplete(FormalMemoryIncompleteReason),
}
impl From<Resource> for PhysicalEntryMemoryErrorV20 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<FormalMemoryObligationError> for PhysicalEntryMemoryErrorV20 {
    fn from(e: FormalMemoryObligationError) -> Self {
        Self::Formal(e)
    }
}
impl From<GuardedResourceErrorV1> for PhysicalEntryMemoryErrorV20 {
    fn from(e: GuardedResourceErrorV1) -> Self {
        Self::Formal(e.into())
    }
}
impl fmt::Display for PhysicalEntryMemoryErrorV20 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(s) => write!(f, "physical-entry formal profile: {s}"),
            Self::Resource(e) => e.fmt(f),
            Self::Formal(e) => e.fmt(f),
            Self::Incomplete(e) => write!(f, "physical-entry formal input incomplete: {e:?}"),
        }
    }
}
impl Error for PhysicalEntryMemoryErrorV20 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Formal(e) => Some(e),
            _ => None,
        }
    }
}
