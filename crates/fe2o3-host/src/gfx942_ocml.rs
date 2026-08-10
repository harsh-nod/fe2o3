use core::fmt;
use fe2o3_amd_target::{AmdTargetId, FeatureState};
use fe2o3_core::{
    BorrowedDeviceOperation, ContextIdentity, DeviceBuffer, GpuContext, GpuFunction, KernelParams,
    LaunchConfig, Stream, launch_kernel_on_stream,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const GFX942_OCML_SIN_F32_IMPORT_SYMBOL_V1: &str = "__ocml_sin_f32";
pub const GFX942_OCML_SIN_F32_KERNEL_SYMBOL_V1: &str = "fe2o3_gfx942_ocml_sin_f32_v1";
pub const GFX942_OCML_SIN_F32_TARGET_V1: &str = "gfx942:xnack-";
pub const GFX942_OCML_SIN_F32_CODE_OBJECT_VERSION_V1: u8 = 5;
pub const GFX942_OCML_SIN_F32_DEVICE_ABI_V1: &str =
    "C(f32)->f32;strict;finite-only-off;unsafe-math-off";
pub const GFX942_OCML_SIN_F32_KERNEL_ABI_V1: &str =
    "C(global const f32*,global mut f32*,u64)->unit";
pub const GFX942_OCML_SIN_F32_WORKGROUP_SIZE_V1: u32 = 256;
pub const GFX942_OCML_SIN_F32_MAX_ELEMENTS_V1: usize = 256;
pub const GFX942_OCML_SIN_F32_MAX_HSACO_BYTES_V1: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942OcmlArtifactIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl Gfx942OcmlArtifactIdentityV1 {
    pub fn calculate(bytes: &[u8]) -> Result<Self, Gfx942OcmlSinErrorV1> {
        if bytes.is_empty() {
            return Err(Gfx942OcmlSinErrorV1::EmptyArtifact);
        }
        if bytes.len() > GFX942_OCML_SIN_F32_MAX_HSACO_BYTES_V1 {
            return Err(Gfx942OcmlSinErrorV1::ArtifactTooLarge);
        }
        Ok(Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        })
    }

    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && self.sha256 == <[u8; 32]>::from(Sha256::digest(bytes))
    }
}

pub struct Gfx942OcmlSinF32KernelV1 {
    function: GpuFunction,
    context_identity: ContextIdentity,
    target: AmdTargetId,
    artifact_identity: Gfx942OcmlArtifactIdentityV1,
}

#[derive(Debug)]
pub enum Gfx942OcmlSinErrorV1 {
    Runtime(fe2o3_core::Error),
    EmptyArtifact,
    ArtifactTooLarge,
    ArtifactSubstitution,
    UnsupportedTarget(AmdTargetId),
    ContextSubstitution,
    LengthMismatch { input: usize, output: usize },
    LengthOutOfRange(usize),
    AliasedInputOutput,
}

impl fmt::Display for Gfx942OcmlSinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => write!(formatter, "{error}"),
            Self::EmptyArtifact => formatter.write_str("OCML HSACO is empty"),
            Self::ArtifactTooLarge => formatter.write_str("OCML HSACO exceeds its byte bound"),
            Self::ArtifactSubstitution => {
                formatter.write_str("OCML HSACO bytes do not match the reviewed identity")
            }
            Self::UnsupportedTarget(target) => write!(
                formatter,
                "OCML sin contract requires gfx942:xnack-, observed {target}"
            ),
            Self::ContextSubstitution => {
                formatter.write_str("OCML kernel, stream, and buffers must share one exact context")
            }
            Self::LengthMismatch { input, output } => write!(
                formatter,
                "OCML sin input length {input} differs from output length {output}"
            ),
            Self::LengthOutOfRange(length) => write!(
                formatter,
                "OCML sin length {length} is outside 1..={GFX942_OCML_SIN_F32_MAX_ELEMENTS_V1}"
            ),
            Self::AliasedInputOutput => {
                formatter.write_str("OCML sin input and output allocations must be distinct")
            }
        }
    }
}

impl std::error::Error for Gfx942OcmlSinErrorV1 {}

impl From<fe2o3_core::Error> for Gfx942OcmlSinErrorV1 {
    fn from(error: fe2o3_core::Error) -> Self {
        Self::Runtime(error)
    }
}

impl Gfx942OcmlSinF32KernelV1 {
    /// Loads a worker-produced OCML kernel after binding exact retained bytes.
    ///
    /// # Safety
    ///
    /// `expected` must come from authenticated direct-worker evidence for the
    /// exact in-process LLVM/LLD link contract named by this module. That
    /// evidence must establish code-object V5, the exact import and kernel
    /// symbols, both physical ABIs, strict math policy, and complete provider
    /// closure. This constructor checks byte identity, runtime target, and
    /// symbol resolution but does not authenticate that evidence itself.
    pub unsafe fn load_reviewed_hsaco_unchecked(
        context: &Arc<GpuContext>,
        bytes: &[u8],
        expected: Gfx942OcmlArtifactIdentityV1,
    ) -> Result<Self, Gfx942OcmlSinErrorV1> {
        Gfx942OcmlArtifactIdentityV1::calculate(bytes)?;
        if !expected.matches(bytes) {
            return Err(Gfx942OcmlSinErrorV1::ArtifactSubstitution);
        }
        let target = require_gfx942_xnack_off(context)?;
        let module = unsafe { context.load_module_from_bytes_unchecked(bytes) }?;
        let function = module.load_function(GFX942_OCML_SIN_F32_KERNEL_SYMBOL_V1)?;
        Ok(Self {
            function,
            context_identity: context.identity(),
            target,
            artifact_identity: expected,
        })
    }

