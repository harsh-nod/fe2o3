#![no_std]
#![forbid(unsafe_code)]
#![doc = "Deterministic LLVM assembly serialization from canonical fe2o3 handoff V2."]

extern crate alloc;

mod emit;
mod validate;

use alloc::vec::Vec;
use core::fmt;

use fe2o3_llvm_handoff::{BlockIdV2, FunctionIdV2, Gfx942HandoffV2, HandoffIdentityV2, ValueIdV2};
use sha2::{Digest as _, Sha256};

/// Maximum number of bytes in one serialized LLVM assembly artifact.
///
/// The serializer checks this limit on every write and returns an error instead
/// of constructing a larger artifact.
pub const MAX_LLVM_ASSEMBLY_BYTES_V2: usize = 16 * 1024 * 1024;

/// The SHA-256 content identity of serialized LLVM assembly bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LlvmAssemblySha256V2([u8; 32]);

impl LlvmAssemblySha256V2 {
    /// Returns the digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for LlvmAssemblySha256V2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Immutable, bounded LLVM assembly produced from a validated gfx942 handoff.
///
/// This artifact is text and identity only. It grants no code generation,
/// linking, loading, or runtime authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942LlvmAssemblyV2 {
    bytes: Vec<u8>,
    sha256: LlvmAssemblySha256V2,
    source_identity: HandoffIdentityV2,
}

/// A named-metadata channel emitted by the closed LLVM subset.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum LlvmNamedMetadataV2 {
    /// `!opencl.ocl.version`.
    OpenClOclVersion,
    /// `!opencl.spir.version`.
    OpenClSpirVersion,
    /// `!llvm.ident`.
    LlvmIdent,
}

impl Gfx942LlvmAssemblyV2 {
    fn from_validated(bytes: Vec<u8>, source_identity: HandoffIdentityV2) -> Self {
        let sha256 = LlvmAssemblySha256V2(Sha256::digest(&bytes).into());
        Self {
            bytes,
            sha256,
            source_identity,
        }
    }

    /// Returns the immutable LLVM assembly bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the LLVM assembly as UTF-8 text.
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes).expect("the emitter writes only ASCII")
    }

    /// Returns the number of LLVM assembly bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the LLVM assembly is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Returns the SHA-256 identity of the exact assembly bytes.
    pub const fn sha256(&self) -> LlvmAssemblySha256V2 {
        self.sha256
    }

    /// Returns the canonical identity of the source V2 handoff.
    pub const fn source_identity(&self) -> HandoffIdentityV2 {
        self.source_identity
    }

    /// Returns whether the bytes contain the exact source identity binding.
    pub fn has_embedded_source_identity(&self) -> bool {
        const HEADER: &[u8] = b"!fe2o3.handoff.identity = !{!";
        const PREFIX: &[u8] = b"!{!\"sha256:";
        const SUFFIX: &[u8] = b"\"}";

        if !contains_bytes(&self.bytes, HEADER) {
            return false;
        }
        let identity = hex_identity(self.source_identity.as_bytes());
        self.bytes
            .windows(PREFIX.len() + identity.len() + SUFFIX.len())
            .any(|window| {
                window.starts_with(PREFIX)
                    && window[PREFIX.len()..PREFIX.len() + identity.len()] == identity
                    && window.ends_with(SUFFIX)
            })
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn hex_identity(identity: &[u8; 32]) -> [u8; 64] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = [0_u8; 64];
    for (index, byte) in identity.iter().copied().enumerate() {
        encoded[index * 2] = HEX[usize::from(byte >> 4)];
        encoded[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
    }
    encoded
}

/// A reason a validated V2 handoff cannot be represented by the closed LLVM
/// assembly subset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SerializeErrorV2 {
    /// A user-defined symbol occupies LLVM's reserved intrinsic namespace.
    ReservedLlvmSymbol,
    /// A scalar-pointee GEP has a number of indices LLVM cannot type-check.
    UnsupportedGetElementPtr {
        /// Function containing the GEP.
        function: FunctionIdV2,
        /// Number of indices supplied by the handoff.
        indices: usize,
    },
    /// A block cannot be reached from the function's entry block.
    UnreachableBlock {
        /// Function containing the block.
        function: FunctionIdV2,
        /// Unreachable block.
        block: BlockIdV2,
    },
    /// A CFG edge targets the function entry block.
    EntryBlockHasPredecessor {
        /// Function containing the edge.
        function: FunctionIdV2,
        /// Block whose terminator targets the entry block.
        predecessor: BlockIdV2,
    },
    /// An ordinary call targets an AMDGPU kernel entry point.
    KernelCall {
        /// Calling function.
        caller: FunctionIdV2,
        /// Kernel function used as a call target.
        callee: FunctionIdV2,
    },
    /// An operand has no definition in its function.
    MissingSsaDefinition {
        /// Function containing the use.
        function: FunctionIdV2,
        /// Referenced value.
        value: ValueIdV2,
        /// Block containing the use.
        use_block: BlockIdV2,
    },
    /// An instruction consumes a value defined later in the same block.
    SsaUseBeforeDefinition {
        /// Function containing the use.
        function: FunctionIdV2,
        /// Referenced value.
        value: ValueIdV2,
        /// Block containing both definition and use.
        block: BlockIdV2,
    },
    /// A cross-block definition does not dominate its use.
    SsaDefinitionDoesNotDominate {
        /// Function containing the definition and use.
        function: FunctionIdV2,
        /// Referenced value.
        value: ValueIdV2,
        /// Block containing the definition.
        definition_block: BlockIdV2,
        /// Block containing the use.
        use_block: BlockIdV2,
    },
    /// Multiple typed metadata entries map to one LLVM named-metadata channel.
    DuplicateEmittedNamedMetadata {
        /// Colliding LLVM named-metadata channel.
        metadata: LlvmNamedMetadataV2,
    },
    /// A parameter name collides with a generated SSA or block name.
    ConflictingEmittedLocalName {
        /// Function containing the conflicting local name.
        function: FunctionIdV2,
    },
    /// The emitted bytes would exceed the fixed artifact bound.
    AssemblyBytesLimitExceeded {
        /// Maximum supported byte length.
        maximum: usize,
    },
    /// A graph relationship was inconsistent with the validated V2 model.
    InconsistentValidatedModel,
}

