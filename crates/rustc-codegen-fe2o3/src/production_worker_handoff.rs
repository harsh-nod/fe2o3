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
    retain_production_compiler_module_text_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerDescriptorSourceV1, CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestErrorV1, PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1,
    ProductionGfx942CompilerFfiEnvelopeKindV1, ProductionGfx950CompilerFfiEnvelopeKindV1,
    construct_production_gfx942_ocml_exp_envelope_v1,
    construct_production_gfx950_ocml_exp_envelope_v1,
    inspect_production_gfx942_compiler_ffi_envelope_v1,
    inspect_production_gfx950_compiler_ffi_envelope_v1,
};
use fe2o3_kernel_ir::{
    F32MathFunction, F32MathImplementation, FloatOperation, FunctionRole, Module, OperationKind,
};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
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
    authenticated: crate::production_pipeline::AuthenticatedProductionTargetModule,
) -> Result<PreparedProductionWorkerHandoff, ProductionWorkerHandoffError> {
    let (formal, target, module, llvm_ir, typed_roots, compiler_ffi_envelope) =
        authenticated.into_parts();
    validate_exact_target_binding(target, &module)?;
    let canonical_kernel_ir_identity = *formal
        .semantic_kir()
        .canonical_kernel_ir_identity()
        .digest();
    let compiler_module = retain_production_compiler_module_text_v1(&module, llvm_ir)
        .map_err(ProductionWorkerHandoffError::CompilerModule)?;
    let envelope = derive_production_compiler_ffi_envelope(
        target,
        &module,
        &compiler_module,
        compiler_ffi_envelope,
        canonical_kernel_ir_identity,
    )?;
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

fn derive_production_compiler_ffi_envelope(
    target: fe2o3_compiler_ffi::DeviceTargetV1,
    module: &Module,
    compiler_module: &crate::kernel_ir_codegen::InertCompilerModuleTextV1,
    observed_source_envelope: Option<CompilerFfiEnvelopeV1>,
    canonical_kernel_ir_identity: [u8; 32],
) -> Result<CompilerFfiEnvelopeV1, ProductionWorkerHandoffError> {
    let profile = match target.to_string().as_str() {
        "gfx942:xnack-" => ProductionOcmlTargetV1::Gfx942,
        "gfx950:xnack-" => ProductionOcmlTargetV1::Gfx950,
        _ => {
            return match observed_source_envelope {
                Some(envelope) => Ok(envelope),
                None => CompilerFfiEnvelopeV1::for_module_without_device_ffi(
                    target,
                    CodeObjectVersion::V6,
                )
                .map_err(ProductionWorkerHandoffError::CompilerEnvelope),
            };
        }
    };

    if observed_source_envelope
        .as_ref()
        .is_some_and(|envelope| envelope.directional_symbols().total_count() != 0)
    {
        return Err(profile.source_ffi_not_admitted());
    }

    let ocml_imports = typed_ocml_imports(module);
    match ocml_imports.as_slice() {
        [] => {
            if !compiler_module.external_declarations().is_empty()
                || compiler_module.llvm_ir().contains("@__ocml_")
            {
                return Err(profile.import_policy_error());
            }
            let envelope =
                CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
                    .map_err(ProductionWorkerHandoffError::CompilerEnvelope)?;
            debug_assert!(profile.inspects_no_device_ffi(&envelope));
            Ok(envelope)
        }
        [symbol] if *symbol == PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1 => {
            if !reachable_ocml_exp_f32_call(module) {
                return Err(profile.exp_not_executable_error());
            }
            if compiler_module.external_declarations() != [PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1]
                || !has_exact_ocml_exp_llvm_shape(compiler_module.llvm_ir())
            {
                return Err(profile.llvm_mismatch_error());
            }
            profile.construct_exp_envelope(canonical_kernel_ir_identity)
        }
        _ => Err(profile.import_policy_error()),
    }
}

#[derive(Clone, Copy)]
enum ProductionOcmlTargetV1 {
    Gfx942,
    Gfx950,
}

impl ProductionOcmlTargetV1 {
    fn construct_exp_envelope(
        self,
        canonical_kernel_ir_identity: [u8; 32],
    ) -> Result<CompilerFfiEnvelopeV1, ProductionWorkerHandoffError> {
        match self {
            Self::Gfx942 => {
                construct_production_gfx942_ocml_exp_envelope_v1(canonical_kernel_ir_identity)
            }
            Self::Gfx950 => {
                construct_production_gfx950_ocml_exp_envelope_v1(canonical_kernel_ir_identity)
            }
        }
        .map_err(ProductionWorkerHandoffError::CompilerEnvelope)
    }

    fn inspects_no_device_ffi(self, envelope: &CompilerFfiEnvelopeV1) -> bool {
        match self {
            Self::Gfx942 => matches!(
                inspect_production_gfx942_compiler_ffi_envelope_v1(envelope),
                Some(ProductionGfx942CompilerFfiEnvelopeKindV1::NoDeviceFfi)
            ),
            Self::Gfx950 => matches!(
                inspect_production_gfx950_compiler_ffi_envelope_v1(envelope),
                Some(ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi)
            ),
        }
    }

    const fn source_ffi_not_admitted(self) -> ProductionWorkerHandoffError {
        match self {
            Self::Gfx942 => ProductionWorkerHandoffError::Gfx942SourceFfiNotAdmitted,
            Self::Gfx950 => ProductionWorkerHandoffError::Gfx950SourceFfiNotAdmitted,
        }
    }

    const fn import_policy_error(self) -> ProductionWorkerHandoffError {
        match self {
            Self::Gfx942 => ProductionWorkerHandoffError::Gfx942OcmlImportPolicy,
            Self::Gfx950 => ProductionWorkerHandoffError::Gfx950OcmlImportPolicy,
        }
    }

    const fn exp_not_executable_error(self) -> ProductionWorkerHandoffError {
        match self {
            Self::Gfx942 => ProductionWorkerHandoffError::Gfx942OcmlExpNotExecutable,
            Self::Gfx950 => ProductionWorkerHandoffError::Gfx950OcmlExpNotExecutable,
        }
    }

    const fn llvm_mismatch_error(self) -> ProductionWorkerHandoffError {
        match self {
            Self::Gfx942 => ProductionWorkerHandoffError::Gfx942OcmlLlvmMismatch,
            Self::Gfx950 => ProductionWorkerHandoffError::Gfx950OcmlLlvmMismatch,
        }
    }
}

fn typed_ocml_imports(module: &Module) -> Vec<&'static str> {
    let mut imports = module
        .functions
        .iter()
        .filter_map(|function| {
            let FloatOperation::F32Math {
                function,
                implementation: F32MathImplementation::OcmlAbiV1,
                ..
            } = FloatOperation::from_intrinsic_id(&function.id)?
            else {
                return None;
            };
            Some(match function {
                F32MathFunction::Exp => PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1,
                F32MathFunction::Sin => "__ocml_sin_f32",
                F32MathFunction::Cos => "__ocml_cos_f32",
                F32MathFunction::Exp2 => "__ocml_exp2_f32",
                F32MathFunction::Ln => "__ocml_log_f32",
                F32MathFunction::Log2 => "__ocml_log2_f32",
                F32MathFunction::Log10 => "__ocml_log10_f32",
                F32MathFunction::Sqrt
                | F32MathFunction::FusedMultiplyAdd
                | F32MathFunction::Floor
                | F32MathFunction::Ceil
                | F32MathFunction::Truncate
                | F32MathFunction::RoundTiesEven => return None,
            })
        })
        .collect::<Vec<_>>();
    imports.sort_unstable();
    imports.dedup();
    imports
}