    pub const fn target(&self) -> AmdTargetId {
        self.target
    }

    pub const fn artifact_identity(&self) -> Gfx942OcmlArtifactIdentityV1 {
        self.artifact_identity
    }

    pub fn launch_scoped<'stream, 'resources, O>(
        &self,
        stream: &'stream Stream,
        input: &'resources DeviceBuffer<f32>,
        output: &'resources mut DeviceBuffer<f32>,
        during: impl for<'operation> FnOnce(
            &'operation BorrowedDeviceOperation<'stream, 'resources>,
        ) -> O,
    ) -> Result<O, Gfx942OcmlSinErrorV1> {
        if self.context_identity != stream.context().identity()
            || self.context_identity != input.context().identity()
            || self.context_identity != output.context().identity()
        {
            return Err(Gfx942OcmlSinErrorV1::ContextSubstitution);
        }
        if input.len() != output.len() {
            return Err(Gfx942OcmlSinErrorV1::LengthMismatch {
                input: input.len(),
                output: output.len(),
            });
        }
        if input.is_empty() || input.len() > GFX942_OCML_SIN_F32_MAX_ELEMENTS_V1 {
            return Err(Gfx942OcmlSinErrorV1::LengthOutOfRange(input.len()));
        }
        if input.allocation_identity() == output.allocation_identity() {
            return Err(Gfx942OcmlSinErrorV1::AliasedInputOutput);
        }

        let input_pointer = input.as_device_ptr();
        let output_pointer = output.as_device_ptr();
        let length = input.len() as u64;
        let function = self.function.clone();
        let config = LaunchConfig {
            grid_dim: (1, 1, 1),
            block_dim: (GFX942_OCML_SIN_F32_WORKGROUP_SIZE_V1, 1, 1),
            shared_mem_bytes: 0,
        };
        unsafe {
            BorrowedDeviceOperation::run_scoped_unchecked(
                stream,
                (function, input, output),
                |(function, _, _)| {
                    let mut params = KernelParams::new();
                    params.push(input_pointer);
                    params.push(output_pointer);
                    params.push(length);
                    launch_kernel_on_stream(function, config, stream, &mut params)
                },
                during,
            )
        }
        .map_err(Into::into)
    }
}

fn require_gfx942_xnack_off(
    context: &Arc<GpuContext>,
) -> Result<AmdTargetId, Gfx942OcmlSinErrorV1> {
    let target = context.observe_target()?.target_id();
    if target.processor() != "gfx942" || target.xnack() != Some(FeatureState::Disabled) {
        return Err(Gfx942OcmlSinErrorV1::UnsupportedTarget(target));
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_identity_is_exact_and_bounded() {
        let bytes = b"bounded reviewed HSACO fixture";
        let identity = Gfx942OcmlArtifactIdentityV1::calculate(bytes).unwrap();
        assert!(identity.matches(bytes));
        assert!(!identity.matches(b"bounded reviewed HSACO fixturE"));
        assert_eq!(identity.byte_len(), bytes.len() as u64);
        assert_ne!(identity.sha256(), &[0; 32]);
        assert!(matches!(
            Gfx942OcmlArtifactIdentityV1::calculate(&[]),
            Err(Gfx942OcmlSinErrorV1::EmptyArtifact)
        ));
        assert!(matches!(
            Gfx942OcmlArtifactIdentityV1::calculate(&vec![
                0;
                GFX942_OCML_SIN_F32_MAX_HSACO_BYTES_V1
                    + 1
            ]),
            Err(Gfx942OcmlSinErrorV1::ArtifactTooLarge)
        ));
    }

    #[test]
    fn reviewed_contract_constants_are_exact() {
        assert_eq!(GFX942_OCML_SIN_F32_IMPORT_SYMBOL_V1, "__ocml_sin_f32");
        assert_eq!(
            GFX942_OCML_SIN_F32_KERNEL_SYMBOL_V1,
            "fe2o3_gfx942_ocml_sin_f32_v1"
        );
        assert_eq!(GFX942_OCML_SIN_F32_TARGET_V1, "gfx942:xnack-");
        assert_eq!(GFX942_OCML_SIN_F32_CODE_OBJECT_VERSION_V1, 5);
        assert_eq!(GFX942_OCML_SIN_F32_WORKGROUP_SIZE_V1, 256);
    }
}