impl fmt::Display for SerializeErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedLlvmSymbol => {
                formatter.write_str("a user symbol uses LLVM's reserved intrinsic namespace")
            }
            Self::UnsupportedGetElementPtr { function, indices } => write!(
                formatter,
                "function {} uses {indices} GEP indices for a scalar pointee",
                function.get()
            ),
            Self::UnreachableBlock { function, block } => write!(
                formatter,
                "block {} in function {} is unreachable from the entry block",
                block.get(),
                function.get()
            ),
            Self::EntryBlockHasPredecessor {
                function,
                predecessor,
            } => write!(
                formatter,
                "block {} targets the entry block of function {}",
                predecessor.get(),
                function.get()
            ),
            Self::KernelCall { caller, callee } => write!(
                formatter,
                "function {} calls AMDGPU kernel entry {}",
                caller.get(),
                callee.get()
            ),
            Self::MissingSsaDefinition {
                function,
                value,
                use_block,
            } => write!(
                formatter,
                "value {} used in block {} of function {} has no definition",
                value.get(),
                use_block.get(),
                function.get()
            ),
            Self::SsaUseBeforeDefinition {
                function,
                value,
                block,
            } => write!(
                formatter,
                "value {} is used before its definition in block {} of function {}",
                value.get(),
                block.get(),
                function.get()
            ),
            Self::SsaDefinitionDoesNotDominate {
                function,
                value,
                definition_block,
                use_block,
            } => write!(
                formatter,
                "value {} from block {} does not dominate block {} in function {}",
                value.get(),
                definition_block.get(),
                use_block.get(),
                function.get()
            ),
            Self::DuplicateEmittedNamedMetadata { metadata } => write!(
                formatter,
                "multiple typed entries map to LLVM named metadata {metadata:?}"
            ),
            Self::ConflictingEmittedLocalName { function } => write!(
                formatter,
                "a parameter name conflicts with a generated local name in function {}",
                function.get()
            ),
            Self::AssemblyBytesLimitExceeded { maximum } => {
                write!(formatter, "LLVM assembly exceeds the {maximum}-byte limit")
            }
            Self::InconsistentValidatedModel => {
                formatter.write_str("the validated V2 model is internally inconsistent")
            }
        }
    }
}

impl core::error::Error for SerializeErrorV2 {}

/// Serializes a validated gfx942 V2 handoff into deterministic LLVM assembly.
///
/// The complete model is admitted before any bytes are returned. Emission uses
/// only typed handoff accessors and fixed LLVM tokens.
pub fn serialize_gfx942_handoff_v2(
    handoff: &Gfx942HandoffV2,
) -> Result<Gfx942LlvmAssemblyV2, SerializeErrorV2> {
    validate::admit(handoff)?;
    let source_identity = handoff.identity();
    let bytes = emit::emit(handoff, source_identity)?;
    Ok(Gfx942LlvmAssemblyV2::from_validated(bytes, source_identity))
}
