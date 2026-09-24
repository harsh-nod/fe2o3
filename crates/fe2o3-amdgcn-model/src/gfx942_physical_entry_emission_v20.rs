//! Content-bound V20 physical body emission from the sole actual verified graph.
//! The result is inert LLVM text, never authenticated source or artifact authority.
use fe2o3_kernel_ir::{
    BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    Gfx942PhysicalEntryBranchEncodingVNext, Gfx942PhysicalEntrySourceSiteVNext, ValueId,
    VerifiedCanonicalKernelIrIdentityV20, VerifiedCanonicalKernelIrModuleV20,
};
use std::{error::Error, fmt};
#[path = "gfx942_physical_entry_emit_v20.rs"]
mod emit;
#[path = "gfx942_physical_entry_native_observation_v20.rs"]
mod native_observation;
pub use native_observation::*;
#[cfg(test)]
#[path = "gfx942_physical_entry_emission_v20_tests.rs"]
mod tests;

pub const GFX942_PHYSICAL_ENTRY_ASSEMBLY_BYTES_V20: usize = 8 * 1024;
pub const GFX942_PHYSICAL_ENTRY_LLVM_BYTES_V20: usize = 32 * 1024;
/// Conservative logical work for <=64 instruction rows, <=4 actual blocks,
/// <=131 clobber units and two-pass bounded rendering/escaping. Not CPU/RSS.
pub const GFX942_PHYSICAL_ENTRY_EMISSION_WORK_V20: usize = 131_072;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PhysicalEntryCanonicalEmissionErrorV20 {
    Resource(Resource),
    Profile(&'static str),
    TextLimit,
}
impl From<Resource> for Gfx942PhysicalEntryCanonicalEmissionErrorV20 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Gfx942PhysicalEntryCanonicalEmissionErrorV20 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical V20 physical-entry emission refused: {self:?}")
    }
}
impl Error for Gfx942PhysicalEntryCanonicalEmissionErrorV20 {}
type Result<T> = std::result::Result<T, Gfx942PhysicalEntryCanonicalEmissionErrorV20>;

/// Exact canonical operation location and inert assembly-line relation.
/// Declaration emits no instruction. Native ordinals are authored expectations,
/// not independently decoded native PCs or a machine-preservation proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryOperationCorrespondenceV20 {
    pub block: BlockId,
    pub operation_ordinal: u8,
    pub source_site: Gfx942PhysicalEntrySourceSiteVNext,
    pub results: [Option<ValueId>; 5],
    pub instruction_descriptor: Option<[u8; 8]>,
    pub native_ordinal: Option<u8>,
    pub assembly_line: Option<u16>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryBlockCorrespondenceV20 {
    pub block: BlockId,
    pub authored_label: u8,
    pub label_site: Gfx942PhysicalEntrySourceSiteVNext,
    pub terminator_site: Gfx942PhysicalEntrySourceSiteVNext,
    pub encoding: Gfx942PhysicalEntryBranchEncodingVNext,
    pub assembly_label_line: u16,
    pub terminator_assembly_line: Option<u16>,
    pub native_ordinal: Option<u8>,
}
/// Immutable output keeps the canonical content identity and bounded diagnostic
/// correspondence. It does not keep source ownership alive for the caller.
#[derive(Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryCanonicalEmissionV20 {
    identity: VerifiedCanonicalKernelIrIdentityV20,
    llvm: String,
    assembly: String,
    operations: [Option<Gfx942PhysicalEntryOperationCorrespondenceV20>; 65],
    blocks: [Option<Gfx942PhysicalEntryBlockCorrespondenceV20>; 4],
    clobbered_register_units: [bool; 131],
}
impl Gfx942PhysicalEntryCanonicalEmissionV20 {
    pub const fn canonical_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV20 {
        &self.identity
    }
    pub fn llvm_ir(&self) -> &str {
        &self.llvm
    }
    pub fn assembly_template(&self) -> &str {
        &self.assembly
    }
    pub fn operations(
        &self,
    ) -> impl Iterator<Item = &Gfx942PhysicalEntryOperationCorrespondenceV20> {
        self.operations.iter().flatten()
    }
    pub fn blocks(&self) -> impl Iterator<Item = &Gfx942PhysicalEntryBlockCorrespondenceV20> {
        self.blocks.iter().flatten()
    }
    /// Written physical units. EXEC is excluded from net clobbers because the
    /// whole verifier proves exact save/restore on every admitted path.
    pub const fn clobbered_register_units(&self) -> &[bool; 131] {
        &self.clobbered_register_units
    }
    /// Demotes this inert relation to ordinary owned text. Keep any prior
    /// receipt reserved until the caller ends its retained-output lifetime.
    pub fn into_llvm_ir(self) -> String {
        self.llvm
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalEntryCanonicalEmissionStorageV20 {
    retained: usize,
}
impl Gfx942PhysicalEntryCanonicalEmissionStorageV20 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
/// Lowers only an actual physical-profile V20 owner. No ordinary Module, plan,
/// packed source, symbol override, target override or caller constraints enter.
///
/// Caller retains and accounts the owner throughout. The same incoming ledger
/// prepays all fixed scratch/output capacity before rendering and preserves its
/// floor, cumulative work, peak and denied history on every exit. The successful
/// output receipt is unreserved; reserve it before subsequent allocations while
/// retaining the output. This bounds logical payload, not LLVM/compiler RSS.
pub fn lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(
    owner: &VerifiedCanonicalKernelIrModuleV20,
    budget: &mut Budget<'_>,
) -> Result<(
    Gfx942PhysicalEntryCanonicalEmissionV20,
    Gfx942PhysicalEntryCanonicalEmissionStorageV20,
)> {
    let retained = std::mem::size_of::<Gfx942PhysicalEntryCanonicalEmissionV20>()
        .checked_add(GFX942_PHYSICAL_ENTRY_ASSEMBLY_BYTES_V20)
        .and_then(|n| n.checked_add(GFX942_PHYSICAL_ENTRY_LLVM_BYTES_V20))
        .ok_or(Resource::Arithmetic)?;
    // Output and construction arrays may coexist across moves; conservatively
    // retain one extra complete header plus bounded formatting/bookkeeping.
    let scratch = retained
        .checked_add(std::mem::size_of::<Gfx942PhysicalEntryCanonicalEmissionV20>())
        .and_then(|n| n.checked_add(4096))
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(
        budget.storage(),
        1,
        GFX942_PHYSICAL_ENTRY_EMISSION_WORK_V20,
        scratch,
        |_| {
            Ok((
                emit::render(owner)?,
                Gfx942PhysicalEntryCanonicalEmissionStorageV20 { retained },
            ))
        },
    )
}