fn reachable_ocml_exp_f32_call(module: &Module) -> bool {
    let mut pending = module
        .kernels
        .iter()
        .map(|kernel| kernel.entry.as_str().to_owned())
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    while let Some(function_id) = pending.pop() {
        if !visited.insert(function_id.clone()) {
            continue;
        }
        let Some(function) = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == function_id)
        else {
            continue;
        };
        let Some(body) = &function.body else {
            continue;
        };
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            let OperationKind::Call { callee, .. } = &operation.kind else {
                continue;
            };
            if FloatOperation::from_intrinsic_id(callee).is_some_and(|operation| {
                matches!(
                    operation,
                    FloatOperation::F32Math {
                        function: F32MathFunction::Exp,
                        implementation: F32MathImplementation::OcmlAbiV1,
                        ..
                    }
                )
            }) {
                return module.function(callee).is_some_and(|declaration| {
                    declaration.role == FunctionRole::ExternalImport && declaration.body.is_none()
                });
            }
            pending.push(callee.as_str().to_owned());
        }
    }
    false
}

fn has_exact_ocml_exp_llvm_shape(llvm_ir: &str) -> bool {
    llvm_ir
        .matches("declare float @__ocml_exp_f32(float)")
        .count()
        == 1
        && llvm_ir.matches("call float @__ocml_exp_f32(float ").count() >= 1
        && llvm_ir
            .split("@__ocml_")
            .skip(1)
            .all(|suffix| suffix.starts_with("exp_f32"))
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
    Gfx942SourceFfiNotAdmitted,
    Gfx942OcmlImportPolicy,
    Gfx942OcmlExpNotExecutable,
    Gfx942OcmlLlvmMismatch,
    Gfx950SourceFfiNotAdmitted,
    Gfx950OcmlImportPolicy,
    Gfx950OcmlExpNotExecutable,
    Gfx950OcmlLlvmMismatch,
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
            Self::Gfx942SourceFfiNotAdmitted => formatter.write_str(
                "production gfx942 admits no caller/source-authored device FFI envelope",
            ),
            Self::Gfx942OcmlImportPolicy => formatter.write_str(
                "production gfx942 admits either no device FFI or only compiler-derived __ocml_exp_f32",
            ),
            Self::Gfx942OcmlExpNotExecutable => formatter.write_str(
                "production gfx942 OCML exp declaration is not called from the executable Kernel IR closure",
            ),
            Self::Gfx942OcmlLlvmMismatch => formatter.write_str(
                "production gfx942 LLVM does not retain the exact Kernel-IR-derived OCML exp declaration/call closure",
            ),
            Self::Gfx950SourceFfiNotAdmitted => formatter.write_str(
                "production gfx950 admits no caller/source-authored device FFI envelope",
            ),
            Self::Gfx950OcmlImportPolicy => formatter.write_str(
                "production gfx950 admits either no device FFI or only compiler-derived __ocml_exp_f32",
            ),
            Self::Gfx950OcmlExpNotExecutable => formatter.write_str(
                "production gfx950 OCML exp declaration is not called from the executable Kernel IR closure",
            ),
            Self::Gfx950OcmlLlvmMismatch => formatter.write_str(
                "production gfx950 LLVM does not retain the exact Kernel-IR-derived OCML exp declaration/call closure",
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
            | Self::Gfx942SourceFfiNotAdmitted
            | Self::Gfx942OcmlImportPolicy
            | Self::Gfx942OcmlExpNotExecutable
            | Self::Gfx942OcmlLlvmMismatch
            | Self::Gfx950SourceFfiNotAdmitted
            | Self::Gfx950OcmlImportPolicy
            | Self::Gfx950OcmlExpNotExecutable
            | Self::Gfx950OcmlLlvmMismatch
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
    use super::*;
    use crate::kernel_ir_codegen::construct_inert_compiler_module_text_for_target_v1;
    use fe2o3_compiler_ffi::DeviceTargetV1;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Signature, Terminator,
        Type, ValueId, WorkgroupSize,
    };

    fn math_module(functions: &[F32MathFunction], called: &[F32MathFunction]) -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        for (index, function) in called.iter().copied().enumerate() {
            block.operations.push(
                FloatOperation::F32Math {
                    function,
                    implementation: F32MathImplementation::OcmlAbiV1,
                    arguments: vec![ValueId(0)],
                }
                .operation(ValueId(u32::try_from(index + 1).unwrap())),
            );
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "kernel",
            Signature::new(vec![Type::F32], vec![]),
            vec![ValueId(0)],
            vec![block],
        );
        let mut kernel = Kernel::new(
            "kernel",
            "kernel",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
        let mut module = Module::new("tests::production_gfx942_ocml");
        module.functions.push(entry);
        for function in functions {
            module.functions.push(
                FloatOperation::F32Math {
                    function: *function,
                    implementation: F32MathImplementation::OcmlAbiV1,
                    arguments: vec![ValueId(0)],
                }
                .declaration(),
            );
        }
        module.kernels.push(kernel);
        module
    }

    fn gfx942_compiler_module(
        module: &Module,
    ) -> crate::kernel_ir_codegen::InertCompilerModuleTextV1 {
        construct_inert_compiler_module_text_for_target_v1(
            module,
            Some(DeviceTargetV1::parse("gfx942:xnack-").unwrap()),
        )
        .unwrap()
    }

    #[test]
    fn reachable_gfx942_exp_derives_compiler_owned_envelope() {
        let module = math_module(&[F32MathFunction::Exp], &[F32MathFunction::Exp]);
        let compiler_module = gfx942_compiler_module(&module);
        let identity = [0x37; 32];
        let envelope = derive_production_compiler_ffi_envelope(
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            &module,
            &compiler_module,
            None,
            identity,
        )
        .unwrap();
        assert_eq!(
            inspect_production_gfx942_compiler_ffi_envelope_v1(&envelope),
            Some(ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 {
                canonical_kernel_ir_identity: identity,
            })
        );
    }

    #[test]
    fn gfx942_exp_rejects_source_ffi_unreachable_and_second_import() {
        let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
        let reachable = math_module(&[F32MathFunction::Exp], &[F32MathFunction::Exp]);
        let reachable_compiler = gfx942_compiler_module(&reachable);
        let source = construct_production_gfx942_ocml_exp_envelope_v1([0x38; 32]).unwrap();
        assert!(matches!(
            derive_production_compiler_ffi_envelope(
                target,
                &reachable,
                &reachable_compiler,
                Some(source),
                [0x37; 32],
            ),
            Err(ProductionWorkerHandoffError::Gfx942SourceFfiNotAdmitted)
        ));

        let unreachable = math_module(&[F32MathFunction::Exp], &[]);
        let unreachable_compiler = gfx942_compiler_module(&unreachable);
        assert!(matches!(
            derive_production_compiler_ffi_envelope(
                target,
                &unreachable,
                &unreachable_compiler,
                None,
                [0x37; 32],
            ),
            Err(ProductionWorkerHandoffError::Gfx942OcmlExpNotExecutable)
        ));

        let second = math_module(
            &[F32MathFunction::Exp, F32MathFunction::Sin],
            &[F32MathFunction::Exp, F32MathFunction::Sin],
        );
        let second_compiler = gfx942_compiler_module(&second);
        assert!(matches!(
            derive_production_compiler_ffi_envelope(
                target,
                &second,
                &second_compiler,
                None,
                [0x37; 32],
            ),
            Err(ProductionWorkerHandoffError::Gfx942OcmlImportPolicy)
        ));
    }

    #[test]
    fn production_source_has_no_qualification_or_worker_v2_dependency() {
        for source in [
            include_str!("production_worker_handoff.rs"),
            include_str!("compiler_module_contract.rs"),
        ] {
            for forbidden in [
                concat!("worker_v2", "_producer"),
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
