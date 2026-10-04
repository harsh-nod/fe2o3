//! Typed conditional memory facts for the exact physical LDS-exchange owner.
//! No constructors, byte imports, mutation API or runtime authority.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalLdsExchangeKernargSlotV22 {
    InputPointer,
    InputLength,
    OutputPointer,
    OutputLength,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangeKernargReadV22 {
    pub(super) location: Location,
    pub(super) site: Site,
    pub(super) slot: PhysicalLdsExchangeKernargSlotV22,
    pub(super) offset: u32,
    pub(super) base: [ValueId; 2],
    pub(super) results: [ValueId; 2],
    pub(super) ready_at: Location,
    pub(super) ready_site: Site,
}
impl PhysicalLdsExchangeKernargReadV22 {
    pub const fn location(self) -> Location {
        self.location
    }
    pub const fn source_site(self) -> Site {
        self.site
    }
    pub const fn slot(self) -> PhysicalLdsExchangeKernargSlotV22 {
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
pub struct PhysicalLdsExchangeAccessV22 {
    pub(super) location: Location,
    pub(super) site: Site,
    pub(super) allocation: FormalAllocationIdentity,
    pub(super) parameter: ValueId,
    pub(super) address: [ValueId; 2],
    pub(super) index: [ValueId; 2],
    pub(super) exec: ValueId,
}
impl PhysicalLdsExchangeAccessV22 {
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
pub struct PhysicalLdsExchangeReadV22 {
    pub(super) access: PhysicalLdsExchangeAccessV22,
    pub(super) result: ValueId,
    pub(super) ready_at: Location,
    pub(super) ready_site: Site,
}
impl PhysicalLdsExchangeReadV22 {
    pub const fn access(self) -> PhysicalLdsExchangeAccessV22 {
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
/// Masked store of the original ready LDS peer read, with actual compare/mask/wait/restore
/// occurrences. The conservative affine output obligation is not tail-exact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangeStoreV22 {
    pub(super) access: PhysicalLdsExchangeAccessV22,
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
impl PhysicalLdsExchangeStoreV22 {
    pub const fn access(self) -> PhysicalLdsExchangeAccessV22 {
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
pub struct PhysicalLdsExchangeRuntimeRequirementsV22 {
    pub(super) input: FormalAllocationIdentity,
    pub(super) output: FormalAllocationIdentity,
    pub(super) minimum_input_bytes: u64,
    pub(super) minimum_output_bytes: u64,
    pub(super) input_readable: bool,
    pub(super) input_initialized: bool,
    pub(super) output_writable: bool,
    pub(super) input_output_disjoint: bool,
    pub(super) required_workgroup: [u32; 3],
    pub(super) required_workgroups: [u32; 3],
}
impl PhysicalLdsExchangeRuntimeRequirementsV22 {
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
    pub const fn required_workgroup(self) -> [u32; 3] {
        self.required_workgroup
    }
    pub const fn required_workgroups(self) -> [u32; 3] {
        self.required_workgroups
    }
    pub const fn requires_all_workgroup_invocations(self) -> bool {
        true
    }
    pub const fn grants_runtime_binding_authority(self) -> bool {
        false
    }
}
/// The compiler-owned ABI pack is not a third logical source allocation.
/// Its prefix must stay live/readable/immutable and disjoint from output writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangeKernargAbiRequirementV22 {
    pub(super) minimum_bytes: u32,
    pub(super) alignment: u32,
    pub(super) disjoint_output: FormalAllocationIdentity,
    pub(super) readable: bool,
    pub(super) live: bool,
    pub(super) immutable: bool,
}
impl PhysicalLdsExchangeKernargAbiRequirementV22 {
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
/// Combined real global/LDS read/write, waits/publication, compiler-ABI and conditional
/// runtime requirements. Consuming only global() is NOT complete admission.
#[derive(Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangeMemoryObligationsV22 {
    pub(super) canonical_identity: [u8; 32],
    pub(super) global: FormalMemoryObligations,
    pub(super) kernarg_reads: [PhysicalLdsExchangeKernargReadV22; 4],
    pub(super) read: PhysicalLdsExchangeReadV22,
    pub(super) store: PhysicalLdsExchangeStoreV22,
    pub(super) runtime: PhysicalLdsExchangeRuntimeRequirementsV22,
    pub(super) abi: PhysicalLdsExchangeKernargAbiRequirementV22,
    pub(super) lds_frame: PhysicalLdsExchangeFrameObligationV22,
    pub(super) lds_write: PhysicalLdsExchangeLdsAccessV22,
    pub(super) lds_read: PhysicalLdsExchangeLdsAccessV22,
    pub(super) publication: PhysicalLdsExchangePublicationV22,
}
impl PhysicalLdsExchangeMemoryObligationsV22 {
    pub const fn canonical_identity(&self) -> &[u8; 32] {
        &self.canonical_identity
    }
    pub const fn global(&self) -> &FormalMemoryObligations {
        &self.global
    }
    pub const fn kernarg_reads(&self) -> &[PhysicalLdsExchangeKernargReadV22; 4] {
        &self.kernarg_reads
    }
    pub const fn input_read(&self) -> PhysicalLdsExchangeReadV22 {
        self.read
    }
    pub const fn output_store(&self) -> PhysicalLdsExchangeStoreV22 {
        self.store
    }
    pub const fn runtime_requirements(&self) -> PhysicalLdsExchangeRuntimeRequirementsV22 {
        self.runtime
    }
    pub const fn kernarg_abi(&self) -> PhysicalLdsExchangeKernargAbiRequirementV22 {
        self.abi
    }
    pub const fn lds_frame(&self) -> PhysicalLdsExchangeFrameObligationV22 {
        self.lds_frame
    }
    pub const fn lds_write(&self) -> PhysicalLdsExchangeLdsAccessV22 {
        self.lds_write
    }
    pub const fn lds_read(&self) -> PhysicalLdsExchangeLdsAccessV22 {
        self.lds_read
    }
    pub const fn publication(&self) -> PhysicalLdsExchangePublicationV22 {
        self.publication
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangeMemoryStorageV22 {
    pub(super) retained: usize,
}
impl PhysicalLdsExchangeMemoryStorageV22 {
    /// Added retained logical payload; not total allocator/RSS accounting.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
#[derive(Debug)]
pub enum PhysicalLdsExchangeMemoryErrorV22 {
    Profile(&'static str),
    Resource(Resource),
    Formal(FormalMemoryObligationError),
    Incomplete(FormalMemoryIncompleteReason),
}
impl From<Resource> for PhysicalLdsExchangeMemoryErrorV22 {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<FormalMemoryObligationError> for PhysicalLdsExchangeMemoryErrorV22 {
    fn from(e: FormalMemoryObligationError) -> Self {
        Self::Formal(e)
    }
}
impl From<GuardedResourceErrorV1> for PhysicalLdsExchangeMemoryErrorV22 {
    fn from(e: GuardedResourceErrorV1) -> Self {
        Self::Formal(e.into())
    }
}
impl fmt::Display for PhysicalLdsExchangeMemoryErrorV22 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(s) => write!(f, "physical LDS-exchange formal profile: {s}"),
            Self::Resource(e) => e.fmt(f),
            Self::Formal(e) => e.fmt(f),
            Self::Incomplete(e) => {
                write!(f, "physical LDS-exchange formal input incomplete: {e:?}")
            }
        }
    }
}
impl Error for PhysicalLdsExchangeMemoryErrorV22 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Formal(e) => Some(e),
            _ => None,
        }
    }
}

/// Exact compiler-owned workgroup allocation, never a logical source argument.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangeFrameObligationV22 {
    pub(super) declaration: Location,
    pub(super) site: Site,
    pub(super) descriptor: crate::Gfx942PhysicalLdsExchangeFrameV1,
    pub(super) local_x: ValueId,
}
impl PhysicalLdsExchangeFrameObligationV22 {
    pub const fn declaration(self) -> Location {
        self.declaration
    }
    pub const fn source_site(self) -> Site {
        self.site
    }
    pub const fn descriptor(self) -> crate::Gfx942PhysicalLdsExchangeFrameV1 {
        self.descriptor
    }
    pub const fn local_x(self) -> ValueId {
        self.local_x
    }
    pub const fn peer_xor_mask(self) -> u32 {
        64
    }
    pub const fn byte_scale(self) -> u32 {
        4
    }
}
/// One real LDS read or write and its distinct completion occurrence.
/// Read values are opaque data, not an affine input-value expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangeLdsAccessV22 {
    pub(super) location: Location,
    pub(super) site: Site,
    pub(super) kind: FormalMemoryAccessKind,
    pub(super) address: ValueId,
    pub(super) value: ValueId,
    pub(super) exec: ValueId,
    pub(super) complete_at: Location,
    pub(super) complete_site: Site,
}
impl PhysicalLdsExchangeLdsAccessV22 {
    pub const fn location(self) -> Location {
        self.location
    }
    pub const fn source_site(self) -> Site {
        self.site
    }
    pub const fn kind(self) -> FormalMemoryAccessKind {
        self.kind
    }
    pub const fn address(self) -> ValueId {
        self.address
    }
    /// Actual ready input SSA for a write, actual pending-result SSA for a read.
    pub const fn value(self) -> ValueId {
        self.value
    }
    pub const fn exec(self) -> ValueId {
        self.exec
    }
    pub const fn byte_width(self) -> u32 {
        4
    }
    pub const fn alignment(self) -> u32 {
        4
    }
    pub const fn complete_at(self) -> Location {
        self.complete_at
    }
    pub const fn complete_source_site(self) -> Site {
        self.complete_site
    }
}
/// The actual full-workgroup publication. It does not complete pending accesses
/// and establishes no global-memory happens-before relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalLdsExchangePublicationV22 {
    pub(super) location: Location,
    pub(super) site: Site,
    pub(super) epoch: u8,
    pub(super) participants: u32,
}
impl PhysicalLdsExchangePublicationV22 {
    pub const fn location(self) -> Location {
        self.location
    }
    pub const fn source_site(self) -> Site {
        self.site
    }
    pub const fn publication_epoch(self) -> u8 {
        self.epoch
    }
    pub const fn participant_count(self) -> u32 {
        self.participants
    }
    pub const fn address_space(self) -> AddressSpace {
        AddressSpace::Workgroup
    }
    pub const fn memory_scope(self) -> crate::SynchronizationScope {
        crate::SynchronizationScope::Workgroup
    }
    pub const fn ordering(self) -> crate::MemoryOrdering {
        crate::MemoryOrdering::AcquireRelease
    }
    pub const fn completes_pending_accesses(self) -> bool {
        false
    }
    pub const fn grants_global_happens_before(self) -> bool {
        false
    }
}
