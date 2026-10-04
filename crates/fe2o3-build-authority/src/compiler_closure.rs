use std::fmt;

use sha2::{Digest, Sha256};

/// Domain for the canonical rustc executable/runtime-tree identity.
const RUSTC_EXECUTABLE_RUNTIME_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3-rustc-executable-runtime-identity-v1\0";

/// Domain for the canonical compiler-closure identity.
const COMPILER_CLOSURE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-compiler-closure-identity-v1\0";

/// Derives the existing canonical rustc executable/runtime-tree identity.
pub fn derive_rustc_executable_runtime_identity_v1(
    rustc_executable_sha256: [u8; 32],
    rustc_runtime_tree_sha256: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(RUSTC_EXECUTABLE_RUNTIME_IDENTITY_DOMAIN_V1);
    digest.update(rustc_executable_sha256);
    digest.update(rustc_runtime_tree_sha256);
    digest.finalize().into()
}

/// Derives the existing canonical compiler-closure identity.
pub fn derive_compiler_closure_identity_v1(
    cargo_executable_sha256: [u8; 32],
    rustc_executable_runtime_identity_sha256: [u8; 32],
    codegen_backend_sha256: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN_V1);
    digest.update(cargo_executable_sha256);
    digest.update(rustc_executable_runtime_identity_sha256);
    digest.update(codegen_backend_sha256);
    digest.finalize().into()
}

/// Canonical version of the Cargo-to-trampoline-to-wrapper transition protocol.
pub const CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1: u16 = 1;

/// Domain for the canonical six-pin compiler-closure identity.
pub const COMPILER_CLOSURE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

/// One content digest in the canonical V2 compiler closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerClosureDigestFieldV2 {
    /// The Cargo executable image.
    CargoExecutable,
    /// The static Cargo binding-trampoline image.
    CargoBindingTrampoline,
    /// The full cargo-fe2o3 binding-wrapper image.
    CargoFe2o3BindingWrapper,
    /// The rustc executable image.
    RustcExecutable,
    /// The complete rustc runtime-tree input.
    RustcRuntimeTree,
    /// The selected rustc codegen-backend image.
    CodegenBackend,
    /// The declared aggregate compiler-closure identity.
    CompilerClosure,
}

impl fmt::Display for CompilerClosureDigestFieldV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::CargoExecutable => "Cargo executable",
            Self::CargoBindingTrampoline => "Cargo binding trampoline",
            Self::CargoFe2o3BindingWrapper => "cargo-fe2o3 binding wrapper",
            Self::RustcExecutable => "rustc executable",
            Self::RustcRuntimeTree => "rustc runtime tree",
            Self::CodegenBackend => "codegen backend",
            Self::CompilerClosure => "compiler closure",
        };
        formatter.write_str(name)
    }
}

/// A failure to construct or validate a canonical V2 compiler closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerClosureErrorV2 {
    /// A required SHA-256 content digest was all zero.
    ZeroDigest {
        /// The rejected digest.
        field: CompilerClosureDigestFieldV2,
    },
    /// The declared transition protocol version is not canonical for V2.
    UnsupportedTransitionProtocolVersion {
        /// The rejected transition protocol version.
        version: u16,
    },
    /// The declared aggregate did not match the canonical derivation.
    IdentityMismatch,
}

impl fmt::Display for CompilerClosureErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDigest { field } => write!(formatter, "{field} digest must be nonzero"),
            Self::UnsupportedTransitionProtocolVersion { version } => write!(
                formatter,
                "Cargo binding transition protocol version {version} is not canonical"
            ),
            Self::IdentityMismatch => {
                formatter.write_str("declared compiler closure does not match its canonical pins")
            }
        }
    }
}

impl std::error::Error for CompilerClosureErrorV2 {}

/// Six independently provisioned compiler pins, a transition protocol, and their aggregate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerClosureV2 {
    cargo_executable_sha256: [u8; 32],
    cargo_binding_trampoline_sha256: [u8; 32],
    cargo_fe2o3_binding_wrapper_sha256: [u8; 32],
    rustc_executable_sha256: [u8; 32],
    rustc_runtime_tree_sha256: [u8; 32],
    codegen_backend_sha256: [u8; 32],
    cargo_binding_transition_protocol_version: u16,
    identity_sha256: [u8; 32],
}

impl CompilerClosureV2 {
    /// Derives the canonical aggregate from six nonzero content pins.
    pub fn new(
        cargo_executable_sha256: [u8; 32],
        cargo_binding_trampoline_sha256: [u8; 32],
        cargo_fe2o3_binding_wrapper_sha256: [u8; 32],
        rustc_executable_sha256: [u8; 32],
        rustc_runtime_tree_sha256: [u8; 32],
        codegen_backend_sha256: [u8; 32],
    ) -> Result<Self, CompilerClosureErrorV2> {
        let identity_sha256 = derive_compiler_closure_identity_v2(
            cargo_executable_sha256,
            cargo_binding_trampoline_sha256,
            cargo_fe2o3_binding_wrapper_sha256,
            rustc_executable_sha256,
            rustc_runtime_tree_sha256,
            codegen_backend_sha256,
            CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1,
        );
        Self::from_pins_and_identity(
            cargo_executable_sha256,
            cargo_binding_trampoline_sha256,
            cargo_fe2o3_binding_wrapper_sha256,
            rustc_executable_sha256,
            rustc_runtime_tree_sha256,
            codegen_backend_sha256,
            CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1,
            identity_sha256,
        )
    }

