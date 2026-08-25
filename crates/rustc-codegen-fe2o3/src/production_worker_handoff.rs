//! Workload-neutral preparation of the production compiler-module handoff.
//!
//! This module is the only feature-free bridge from the target-lowered production
//! transaction to the canonical compiler/worker protocol. Qualification workers
//! and exact workload adapters live outside this dependency path.

use crate::compiler_descriptor::{
    CompilerDescriptorError, construct_production_v1_compiler_descriptor_source_v1,
};
use crate::compiler_module_contract::{
    CompilerModuleRoleError, ExactTargetBindingError, construct_symbol_manifest,
    validate_envelope_module_roles, validate_exact_target_binding,
};
use crate::kernel_ir_codegen::{
    CompilerModuleConstructionError, bind_compiler_descriptor_source_v1,
    retain_production_gfx942_compiler_module_text_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerDescriptorSourceV1, CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestErrorV1, DeviceTargetV1,
};
use fe2o3_kernel_ir::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME;
use sha2::{Digest as _, Sha256};
use std::error::Error;
use std::fmt;

/// Move-only output of the sole production compiler pipeline.
///
/// It retains the exact LLVM identity and descriptor embedded in the canonical
/// module handoff, but grants no worker, artifact, load, or launch authority.
pub(crate) struct PreparedProductionWorkerHandoff {
    llvm_ir_sha256: [u8; 32],
    handoff: CompilerModuleHandoffV2,
    compiler_descriptor_source: CompilerDescriptorSourceV1,
}

impl PreparedProductionWorkerHandoff {
    pub(crate) fn into_validated_parts(
        self,
    ) -> Result<(CompilerModuleHandoffV2, CompilerDescriptorSourceV1), ProductionWorkerHandoffError>
    {
        let Self {
            llvm_ir_sha256,
            handoff,
            compiler_descriptor_source,
        } = self;
        if Sha256::digest(handoff.module_bytes()).as_slice() != llvm_ir_sha256 {
            return Err(ProductionWorkerHandoffError::MissingProductionBindings);
        }
        Ok((handoff, compiler_descriptor_source))
    }
}

/// Prepares the exact target-lowered production module for the managed worker.
///
/// This transition derives the closed symbol manifest, binds the target and
/// code-object envelope, and constructs canonical coordination bytes. It does
/// not invoke LLVM, link, publish an artifact, load, or launch.
pub(crate) fn prepare_production_worker_handoff(
    authenticated: crate::production_pipeline_v1::AuthenticatedProductionGfx942ModuleV1,
) -> Result<PreparedProductionWorkerHandoff, ProductionWorkerHandoffError> {
    let (formal, module, llvm_ir, typed_roots, compiler_ffi_envelope) = authenticated.into_parts();
    let target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("fixed production target is valid");
    validate_exact_target_binding(target, &module)?;
    let compiler_module = retain_production_gfx942_compiler_module_text_v1(&module, llvm_ir)
        .map_err(ProductionWorkerHandoffError::CompilerModule)?;
    let envelope = match compiler_ffi_envelope {
        Some(envelope) => envelope,
        None => CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .map_err(ProductionWorkerHandoffError::CompilerEnvelope)?,
    };
    validate_exact_target_binding(envelope.target(), &module)?;
    validate_envelope_module_roles(&envelope, &compiler_module)?;
    let descriptor_source = construct_production_v1_compiler_descriptor_source_v1(
        &envelope,
        &module,
        &compiler_module,
        &typed_roots,
        &formal,
    )
    .map_err(ProductionWorkerHandoffError::CompilerDescriptor)?;
    let compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &descriptor_source)
        .map_err(ProductionWorkerHandoffError::CompilerModule)?;
    let symbol_manifest = construct_symbol_manifest(&compiler_module)
        .map_err(ProductionWorkerHandoffError::SymbolManifest)?;
    let llvm_ir_sha256 = Sha256::digest(compiler_module.llvm_ir().as_bytes()).into();
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(ProductionWorkerHandoffError::Handoff)?;
    Ok(PreparedProductionWorkerHandoff {
        llvm_ir_sha256,
        handoff,
        compiler_descriptor_source: descriptor_source,
    })
}

