use core::{error::Error, fmt};

use dialect_amdgcn::{AmdgcnPlironLlvmProfileV1, AmdgcnPlironLlvmRejectionV1};
use fe2o3_llvm_handoff::{Gfx942HandoffV2, HandoffIdentityV2};
use fe2o3_pliron::ContextIdentity;

/// Maximum canonical receipt length accepted by the V1 lane.
pub const MAX_LOWERING_RECEIPT_BYTES_V1: usize = 8 * 1024 * 1024;

/// Stable construction stages used by bounded diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionStageV1 {
    /// Process-local owner identity initialization.
    ContextIdentity,
    /// Pliron LLVM graph creation.
    DialectGraph,
    /// Recursive Pliron verification.
    DialectVerification,
    /// Owner-checked live graph inspection.
    DialectInspection,
    /// Canonical receipt construction.
    Receipt,
}

/// Failure from the bounded typed lowering lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoweringErrorV1 {
    /// Typed AMDGPU admission rejected the request.
    Admission(AmdgcnPlironLlvmRejectionV1),
    /// A bounded internal construction stage failed closed.
    Construction(ConstructionStageV1),
    /// An upstream Pliron panic was contained.
    UpstreamPanicked,
}

impl fmt::Display for LoweringErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "typed AMDGPU-to-Pliron-LLVM lowering failed: {self:?}"
        )
    }
}

impl Error for LoweringErrorV1 {}

/// Failure while re-inspecting the private live Pliron graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionErrorV1 {
    /// The retained owner identity is absent or changed.
    ContextIdentity,
    /// The private module belongs to a different context.
    ForeignOwner,
    /// The private module arena entry is no longer live.
    StaleModule,
    /// Recursive Pliron verification failed.
    DialectVerification,
    /// The graph contains an operation or shape outside V1.
    UnexpectedGraph,
    /// An upstream Pliron panic was contained.
    UpstreamPanicked,
}

impl fmt::Display for InspectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "typed Pliron LLVM inspection failed: {self:?}")
    }
}

impl Error for InspectionErrorV1 {}

/// SHA-256 identity of exact canonical source and typed graph facts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LoweringReceiptIdentityV1(pub(crate) [u8; 32]);

impl LoweringReceiptIdentityV1 {
    /// Returns the exact digest bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Deterministic canonical receipt for one typed lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalLoweringReceiptV1 {
    pub(crate) bytes: Vec<u8>,
    pub(crate) identity: LoweringReceiptIdentityV1,
}

impl CanonicalLoweringReceiptV1 {
    /// Returns canonical fe2o3-owned bytes. These bytes grant no artifact or runtime authority.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the SHA-256 identity of the exact canonical receipt.
    pub const fn identity(&self) -> LoweringReceiptIdentityV1 {
        self.identity
    }
}

/// Owner-checked facts recovered from the live Pliron LLVM graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveGraphInspectionV1 {
    pub(crate) global_count: u32,
    pub(crate) function_count: u32,
    pub(crate) block_count: u32,
    pub(crate) block_argument_count: u32,
    pub(crate) operation_count: u32,
    pub(crate) graph_sha256: [u8; 32],
    pub(crate) strict_float: bool,
    pub(crate) exact_memory_alignment: bool,
}

impl LiveGraphInspectionV1 {
    /// Returns the number of live `llvm.global` operations.
    pub const fn global_count(self) -> u32 {
        self.global_count
    }

    /// Returns the number of live `llvm.func` operations.
    pub const fn function_count(self) -> u32 {
        self.function_count
    }

    /// Returns the number of live basic blocks.
    pub const fn block_count(self) -> u32 {
        self.block_count
    }

    /// Returns the number of block arguments, including function parameters and lowered phi values.
    pub const fn block_argument_count(self) -> u32 {
        self.block_argument_count
    }

    /// Returns the number of body operations, including terminators.
    pub const fn operation_count(self) -> u32 {
        self.operation_count
    }

    /// Returns the digest of typed live graph shape facts.
    pub const fn graph_sha256(self) -> [u8; 32] {
        self.graph_sha256
    }

    /// Returns whether every live floating operation has empty fast-math flags.
    pub const fn strict_float(self) -> bool {
        self.strict_float
    }

    /// Returns whether every live load and store retains its exact typed alignment.
    pub const fn exact_memory_alignment(self) -> bool {
        self.exact_memory_alignment
    }
}

pub(crate) struct OwnedDialectModuleV1 {
    pub(crate) owner: ContextIdentity,
    pub(crate) module: pliron::builtin::ops::ModuleOp,
}

/// One privately owner-bound, verified Pliron LLVM graph and its exact typed policy source.
///
/// Raw contexts, arena pointers, operation wrappers, and printer text do not
/// cross this boundary. This value grants no object, linking, loading, or
/// execution authority.
pub struct LoweredAmdgcnPlironLlvmV1 {
    pub(crate) context: pliron::context::Context,
    pub(crate) module: OwnedDialectModuleV1,
    pub(crate) context_identity: ContextIdentity,
    pub(crate) source: Gfx942HandoffV2,
    pub(crate) source_identity: HandoffIdentityV2,
    pub(crate) profile: AmdgcnPlironLlvmProfileV1,
    pub(crate) inspection: LiveGraphInspectionV1,
    pub(crate) receipt: CanonicalLoweringReceiptV1,
}

impl LoweredAmdgcnPlironLlvmV1 {
    /// Returns non-durable process-local owner provenance.
    pub const fn context_identity(&self) -> ContextIdentity {
        self.context_identity
    }

    /// Returns the exact canonical typed source retained as policy authority.
    pub const fn source_handoff(&self) -> &Gfx942HandoffV2 {
        &self.source
    }

    /// Returns the exact canonical source identity.
    pub const fn source_identity(&self) -> HandoffIdentityV2 {
        self.source_identity
    }

    /// Returns the closed admitted source profile.
    pub const fn profile(&self) -> AmdgcnPlironLlvmProfileV1 {
        self.profile
    }

    /// Returns deterministic canonical source and graph receipt bytes.
    pub const fn receipt(&self) -> &CanonicalLoweringReceiptV1 {
        &self.receipt
    }

    /// Returns facts captured by owner-checked inspection at construction.
    pub const fn construction_inspection(&self) -> LiveGraphInspectionV1 {
        self.inspection
    }

    /// Revalidates ownership, liveness, recursive verification, and typed graph facts.
    pub fn inspect_live_graph(&self) -> Result<LiveGraphInspectionV1, InspectionErrorV1> {
        crate::lower::inspect_lowered(self)
    }

    /// This structural lowering grants no artifact or runtime authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }
}