    /// Validates a declared aggregate against six pins and the canonical transition protocol.
    #[allow(clippy::too_many_arguments)]
    pub fn from_pins_and_identity(
        cargo_executable_sha256: [u8; 32],
        cargo_binding_trampoline_sha256: [u8; 32],
        cargo_fe2o3_binding_wrapper_sha256: [u8; 32],
        rustc_executable_sha256: [u8; 32],
        rustc_runtime_tree_sha256: [u8; 32],
        codegen_backend_sha256: [u8; 32],
        cargo_binding_transition_protocol_version: u16,
        identity_sha256: [u8; 32],
    ) -> Result<Self, CompilerClosureErrorV2> {
        if cargo_binding_transition_protocol_version != CARGO_BINDING_TRANSITION_PROTOCOL_VERSION_V1
        {
            return Err(
                CompilerClosureErrorV2::UnsupportedTransitionProtocolVersion {
                    version: cargo_binding_transition_protocol_version,
                },
            );
        }

        for (field, digest) in [
            (
                CompilerClosureDigestFieldV2::CargoExecutable,
                cargo_executable_sha256,
            ),
            (
                CompilerClosureDigestFieldV2::CargoBindingTrampoline,
                cargo_binding_trampoline_sha256,
            ),
            (
                CompilerClosureDigestFieldV2::CargoFe2o3BindingWrapper,
                cargo_fe2o3_binding_wrapper_sha256,
            ),
            (
                CompilerClosureDigestFieldV2::RustcExecutable,
                rustc_executable_sha256,
            ),
            (
                CompilerClosureDigestFieldV2::RustcRuntimeTree,
                rustc_runtime_tree_sha256,
            ),
            (
                CompilerClosureDigestFieldV2::CodegenBackend,
                codegen_backend_sha256,
            ),
            (
                CompilerClosureDigestFieldV2::CompilerClosure,
                identity_sha256,
            ),
        ] {
            if digest == [0; 32] {
                return Err(CompilerClosureErrorV2::ZeroDigest { field });
            }
        }

        let expected = derive_compiler_closure_identity_v2(
            cargo_executable_sha256,
            cargo_binding_trampoline_sha256,
            cargo_fe2o3_binding_wrapper_sha256,
            rustc_executable_sha256,
            rustc_runtime_tree_sha256,
            codegen_backend_sha256,
            cargo_binding_transition_protocol_version,
        );
        if identity_sha256 != expected {
            return Err(CompilerClosureErrorV2::IdentityMismatch);
        }

        Ok(Self {
            cargo_executable_sha256,
            cargo_binding_trampoline_sha256,
            cargo_fe2o3_binding_wrapper_sha256,
            rustc_executable_sha256,
            rustc_runtime_tree_sha256,
            codegen_backend_sha256,
            cargo_binding_transition_protocol_version,
            identity_sha256,
        })
    }

    /// Returns the Cargo executable content digest.
    pub const fn cargo_executable_sha256(self) -> [u8; 32] {
        self.cargo_executable_sha256
    }

    /// Returns the static Cargo binding-trampoline content digest.
    pub const fn cargo_binding_trampoline_sha256(self) -> [u8; 32] {
        self.cargo_binding_trampoline_sha256
    }

    /// Returns the full cargo-fe2o3 binding-wrapper content digest.
    pub const fn cargo_fe2o3_binding_wrapper_sha256(self) -> [u8; 32] {
        self.cargo_fe2o3_binding_wrapper_sha256
    }

    /// Returns the rustc executable content digest.
    pub const fn rustc_executable_sha256(self) -> [u8; 32] {
        self.rustc_executable_sha256
    }

    /// Returns the rustc runtime-tree content digest.
    pub const fn rustc_runtime_tree_sha256(self) -> [u8; 32] {
        self.rustc_runtime_tree_sha256
    }

    /// Returns the codegen-backend content digest.
    pub const fn codegen_backend_sha256(self) -> [u8; 32] {
        self.codegen_backend_sha256
    }

    /// Returns the canonical Cargo binding transition protocol version.
    pub const fn cargo_binding_transition_protocol_version(self) -> u16 {
        self.cargo_binding_transition_protocol_version
    }

    /// Returns the canonical aggregate compiler-closure identity.
    pub const fn identity_sha256(self) -> [u8; 32] {
        self.identity_sha256
    }
}

/// Derives a V2 compiler-closure identity from six pins and a transition protocol version.
///
/// The canonical transcript is the V2 domain, the little-endian protocol version, and the six
/// content pins in the same order as the parameters below.
#[allow(clippy::too_many_arguments)]
pub fn derive_compiler_closure_identity_v2(
    cargo_executable_sha256: [u8; 32],
    cargo_binding_trampoline_sha256: [u8; 32],
    cargo_fe2o3_binding_wrapper_sha256: [u8; 32],
    rustc_executable_sha256: [u8; 32],
    rustc_runtime_tree_sha256: [u8; 32],
    codegen_backend_sha256: [u8; 32],
    cargo_binding_transition_protocol_version: u16,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN_V2);
    digest.update(cargo_binding_transition_protocol_version.to_le_bytes());
    digest.update(cargo_executable_sha256);
    digest.update(cargo_binding_trampoline_sha256);
    digest.update(cargo_fe2o3_binding_wrapper_sha256);
    digest.update(rustc_executable_sha256);
    digest.update(rustc_runtime_tree_sha256);
    digest.update(codegen_backend_sha256);
    digest.finalize().into()
}
