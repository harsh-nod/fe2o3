//! Content-bound V22 physical lds-exchange body emission from the sole actual verified graph.
//! The result is inert LLVM text, never authenticated source or artifact authority.
use fe2o3_kernel_ir::{
    BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    Gfx942PhysicalEntryBranchEncodingVNext, Gfx942PhysicalEntrySourceSiteVNext,
    Gfx942PhysicalLdsExchangeFrameV1, ValueId, VerifiedCanonicalKernelIrIdentityV22,
    VerifiedCanonicalKernelIrModuleV22,
};
use std::{error::Error, fmt};
#[path = "gfx942_physical_lds_exchange_emit_v22.rs"]
mod emit;
#[path = "gfx942_physical_lds_exchange_native_observation_v22.rs"]
mod native_observation;
pub use native_observation::*;
#[cfg(test)]
#[path = "gfx942_physical_lds_exchange_emission_v22_tests.rs"]
mod tests;

pub const GFX942_PHYSICAL_LDS_EXCHANGE_ASSEMBLY_BYTES_V22: usize = 8 * 1024;
pub const GFX942_PHYSICAL_LDS_EXCHANGE_LLVM_BYTES_V22: usize = 32 * 1024;
/// Conservative logical work for <=40 instruction rows, one actual block,
/// <=131 clobber units and two-pass bounded rendering/escaping. Not CPU/RSS.
pub const GFX942_PHYSICAL_LDS_EXCHANGE_EMISSION_WORK_V22: usize = 131_072;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22 {
    Resource(Resource),
    Profile(&'static str),
    TextLimit,
}
impl From<Resource> for Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "canonical V22 physical-lds-exchange emission refused: {self:?}"
        )
    }
}
impl Error for Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22 {}
type Result<T> = std::result::Result<T, Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22>;

/// Exact canonical operation location and inert assembly-line relation.
/// Declaration emits no instruction. Native ordinals are authored expectations,
/// not independently decoded native PCs or a machine-preservation proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalLdsExchangeOperationCorrespondenceV22 {
    pub block: BlockId,
    pub operation_ordinal: u8,
    pub source_site: Gfx942PhysicalEntrySourceSiteVNext,
    pub results: [Option<ValueId>; 5],
    pub instruction_descriptor: Option<[u8; 8]>,
    pub native_ordinal: Option<u8>,
    pub assembly_line: Option<u16>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942PhysicalLdsExchangeBlockCorrespondenceV22 {
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
pub struct Gfx942PhysicalLdsExchangeCanonicalEmissionV22 {
    identity: VerifiedCanonicalKernelIrIdentityV22,
    llvm: String,
    assembly: String,
    operations: [Option<Gfx942PhysicalLdsExchangeOperationCorrespondenceV22>; 40],
    blocks: [Option<Gfx942PhysicalLdsExchangeBlockCorrespondenceV22>; 1],
    clobbered_register_units: [bool; 131],
    lds_frame: Gfx942PhysicalLdsExchangeFrameV1,
}
impl Gfx942PhysicalLdsExchangeCanonicalEmissionV22 {
    pub const fn canonical_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV22 {
        &self.identity
    }
    /// Sole exact frame from the same canonical owner. This is structural
    /// backend reservation metadata, not host/runtime publication authority.
    pub const fn lds_frame(&self) -> &Gfx942PhysicalLdsExchangeFrameV1 {
        &self.lds_frame
    }
    pub fn llvm_ir(&self) -> &str {
        &self.llvm
    }
    pub fn assembly_template(&self) -> &str {
        &self.assembly
    }
    pub fn operations(
        &self,
    ) -> impl Iterator<Item = &Gfx942PhysicalLdsExchangeOperationCorrespondenceV22> {
        self.operations.iter().flatten()
    }
    pub fn blocks(&self) -> impl Iterator<Item = &Gfx942PhysicalLdsExchangeBlockCorrespondenceV22> {
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
pub struct Gfx942PhysicalLdsExchangeCanonicalEmissionStorageV22 {
    retained: usize,
}
impl Gfx942PhysicalLdsExchangeCanonicalEmissionStorageV22 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
/// Lowers only an actual physical-profile V22 owner. No ordinary Module, plan,
/// packed source, symbol override, target override or caller constraints enter.
///
/// Caller retains and accounts the owner throughout. The same incoming ledger
/// prepays all fixed scratch/output capacity before rendering and preserves its
/// floor, cumulative work, peak and denied history on every exit. The successful
/// output receipt is unreserved; reserve it before subsequent allocations while
/// retaining the output. This bounds logical payload, not LLVM/compiler RSS.
pub fn lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(
    owner: &VerifiedCanonicalKernelIrModuleV22,
    budget: &mut Budget<'_>,
) -> Result<(
    Gfx942PhysicalLdsExchangeCanonicalEmissionV22,
    Gfx942PhysicalLdsExchangeCanonicalEmissionStorageV22,
)> {
    let retained = std::mem::size_of::<Gfx942PhysicalLdsExchangeCanonicalEmissionV22>()
        .checked_add(GFX942_PHYSICAL_LDS_EXCHANGE_ASSEMBLY_BYTES_V22)
        .and_then(|n| n.checked_add(GFX942_PHYSICAL_LDS_EXCHANGE_LLVM_BYTES_V22))
        .ok_or(Resource::Arithmetic)?;
    // Output and construction arrays may coexist across moves; conservatively
    // retain one extra complete header plus bounded formatting/bookkeeping.
    let scratch = retained
        .checked_add(std::mem::size_of::<
            Gfx942PhysicalLdsExchangeCanonicalEmissionV22,
        >())
        .and_then(|n| n.checked_add(4096))
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(
        budget.storage(),
        1,
        GFX942_PHYSICAL_LDS_EXCHANGE_EMISSION_WORK_V22,
        scratch,
        |_| {
            Ok((
                emit::render(owner)?,
                Gfx942PhysicalLdsExchangeCanonicalEmissionStorageV22 { retained },
            ))
        },
    )
}
