use std::fmt;

use sha2::{Digest, Sha256};

/// Domain for the canonical rustc executable/runtime-tree identity.
pub const RUSTC_EXECUTABLE_RUNTIME_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3-rustc-executable-runtime-identity-v1\0";

/// Domain for the canonical compiler-closure identity.
pub const COMPILER_CLOSURE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3-compiler-closure-identity-v1\0";

/// One content digest in the canonical compiler closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerClosureDigestFieldV1 {
    /// The Cargo executable image.
    CargoExecutable,
    /// The rustc executable image.
    RustcExecutable,
    /// The complete rustc runtime-tree input covered by the existing contract.
    RustcRuntimeTree,
    /// The selected rustc codegen-backend image.
    CodegenBackend,
    /// The declared aggregate compiler-closure identity.
    CompilerClosure,
}

impl fmt::Display for CompilerClosureDigestFieldV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::CargoExecutable => "Cargo executable",
            Self::RustcExecutable => "rustc executable",
            Self::RustcRuntimeTree => "rustc runtime tree",
            Self::CodegenBackend => "codegen backend",
            Self::CompilerClosure => "compiler closure",
        };
        formatter.write_str(name)
    }
}

/// A failure to construct or validate a canonical compiler closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerClosureErrorV1 {
    /// A required SHA-256 content digest was all zero.
    ZeroDigest {
        /// The rejected digest.
        field: CompilerClosureDigestFieldV1,
    },
    /// The declared aggregate did not match the canonical derivation.
    IdentityMismatch,
}

impl fmt::Display for CompilerClosureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDigest { field } => write!(formatter, "{field} digest must be nonzero"),
            Self::IdentityMismatch => {
                formatter.write_str("declared compiler closure does not match its canonical pins")
            }
        }
    }
}

impl std::error::Error for CompilerClosureErrorV1 {}

/// The four independently provisioned compiler pins and their canonical aggregate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerClosureV1 {
    cargo_executable_sha256: [u8; 32],
    rustc_executable_sha256: [u8; 32],
    rustc_runtime_tree_sha256: [u8; 32],
    codegen_backend_sha256: [u8; 32],
    identity_sha256: [u8; 32],
}

impl CompilerClosureV1 {
    /// Derives the canonical aggregate from four nonzero content pins.
    pub fn new(
        cargo_executable_sha256: [u8; 32],
        rustc_executable_sha256: [u8; 32],
        rustc_runtime_tree_sha256: [u8; 32],
        codegen_backend_sha256: [u8; 32],
    ) -> Result<Self, CompilerClosureErrorV1> {
        let rustc_identity_sha256 = derive_rustc_executable_runtime_identity_v1(
            rustc_executable_sha256,
            rustc_runtime_tree_sha256,
        );
        let identity_sha256 = derive_compiler_closure_identity_v1(
            cargo_executable_sha256,
            rustc_identity_sha256,
            codegen_backend_sha256,
        );
        Self::from_pins_and_identity(
            cargo_executable_sha256,
            rustc_executable_sha256,
            rustc_runtime_tree_sha256,
            codegen_backend_sha256,
            identity_sha256,
        )
    }

    /// Validates a declared aggregate against four nonzero content pins.
    pub fn from_pins_and_identity(
        cargo_executable_sha256: [u8; 32],
        rustc_executable_sha256: [u8; 32],
        rustc_runtime_tree_sha256: [u8; 32],
        codegen_backend_sha256: [u8; 32],
        identity_sha256: [u8; 32],
    ) -> Result<Self, CompilerClosureErrorV1> {
        for (field, digest) in [
            (
                CompilerClosureDigestFieldV1::CargoExecutable,
                cargo_executable_sha256,
            ),
            (
                CompilerClosureDigestFieldV1::RustcExecutable,
                rustc_executable_sha256,
            ),
            (
                CompilerClosureDigestFieldV1::RustcRuntimeTree,
                rustc_runtime_tree_sha256,
            ),
            (
                CompilerClosureDigestFieldV1::CodegenBackend,
                codegen_backend_sha256,
            ),
            (
                CompilerClosureDigestFieldV1::CompilerClosure,
                identity_sha256,
            ),
        ] {
            if digest == [0; 32] {
                return Err(CompilerClosureErrorV1::ZeroDigest { field });
            }
        }

        let rustc_identity_sha256 = derive_rustc_executable_runtime_identity_v1(
            rustc_executable_sha256,
            rustc_runtime_tree_sha256,
        );
        let expected = derive_compiler_closure_identity_v1(
            cargo_executable_sha256,
            rustc_identity_sha256,
            codegen_backend_sha256,
        );
        if identity_sha256 != expected {
            return Err(CompilerClosureErrorV1::IdentityMismatch);
        }

        Ok(Self {
            cargo_executable_sha256,
            rustc_executable_sha256,
            rustc_runtime_tree_sha256,
            codegen_backend_sha256,
            identity_sha256,
        })
    }

    /// Returns the Cargo executable content digest.
    pub const fn cargo_executable_sha256(self) -> [u8; 32] {
        self.cargo_executable_sha256
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

    /// Returns the canonical aggregate compiler-closure identity.
    pub const fn identity_sha256(self) -> [u8; 32] {
        self.identity_sha256
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_compiler_closure_golden_vector_is_stable() {
        let closure = CompilerClosureV1::new([0x05; 32], [0x06; 32], [0x07; 32], [0x08; 32])
            .expect("golden pins are nonzero");
        assert_eq!(
            closure.identity_sha256(),
            [
                0x1f, 0xea, 0xcf, 0xc5, 0x87, 0x9b, 0x85, 0x3c, 0x7b, 0xa5, 0x5c, 0x34, 0x53, 0x93,
                0x98, 0xe8, 0x57, 0xc0, 0xf9, 0x7d, 0x68, 0x6c, 0xbb, 0x63, 0xcf, 0x99, 0x79, 0x5a,
                0x6a, 0xa0, 0x9e, 0xc9,
            ]
        );
    }
}
