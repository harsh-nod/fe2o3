use fe2o3_build_authority::CompilerClosureV2;

use crate::{CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2, ValidationError};

/// Maximum size of one complete encoded V3 descriptor.
///
/// The bound admits every valid V2 descriptor plus the fixed-size canonical
/// [`CompilerClosureV2`] preimage added by V3.
pub const MAX_DESCRIPTOR_BYTES_V3: usize = crate::MAX_DESCRIPTOR_BYTES_V2 + 2 + 6 * 32;

/// A canonical V3 identity for one exact rustc process and its compiler closure.
///
/// V3 preserves the complete V2 process identity and adds all six content pins
/// and the transition-protocol version that form the canonical
/// [`CompilerClosureV2`] identity preimage. The descriptor's rustc and backend
/// digests must equal the pins assigned those roles in the closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcInvocationDescriptorV3 {
    pub(crate) descriptor_v2: RustcInvocationDescriptorV2,
    pub(crate) compiler_closure: CompilerClosureV2,
}

impl RustcInvocationDescriptorV3 {
    /// Constructs V3 from an exact V2 process descriptor and a compiler closure.
    ///
    /// This is the explicit V2-plus-closure upgrade. It rejects a closure whose
    /// rustc-executable or codegen-backend pin differs from the digest already
    /// carried in the V2 descriptor.
    pub fn new(
        descriptor_v2: RustcInvocationDescriptorV2,
        compiler_closure: CompilerClosureV2,
    ) -> Result<Self, ValidationError> {
        Self::from_v2_and_compiler_closure(descriptor_v2, compiler_closure)
    }

    /// Explicitly upgrades a V2 descriptor with its canonical compiler closure.
    pub fn from_v2_and_compiler_closure(
        descriptor_v2: RustcInvocationDescriptorV2,
        compiler_closure: CompilerClosureV2,
    ) -> Result<Self, ValidationError> {
        let descriptor = Self {
            descriptor_v2,
            compiler_closure,
        };
        descriptor.validate_compiler_closure_pins()?;
        Ok(descriptor)
    }

    /// Returns the exact V2 process descriptor embedded by V3.
    pub const fn descriptor_v2(&self) -> &RustcInvocationDescriptorV2 {
        &self.descriptor_v2
    }

    /// Returns the canonical compiler closure embedded by V3.
    pub const fn compiler_closure(&self) -> &CompilerClosureV2 {
        &self.compiler_closure
    }

    /// Returns the canonical aggregate compiler-closure identity.
    pub const fn compiler_closure_identity_sha256(&self) -> [u8; 32] {
        self.compiler_closure.identity_sha256()
    }

    /// Returns the SHA-256 digest of the rustc executable bytes.
    pub const fn rustc_executable_sha256(&self) -> &[u8; 32] {
        self.descriptor_v2.rustc_executable_sha256()
    }

    /// Returns the SHA-256 digest of the codegen-backend bytes.
    pub const fn codegen_backend_sha256(&self) -> &[u8; 32] {
        self.descriptor_v2.codegen_backend_sha256()
    }

    /// Returns rustc's exact working directory and final argument vector.
    pub const fn rustc(&self) -> &RustcUnitV2 {
        self.descriptor_v2.rustc()
    }

    /// Returns the complete intended rustc environment.
    pub const fn compile_environment(&self) -> &CompileEnvironmentV2 {
        self.descriptor_v2.compile_environment()
    }

    /// Returns the rustc path represented once in `argv[0]`.
    pub fn rustc_executable_path(&self) -> &str {
        self.descriptor_v2.rustc_executable_path()
    }

    /// Returns the codegen-backend path represented once in rustc's arguments.
    pub fn codegen_backend_path(&self) -> &str {
        self.descriptor_v2.codegen_backend_path()
    }

    /// Returns the canonical AMD target represented once in the environment.
    pub fn amd_target(&self) -> &str {
        self.descriptor_v2.amd_target()
    }

    /// Returns the canonical artifact directory represented once in the environment.
    pub fn artifact_output_directory(&self) -> &str {
        self.descriptor_v2.artifact_output_directory()
    }

    /// Reports whether kernel-IR verification is enabled by the environment.
    pub fn verification_required(&self) -> bool {
        self.descriptor_v2.verification_required()
    }

    pub(crate) fn validate_compiler_closure_pins(&self) -> Result<(), ValidationError> {
        if *self.descriptor_v2.rustc_executable_sha256()
            != self.compiler_closure.rustc_executable_sha256()
        {
            return Err(ValidationError::CompilerClosurePinMismatch {
                field: "rustc executable",
            });
        }
        if *self.descriptor_v2.codegen_backend_sha256()
            != self.compiler_closure.codegen_backend_sha256()
        {
            return Err(ValidationError::CompilerClosurePinMismatch {
                field: "codegen backend",
            });
        }
        Ok(())
    }
}