#[derive(Debug)]
pub(crate) enum ProductionWorkerHandoffError {
    MissingProductionBindings,
    MissingExternalDeclaration(String),
    MissingCompilerDefinition(String),
    TargetBindingMismatch {
        module: Vec<String>,
        envelope: String,
    },
    CompilerModule(CompilerModuleConstructionError),
    CompilerEnvelope(CompilerFfiEnvelopeError),
    CompilerDescriptor(CompilerDescriptorError),
    SymbolManifest(CompilerModuleSymbolManifestErrorV1),
    Handoff(CompilerModuleHandoffErrorV2),
}

impl fmt::Display for ProductionWorkerHandoffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProductionBindings => formatter
                .write_str("production compiler handoff lost its exact LLVM identity binding"),
            Self::MissingExternalDeclaration(symbol) => write!(
                formatter,
                "compiler FFI import {symbol:?} is absent from the whole Kernel IR module"
            ),
            Self::MissingCompilerDefinition(symbol) => write!(
                formatter,
                "compiler FFI export {symbol:?} is absent from the whole Kernel IR module"
            ),
            Self::TargetBindingMismatch { module, envelope } => write!(
                formatter,
                "compiler-module exact target bindings {module:?} do not match production envelope target {envelope:?}"
            ),
            Self::CompilerModule(error) => {
                write!(
                    formatter,
                    "whole compiler-module construction failed: {error}"
                )
            }
            Self::CompilerEnvelope(error) => {
                write!(formatter, "exact compiler FFI envelope failed: {error}")
            }
            Self::CompilerDescriptor(error) => {
                write!(
                    formatter,
                    "compiler descriptor construction failed: {error}"
                )
            }
            Self::SymbolManifest(error) => {
                write!(
                    formatter,
                    "compiler symbol manifest construction failed: {error}"
                )
            }
            Self::Handoff(error) => {
                write!(
                    formatter,
                    "compiler-module handoff construction failed: {error}"
                )
            }
        }
    }
}

impl Error for ProductionWorkerHandoffError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CompilerModule(error) => Some(error),
            Self::CompilerEnvelope(error) => Some(error),
            Self::CompilerDescriptor(error) => Some(error),
            Self::SymbolManifest(error) => Some(error),
            Self::Handoff(error) => Some(error),
            Self::MissingProductionBindings
            | Self::MissingExternalDeclaration(_)
            | Self::MissingCompilerDefinition(_)
            | Self::TargetBindingMismatch { .. } => None,
        }
    }
}

impl From<ExactTargetBindingError> for ProductionWorkerHandoffError {
    fn from(error: ExactTargetBindingError) -> Self {
        Self::TargetBindingMismatch {
            module: error.module,
            envelope: error.envelope,
        }
    }
}

impl From<CompilerModuleRoleError> for ProductionWorkerHandoffError {
    fn from(error: CompilerModuleRoleError) -> Self {
        match error {
            CompilerModuleRoleError::MissingExternalDeclaration(symbol) => {
                Self::MissingExternalDeclaration(symbol)
            }
            CompilerModuleRoleError::MissingCompilerDefinition(symbol) => {
                Self::MissingCompilerDefinition(symbol)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn production_source_has_no_qualification_or_worker_v2_dependency() {
        for source in [
            include_str!("production_worker_handoff.rs"),
            include_str!("compiler_module_contract.rs"),
        ] {
            for forbidden in [
                concat!("worker_v2", "_producer"),
                concat!("qualification-oracles", "-test-only"),
                concat!("collected_scalar", "_gemm"),
                concat!("collected_tiled", "_gemm"),
                concat!("collected_flash", "_attention"),
                concat!("collected_", "moe"),
                concat!("collected_row", "_softmax"),
            ] {
                assert!(
                    !source.contains(forbidden),
                    "production handoff depends on obsolete variant {forbidden:?}"
                );
            }
        }
    }
}
