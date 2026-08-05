use std::fmt;

use crate::{
    ArtifactContainerV1, Capability, CodeObjectFormat, CodeObjectIdentity, DigestBytes, Endianness,
    KernelEntry, ManifestV1, PointerWidth, TargetIdentity,
};

/// A kernel entry and native payload borrowed from one validated container.
///
/// The token cannot outlive or be detached from the container that established
/// exact payload closure. It does not establish target compatibility,
/// authenticity, ABI compatibility with host types, or launch safety.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedNativeKernel<'container> {
    manifest: &'container ManifestV1,
    kernel: &'container KernelEntry,
    code_object: &'container CodeObjectIdentity,
    payload: &'container [u8],
}

impl<'container> SelectedNativeKernel<'container> {
    pub const fn manifest(self) -> &'container ManifestV1 {
        self.manifest
    }

    pub const fn target(self) -> &'container TargetIdentity {
        self.manifest.target()
    }

    pub const fn kernel(self) -> &'container KernelEntry {
        self.kernel
    }

    pub const fn code_object(self) -> &'container CodeObjectIdentity {
        self.code_object
    }

    pub const fn payload(self) -> &'container [u8] {
        self.payload
    }

    /// Compares the manifest's target declaration with another declaration.
    ///
    /// Triple and architecture are compared as exact, opaque strings. Candidate
    /// capabilities may be a superset of the kernel's declared requirements.
    /// This does not inspect payload metadata, discover hardware, or implement
    /// AMD target-feature compatibility rules.
    pub fn check_declared_target(
        self,
        candidate_target: &TargetIdentity,
    ) -> Result<(), DeclaredTargetMismatch> {
        let artifact = self.target();
        if artifact.triple() != candidate_target.triple() {
            return Err(DeclaredTargetMismatch::Triple);
        }
        if artifact.architecture() != candidate_target.architecture() {
            return Err(DeclaredTargetMismatch::Architecture);
        }
        if artifact.pointer_width() != candidate_target.pointer_width() {
            return Err(DeclaredTargetMismatch::PointerWidth {
                artifact: artifact.pointer_width(),
                candidate: candidate_target.pointer_width(),
            });
        }
        if artifact.endianness() != candidate_target.endianness() {
            return Err(DeclaredTargetMismatch::Endianness {
                artifact: artifact.endianness(),
                candidate: candidate_target.endianness(),
            });
        }
        for capability in self.kernel.required_capabilities() {
            if candidate_target
                .capabilities()
                .binary_search(capability)
                .is_err()
            {
                return Err(DeclaredTargetMismatch::MissingCapability(*capability));
            }
        }

        Ok(())
    }
}

impl ArtifactContainerV1 {
    /// Selects one native executable by its stable manifest-owned kernel ID.
    ///
    /// Payload presence, length, and digest were established when the container
    /// was constructed or decoded. Target and generated-host ABI checks belong
    /// to the runtime integration that consumes the returned token.
    pub fn select_native_kernel(
        &self,
        kernel_id: DigestBytes,
    ) -> Result<SelectedNativeKernel<'_>, KernelSelectionError> {
        let kernel = self
            .manifest()
            .kernels()
            .binary_search_by_key(&kernel_id, KernelEntry::kernel_id)
            .ok()
            .map(|index| &self.manifest().kernels()[index])
            .ok_or(KernelSelectionError::UnknownKernel(kernel_id))?;

        let code_object = self
            .manifest()
            .code_objects()
            .binary_search_by_key(&kernel.code_object_digest(), CodeObjectIdentity::digest)
            .ok()
            .map(|index| &self.manifest().code_objects()[index])
            .expect("validated manifest kernel must reference a code object");
        if code_object.format() != CodeObjectFormat::NativeExecutable {
            return Err(KernelSelectionError::UnsupportedFormat(
                code_object.format(),
            ));
        }

        let payload = self
            .payloads()
            .binary_search_by_key(&code_object.digest(), |payload| payload.digest().bytes())
            .ok()
            .map(|index| &self.payloads()[index])
            .expect("validated container must contain every manifest code object");

        Ok(SelectedNativeKernel {
            manifest: self.manifest(),
            kernel,
            code_object,
            payload: payload.bytes(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum KernelSelectionError {
    UnknownKernel(DigestBytes),
    UnsupportedFormat(CodeObjectFormat),
}

impl fmt::Display for KernelSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKernel(_) => write!(formatter, "artifact does not contain the kernel"),
            Self::UnsupportedFormat(format) => {
                write!(
                    formatter,
                    "kernel payload format {format:?} is not directly loadable"
                )
            }
        }
    }
}

impl std::error::Error for KernelSelectionError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DeclaredTargetMismatch {
    Triple,
    Architecture,
    PointerWidth {
        artifact: PointerWidth,
        candidate: PointerWidth,
    },
    Endianness {
        artifact: Endianness,
        candidate: Endianness,
    },
    MissingCapability(Capability),
}

impl fmt::Display for DeclaredTargetMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Triple => write!(
                formatter,
                "candidate target triple does not match the artifact declaration"
            ),
            Self::Architecture => {
                write!(
                    formatter,
                    "candidate architecture does not match the artifact declaration"
                )
            }
            Self::PointerWidth {
                artifact,
                candidate,
            } => write!(
                formatter,
                "candidate pointer width {candidate:?} does not match artifact declaration {artifact:?}"
            ),
            Self::Endianness {
                artifact,
                candidate,
            } => write!(
                formatter,
                "candidate endianness {candidate:?} does not match artifact declaration {artifact:?}"
            ),
            Self::MissingCapability(capability) => write!(
                formatter,
                "candidate target lacks required capability {}",
                capability.name()
            ),
        }
    }
}

impl std::error::Error for DeclaredTargetMismatch {}
