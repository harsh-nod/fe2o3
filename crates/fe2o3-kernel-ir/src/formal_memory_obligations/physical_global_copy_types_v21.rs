//! Typed conditional memory facts for the exact physical global-copy owner.
//! No constructors, byte imports, mutation API or runtime authority.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalGlobalCopyKernargSlotV21 {
    InputPointer,
    InputLength,
    OutputPointer,
    OutputLength,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyKernargReadV21 {
    pub(super) location: Location,
    pub(super) site: Site,
    pub(super) slot: PhysicalGlobalCopyKernargSlotV21,
    pub(super) offset: u32,
    pub(super) base: [ValueId; 2],
    pub(super) results: [ValueId; 2],
    pub(super) ready_at: Location,
    pub(super) ready_site: Site,
}
impl PhysicalGlobalCopyKernargReadV21 {
    pub const fn location(self) -> Location {
        self.location
    }
    pub const fn source_site(self) -> Site {
        self.site
    }
    pub const fn slot(self) -> PhysicalGlobalCopyKernargSlotV21 {
        self.slot
    }
    pub const fn byte_offset(self) -> u32 {
        self.offset
    }
    pub const fn byte_width(self) -> u32 {
        8
    }
    pub const fn alignment(self) -> u32 {
        8
    }
    pub const fn base(self) -> [ValueId; 2] {
        self.base
    }
    pub const fn results(self) -> [ValueId; 2] {
        self.results
    }
    pub const fn ready_at(self) -> Location {
        self.ready_at
    }
    pub const fn ready_source_site(self) -> Site {
        self.ready_site
    }
}
/// One actual global operation, attributed to a logical slice rather than
/// numerical pointer words. The index is the exact original low/high SSA pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyAccessV21 {
    pub(super) location: Location,
    pub(super) site: Site,
    pub(super) allocation: FormalAllocationIdentity,
    pub(super) parameter: ValueId,
    pub(super) address: [ValueId; 2],
    pub(super) index: [ValueId; 2],
    pub(super) exec: ValueId,
}
impl PhysicalGlobalCopyAccessV21 {
    pub const fn location(self) -> Location {
        self.location
    }
    pub const fn source_site(self) -> Site {
        self.site
    }
    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.allocation
    }
    pub const fn parameter(self) -> ValueId {
        self.parameter
    }
    pub const fn address(self) -> [ValueId; 2] {
        self.address
    }
    pub const fn index(self) -> [ValueId; 2] {
        self.index
    }
    pub const fn exec(self) -> ValueId {
        self.exec
    }
}
/// Full-EXEC input read. The result remains opaque data; the explicit wait
/// changes readiness of this same SSA value, not its identity or meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyReadV21 {
    pub(super) access: PhysicalGlobalCopyAccessV21,
    pub(super) result: ValueId,
    pub(super) ready_at: Location,
    pub(super) ready_site: Site,
}
impl PhysicalGlobalCopyReadV21 {
    pub const fn access(self) -> PhysicalGlobalCopyAccessV21 {
        self.access
    }
    pub const fn result(self) -> ValueId {
        self.result
    }
    pub const fn ready_at(self) -> Location {
        self.ready_at
    }
    pub const fn ready_source_site(self) -> Site {
        self.ready_site
    }
}
/// Masked store of the original ready load, with actual compare/mask/wait/restore
/// occurrences. The conservative affine output obligation is not tail-exact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyStoreV21 {
    pub(super) access: PhysicalGlobalCopyAccessV21,
    pub(super) value: ValueId,
    pub(super) mask_at: Location,
    pub(super) mask_site: Site,
    pub(super) comparison_at: Location,
    pub(super) comparison_site: Site,
    pub(super) length: [ValueId; 2],
    pub(super) ready_at: Location,
    pub(super) ready_site: Site,
    pub(super) restore_at: Location,
    pub(super) restore_site: Site,
}
impl PhysicalGlobalCopyStoreV21 {
    pub const fn access(self) -> PhysicalGlobalCopyAccessV21 {
        self.access
    }
    pub const fn value(self) -> ValueId {
        self.value
    }
    pub const fn mask_at(self) -> Location {
        self.mask_at
    }
    pub const fn mask_source_site(self) -> Site {
        self.mask_site
    }
    pub const fn comparison_at(self) -> Location {
        self.comparison_at
    }
    pub const fn comparison_source_site(self) -> Site {
        self.comparison_site
    }
    pub const fn length(self) -> [ValueId; 2] {
        self.length
    }
    pub const fn ready_at(self) -> Location {
        self.ready_at
    }
    pub const fn ready_source_site(self) -> Site {
        self.ready_site
    }
    pub const fn restore_at(self) -> Location {
        self.restore_at
    }
    pub const fn restore_source_site(self) -> Site {
        self.restore_site
    }
}
/// Conditions on authenticated runtime bindings, NOT established host facts.
/// The unguarded full-EXEC input needs the entire launch prefix even when the
/// output length suppresses every store. No invented input-length branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyRuntimeRequirementsV21 {
    pub(super) input: FormalAllocationIdentity,
    pub(super) output: FormalAllocationIdentity,
    pub(super) minimum_input_bytes: u64,
    pub(super) minimum_output_bytes: u64,
    pub(super) input_readable: bool,
    pub(super) input_initialized: bool,
    pub(super) output_writable: bool,
    pub(super) input_output_disjoint: bool,
}
impl PhysicalGlobalCopyRuntimeRequirementsV21 {
    pub const fn input(self) -> FormalAllocationIdentity {
        self.input
    }
    pub const fn output(self) -> FormalAllocationIdentity {
        self.output
    }
    pub const fn minimum_input_bytes(self) -> u64 {
        self.minimum_input_bytes
    }
    pub const fn minimum_output_bytes(self) -> u64 {
        self.minimum_output_bytes
    }
    pub const fn requires_input_readable(self) -> bool {
        self.input_readable
    }
    pub const fn requires_input_initialized(self) -> bool {
        self.input_initialized
    }
    pub const fn requires_output_writable(self) -> bool {
        self.output_writable
    }
    pub const fn requires_input_output_disjoint(self) -> bool {
        self.input_output_disjoint
    }
    pub const fn grants_runtime_binding_authority(self) -> bool {
        false
    }
}
/// The compiler-owned ABI pack is not a third logical source allocation.
/// Its prefix must stay live/readable/immutable and disjoint from output writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyKernargAbiRequirementV21 {
    pub(super) minimum_bytes: u32,
    pub(super) alignment: u32,
    pub(super) disjoint_output: FormalAllocationIdentity,
    pub(super) readable: bool,
    pub(super) live: bool,
    pub(super) immutable: bool,
}
impl PhysicalGlobalCopyKernargAbiRequirementV21 {
    pub const fn minimum_bytes(self) -> u32 {
        self.minimum_bytes
    }
    pub const fn alignment(self) -> u32 {
        self.alignment
    }
    pub const fn disjoint_output(self) -> FormalAllocationIdentity {
        self.disjoint_output
    }
    pub const fn requires_readable_kernarg(self) -> bool {
        self.readable
    }
    pub const fn requires_live_kernarg(self) -> bool {
        self.live
    }
    pub const fn requires_immutable_kernarg(self) -> bool {
        self.immutable
    }
}
/// Combined real global read/write, readiness, compiler-ABI and conditional
/// runtime requirements. Consuming only global() is NOT complete admission.
#[derive(Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyMemoryObligationsV21 {
    pub(super) canonical_identity: [u8; 32],
    pub(super) global: FormalMemoryObligations,
    pub(super) kernarg_reads: [PhysicalGlobalCopyKernargReadV21; 4],
    pub(super) read: PhysicalGlobalCopyReadV21,
    pub(super) store: PhysicalGlobalCopyStoreV21,
    pub(super) runtime: PhysicalGlobalCopyRuntimeRequirementsV21,
    pub(super) abi: PhysicalGlobalCopyKernargAbiRequirementV21,
}
impl PhysicalGlobalCopyMemoryObligationsV21 {
    pub const fn canonical_identity(&self) -> &[u8; 32] {
        &self.canonical_identity
    }
    pub const fn global(&self) -> &FormalMemoryObligations {
        &self.global
    }
    pub const fn kernarg_reads(&self) -> &[PhysicalGlobalCopyKernargReadV21; 4] {
        &self.kernarg_reads
    }
    pub const fn input_read(&self) -> PhysicalGlobalCopyReadV21 {
        self.read
    }
    pub const fn output_store(&self) -> PhysicalGlobalCopyStoreV21 {
        self.store
    }
    pub const fn runtime_requirements(&self) -> PhysicalGlobalCopyRuntimeRequirementsV21 {
        self.runtime
    }
    pub const fn kernarg_abi(&self) -> PhysicalGlobalCopyKernargAbiRequirementV21 {
        self.abi
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalGlobalCopyMemoryStorageV21 {
    pub(super) retained: usize,
}
impl PhysicalGlobalCopyMemoryStorageV21 {
    /// Added retained logical payload; not total allocator/RSS accounting.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
#[derive(Debug)]
pub enum PhysicalGlobalCopyMemoryErrorV21 {
    Profile(&'static str),
    Resource(Resource),
    Formal(FormalMemoryObligationError),
    Incomplete(FormalMemoryIncompleteReason),
}
impl From<Resource> for PhysicalGlobalCopyMemoryErrorV21 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<FormalMemoryObligationError> for PhysicalGlobalCopyMemoryErrorV21 {
    fn from(e: FormalMemoryObligationError) -> Self {
        Self::Formal(e)
    }
}
impl From<GuardedResourceErrorV1> for PhysicalGlobalCopyMemoryErrorV21 {
    fn from(e: GuardedResourceErrorV1) -> Self {
        Self::Formal(e.into())
    }
}
impl fmt::Display for PhysicalGlobalCopyMemoryErrorV21 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(s) => write!(f, "physical global-copy formal profile: {s}"),
            Self::Resource(e) => e.fmt(f),
            Self::Formal(e) => e.fmt(f),
            Self::Incomplete(e) => write!(f, "physical global-copy formal input incomplete: {e:?}"),
        }
    }
}
impl Error for PhysicalGlobalCopyMemoryErrorV21 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Formal(e) => Some(e),
            _ => None,
        }
    }
}
