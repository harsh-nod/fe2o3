//! Exact-kernel legalization for the opt-in production kernel-IR pipeline.
//!
//! Helper names are matched here only after `mir_import` classified their rustc `DefId` and
//! `translate_and_verify` produced this in-memory module. This module must not be used to grant
//! the same authority to decoded or caller-constructed kernel IR.

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
use crate::QUALIFICATION_ORACLE_ENV;
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
use crate::amdgpu_llvm::{EmitError, PreparedDeviceKernel};
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
use crate::trusted_device_items::TrustedDeviceItem;
#[cfg(test)]
use fe2o3_compiler_ffi::DeviceTargetV1;
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SECTION_NAME_V1, CompilerDescriptorSourceIdentityV1,
    CompilerDescriptorSourceV1,
};
#[cfg(test)]
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
};
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant,
    FunctionBody, IntrinsicOperation, KernelId, MemoryAccess, ValueDef, ValueId, WorkgroupSize,
};
use fe2o3_kernel_ir::{
    F32MathFunction, F32MathImplementation, FloatOperation, FunctionRole, Module, Operation,
    OperationKind, TargetCapability, Terminator, Type, verify_module,
};
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
use sha2::{Digest as _, Sha256};
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
use std::{fs, path::Path};

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
const NO_QUALIFICATION_FALLBACK_HINT: &str =
    "production compilation does not fall back to a qualification-only lowering route";
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
const WORKGROUP_X: u32 = 256;
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) const TILED_GEMM_FRONTEND_TEST_LLVM_FILE: &str =
    "tiled_gemm_frontend_v1.imported.gfx942-xnack-.ll";
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) const TILED_GEMM_LDS_SLICE1_AUTHORITY_SECTION_V1: &str =
    ".fe2o3.tiled-lds-slice1-auth.v1";
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) const TILED_GEMM_LDS_SLICE1_RESOURCE_SECTION_V1: &str =
    ".fe2o3.tiled-lds-slice1-resources.v1";
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) const MOE_TOP2_SECTION_PREFIX_V1: &str = ".fe2o3.moe";
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
const REVIEWED_ROW_SOFTMAX_LEGACY_LLVM_SHA256: [u8; 32] = [
    0x0a, 0x33, 0x13, 0x67, 0x53, 0x44, 0x43, 0x7b, 0xc7, 0xb8, 0x94, 0xad, 0x2f, 0x4d, 0xad, 0xb3,
    0x81, 0x07, 0xd9, 0x02, 0x96, 0xa9, 0x66, 0x5b, 0x76, 0x32, 0x34, 0xac, 0xd2, 0x40, 0x5a, 0xcc,
];
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
const REVIEWED_ROW_SOFTMAX_V1_LLVM_SHA256: [u8; 32] = [
    0xd4, 0x8d, 0x33, 0x20, 0xc2, 0x86, 0xc6, 0xda, 0x22, 0x53, 0xa1, 0x04, 0x38, 0x60, 0x89, 0xe3,
    0x89, 0x64, 0x8f, 0x42, 0x60, 0xf2, 0xe7, 0xef, 0xda, 0x21, 0x26, 0x9f, 0xef, 0x95, 0x1c, 0x2c,
];
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
const ROW_SOFTMAX_UPSTREAM_LLVM_BUILD_IDENTITY_V1: &str =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
const ROW_SOFTMAX_UPSTREAM_LLVM_TARGET_TRIPLE_V1: &str = "amdgcn-amd-amdhsa";
/// Reviewed observation from the exact upstream LLVM build named above.
///
/// The value was measured by initializing LLVM's AMDGPU target, looking up
/// `amdgcn` for `amdgcn-amd-amdhsa`, and calling `createTargetMachine` with
/// CPU `gfx942`, features `-xnack`, `Reloc::PIC_`, `CodeModel::Small`, and
/// `CodeGenOptLevel::None`. It is the exact string returned by
/// `TargetMachine::createDataLayout().getStringRepresentation()` in that
/// configuration. The test fixture
/// `tests/fixtures/row-softmax-llvm22-target-layout.cpp` repeats that API
/// observation against a configured upstream build.
///
/// The Rust producer does not load or query LLVM. It binds this reviewed
/// constant only after authenticating the complete legacy lowering digest;
/// Worker V2 independently compares the submitted layout with the live target
/// machine before OCML import or native linking.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) const ROW_SOFTMAX_UPSTREAM_LLVM_DATA_LAYOUT_V1: &str = "e-m:e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9";
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) const FLASH_ATTENTION_AUTHORITY_TRANSCRIPT_SECTION_V1: &str =
    ".fe2o3.flash-attention-authority-transcript.v1";
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) const FLASH_ATTENTION_AUTHORITY_SECTION_V1: &str = ".fe2o3.flash-attention-auth.v1";
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) const FLASH_ATTENTION_OCML_BOUNDARY_SECTION_V1: &str =
    ".fe2o3.flash-attention-ocml-exp.v1";

const MAX_COMPILER_MODULE_ID_BYTES: usize = 256;
const MAX_COMPILER_MODULE_SYMBOL_BYTES: usize = 256;
const MAX_COMPILER_MODULE_FUNCTIONS: usize = 1_024;
const MAX_COMPILER_MODULE_KERNELS: usize = 256;
const MAX_COMPILER_MODULE_CAPABILITIES: usize = 4_096;
const MAX_COMPILER_MODULE_PARAMETERS: usize = 64;
const MAX_COMPILER_MODULE_RESULTS: usize = 8;
const MAX_COMPILER_MODULE_BLOCKS: usize = 16_384;
const MAX_COMPILER_MODULE_BLOCK_PARAMETERS: usize = 65_536;
const MAX_COMPILER_MODULE_OPERATIONS: usize = 131_072;
const MAX_COMPILER_MODULE_OPERATION_RESULTS: usize = 131_072;
const MAX_COMPILER_MODULE_CALL_ARGUMENTS: usize = 64;
const MAX_COMPILER_MODULE_CFG_ARGUMENTS: usize = 65_536;
const MAX_COMPILER_MODULE_SWITCH_CASES: usize = 65_536;
const MAX_COMPILER_MODULE_TYPE_DEPTH: usize = 8;

/// One inert, deterministic textual LLVM AMDGPU module.
///
/// This value is not LLVM bitcode, a link result, a code object, compiler provenance, or load
/// authority. The Worker V2 producer may place its exact text in an attempt-scoped handoff after
/// checking the compiler FFI roles. A reviewed target header may be present for parser/target-machine
/// compatibility, but it grants no target-machine or final-artifact authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InertCompilerModuleTextV1 {
    llvm_ir: String,
    kernel_entries: Vec<String>,
    device_definitions: Vec<String>,
    internal_helpers: Vec<String>,
    device_ffi_exports: Vec<String>,
    external_declarations: Vec<String>,
    descriptor_source_identity: Option<CompilerDescriptorSourceIdentityV1>,
}

/// Private binding of the reviewed LLVM 22.1.8 measurement. This is not a
/// dynamic LLVM query, and no caller-supplied layout text enters the value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
struct RowSoftmaxReviewedLlvmLayoutMeasurementV1 {
    llvm_build_identity: &'static str,
    target_triple: &'static str,
    data_layout: &'static str,
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn reviewed_row_softmax_upstream_llvm_layout_v1() -> RowSoftmaxReviewedLlvmLayoutMeasurementV1 {
    RowSoftmaxReviewedLlvmLayoutMeasurementV1 {
        llvm_build_identity: ROW_SOFTMAX_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
        target_triple: ROW_SOFTMAX_UPSTREAM_LLVM_TARGET_TRIPLE_V1,
        data_layout: ROW_SOFTMAX_UPSTREAM_LLVM_DATA_LAYOUT_V1,
    }
}

impl InertCompilerModuleTextV1 {
    pub(crate) fn llvm_ir(&self) -> &str {
        &self.llvm_ir
    }

    pub(crate) fn kernel_entries(&self) -> &[String] {
        &self.kernel_entries
    }

    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    pub(crate) fn device_definitions(&self) -> &[String] {
        &self.device_definitions
    }

    pub(crate) fn internal_helpers(&self) -> &[String] {
        &self.internal_helpers
    }

    pub(crate) fn device_ffi_exports(&self) -> &[String] {
        &self.device_ffi_exports
    }

    pub(crate) fn external_declarations(&self) -> &[String] {
        &self.external_declarations
    }

    #[cfg(test)]
    pub(crate) const fn descriptor_source_identity(
        &self,
    ) -> Option<CompilerDescriptorSourceIdentityV1> {
        self.descriptor_source_identity
    }
}

struct CompilerModuleSymbolClosureV1 {
    kernel_entries: Vec<String>,
    device_definitions: Vec<String>,
    internal_helpers: Vec<String>,
    device_ffi_exports: Vec<String>,
    external_declarations: Vec<String>,
}

/// Fail-closed compiler-module construction error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompilerModuleConstructionError {
    LimitExceeded {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    DescriptorSourceAlreadyBound,
    DescriptorKernelEntryClosureMismatch,
    DescriptorSymbolClosureMismatch,
    #[cfg(test)]
    UnsupportedFloatTarget(String),
    #[cfg(test)]
    UnsupportedTargetBinding(String),
    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    SourceDebug(crate::source_debug::SourceDebugError),
    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    ScalarGemmLowering(String),
    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    TiledGemmLowering(String),
    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    TiledGemmLdsSlice1Lowering(String),
    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    RowSoftmaxLowering(String),
    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    MoeTop2Lowering(String),
    #[cfg(all(test, feature = "qualification-oracles-test-only"))]
    FlashAttentionLowering(String),
    Verification(fe2o3_kernel_ir::VerificationErrors),
    #[cfg(test)]
    Lowering(dialect_amdgcn::LoweringErrors),
}

impl fmt::Display for CompilerModuleConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded { field, actual, max } => {
                write!(formatter, "{field} count/size {actual} exceeds limit {max}")
            }
            Self::DescriptorSourceAlreadyBound => {
                formatter.write_str("compiler module already has a descriptor source")
            }
            Self::DescriptorKernelEntryClosureMismatch => formatter
                .write_str("compiler descriptor kernel entries do not match the module closure"),
            Self::DescriptorSymbolClosureMismatch => {
                formatter.write_str("compiler descriptor symbols do not match the module closure")
            }
            #[cfg(test)]
            Self::UnsupportedFloatTarget(target) => write!(
                formatter,
                "compiler-module float contracts require exact gfx942 lowering; found target `{target}`"
            ),
            #[cfg(test)]
            Self::UnsupportedTargetBinding(target) => write!(
                formatter,
                "compiler-module exact target binding requires gfx942:xnack-; found target `{target}`"
            ),
            #[cfg(all(test, feature = "qualification-oracles-test-only"))]
            Self::SourceDebug(error) => {
                write!(formatter, "source-debug metadata rejected: {error}")
            }
            #[cfg(all(test, feature = "qualification-oracles-test-only"))]
            Self::ScalarGemmLowering(error) => {
                write!(formatter, "exact scalar GEMM lowering rejected: {error}")
            }
            #[cfg(all(test, feature = "qualification-oracles-test-only"))]
            Self::TiledGemmLowering(error) => {
                write!(formatter, "exact tiled GEMM lowering rejected: {error}")
            }
            #[cfg(all(test, feature = "qualification-oracles-test-only"))]
            Self::TiledGemmLdsSlice1Lowering(error) => {
                write!(formatter, "exact LDS Slice 1 lowering rejected: {error}")
            }
            #[cfg(all(test, feature = "qualification-oracles-test-only"))]
            Self::RowSoftmaxLowering(error) => {
                write!(formatter, "exact row-softmax lowering rejected: {error}")
            }
            #[cfg(all(test, feature = "qualification-oracles-test-only"))]
            Self::MoeTop2Lowering(error) => {
                write!(formatter, "exact MoE top-2 lowering rejected: {error}")
            }
            #[cfg(all(test, feature = "qualification-oracles-test-only"))]
            Self::FlashAttentionLowering(error) => {
                write!(formatter, "exact FlashAttention lowering rejected: {error}")
            }
            Self::Verification(error) => write!(formatter, "{error}"),
            #[cfg(test)]
            Self::Lowering(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn bind_source_debug_metadata_v1(
    mut module: InertCompilerModuleTextV1,
    profile: &crate::source_debug::AlphaSourceDebugV2,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    let llvm_ir = crate::source_debug::inject_alpha_dwarf_v1(&module.llvm_ir, profile)
        .map_err(CompilerModuleConstructionError::SourceDebug)?;
    enforce_source_debug_text_bound(&llvm_ir)?;
    module.llvm_ir = llvm_ir;
    Ok(module)
}

fn enforce_source_debug_text_bound(llvm_ir: &str) -> Result<(), CompilerModuleConstructionError> {
    check_compiler_module_limit(
        "source-debug-injected compiler-module LLVM text bytes",
        llvm_ir.len(),
        dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES,
    )
}

impl std::error::Error for CompilerModuleConstructionError {}

/// Constructs one bounded canonical textual module without invoking or wiring LLVM.
///
/// Structural bounds are checked before kernel-IR verification. The dialect lowerer then
/// preflights every kernel, helper, declaration, call, attribute, and metadata record before its
/// private capacity-limited emission pass. An error returns no partially constructed module.
#[cfg(test)]
pub(crate) fn construct_inert_compiler_module_text_v1(
    module: &Module,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    construct_inert_compiler_module_text_for_target_v1(module, None)
}

#[cfg(test)]
pub(crate) fn construct_inert_compiler_module_text_for_target_v1(
    module: &Module,
    target: Option<DeviceTargetV1>,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    enforce_compiler_module_bounds(module)?;
    let has_float_contracts = module
        .functions
        .iter()
        .any(|function| FloatOperation::from_intrinsic_id(&function.id).is_some());
    let has_exact_target_binding = module.effective_capabilities().iter().any(|capability| {
        matches!(
            capability,
            TargetCapability::Extension { namespace, name }
                if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                    && name == AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME
        )
    });
    let exact_target = DeviceTargetV1::parse(AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
        .expect("exact gfx942 target binding is canonical");
    let llvm_ir = match (has_float_contracts, has_exact_target_binding, target) {
        (_, true, Some(target)) if target == exact_target => {
            dialect_amdgcn::lower_device_module_to_gfx942_xnack_minus_llvm_ir(module)
        }
        (_, true, Some(target)) => {
            return Err(CompilerModuleConstructionError::UnsupportedTargetBinding(
                target.to_string(),
            ));
        }
        (_, true, None) => {
            return Err(CompilerModuleConstructionError::UnsupportedTargetBinding(
                "<unbound>".to_owned(),
            ));
        }
        (true, false, Some(target)) if target.as_amd_target_id().processor() == "gfx942" => {
            dialect_amdgcn::lower_compiler_module_to_gfx942_llvm_ir(module)
        }
        (true, false, Some(target)) => {
            return Err(CompilerModuleConstructionError::UnsupportedFloatTarget(
                target.to_string(),
            ));
        }
        (true, false, None) => {
            return Err(CompilerModuleConstructionError::UnsupportedFloatTarget(
                "<unbound>".to_owned(),
            ));
        }
        (false, false, _) => dialect_amdgcn::lower_compiler_module_to_llvm_ir(module),
    }
    .map_err(CompilerModuleConstructionError::Lowering)?;

    let symbols = compiler_module_symbol_closure_v1(module);

    Ok(InertCompilerModuleTextV1 {
        llvm_ir,
        kernel_entries: symbols.kernel_entries,
        device_definitions: symbols.device_definitions,
        internal_helpers: symbols.internal_helpers,
        device_ffi_exports: symbols.device_ffi_exports,
        external_declarations: symbols.external_declarations,
        descriptor_source_identity: None,
    })
}

/// Retains exact target-bound LLVM text already produced by the move-only
/// production transaction. This path performs no profile recognition,
/// target rebinding, or second KIR-to-LLVM lowering.
pub(crate) fn retain_production_compiler_module_text_v1(
    module: &Module,
    llvm_ir: String,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    enforce_compiler_module_bounds(module)?;
    verify_module(module).map_err(CompilerModuleConstructionError::Verification)?;
    enforce_source_debug_text_bound(&llvm_ir)?;
    let symbols = compiler_module_symbol_closure_v1(module);
    Ok(InertCompilerModuleTextV1 {
        llvm_ir,
        kernel_entries: symbols.kernel_entries,
        device_definitions: symbols.device_definitions,
        internal_helpers: symbols.internal_helpers,
        device_ffi_exports: symbols.device_ffi_exports,
        external_declarations: symbols.external_declarations,
        descriptor_source_identity: None,
    })
}

fn compiler_module_symbol_closure_v1(module: &Module) -> CompilerModuleSymbolClosureV1 {
    let mut kernel_entries = module
        .kernels
        .iter()
        .map(|kernel| kernel.id.as_str().to_string())
        .collect::<Vec<_>>();
    let mut device_definitions = module
        .functions
        .iter()
        .filter(|function| {
            matches!(
                function.role,
                FunctionRole::InternalHelper | FunctionRole::DeviceFfiExport
            )
        })
        .map(|function| function.id.as_str().to_string())
        .collect::<Vec<_>>();
    let mut internal_helpers = module
        .functions
        .iter()
        .filter(|function| function.role == FunctionRole::InternalHelper)
        .map(|function| function.id.as_str().to_string())
        .collect::<Vec<_>>();
    let mut device_ffi_exports = module
        .functions
        .iter()
        .filter(|function| function.role == FunctionRole::DeviceFfiExport)
        .map(|function| function.id.as_str().to_string())
        .collect::<Vec<_>>();
    let mut external_declarations = module
        .functions
        .iter()
        .filter(|function| {
            function.role == FunctionRole::ExternalImport
                && FloatOperation::from_intrinsic_id(&function.id).is_none()
        })
        .map(|function| function.id.as_str().to_string())
        .collect::<Vec<_>>();
    external_declarations.extend(ocml_link_imports(module).map(str::to_owned));
    kernel_entries.sort();
    device_definitions.sort();
    internal_helpers.sort();
    device_ffi_exports.sort();
    external_declarations.sort();

    CompilerModuleSymbolClosureV1 {
        kernel_entries,
        device_definitions,
        internal_helpers,
        device_ffi_exports,
        external_declarations,
    }
}

/// Constructs the exact reviewed gfx942:xnack-/COV6 scalar GEMM LLVM module.
///
/// This keeps the scalar-specific floating-point and target audit in the same
/// path that later embeds compiler-authenticated descriptor source bytes.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn construct_inert_scalar_gemm_v1_module_text(
    module: &Module,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    enforce_compiler_module_bounds(module)?;
    let llvm_ir = dialect_amdgcn::lower_scalar_gemm_v1_to_gfx942_llvm_ir(
        module,
        fe2o3_kernel_ir::ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6(),
    )
    .map_err(|error| CompilerModuleConstructionError::ScalarGemmLowering(error.to_string()))?
    .into_string();

    Ok(InertCompilerModuleTextV1 {
        llvm_ir,
        kernel_entries: vec![fe2o3_kernel_ir::SCALAR_GEMM_V1_KERNEL_ID.to_owned()],
        device_definitions: Vec::new(),
        internal_helpers: Vec::new(),
        device_ffi_exports: Vec::new(),
        external_declarations: Vec::new(),
        descriptor_source_identity: None,
    })
}

/// Constructs the exact reviewed gfx942:xnack-/COV6 tiled GEMM LLVM module.
///
/// This calls the dedicated canonical lowering API directly. It performs no
/// code-object construction, linking, publication, execution, or COMGR work.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn construct_inert_tiled_gemm_v1_module_text(
    module: &Module,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    enforce_compiler_module_bounds(module)?;
    let llvm_ir = dialect_amdgcn::lower_tiled_gemm_v1_to_gfx942_llvm_ir(
        module,
        fe2o3_kernel_ir::TiledGemmV1Profile::exact_gfx942_xnack_minus_cov6(),
    )
    .map_err(|error| CompilerModuleConstructionError::TiledGemmLowering(error.to_string()))?
    .into_string();

    Ok(InertCompilerModuleTextV1 {
        llvm_ir,
        kernel_entries: vec![fe2o3_kernel_ir::TILED_GEMM_V1_KERNEL_ID.to_owned()],
        device_definitions: Vec::new(),
        internal_helpers: Vec::new(),
        device_ffi_exports: Vec::new(),
        external_declarations: Vec::new(),
        descriptor_source_identity: None,
    })
}

/// Constructs only the reviewed gfx942:xnack-/COV6 LDS Slice 1 LLVM module.
///
/// The dedicated dialect entry point verifies the exact canonical graph and
/// its two 512-byte static LDS allocations before returning inert LLVM text.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn construct_inert_tiled_gemm_lds_slice1_module_text(
    module: &Module,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    enforce_compiler_module_bounds(module)?;
    let llvm_ir = dialect_amdgcn::lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(
        module,
        fe2o3_kernel_ir::TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6(),
    )
    .map_err(|error| {
        CompilerModuleConstructionError::TiledGemmLdsSlice1Lowering(error.to_string())
    })?
    .into_string();

    Ok(InertCompilerModuleTextV1 {
        llvm_ir,
        kernel_entries: vec![fe2o3_kernel_ir::TILED_GEMM_LDS_V1_KERNEL_ID.to_owned()],
        device_definitions: Vec::new(),
        internal_helpers: Vec::new(),
        device_ffi_exports: Vec::new(),
        external_declarations: Vec::new(),
        descriptor_source_identity: None,
    })
}

/// Lowers only the source-authenticated canonical row-softmax graph through
/// the commitment-gated gfx942 row-softmax dialect profile.
///
/// The result retains exactly one unresolved OCML import. This function does
/// not locate OCML bitcode, invoke a worker, link, or construct an artifact.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn construct_inert_row_softmax_v1_module_text(
    module: &Module,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    enforce_compiler_module_bounds(module)?;
    if module != &crate::collected_row_softmax_v1::canonical_row_softmax_v1_module() {
        return Err(CompilerModuleConstructionError::RowSoftmaxLowering(
            "module differs from the reviewed canonical row_softmax_v1 graph".to_owned(),
        ));
    }
    let row_profile = dialect_amdgcn::authenticate_gfx942_row_softmax_lowering_profile_v1(module)
        .map_err(CompilerModuleConstructionError::Lowering)?;
    let legacy_llvm_ir =
        dialect_amdgcn::lower_authenticated_row_softmax_module_to_gfx942_xnack_minus_llvm_ir_v1(
            module,
            &row_profile,
        )
        .map_err(CompilerModuleConstructionError::Lowering)?;
    let llvm_ir = bind_reviewed_row_softmax_upstream_llvm_layout_v1(
        legacy_llvm_ir,
        reviewed_row_softmax_upstream_llvm_layout_v1(),
    )?;
    let actual_llvm_sha256 = <[u8; 32]>::from(Sha256::digest(llvm_ir.as_bytes()));
    if actual_llvm_sha256 != REVIEWED_ROW_SOFTMAX_V1_LLVM_SHA256 {
        return Err(CompilerModuleConstructionError::RowSoftmaxLowering(
            format!(
                "LLVM 22.1.8 layout-bound gfx942 digest differs from the reviewed row_softmax_v1 lowering: expected {:02x?}, found {actual_llvm_sha256:02x?}",
                REVIEWED_ROW_SOFTMAX_V1_LLVM_SHA256,
            ),
        ));
    }
    let declaration = "declare float @__ocml_exp_f32(float)";
    let call = "call float @__ocml_exp_f32(float ";
    if llvm_ir.matches(declaration).count() != 1
        || llvm_ir.matches(call).count() != 2
        || llvm_ir.contains("__fe2o3_ir_float_v1_exp_f32")
    {
        return Err(CompilerModuleConstructionError::RowSoftmaxLowering(
            "authenticated gfx942 lowering did not preserve the exact two-call OCML exp closure"
                .to_owned(),
        ));
    }

    Ok(InertCompilerModuleTextV1 {
        llvm_ir,
        kernel_entries: vec![
            crate::collected_row_softmax_v1::ROW_SOFTMAX_KERNEL_SYMBOL_V1.to_owned(),
        ],
        device_definitions: Vec::new(),
        internal_helpers: Vec::new(),
        device_ffi_exports: Vec::new(),
        external_declarations: vec!["__ocml_exp_f32".to_owned()],
        descriptor_source_identity: None,
    })
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn bind_reviewed_row_softmax_upstream_llvm_layout_v1(
    legacy_llvm_ir: String,
    measurement: RowSoftmaxReviewedLlvmLayoutMeasurementV1,
) -> Result<String, CompilerModuleConstructionError> {
    if <[u8; 32]>::from(Sha256::digest(legacy_llvm_ir.as_bytes()))
        != REVIEWED_ROW_SOFTMAX_LEGACY_LLVM_SHA256
    {
        return Err(CompilerModuleConstructionError::RowSoftmaxLowering(
            "pre-layout row-softmax LLVM differs from the reviewed complete legacy lowering"
                .to_owned(),
        ));
    }
    if measurement.llvm_build_identity != ROW_SOFTMAX_UPSTREAM_LLVM_BUILD_IDENTITY_V1
        || measurement.target_triple != ROW_SOFTMAX_UPSTREAM_LLVM_TARGET_TRIPLE_V1
        || measurement.data_layout != ROW_SOFTMAX_UPSTREAM_LLVM_DATA_LAYOUT_V1
    {
        return Err(CompilerModuleConstructionError::RowSoftmaxLowering(
            "row-softmax LLVM layout binding is not the reviewed LLVM 22.1.8 target-machine measurement"
                .to_owned(),
        ));
    }

    let legacy_triple = format!("target triple = \"{}\"\n", measurement.target_triple);
    let legacy_layout = format!(
        "target datalayout = \"{}\"\n",
        dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT,
    );
    let mut lines = legacy_llvm_ir.split_inclusive('\n');
    if lines.next() != Some(legacy_triple.as_str())
        || lines.next() != Some(legacy_layout.as_str())
        || lines.next() != Some("\n")
    {
        return Err(CompilerModuleConstructionError::RowSoftmaxLowering(
            "reviewed pre-layout row-softmax LLVM target header is missing or reordered".to_owned(),
        ));
    }
    let body = lines.collect::<String>();
    if legacy_llvm_ir.matches("target triple =").count() != 1
        || legacy_llvm_ir.matches("target datalayout =").count() != 1
    {
        return Err(CompilerModuleConstructionError::RowSoftmaxLowering(
            "reviewed pre-layout row-softmax LLVM target header is duplicated".to_owned(),
        ));
    }

    let mut bound = String::with_capacity(
        legacy_llvm_ir.len()
            + measurement
                .data_layout
                .len()
                .saturating_sub(dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT.len()),
    );
    bound.push_str(&legacy_triple);
    bound.push_str("target datalayout = \"");
    bound.push_str(measurement.data_layout);
    bound.push_str("\"\n\n");
    bound.push_str(&body);
    validate_row_softmax_upstream_llvm_layout_v1(&bound, measurement)
        .map_err(|detail| CompilerModuleConstructionError::RowSoftmaxLowering(detail.to_owned()))?;
    Ok(bound)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn validate_row_softmax_upstream_llvm_layout_v1(
    llvm_ir: &str,
    measurement: RowSoftmaxReviewedLlvmLayoutMeasurementV1,
) -> Result<(), &'static str> {
    let expected_triple = format!("target triple = \"{}\"\n", measurement.target_triple);
    let expected_layout = format!("target datalayout = \"{}\"\n", measurement.data_layout);
    let mut lines = llvm_ir.split_inclusive('\n');
    if lines.next() != Some(expected_triple.as_str())
        || lines.next() != Some(expected_layout.as_str())
        || lines.next() != Some("\n")
    {
        return Err("row-softmax LLVM 22.1.8 target-machine layout header is missing or reordered");
    }
    if llvm_ir.matches("target triple =").count() != 1
        || llvm_ir.matches("target datalayout =").count() != 1
    {
        return Err("row-softmax LLVM 22.1.8 target-machine layout header is duplicated");
    }
    Ok(())
}

/// Constructs the sole LLVM module admitted by the authenticated fixed-shape
/// FlashAttention sidecar. The two call sites retain one unresolved OCML exp
/// import; provider selection and native finalization remain later stages.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn construct_inert_flash_attention_v1_module_text(
    ir: &fe2o3_kernel_ir::FlashAttentionKernelIrV1,
    profile: &fe2o3_kernel_ir::FlashAttentionProfileV1,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    fe2o3_kernel_ir::verify_flash_attention_v1(ir, profile).map_err(|error| {
        CompilerModuleConstructionError::FlashAttentionLowering(error.to_string())
    })?;
    let llvm_ir = canonical_flash_attention_v1_llvm();
    let declaration = "declare float @__ocml_exp_f32(float)";
    let call = "call float @__ocml_exp_f32(float ";
    let exact = llvm_ir.matches("define amdgpu_kernel").count() == 1
        && llvm_ir.matches(declaration).count() == 1
        && llvm_ir.matches(call).count() == 2
        && llvm_ir.matches("call void @llvm.trap()").count() == 1
        && llvm_ir.contains("\"target-cpu\"=\"gfx942\"")
        && llvm_ir.contains("\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"")
        && llvm_ir.contains("\"fp-contract\"=\"off\"")
        && !llvm_ir.contains(" fast ")
        && !llvm_ir.contains("contract ")
        && !llvm_ir.contains("reassoc ")
        && !llvm_ir.contains("comgr")
        && !llvm_ir.contains("COMGR");
    if !exact {
        return Err(CompilerModuleConstructionError::FlashAttentionLowering(
            "canonical LLVM closure audit failed".to_owned(),
        ));
    }
    Ok(InertCompilerModuleTextV1 {
        llvm_ir,
        kernel_entries: vec![fe2o3_kernel_ir::FLASH_ATTENTION_V1_KERNEL_ID.to_owned()],
        device_definitions: Vec::new(),
        internal_helpers: Vec::new(),
        device_ffi_exports: Vec::new(),
        external_declarations: vec!["__ocml_exp_f32".to_owned()],
        descriptor_source_identity: None,
    })
}

/// Lowers only the source-authenticated exact T8/E4/K2/C4 MoE sidecar.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn construct_inert_moe_top2_v1_module_text(
    ir: &fe2o3_kernel_ir::MoeTop2KernelIrV1,
    profile: &fe2o3_kernel_ir::MoeTop2ProfileV1,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    let llvm_ir = crate::moe_top2_v1_codegen::lower_exact_moe_top2_v1(ir, profile)
        .map_err(|error| CompilerModuleConstructionError::MoeTop2Lowering(error.to_owned()))?;
    enforce_source_debug_text_bound(&llvm_ir)?;
    Ok(InertCompilerModuleTextV1 {
        llvm_ir,
        kernel_entries: vec![fe2o3_kernel_ir::MOE_TOP2_V1_KERNEL_ID.to_owned()],
        device_definitions: Vec::new(),
        internal_helpers: vec![
            "__fe2o3_moe_select_expert_v1".to_owned(),
            "__fe2o3_moe_requested_count_v1".to_owned(),
            "__fe2o3_moe_admitted_count_v1".to_owned(),
            "__fe2o3_moe_expert_offset_v1".to_owned(),
            "__fe2o3_moe_route_slot_v1".to_owned(),
        ],
        device_ffi_exports: Vec::new(),
        external_declarations: Vec::new(),
        descriptor_source_identity: None,
    })
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn canonical_flash_attention_v1_llvm() -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(48 * 1024);
    writeln!(output, "target triple = \"amdgcn-amd-amdhsa\"").unwrap();
    writeln!(
        output,
        "target datalayout = \"{}\"\n",
        dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT
    )
    .unwrap();
    output.push_str(
        "declare i32 @llvm.amdgcn.workitem.id.x() #1\n\
         declare void @llvm.trap()\n\
         declare float @__ocml_exp_f32(float)\n\n\
         define amdgpu_kernel void @flash_attention_causal_f32_b1_h1_n8_d16_v1(ptr addrspace(1) nocapture readonly align 4 %q.data, i64 %q.len, ptr addrspace(1) nocapture readonly align 4 %k.data, i64 %k.len, ptr addrspace(1) nocapture readonly align 4 %v.data, i64 %v.len, ptr addrspace(1) noalias nocapture writeonly align 4 %output.data, i64 %output.len) #0 !reqd_work_group_size !0 !kernel_arg_access_qual !1 !kernel_arg_type !2 !kernel_arg_base_type !2 !kernel_arg_type_qual !3 {\n\
         entry:\n\
           %lane.i32 = call i32 @llvm.amdgcn.workitem.id.x()\n\
           %lane = zext i32 %lane.i32 to i64\n\
           %lane.ok = icmp ult i64 %lane, 64\n\
           %q.len.ok = icmp eq i64 %q.len, 128\n\
           %k.len.ok = icmp eq i64 %k.len, 128\n\
           %v.len.ok = icmp eq i64 %v.len, 128\n\
           %output.len.ok = icmp eq i64 %output.len, 128\n\
           %shape.ok.0 = and i1 %lane.ok, %q.len.ok\n\
           %shape.ok.1 = and i1 %shape.ok.0, %k.len.ok\n\
           %shape.ok.2 = and i1 %shape.ok.1, %v.len.ok\n\
           %shape.ok = and i1 %shape.ok.2, %output.len.ok\n\
           br i1 %shape.ok, label %scan.cond, label %trap\n\n\
         scan.cond:\n\
           %scan.index = phi i64 [ 0, %entry ], [ %scan.next, %scan.body ]\n\
           %scan.more = icmp ult i64 %scan.index, 128\n\
           br i1 %scan.more, label %scan.body, label %scan.done\n\n\
         scan.body:\n\
           %scan.q.ptr = getelementptr inbounds float, ptr addrspace(1) %q.data, i64 %scan.index\n\
           %scan.k.ptr = getelementptr inbounds float, ptr addrspace(1) %k.data, i64 %scan.index\n\
           %scan.v.ptr = getelementptr inbounds float, ptr addrspace(1) %v.data, i64 %scan.index\n\
           %scan.q = load float, ptr addrspace(1) %scan.q.ptr, align 4\n\
           %scan.k = load float, ptr addrspace(1) %scan.k.ptr, align 4\n\
           %scan.v = load float, ptr addrspace(1) %scan.v.ptr, align 4\n\
           %scan.q.bits = bitcast float %scan.q to i32\n\
           %scan.k.bits = bitcast float %scan.k to i32\n\
           %scan.v.bits = bitcast float %scan.v to i32\n\
           %scan.q.exponent = and i32 %scan.q.bits, 2139095040\n\
           %scan.k.exponent = and i32 %scan.k.bits, 2139095040\n\
           %scan.v.exponent = and i32 %scan.v.bits, 2139095040\n\
           %scan.q.finite = icmp ne i32 %scan.q.exponent, 2139095040\n\
           %scan.k.finite = icmp ne i32 %scan.k.exponent, 2139095040\n\
           %scan.v.finite = icmp ne i32 %scan.v.exponent, 2139095040\n\
           %scan.qk.finite = and i1 %scan.q.finite, %scan.k.finite\n\
           %scan.all.finite = and i1 %scan.qk.finite, %scan.v.finite\n\
           %scan.next = add nuw i64 %scan.index, 1\n\
           br i1 %scan.all.finite, label %scan.cond, label %trap\n\n\
         scan.done:\n\
           %first = shl nuw nsw i64 %lane, 1\n\
           %query = udiv i64 %first, 16\n\
           %column = urem i64 %first, 16\n",
    );
    emit_flash_score(&mut output, "initial", "0");
    output.push_str(
        "  br i1 %initial.score.finite, label %initial.ok, label %trap\n\n\
         initial.ok:\n\
           %initial.value0.index = add i64 %column, 0\n\
           %initial.value1.index = add i64 %column, 1\n\
           %initial.value0.ptr = getelementptr inbounds float, ptr addrspace(1) %v.data, i64 %initial.value0.index\n\
           %initial.value1.ptr = getelementptr inbounds float, ptr addrspace(1) %v.data, i64 %initial.value1.index\n\
           %initial.value0 = load float, ptr addrspace(1) %initial.value0.ptr, align 4\n\
           %initial.value1 = load float, ptr addrspace(1) %initial.value1.ptr, align 4\
           br label %recur.cond\n\n\
         recur.cond:\n\
           %key = phi i64 [ 1, %initial.ok ], [ %next.key, %recur.ok ]\n\
           %running.max = phi float [ %initial.score, %initial.ok ], [ %next.max, %recur.ok ]\n\
           %running.sum = phi float [ 1.000000e+00, %initial.ok ], [ %next.sum, %recur.ok ]\n\
           %numerator0 = phi float [ %initial.value0, %initial.ok ], [ %next.numerator0, %recur.ok ]\n\
           %numerator1 = phi float [ %initial.value1, %initial.ok ], [ %next.numerator1, %recur.ok ]\n\
           %recur.more = icmp ule i64 %key, %query\n\
           br i1 %recur.more, label %recur.body, label %finish\n\n\
         recur.body:\n",
    );
    emit_flash_score(&mut output, "next", "%key");
    output.push_str(
        "  br i1 %next.score.finite, label %recur.score.ok, label %trap\n\n\
         recur.score.ok:\n\
           %score.greater = fcmp ogt float %next.score, %running.max\n\
           %next.max = select i1 %score.greater, float %next.score, float %running.max\n\
           %previous.delta = fsub float %running.max, %next.max\n\
           %current.delta = fsub float %next.score, %next.max\n\
           %previous.weight = call float @__ocml_exp_f32(float %previous.delta)\n\
           %current.weight = call float @__ocml_exp_f32(float %current.delta)\n\
           %previous.weight.bits = bitcast float %previous.weight to i32\n\
           %current.weight.bits = bitcast float %current.weight to i32\n\
           %previous.weight.exponent = and i32 %previous.weight.bits, 2139095040\n\
           %current.weight.exponent = and i32 %current.weight.bits, 2139095040\n\
           %previous.weight.finite = icmp ne i32 %previous.weight.exponent, 2139095040\n\
           %current.weight.finite = icmp ne i32 %current.weight.exponent, 2139095040\n\
           %weights.finite = and i1 %previous.weight.finite, %current.weight.finite\n\
           %next.value.base = mul nuw i64 %key, 16\n\
           %next.value0.index = add nuw i64 %next.value.base, %column\n\
           %next.value1.index = add nuw i64 %next.value0.index, 1\n\
           %next.value0.ptr = getelementptr inbounds float, ptr addrspace(1) %v.data, i64 %next.value0.index\n\
           %next.value1.ptr = getelementptr inbounds float, ptr addrspace(1) %v.data, i64 %next.value1.index\n\
           %next.value0 = load float, ptr addrspace(1) %next.value0.ptr, align 4\n\
           %next.value1 = load float, ptr addrspace(1) %next.value1.ptr, align 4\n\
           %weighted.sum = fmul float %running.sum, %previous.weight\n\
           %next.sum = fadd float %weighted.sum, %current.weight\n\
           %weighted.numerator0 = fmul float %numerator0, %previous.weight\n\
           %weighted.current0 = fmul float %next.value0, %current.weight\n\
           %next.numerator0 = fadd float %weighted.numerator0, %weighted.current0\n\
           %weighted.numerator1 = fmul float %numerator1, %previous.weight\n\
           %weighted.current1 = fmul float %next.value1, %current.weight\n\
           %next.numerator1 = fadd float %weighted.numerator1, %weighted.current1\n\
           %next.sum.bits = bitcast float %next.sum to i32\n\
           %next.numerator0.bits = bitcast float %next.numerator0 to i32\n\
           %next.numerator1.bits = bitcast float %next.numerator1 to i32\n\
           %next.sum.exponent = and i32 %next.sum.bits, 2139095040\n\
           %next.numerator0.exponent = and i32 %next.numerator0.bits, 2139095040\n\
           %next.numerator1.exponent = and i32 %next.numerator1.bits, 2139095040\n\
           %next.sum.finite = icmp ne i32 %next.sum.exponent, 2139095040\n\
           %next.numerator0.finite = icmp ne i32 %next.numerator0.exponent, 2139095040\n\
           %next.numerator1.finite = icmp ne i32 %next.numerator1.exponent, 2139095040\n\
           %next.sum.positive = fcmp ogt float %next.sum, 0.000000e+00\n\
           %recur.valid.0 = and i1 %weights.finite, %next.sum.finite\n\
           %recur.valid.1 = and i1 %recur.valid.0, %next.sum.positive\n\
           %recur.valid.2 = and i1 %recur.valid.1, %next.numerator0.finite\n\
           %recur.valid = and i1 %recur.valid.2, %next.numerator1.finite\n\
           br i1 %recur.valid, label %recur.ok, label %trap\n\n\
         recur.ok:\n\
           %next.key = add nuw i64 %key, 1\n\
           br label %recur.cond\n\n\
         finish:\n\
           %output0 = fdiv float %numerator0, %running.sum\n\
           %output1 = fdiv float %numerator1, %running.sum\n\
           %output0.bits = bitcast float %output0 to i32\n\
           %output1.bits = bitcast float %output1 to i32\n\
           %output0.exponent = and i32 %output0.bits, 2139095040\n\
           %output1.exponent = and i32 %output1.bits, 2139095040\n\
           %output0.finite = icmp ne i32 %output0.exponent, 2139095040\n\
           %output1.finite = icmp ne i32 %output1.exponent, 2139095040\n\
           %outputs.finite = and i1 %output0.finite, %output1.finite\n\
           br i1 %outputs.finite, label %store, label %trap\n\n\
         store:\n\
           %second = add nuw nsw i64 %first, 1\n\
           %output0.ptr = getelementptr inbounds float, ptr addrspace(1) %output.data, i64 %first\n\
           %output1.ptr = getelementptr inbounds float, ptr addrspace(1) %output.data, i64 %second\n\
           store float %output0, ptr addrspace(1) %output0.ptr, align 4\n\
           store float %output1, ptr addrspace(1) %output1.ptr, align 4\n\
           ret void\n\n\
         trap:\n\
           call void @llvm.trap()\n\
           ret void\n\
         }\n\n\
         attributes #0 = { nounwind \"amdgpu-flat-work-group-size\"=\"64,64\" \"target-cpu\"=\"gfx942\" \"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\" \"denormal-fp-math-f32\"=\"ieee,ieee\" \"unsafe-fp-math\"=\"false\" \"no-infs-fp-math\"=\"false\" \"no-nans-fp-math\"=\"false\" \"no-signed-zeros-fp-math\"=\"false\" \"approx-func-fp-math\"=\"false\" \"fp-contract\"=\"off\" }\n\
         attributes #1 = { nounwind readnone speculatable willreturn }\n\n\
         !0 = !{i32 64, i32 1, i32 1}\n\
         !1 = !{!\"read_only\", !\"none\", !\"read_only\", !\"none\", !\"read_only\", !\"none\", !\"read_write\", !\"none\"}\n\
         !2 = !{!\"float*\", !\"ulong\", !\"float*\", !\"ulong\", !\"float*\", !\"ulong\", !\"float*\", !\"ulong\"}\n\
         !3 = !{!\"const\", !\"\", !\"const\", !\"\", !\"const\", !\"\", !\"restrict\", !\"\"}\n",
    );
    output
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn emit_flash_score(output: &mut String, prefix: &str, key: &str) {
    use std::fmt::Write as _;

    writeln!(output, "  %{prefix}.q.base = mul nuw i64 %query, 16").unwrap();
    writeln!(output, "  %{prefix}.k.base = mul nuw i64 {key}, 16").unwrap();
    let mut previous_dot = "0.000000e+00".to_owned();
    let mut previous_valid = "true".to_owned();
    for feature in 0..16 {
        writeln!(
            output,
            "  %{prefix}.q.index.{feature} = add nuw i64 %{prefix}.q.base, {feature}"
        )
        .unwrap();
        writeln!(
            output,
            "  %{prefix}.k.index.{feature} = add nuw i64 %{prefix}.k.base, {feature}"
        )
        .unwrap();
        writeln!(output, "  %{prefix}.q.ptr.{feature} = getelementptr inbounds float, ptr addrspace(1) %q.data, i64 %{prefix}.q.index.{feature}").unwrap();
        writeln!(output, "  %{prefix}.k.ptr.{feature} = getelementptr inbounds float, ptr addrspace(1) %k.data, i64 %{prefix}.k.index.{feature}").unwrap();
        writeln!(output, "  %{prefix}.q.{feature} = load float, ptr addrspace(1) %{prefix}.q.ptr.{feature}, align 4").unwrap();
        writeln!(output, "  %{prefix}.k.{feature} = load float, ptr addrspace(1) %{prefix}.k.ptr.{feature}, align 4").unwrap();
        writeln!(output, "  %{prefix}.product.{feature} = fmul float %{prefix}.q.{feature}, %{prefix}.k.{feature}").unwrap();
        writeln!(
            output,
            "  %{prefix}.product.bits.{feature} = bitcast float %{prefix}.product.{feature} to i32"
        )
        .unwrap();
        writeln!(output, "  %{prefix}.product.exponent.{feature} = and i32 %{prefix}.product.bits.{feature}, 2139095040").unwrap();
        writeln!(output, "  %{prefix}.product.finite.{feature} = icmp ne i32 %{prefix}.product.exponent.{feature}, 2139095040").unwrap();
        writeln!(
            output,
            "  %{prefix}.dot.{feature} = fadd float {previous_dot}, %{prefix}.product.{feature}"
        )
        .unwrap();
        writeln!(
            output,
            "  %{prefix}.dot.bits.{feature} = bitcast float %{prefix}.dot.{feature} to i32"
        )
        .unwrap();
        writeln!(
            output,
            "  %{prefix}.dot.exponent.{feature} = and i32 %{prefix}.dot.bits.{feature}, 2139095040"
        )
        .unwrap();
        writeln!(output, "  %{prefix}.dot.finite.{feature} = icmp ne i32 %{prefix}.dot.exponent.{feature}, 2139095040").unwrap();
        writeln!(output, "  %{prefix}.step.finite.{feature} = and i1 %{prefix}.product.finite.{feature}, %{prefix}.dot.finite.{feature}").unwrap();
        writeln!(output, "  %{prefix}.prefix.finite.{feature} = and i1 {previous_valid}, %{prefix}.step.finite.{feature}").unwrap();
        previous_dot = format!("%{prefix}.dot.{feature}");
        previous_valid = format!("%{prefix}.prefix.finite.{feature}");
    }
    writeln!(
        output,
        "  %{prefix}.score = fmul float {previous_dot}, 2.500000e-01"
    )
    .unwrap();
    writeln!(
        output,
        "  %{prefix}.score.bits = bitcast float %{prefix}.score to i32"
    )
    .unwrap();
    writeln!(
        output,
        "  %{prefix}.score.exponent = and i32 %{prefix}.score.bits, 2139095040"
    )
    .unwrap();
    writeln!(
        output,
        "  %{prefix}.scaled.finite = icmp ne i32 %{prefix}.score.exponent, 2139095040"
    )
    .unwrap();
    writeln!(
        output,
        "  %{prefix}.score.finite = and i1 {previous_valid}, %{prefix}.scaled.finite"
    )
    .unwrap();
}

fn ocml_link_imports(module: &Module) -> impl Iterator<Item = &'static str> + '_ {
    module.functions.iter().filter_map(|function| {
        let FloatOperation::F32Math {
            function,
            implementation: F32MathImplementation::OcmlAbiV1,
            ..
        } = FloatOperation::from_intrinsic_id(&function.id)?
        else {
            return None;
        };
        Some(match function {
            F32MathFunction::Sin => "__ocml_sin_f32",
            F32MathFunction::Cos => "__ocml_cos_f32",
            F32MathFunction::Exp => "__ocml_exp_f32",
            F32MathFunction::Exp2 => "__ocml_exp2_f32",
            F32MathFunction::Ln => "__ocml_log_f32",
            F32MathFunction::Log2 => "__ocml_log2_f32",
            F32MathFunction::Log10 => "__ocml_log10_f32",
            F32MathFunction::Sqrt
            | F32MathFunction::FusedMultiplyAdd
            | F32MathFunction::Floor
            | F32MathFunction::Ceil
            | F32MathFunction::Truncate
            | F32MathFunction::RoundTiesEven => {
                unreachable!("canonical implementation excludes constrained LLVM from OCML")
            }
        })
    })
}

/// Embeds one exact zero-digest descriptor source in compiler-owned LLVM module assembly.
///
/// The empty ELF flag string is intentional: LLVM and LLD preserve this as a non-allocatable,
/// non-writable, non-executable `SHT_PROGBITS` section. The existing Worker V2 module identity
/// therefore commits to the descriptor bytes without a second transport or linker input.
pub(crate) fn bind_compiler_descriptor_source_v1(
    mut module: InertCompilerModuleTextV1,
    source: &CompilerDescriptorSourceV1,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    if module.descriptor_source_identity.is_some() {
        return Err(CompilerModuleConstructionError::DescriptorSourceAlreadyBound);
    }

    let mut entries = source
        .table()
        .kernels()
        .iter()
        .map(|kernel| kernel.entry_name().as_str())
        .collect::<Vec<_>>();
    entries.sort_unstable();
    if entries
        != module
            .kernel_entries
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    {
        return Err(CompilerModuleConstructionError::DescriptorKernelEntryClosureMismatch);
    }

    let mut descriptors = source
        .table()
        .kernels()
        .iter()
        .map(|kernel| kernel.descriptor_symbol().as_str())
        .collect::<Vec<_>>();
    descriptors.sort_unstable();
    let expected_descriptors = module
        .kernel_entries
        .iter()
        .map(|entry| format!("{entry}.kd"))
        .collect::<Vec<_>>();
    if descriptors
        != expected_descriptors
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    {
        return Err(CompilerModuleConstructionError::DescriptorSymbolClosureMismatch);
    }

    append_descriptor_module_assembly(&mut module.llvm_ir, source.canonical_bytes());
    module.descriptor_source_identity = Some(source.identity());
    Ok(module)
}

/// Binds the exact single-use scalar frontend authority into compiler-owned
/// non-allocatable module assembly retained by the Worker V2 request.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn bind_scalar_gemm_frontend_authority_v1(
    mut module: InertCompilerModuleTextV1,
    authority: [u8; 32],
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    module
        .llvm_ir
        .push_str("\nmodule asm \".section .fe2o3.scalar-auth.v1,\\22\\22,@progbits\"\n");
    module.llvm_ir.push_str("module asm \".balign 8\"\n");
    append_module_asm_bytes(&mut module.llvm_ir, &authority);
    enforce_source_debug_text_bound(&module.llvm_ir)?;
    Ok(module)
}

/// Binds the single-use tiled frontend authority to the exact LLVM handoff.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn bind_tiled_gemm_frontend_authority_v1(
    mut module: InertCompilerModuleTextV1,
    authority: [u8; 32],
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    module
        .llvm_ir
        .push_str("\nmodule asm \".section .fe2o3.tiled-auth.v1,\\22\\22,@progbits\"\n");
    module.llvm_ir.push_str("module asm \".balign 8\"\n");
    append_module_asm_bytes(&mut module.llvm_ir, &authority);
    enforce_source_debug_text_bound(&module.llvm_ir)?;
    Ok(module)
}

/// Binds the consumed attributed-source authority and exact compiler resource
/// transcript to the LDS Slice 1 Worker V2 module.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn bind_tiled_gemm_lds_slice1_authority_v1(
    mut module: InertCompilerModuleTextV1,
    authority: [u8; 32],
    resource_transcript: &[u8],
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    if resource_transcript.is_empty() {
        return Err(CompilerModuleConstructionError::TiledGemmLdsSlice1Lowering(
            "resource transcript is empty".to_owned(),
        ));
    }
    append_commitment_section(
        &mut module.llvm_ir,
        TILED_GEMM_LDS_SLICE1_AUTHORITY_SECTION_V1,
        &authority,
    );
    append_commitment_section(
        &mut module.llvm_ir,
        TILED_GEMM_LDS_SLICE1_RESOURCE_SECTION_V1,
        resource_transcript,
    );
    enforce_source_debug_text_bound(&module.llvm_ir)?;
    Ok(module)
}

/// Binds the private row frontend authority and the retained abstract-exp proof
/// boundary into distinct compiler-owned, non-allocatable sections.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn bind_row_softmax_frontend_authority_v1(
    mut module: InertCompilerModuleTextV1,
    authority_transcript: &[u8],
    authority: [u8; 32],
    exponential_boundary: [u8; 32],
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    if authority_transcript.is_empty()
        || authority_transcript.len()
            > crate::collected_row_softmax_v1::MAX_ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_BYTES_V1
        || <[u8; 32]>::from(Sha256::digest(authority_transcript)) != authority
    {
        return Err(CompilerModuleConstructionError::RowSoftmaxLowering(
            "authority transcript is empty, oversized, or differs from its commitment".to_owned(),
        ));
    }
    append_commitment_section(
        &mut module.llvm_ir,
        ".fe2o3.row-softmax-authority-transcript.v1",
        authority_transcript,
    );
    append_commitment_section(
        &mut module.llvm_ir,
        ".fe2o3.row-softmax-auth.v1",
        &authority,
    );
    append_commitment_section(
        &mut module.llvm_ir,
        ".fe2o3.row-exp.v1",
        &exponential_boundary,
    );
    enforce_source_debug_text_bound(&module.llvm_ir)?;
    Ok(module)
}

/// Binds the complete authenticated Flash transcript, its commitment, and the
/// deliberately limited OCML boundary to the exact compiler module.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn bind_flash_attention_v1_authority(
    mut module: InertCompilerModuleTextV1,
    authority_transcript: &[u8],
    authority: [u8; 32],
    ocml_boundary: [u8; 32],
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    if authority_transcript.is_empty()
        || authority_transcript.len() > 4096
        || <[u8; 32]>::from(Sha256::digest(authority_transcript)) != authority
    {
        return Err(CompilerModuleConstructionError::FlashAttentionLowering(
            "authority transcript is empty, oversized, or differs from its commitment".to_owned(),
        ));
    }
    append_commitment_section(
        &mut module.llvm_ir,
        FLASH_ATTENTION_AUTHORITY_TRANSCRIPT_SECTION_V1,
        authority_transcript,
    );
    append_commitment_section(
        &mut module.llvm_ir,
        FLASH_ATTENTION_AUTHORITY_SECTION_V1,
        &authority,
    );
    append_commitment_section(
        &mut module.llvm_ir,
        FLASH_ATTENTION_OCML_BOUNDARY_SECTION_V1,
        &ocml_boundary,
    );
    enforce_source_debug_text_bound(&module.llvm_ir)?;
    Ok(module)
}

/// Binds every identity retained by the consumed exact MoE receipt to the
/// compiler-owned module. The provider section commits an empty closure.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn bind_moe_top2_v1_identities(
    mut module: InertCompilerModuleTextV1,
    parts: &crate::collected_moe_top2_v1::AuthenticatedMoeTop2WorkerPartsV1,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    let provider = Sha256::digest(crate::moe_top2_v1_codegen::EMPTY_PROVIDER_CLOSURE_V1);
    let layout =
        Sha256::digest(crate::moe_top2_v1_codegen::EXACT_MOE_TOP2_GFX942_DATA_LAYOUT_V1.as_bytes());
    for (suffix, bytes) in [
        ("source.v1", parts.source_identity.as_slice()),
        ("namespace.v1", parts.source_namespace.as_slice()),
        ("crate.v1", parts.compiler_crate_binding.as_slice()),
        ("authority.v1", parts.source_authority_identity.as_slice()),
        ("mir.v1", parts.portable_mir_identity.as_slice()),
        ("fnabi.v1", parts.fn_abi_identity.as_slice()),
        ("compiler.v1", parts.compiler_semantics_identity.as_slice()),
        (
            "terminals.v3",
            parts.trusted_definitions_identity.as_slice(),
        ),
        ("abi.v1", parts.abi_identity.as_slice()),
        ("effects.v1", parts.effects_identity.as_slice()),
        ("profile.v1", parts.profile_launch_identity.as_slice()),
        ("routing.v1", parts.routing_identity.as_slice()),
        ("kir.v1", parts.canonical_ir_identity.as_slice()),
        ("descriptor.v1", parts.descriptor_identity.as_slice()),
        ("provider.v1", provider.as_slice()),
        ("layout.v1", layout.as_slice()),
    ] {
        append_commitment_section(
            &mut module.llvm_ir,
            &format!("{MOE_TOP2_SECTION_PREFIX_V1}.{suffix}"),
            bytes,
        );
    }
    enforce_source_debug_text_bound(&module.llvm_ir)?;
    Ok(module)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn append_commitment_section(llvm_ir: &mut String, section: &str, bytes: &[u8]) {
    llvm_ir.push_str("module asm \".section ");
    llvm_ir.push_str(section);
    llvm_ir.push_str(",\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n");
    append_module_asm_bytes(llvm_ir, bytes);
}

fn append_descriptor_module_assembly(llvm_ir: &mut String, bytes: &[u8]) {
    llvm_ir.push_str("\nmodule asm \".section ");
    llvm_ir.push_str(COMPILER_DESCRIPTOR_SECTION_NAME_V1);
    llvm_ir.push_str(",\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n");
    append_module_asm_bytes(llvm_ir, bytes);
}

fn append_module_asm_bytes(llvm_ir: &mut String, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for chunk in bytes.chunks(16) {
        llvm_ir.push_str("module asm \".byte ");
        for (index, byte) in chunk.iter().copied().enumerate() {
            if index != 0 {
                llvm_ir.push_str(", ");
            }
            llvm_ir.push_str("0x");
            llvm_ir.push(HEX[usize::from(byte >> 4)] as char);
            llvm_ir.push(HEX[usize::from(byte & 0x0f)] as char);
        }
        llvm_ir.push_str("\"\n");
    }
}

fn enforce_compiler_module_bounds(module: &Module) -> Result<(), CompilerModuleConstructionError> {
    check_compiler_module_limit(
        "compiler-module ID bytes",
        module.id.as_str().len(),
        MAX_COMPILER_MODULE_ID_BYTES,
    )?;
    check_compiler_module_limit(
        "compiler-module functions",
        module.functions.len(),
        MAX_COMPILER_MODULE_FUNCTIONS,
    )?;
    check_compiler_module_limit(
        "compiler-module kernels",
        module.kernels.len(),
        MAX_COMPILER_MODULE_KERNELS,
    )?;

    let mut total_capabilities = module.required_capabilities.len();
    check_compiler_module_limit(
        "compiler-module capabilities",
        total_capabilities,
        MAX_COMPILER_MODULE_CAPABILITIES,
    )?;
    check_capability_text(&module.required_capabilities)?;
    let mut total_blocks = 0usize;
    let mut total_block_parameters = 0usize;
    let mut total_operations = 0usize;
    let mut total_operation_results = 0usize;
    let mut total_cfg_arguments = 0usize;
    let mut total_switch_cases = 0usize;

    for function in &module.functions {
        check_symbol_bytes(function.id.as_str())?;
        check_compiler_module_limit(
            "compiler-module function parameters",
            function.signature.parameters.len(),
            MAX_COMPILER_MODULE_PARAMETERS,
        )?;
        check_compiler_module_limit(
            "compiler-module function results",
            function.signature.results.len(),
            MAX_COMPILER_MODULE_RESULTS,
        )?;
        for ty in function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
        {
            check_type_depth(ty, 0)?;
        }
        add_compiler_module_count(
            "compiler-module capabilities",
            &mut total_capabilities,
            function.required_capabilities.len(),
            MAX_COMPILER_MODULE_CAPABILITIES,
        )?;
        check_capability_text(&function.required_capabilities)?;

        let Some(body) = &function.body else {
            continue;
        };
        check_compiler_module_limit(
            "compiler-module body parameters",
            body.parameters.len(),
            MAX_COMPILER_MODULE_PARAMETERS,
        )?;
        add_compiler_module_count(
            "compiler-module blocks",
            &mut total_blocks,
            body.blocks.len(),
            MAX_COMPILER_MODULE_BLOCKS,
        )?;
        for block in &body.blocks {
            add_compiler_module_count(
                "compiler-module block parameters",
                &mut total_block_parameters,
                block.parameters.len(),
                MAX_COMPILER_MODULE_BLOCK_PARAMETERS,
            )?;
            for parameter in &block.parameters {
                check_type_depth(&parameter.ty, 0)?;
            }
            add_compiler_module_count(
                "compiler-module operations",
                &mut total_operations,
                block.operations.len(),
                MAX_COMPILER_MODULE_OPERATIONS,
            )?;
            for operation in &block.operations {
                add_compiler_module_count(
                    "compiler-module operation results",
                    &mut total_operation_results,
                    operation.results.len(),
                    MAX_COMPILER_MODULE_OPERATION_RESULTS,
                )?;
                for result in &operation.results {
                    check_type_depth(&result.ty, 0)?;
                }
                check_operation_bounds(operation)?;
            }
            if let Some(terminator) = &block.terminator {
                check_terminator_bounds(
                    terminator,
                    &mut total_cfg_arguments,
                    &mut total_switch_cases,
                )?;
            }
        }
    }

    for kernel in &module.kernels {
        check_symbol_bytes(kernel.id.as_str())?;
        check_symbol_bytes(kernel.entry.as_str())?;
        add_compiler_module_count(
            "compiler-module capabilities",
            &mut total_capabilities,
            kernel.required_capabilities.len(),
            MAX_COMPILER_MODULE_CAPABILITIES,
        )?;
        check_capability_text(&kernel.required_capabilities)?;
    }
    Ok(())
}

fn check_operation_bounds(operation: &Operation) -> Result<(), CompilerModuleConstructionError> {
    match &operation.kind {
        OperationKind::Call { callee, arguments } => {
            check_symbol_bytes(callee.as_str())?;
            check_compiler_module_limit(
                "compiler-module call arguments",
                arguments.len(),
                MAX_COMPILER_MODULE_CALL_ARGUMENTS,
            )?;
        }
        OperationKind::Intrinsic(intrinsic) => check_type_depth(&intrinsic.result_type, 0)?,
        OperationKind::Cast { to, .. } => check_type_depth(to, 0)?,
        OperationKind::Alloca { element, .. }
        | OperationKind::WorkgroupMemory(fe2o3_kernel_ir::WorkgroupMemory { element, .. }) => {
            check_type_depth(element, 0)?;
        }
        _ => {}
    }
    Ok(())
}

fn check_terminator_bounds(
    terminator: &Terminator,
    total_arguments: &mut usize,
    total_cases: &mut usize,
) -> Result<(), CompilerModuleConstructionError> {
    let (arguments, cases) = match terminator {
        Terminator::Branch { arguments, .. } => (arguments.len(), 0),
        Terminator::ConditionalBranch {
            then_arguments,
            else_arguments,
            ..
        } => (then_arguments.len().saturating_add(else_arguments.len()), 0),
        Terminator::Switch {
            cases,
            default_arguments,
            ..
        } => (
            cases.iter().fold(default_arguments.len(), |total, case| {
                total.saturating_add(case.arguments.len())
            }),
            cases.len(),
        ),
        Terminator::IntegerSwitch {
            cases,
            default_arguments,
            ..
        } => (
            cases.iter().fold(default_arguments.len(), |total, case| {
                total.saturating_add(case.arguments.len())
            }),
            cases.len(),
        ),
        Terminator::Return { values } => (values.len(), 0),
        Terminator::Unreachable => (0, 0),
    };
    add_compiler_module_count(
        "compiler-module CFG arguments",
        total_arguments,
        arguments,
        MAX_COMPILER_MODULE_CFG_ARGUMENTS,
    )?;
    add_compiler_module_count(
        "compiler-module switch cases",
        total_cases,
        cases,
        MAX_COMPILER_MODULE_SWITCH_CASES,
    )
}

fn check_type_depth(ty: &Type, depth: usize) -> Result<(), CompilerModuleConstructionError> {
    if depth > MAX_COMPILER_MODULE_TYPE_DEPTH {
        return Err(CompilerModuleConstructionError::LimitExceeded {
            field: "compiler-module type nesting",
            actual: depth,
            max: MAX_COMPILER_MODULE_TYPE_DEPTH,
        });
    }
    match ty {
        Type::Pointer(pointer) => check_type_depth(&pointer.pointee, depth + 1),
        Type::Slice(slice) => check_type_depth(&slice.element, depth + 1),
        Type::Unit | Type::Scalar(_) => Ok(()),
    }
}

fn check_capability_text(
    capabilities: &BTreeSet<TargetCapability>,
) -> Result<(), CompilerModuleConstructionError> {
    for capability in capabilities {
        if let TargetCapability::Extension { namespace, name } = capability {
            check_compiler_module_limit(
                "compiler-module capability namespace bytes",
                namespace.len(),
                MAX_COMPILER_MODULE_SYMBOL_BYTES,
            )?;
            check_compiler_module_limit(
                "compiler-module capability name bytes",
                name.len(),
                MAX_COMPILER_MODULE_SYMBOL_BYTES,
            )?;
        }
    }
    Ok(())
}

fn check_symbol_bytes(symbol: &str) -> Result<(), CompilerModuleConstructionError> {
    check_compiler_module_limit(
        "compiler-module symbol bytes",
        symbol.len(),
        MAX_COMPILER_MODULE_SYMBOL_BYTES,
    )
}

fn add_compiler_module_count(
    field: &'static str,
    total: &mut usize,
    increment: usize,
    max: usize,
) -> Result<(), CompilerModuleConstructionError> {
    *total = total.saturating_add(increment);
    check_compiler_module_limit(field, *total, max)
}

fn check_compiler_module_limit(
    field: &'static str,
    actual: usize,
    max: usize,
) -> Result<(), CompilerModuleConstructionError> {
    if actual > max {
        Err(CompilerModuleConstructionError::LimitExceeded { field, actual, max })
    } else {
        Ok(())
    }
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn prepare_fill_collection(
    mut module: Module,
    expected_kernel_names: &[String],
) -> Result<Vec<PreparedDeviceKernel>, EmitError> {
    let [authenticated_export] = expected_kernel_names else {
        return Err(reject(format!(
            "supports exactly one authenticated kernel export for the bounded fill or vecadd terminal profiles; collected {} exports; {NO_QUALIFICATION_FALLBACK_HINT}",
            expected_kernel_names.len()
        )));
    };
    if authenticated_export.len() > MAX_COMPILER_MODULE_SYMBOL_BYTES {
        return Err(reject(format!(
            "authenticated kernel export identity exceeds the {MAX_COMPILER_MODULE_SYMBOL_BYTES}-byte visible compiler-module bound; {NO_QUALIFICATION_FALLBACK_HINT}"
        )));
    }
    enforce_compiler_module_bounds(&module).map_err(|error| {
        reject(format!(
            "authenticated kernel IR exceeds the bounded terminal-profile model: {error}; {NO_QUALIFICATION_FALLBACK_HINT}"
        ))
    })?;
    verify_module(&module).map_err(|errors| {
        reject(format!(
            "received invalid verified kernel IR before terminal-profile legalization: {errors}"
        ))
    })?;

    let [translated_kernel] = module.kernels.as_slice() else {
        let translated = module
            .kernels
            .iter()
            .map(|kernel| kernel.id.as_str())
            .collect::<Vec<_>>();
        return Err(reject(format!(
            "translated kernel identities {translated:?} do not match the single collected kernel identity {authenticated_export:?}; {NO_QUALIFICATION_FALLBACK_HINT}"
        )));
    };
    if translated_kernel.id.as_str() != authenticated_export {
        return Err(reject(format!(
            "translated kernel identity {:?} does not match collected kernel identity {authenticated_export:?}; {NO_QUALIFICATION_FALLBACK_HINT}",
            translated_kernel.id.as_str()
        )));
    }

    let kernel = module
        .kernels
        .first_mut()
        .expect("exactly one authenticated kernel was established");
    let required_workgroup_size = WorkgroupSize::new(WORKGROUP_X, 1, 1);
    match kernel.workgroup_size {
        None => kernel.workgroup_size = Some(required_workgroup_size),
        Some(observed) if observed == required_workgroup_size => {}
        Some(observed) => {
            return Err(reject(format!(
                "`{authenticated_export}` authenticated workgroup size {observed:?} conflicts with the exact {WORKGROUP_X}x1x1 executable profile"
            )));
        }
    }
    let kernel_id = kernel.id.clone();
    let entry = kernel.entry.clone();

    let function = module
        .functions
        .iter_mut()
        .find(|function| function.id == entry)
        .expect("initial verification established the kernel entry");
    let body = function.body.as_mut().expect("verified kernel entry body");
    legalize_terminal_profile_v1(
        &kernel_id,
        &function.signature.parameters,
        &function.signature.results,
        body,
    )?;

    verify_module(&module).map_err(|errors| {
        reject(format!(
            "{kernel_id} legalization produced invalid kernel IR and was not emitted: {errors}"
        ))
    })?;
    let llvm_ir =
        dialect_amdgcn::lower_kernel_to_llvm_ir(&module, &kernel_id).map_err(|errors| {
            reject(format!(
                "G1 AMDGPU lowering rejected `{kernel_id}`: {errors}"
            ))
        })?;

    Ok(vec![PreparedDeviceKernel {
        name: kernel_id.as_str().to_string(),
        llvm_ir,
    }])
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KernelIrV1TerminalProfile {
    Fill,
    Vecadd,
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn legalize_terminal_profile_v1(
    kernel_id: &KernelId,
    parameters: &[Type],
    results: &[Type],
    body: &mut FunctionBody,
) -> Result<(), EmitError> {
    let fill_parameters = [writable_f32_slice()];
    let vecadd_parameters = [
        readonly_f32_slice(),
        readonly_f32_slice(),
        writable_f32_slice(),
    ];
    let profile = if results.is_empty() && parameters == fill_parameters {
        KernelIrV1TerminalProfile::Fill
    } else if results.is_empty() && parameters == vecadd_parameters {
        KernelIrV1TerminalProfile::Vecadd
    } else {
        return Err(reject(format!(
            "does not support kernel export {:?} with authenticated verified kernel IR signature {parameters:?} -> {results:?}; bounded terminal profiles require fill {fill_parameters:?} -> () or vecadd {vecadd_parameters:?} -> (); {NO_QUALIFICATION_FALLBACK_HINT}",
            kernel_id.as_str()
        )));
    };

    match profile {
        KernelIrV1TerminalProfile::Fill => legalize_fill_body(body, parameters),
        KernelIrV1TerminalProfile::Vecadd => legalize_vecadd_body(body, parameters),
    }
}

/// Retains an audited, test-only observation of the rustc-imported matrix module.
///
/// This writes textual LLVM IR and grants no artifact, publication, load, or
/// execution authority. The ordinary production export gate still runs after
/// this observation is retained.
#[cfg(all(test, feature = "qualification-oracles-test-only"))]
pub(crate) fn retain_tiled_gemm_frontend_test_llvm(
    module: &Module,
    directory: &Path,
) -> Result<(), EmitError> {
    verify_module(module).map_err(|errors| {
        reject(format!(
            "test-only tiled GEMM retention received invalid kernel IR: {errors}"
        ))
    })?;
    let [kernel] = module.kernels.as_slice() else {
        return Err(reject(
            "test-only tiled GEMM retention requires exactly one kernel",
        ));
    };
    if kernel.id.as_str() != "tiled_gemm_frontend_v1" || module.functions.len() != 1 {
        return Err(reject(
            "test-only tiled GEMM retention accepts only the exact imported frontend fixture",
        ));
    }
    let llvm = dialect_amdgcn::lower_kernel_to_gfx942_xnack_minus_llvm_ir(module, &kernel.id)
        .map_err(|errors| {
            reject(format!(
                "test-only tiled GEMM exact gfx942:xnack- lowering failed: {errors}"
            ))
        })?;
    audit_tiled_gemm_frontend_test_llvm(&llvm)?;
    fs::create_dir_all(directory).map_err(|error| {
        reject(format!(
            "cannot create test-only tiled GEMM retention directory: {error}"
        ))
    })?;
    let path = directory.join(TILED_GEMM_FRONTEND_TEST_LLVM_FILE);
    let retained =
        format!("; TEST-ONLY RUSTC IMPORT OBSERVATION; NO ARTIFACT OR EXECUTION AUTHORITY\n{llvm}");
    fs::write(&path, retained).map_err(|error| {
        reject(format!(
            "cannot retain test-only tiled GEMM LLVM observation at {}: {error}",
            path.display()
        ))
    })?;
    Ok(())
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn audit_tiled_gemm_frontend_test_llvm(llvm: &str) -> Result<(), EmitError> {
    for required in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT,
        "llvm.amdgcn.mfma.f32.16x16x16bf16.1k",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "\"denormal-fp-math-f32\"=\"ieee,ieee\"",
        "\"unsafe-fp-math\"=\"false\"",
        "\"fp-contract\"=\"off\"",
        "fe2o3.projected-kernarg-policy.v1 sha256=",
        "fe2o3.projected-kernarg explicit-size=32 implicit-bytes=256 segment-size=288 segment-align=8 source=compiler-policy-not-rustc-observation",
        "fe2o3.projected-kernarg.param index=0 source=0 lane=0 type=bf16 offset=0 size=2 align=2",
        "fe2o3.projected-kernarg.param index=7 source=1 lane=3 type=bf16 offset=14 size=2 align=2",
        "fe2o3.projected-kernarg.param index=8 source=2 lane=0 type=f32 offset=16 size=4 align=4",
        "fe2o3.projected-kernarg.param index=11 source=2 lane=3 type=f32 offset=28 size=4 align=4",
        "define amdgpu_kernel void @tiled_gemm_frontend_v1(i16 %arg0, i16 %arg1, i16 %arg2, i16 %arg3, i16 %arg4, i16 %arg5, i16 %arg6, i16 %arg7, float %arg8, float %arg9, float %arg10, float %arg11)",
    ] {
        if !llvm.contains(required) {
            return Err(reject(format!(
                "test-only tiled GEMM LLVM audit is missing `{required}`"
            )));
        }
    }
    for forbidden in [
        "DeviceMatrix",
        "fe2o3_device",
        "panicking",
        "panic",
        "unreachable",
        "+xnack",
        "+wavefrontsize32",
        "fast ",
        "matrix-frontend-physical-abi",
        "fe2o3.kernarg",
    ] {
        if llvm.contains(forbidden) {
            return Err(reject(format!(
                "test-only tiled GEMM LLVM audit found forbidden `{forbidden}`"
            )));
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn legalize_fill_body(body: &mut FunctionBody, parameters: &[Type]) -> Result<(), EmitError> {
    if body.parameters.len() != parameters.len() {
        return Err(reject(
            "fill entry parameter identities do not match its signature",
        ));
    }

    let value_types = collect_value_types(body, parameters);
    let mut next_value = value_types.keys().next_back().map_or(Ok(0), |value| {
        value
            .0
            .checked_add(1)
            .ok_or_else(|| reject("fill kernel exhausted kernel IR value identities"))
    })?;
    let mut option_conditions = BTreeSet::new();
    let mut thread_calls = 0usize;
    let mut get_mut_calls = 0usize;
    let mut thread_index = None;
    let mut get_mut_index = None;
    let mut output_slice = None;
    let mut output_pointer = None;

    for block in &mut body.blocks {
        let mut legalized = Vec::with_capacity(block.operations.len() + 4);
        for operation in std::mem::take(&mut block.operations) {
            let OperationKind::Call { callee, arguments } = &operation.kind else {
                legalized.push(operation);
                continue;
            };

            if callee.as_str() == TrustedDeviceItem::ThreadIndex1d.canonical_path() {
                require_call_shape(
                    "thread::index_1d",
                    &operation,
                    arguments,
                    &[],
                    &[Type::INDEX],
                    &value_types,
                )?;
                thread_calls += 1;
                thread_index = Some(operation.results[0].id);
                legalized.push(Operation::effect_free(
                    operation.results[0].clone(),
                    OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                ));
                continue;
            }

            if callee.as_str() == TrustedDeviceItem::DisjointSliceGetMut.canonical_path() {
                let pointer = writable_f32_pointer();
                require_call_shape(
                    "DisjointSlice::get_mut",
                    &operation,
                    arguments,
                    &[writable_f32_slice(), Type::INDEX],
                    &[Type::INDEX, pointer.clone()],
                    &value_types,
                )?;
                get_mut_calls += 1;
                get_mut_index = Some(arguments[1]);
                output_slice = Some(arguments[0]);
                output_pointer = Some(operation.results[1].id);

                let length = fresh_value(&mut next_value, Type::INDEX)?;
                legalized.push(Operation::effect_free(
                    length.clone(),
                    OperationKind::SliceLength {
                        slice: arguments[0],
                    },
                ));

                let condition = ValueDef::new(operation.results[0].id, Type::BOOL);
                option_conditions.insert(condition.id);
                legalized.push(Operation::effect_free(
                    condition,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: arguments[1],
                        rhs: length.id,
                    },
                ));

                let data = fresh_value(&mut next_value, pointer)?;
                legalized.push(Operation::effect_free(
                    data.clone(),
                    OperationKind::SliceData {
                        slice: arguments[0],
                    },
                ));
                legalized.push(Operation::effect_free(
                    operation.results[1].clone(),
                    OperationKind::GetElementPointer {
                        base: data.id,
                        offset: arguments[1],
                    },
                ));
                continue;
            }

            return Err(reject(format!(
                "fill legalization does not support call `{callee}`; no alternate codegen route was attempted"
            )));
        }
        block.operations = legalized;
    }

    if thread_calls != 1 || get_mut_calls != 1 {
        return Err(reject(format!(
            "fill legalization requires exactly one trusted thread::index_1d call and one trusted DisjointSlice::get_mut call; found {thread_calls} and {get_mut_calls}"
        )));
    }
    if thread_index != get_mut_index {
        return Err(reject(format!(
            "fill DisjointSlice::get_mut must use the exact trusted global thread index; found thread result {thread_index:?} and get_mut index {get_mut_index:?}"
        )));
    }

    let thread_index = thread_index.expect("exact call count checked");
    let output_pointer = output_pointer.expect("exact get_mut call count checked");
    let output_condition = *option_conditions
        .iter()
        .next()
        .expect("exact get_mut call count checked");
    let [expected_output] = body.parameters.as_slice() else {
        unreachable!("fill signature and parameter count checked")
    };
    let expected_output = *expected_output;
    if output_slice != Some(expected_output) {
        return Err(reject(format!(
            "fill DisjointSlice::get_mut must derive its pointer from output parameter {expected_output}; found {output_slice:?}"
        )));
    }

    legalize_option_switches(body, "fill terminal profile", &option_conditions)?;
    require_exact_fill_shape(
        body,
        expected_output,
        thread_index,
        output_pointer,
        output_condition,
    )
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn require_exact_fill_shape(
    body: &FunctionBody,
    output: ValueId,
    thread_index: ValueId,
    output_pointer: ValueId,
    output_condition: ValueId,
) -> Result<(), EmitError> {
    let mut thread_intrinsics = Vec::new();
    let mut lengths = Vec::new();
    let mut data_pointers = Vec::new();
    let mut geps = Vec::new();
    let mut compares = Vec::new();
    let mut fill_values = Vec::new();
    let mut stores = Vec::new();

    for block in &body.blocks {
        if !block.parameters.is_empty() {
            return Err(reject(format!(
                "fill terminal profile does not permit block parameters in {}",
                block.id
            )));
        }
        for operation in &block.operations {
            match &operation.kind {
                OperationKind::Intrinsic(intrinsic)
                    if intrinsic == &IntrinsicOperation::global_id_1d() =>
                {
                    thread_intrinsics
                        .push((fill_single_result(operation, &Type::INDEX)?, block.id));
                }
                OperationKind::SliceLength { slice } => lengths.push((
                    fill_single_result(operation, &Type::INDEX)?,
                    *slice,
                    block.id,
                )),
                OperationKind::SliceData { slice } => data_pointers.push((
                    fill_single_result(operation, &writable_f32_pointer())?,
                    *slice,
                    block.id,
                )),
                OperationKind::GetElementPointer { base, offset } => geps.push((
                    fill_single_result(operation, &writable_f32_pointer())?,
                    *base,
                    *offset,
                    block.id,
                )),
                OperationKind::Compare {
                    predicate,
                    lhs,
                    rhs,
                } => compares.push((
                    fill_single_result(operation, &Type::BOOL)?,
                    *predicate,
                    *lhs,
                    *rhs,
                    block.id,
                )),
                OperationKind::Constant(Constant::F32Bits(bits)) => {
                    fill_values.push((fill_single_result(operation, &Type::F32)?, *bits, block.id))
                }
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                } => {
                    if !operation.results.is_empty() {
                        return Err(reject("fill store must not produce kernel IR results"));
                    }
                    stores.push((*pointer, *value, *access, block.id));
                }
                other => {
                    return Err(reject(format!(
                        "fill contains unsupported operation {other:?}; no alternate codegen route was attempted"
                    )));
                }
            }
        }
    }

    let [(observed_thread_index, thread_block)] = thread_intrinsics.as_slice() else {
        return Err(reject(format!(
            "fill requires exactly one legalized global thread intrinsic; found {}",
            thread_intrinsics.len()
        )));
    };
    if *observed_thread_index != thread_index {
        return Err(reject(
            "fill global thread intrinsic does not preserve the authenticated thread index",
        ));
    }
    let [(length, length_slice, length_block)] = lengths.as_slice() else {
        return Err(reject(format!(
            "fill requires exactly one output length projection; found {}",
            lengths.len()
        )));
    };
    let [(data, data_slice, data_block)] = data_pointers.as_slice() else {
        return Err(reject(format!(
            "fill requires exactly one output data projection; found {}",
            data_pointers.len()
        )));
    };
    let [(gep, gep_base, gep_offset, gep_block)] = geps.as_slice() else {
        return Err(reject(format!(
            "fill requires exactly one output element pointer; found {}",
            geps.len()
        )));
    };
    if *length_slice != output
        || *data_slice != output
        || *gep != output_pointer
        || *gep_base != *data
        || *gep_offset != thread_index
        || length_block != data_block
        || data_block != gep_block
    {
        return Err(reject(
            "fill output length, data, and element pointer must derive solely from the authenticated output and global thread index",
        ));
    }

    let [(condition, predicate, compare_lhs, compare_rhs, compare_block)] = compares.as_slice()
    else {
        return Err(reject(format!(
            "fill requires exactly one output bounds comparison; found {}",
            compares.len()
        )));
    };
    if (*condition, *predicate, *compare_lhs, *compare_rhs)
        != (
            output_condition,
            ComparePredicate::LessThan,
            thread_index,
            *length,
        )
        || compare_block != length_block
    {
        return Err(reject(
            "fill bounds comparison must guard the authenticated global index against the exact output length",
        ));
    }

    let [(fill_value, fill_bits, fill_value_block)] = fill_values.as_slice() else {
        return Err(reject(format!(
            "fill requires exactly one f32 fill value; found {}",
            fill_values.len()
        )));
    };
    if *fill_bits != 42.5f32.to_bits() {
        return Err(reject(
            "fill value must remain the exact reviewed 42.5f32 bit pattern",
        ));
    }
    let [(store_pointer, store_value, store_access, store_block)] = stores.as_slice() else {
        return Err(reject(format!(
            "fill requires exactly one disjoint f32 store; found {}",
            stores.len()
        )));
    };
    if (*store_pointer, *store_value, *store_access)
        != (
            output_pointer,
            *fill_value,
            MemoryAccess::new(AddressSpace::Global, 4),
        )
        || fill_value_block != store_block
    {
        return Err(reject(
            "fill store must write the exact reviewed f32 value through the authenticated disjoint output pointer with aligned non-volatile global access",
        ));
    }

    let mut branches = BTreeMap::new();
    let mut conditional_branches = Vec::new();
    let mut return_blocks = Vec::new();
    let mut unreachable_blocks = Vec::new();
    for block in &body.blocks {
        match block.terminator.as_ref().expect("verified terminator") {
            Terminator::Branch { target, arguments } if arguments.is_empty() => {
                branches.insert(block.id, *target);
            }
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } if then_arguments.is_empty() && else_arguments.is_empty() => {
                conditional_branches.push((block.id, *condition, *then_target, *else_target));
            }
            Terminator::Return { values } if values.is_empty() => return_blocks.push(block.id),
            Terminator::Unreachable => unreachable_blocks.push(block.id),
            terminator => {
                return Err(reject(format!(
                    "fill contains unsupported terminator {terminator:?}; no alternate codegen route was attempted"
                )));
            }
        }
    }
    let [(condition_block, branch_condition, then_target, else_target)] =
        conditional_branches.as_slice()
    else {
        return Err(reject(format!(
            "fill requires exactly one output-admission branch; found {}",
            conditional_branches.len()
        )));
    };
    let [return_block] = return_blocks.as_slice() else {
        return Err(reject(format!(
            "fill requires exactly one return block; found {}",
            return_blocks.len()
        )));
    };
    let [unreachable_block] = unreachable_blocks.as_slice() else {
        return Err(reject(format!(
            "fill requires exactly one retained Option trap block; found {}",
            unreachable_blocks.len()
        )));
    };
    let expected_blocks = [
        *thread_block,
        *length_block,
        *condition_block,
        *store_block,
        *return_block,
        *unreachable_block,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let actual_blocks = body
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    if body.blocks.len() != 6
        || expected_blocks.len() != 6
        || actual_blocks != expected_blocks
        || branches.len() != 3
        || branches.get(thread_block) != Some(length_block)
        || branches.get(length_block) != Some(condition_block)
        || branches.get(store_block) != Some(return_block)
        || *branch_condition != output_condition
        || *then_target != *store_block
        || *else_target != *return_block
    {
        return Err(reject(
            "fill control-flow edges do not match global index, checked disjoint output, exact store, retained trap, and return",
        ));
    }
    Ok(())
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn fill_single_result(operation: &Operation, expected_type: &Type) -> Result<ValueId, EmitError> {
    let [result] = operation.results.as_slice() else {
        return Err(reject(format!(
            "fill operation {:?} must have exactly one result",
            operation.kind
        )));
    };
    if &result.ty != expected_type {
        return Err(reject(format!(
            "fill operation {:?} has result type {:?}; expected {expected_type:?}",
            operation.kind, result.ty
        )));
    }
    Ok(result.id)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn legalize_vecadd_body(body: &mut FunctionBody, parameters: &[Type]) -> Result<(), EmitError> {
    if body.parameters.len() != parameters.len() {
        return Err(reject(
            "vecadd entry parameter identities do not match its signature",
        ));
    }

    let value_types = collect_value_types(body, parameters);
    let mut next_value = value_types.keys().next_back().map_or(Ok(0), |value| {
        value
            .0
            .checked_add(1)
            .ok_or_else(|| reject("vecadd kernel exhausted kernel IR value identities"))
    })?;
    let mut thread_index = None;
    let mut read_index = None;
    let mut identity_zero = None;
    let mut get_mut_index = None;
    let mut output_slice = None;
    let mut output_pointer = None;
    let mut option_conditions = BTreeSet::new();
    let mut thread_calls = 0usize;
    let mut get_calls = 0usize;
    let mut get_mut_calls = 0usize;

    for block in &mut body.blocks {
        let mut legalized = Vec::with_capacity(block.operations.len() + 6);
        for operation in std::mem::take(&mut block.operations) {
            let OperationKind::Call { callee, arguments } = &operation.kind else {
                legalized.push(operation);
                continue;
            };

            if callee.as_str() == TrustedDeviceItem::ThreadIndex1d.canonical_path() {
                require_call_shape(
                    "thread::index_1d",
                    &operation,
                    arguments,
                    &[],
                    &[Type::INDEX],
                    &value_types,
                )?;
                thread_calls += 1;
                thread_index = Some(operation.results[0].id);
                legalized.push(Operation::effect_free(
                    operation.results[0].clone(),
                    OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                ));
                continue;
            }

            if callee.as_str() == TrustedDeviceItem::ThreadIndexGet.canonical_path() {
                require_call_shape(
                    "ThreadIndex::get",
                    &operation,
                    arguments,
                    &[Type::INDEX],
                    &[Type::INDEX],
                    &value_types,
                )?;
                get_calls += 1;
                read_index = Some(operation.results[0].id);
                let zero = fresh_value(&mut next_value, Type::INDEX)?;
                identity_zero = Some(zero.id);
                legalized.push(Operation::effect_free(
                    zero.clone(),
                    OperationKind::Constant(Constant::Index(0)),
                ));
                legalized.push(Operation::effect_free(
                    operation.results[0].clone(),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: arguments[0],
                        rhs: zero.id,
                    },
                ));
                continue;
            }

            if callee.as_str() == TrustedDeviceItem::DisjointSliceGetMut.canonical_path() {
                let pointer = writable_f32_pointer();
                require_call_shape(
                    "DisjointSlice::get_mut",
                    &operation,
                    arguments,
                    &[writable_f32_slice(), Type::INDEX],
                    &[Type::INDEX, pointer.clone()],
                    &value_types,
                )?;
                get_mut_calls += 1;
                output_slice = Some(arguments[0]);
                get_mut_index = Some(arguments[1]);

                let length = fresh_value(&mut next_value, Type::INDEX)?;
                legalized.push(Operation::effect_free(
                    length.clone(),
                    OperationKind::SliceLength {
                        slice: arguments[0],
                    },
                ));

                let condition = ValueDef::new(operation.results[0].id, Type::BOOL);
                option_conditions.insert(condition.id);
                legalized.push(Operation::effect_free(
                    condition,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: arguments[1],
                        rhs: length.id,
                    },
                ));

                let data = fresh_value(&mut next_value, pointer)?;
                legalized.push(Operation::effect_free(
                    data.clone(),
                    OperationKind::SliceData {
                        slice: arguments[0],
                    },
                ));
                output_pointer = Some(operation.results[1].id);
                legalized.push(Operation::effect_free(
                    operation.results[1].clone(),
                    OperationKind::GetElementPointer {
                        base: data.id,
                        offset: arguments[1],
                    },
                ));
                continue;
            }

            return Err(reject(format!(
                "vecadd legalization does not support call `{callee}`; no alternate codegen route was attempted"
            )));
        }
        block.operations = legalized;
    }

    if thread_calls != 1 || get_calls != 1 || get_mut_calls != 1 {
        return Err(reject(format!(
            "vecadd legalization requires exactly one trusted thread::index_1d call, one trusted ThreadIndex::get call, and one trusted DisjointSlice::get_mut call; found {thread_calls}, {get_calls}, and {get_mut_calls}"
        )));
    }
    if thread_index != get_mut_index {
        return Err(reject(format!(
            "vecadd DisjointSlice::get_mut must consume the exact trusted global thread index; found thread result {thread_index:?} and get_mut index {get_mut_index:?}"
        )));
    }
    let thread_index = thread_index.expect("exact call count checked");
    let read_index = read_index.expect("exact call count checked");
    let identity_zero = identity_zero.expect("exact call count checked");
    let output_pointer = output_pointer.expect("exact call count checked");
    let output_condition = *option_conditions
        .iter()
        .next()
        .expect("exact get_mut call count checked");
    let [first_input, second_input, expected_output] = body.parameters.as_slice() else {
        unreachable!("vecadd signature and parameter count checked")
    };
    let parameters = [*first_input, *second_input, *expected_output];
    if output_slice != Some(*expected_output) {
        return Err(reject(format!(
            "vecadd DisjointSlice::get_mut must derive its pointer from output parameter {expected_output}; found {output_slice:?}"
        )));
    }

    legalize_option_switches(body, "vecadd terminal profile", &option_conditions)?;
    require_exact_vecadd_shape(
        body,
        parameters,
        thread_index,
        read_index,
        identity_zero,
        output_pointer,
        output_condition,
    )
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn require_exact_vecadd_shape(
    body: &FunctionBody,
    parameters: [ValueId; 3],
    thread_index: ValueId,
    read_index: ValueId,
    identity_zero: ValueId,
    output_pointer: ValueId,
    output_condition: ValueId,
) -> Result<(), EmitError> {
    let mut lengths = BTreeMap::new();
    let mut data_pointers = BTreeMap::new();
    let mut geps = BTreeMap::new();
    let mut loads = BTreeMap::new();
    let mut compares = Vec::new();
    let mut float_adds = Vec::new();
    let mut stores = Vec::new();
    let mut result_blocks = BTreeMap::new();
    let mut store_block = None;
    let mut saw_thread_intrinsic = false;
    let mut saw_identity_zero = false;
    let mut saw_index_identity = false;

    for block in &body.blocks {
        for operation in &block.operations {
            for result in &operation.results {
                result_blocks.insert(result.id, block.id);
            }
            match &operation.kind {
                OperationKind::Intrinsic(intrinsic)
                    if intrinsic == &IntrinsicOperation::global_id_1d()
                        && operation.results.len() == 1
                        && operation.results[0].id == thread_index =>
                {
                    single_result(operation, &Type::INDEX)?;
                    if saw_thread_intrinsic {
                        return Err(reject("vecadd contains duplicate global thread intrinsics"));
                    }
                    saw_thread_intrinsic = true;
                }
                OperationKind::Constant(Constant::Index(0))
                    if operation.results.len() == 1 && operation.results[0].id == identity_zero =>
                {
                    single_result(operation, &Type::INDEX)?;
                    if saw_identity_zero {
                        return Err(reject("vecadd contains duplicate index identity constants"));
                    }
                    saw_identity_zero = true;
                }
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs,
                    rhs,
                } if operation.results.len() == 1
                    && operation.results[0].id == read_index
                    && *lhs == thread_index
                    && *rhs == identity_zero =>
                {
                    single_result(operation, &Type::INDEX)?;
                    if saw_index_identity {
                        return Err(reject("vecadd contains duplicate thread-index identities"));
                    }
                    saw_index_identity = true;
                }
                OperationKind::SliceLength { slice } => {
                    insert_unique(
                        &mut lengths,
                        *slice,
                        single_result(operation, &Type::INDEX)?,
                        "slice length",
                    )?;
                }
                OperationKind::SliceData { slice } => {
                    let expected_type = if *slice == parameters[2] {
                        writable_f32_pointer()
                    } else {
                        readonly_f32_pointer()
                    };
                    insert_unique(
                        &mut data_pointers,
                        *slice,
                        single_result(operation, &expected_type)?,
                        "slice data",
                    )?;
                }
                OperationKind::GetElementPointer { base, offset } => {
                    let result = operation
                        .results
                        .first()
                        .ok_or_else(|| reject("vecadd GEP has no result"))?;
                    if operation.results.len() != 1 {
                        return Err(reject("vecadd GEP must have exactly one result"));
                    }
                    geps.insert(result.id, (*base, *offset, result.ty.clone()));
                }
                OperationKind::Load { pointer, access } => {
                    let result = single_result(operation, &Type::F32)?;
                    loads.insert(result, (*pointer, *access));
                }
                OperationKind::Compare {
                    predicate,
                    lhs,
                    rhs,
                } => compares.push((
                    single_result(operation, &Type::BOOL)?,
                    *predicate,
                    *lhs,
                    *rhs,
                )),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs,
                    rhs,
                } if operation
                    .results
                    .as_slice()
                    .first()
                    .is_some_and(|result| result.ty == Type::F32) =>
                {
                    float_adds.push((single_result(operation, &Type::F32)?, *lhs, *rhs));
                }
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                } => {
                    stores.push((*pointer, *value, *access));
                    store_block = Some(block.id);
                }
                other => {
                    return Err(reject(format!(
                        "vecadd contains unsupported operation {other:?}; no alternate codegen route was attempted"
                    )));
                }
            }
        }
    }

    if !saw_thread_intrinsic || !saw_identity_zero || !saw_index_identity {
        return Err(reject(
            "vecadd did not preserve its trusted global thread-index dataflow",
        ));
    }
    if lengths.len() != 3 || data_pointers.len() != 3 {
        return Err(reject(format!(
            "vecadd requires one length and one data projection for each slice; found {} lengths and {} data projections",
            lengths.len(),
            data_pointers.len()
        )));
    }
    let first_length = require_map_value(&lengths, parameters[0], "first input length")?;
    let second_length = require_map_value(&lengths, parameters[1], "second input length")?;
    let output_length = require_map_value(&lengths, parameters[2], "output length")?;
    let first_data = require_map_value(&data_pointers, parameters[0], "first input data")?;
    let second_data = require_map_value(&data_pointers, parameters[1], "second input data")?;
    let output_data = require_map_value(&data_pointers, parameters[2], "output data")?;
    let input_access = MemoryAccess::new(AddressSpace::Global, 4);
    let first_gep = require_gep(
        &geps,
        first_data,
        read_index,
        &readonly_f32_pointer(),
        "first input",
    )?;
    let second_gep = require_gep(
        &geps,
        second_data,
        read_index,
        &readonly_f32_pointer(),
        "second input",
    )?;
    let expected_output_pointer = require_gep(
        &geps,
        output_data,
        thread_index,
        &writable_f32_pointer(),
        "output",
    )?;
    if geps.len() != 3 || expected_output_pointer != output_pointer {
        return Err(reject(
            "vecadd output store pointer was not derived solely from trusted DisjointSlice::get_mut",
        ));
    }

    if loads.len() != 2 {
        return Err(reject(format!(
            "vecadd requires exactly two f32 loads; found {}",
            loads.len()
        )));
    }
    let first_load = require_load(&loads, first_gep, input_access, "first input")?;
    let second_load = require_load(&loads, second_gep, input_access, "second input")?;
    let [(sum, lhs, rhs)] = float_adds.as_slice() else {
        return Err(reject(format!(
            "vecadd requires exactly one f32 add; found {}",
            float_adds.len()
        )));
    };
    if (*lhs, *rhs) != (first_load, second_load) {
        return Err(reject(
            "vecadd f32 add must combine the first and second input loads in parameter order",
        ));
    }
    let [(store_pointer, store_value, store_access)] = stores.as_slice() else {
        return Err(reject(format!(
            "vecadd requires exactly one disjoint f32 store; found {}",
            stores.len()
        )));
    };
    if (*store_pointer, *store_value, *store_access)
        != (
            output_pointer,
            *sum,
            MemoryAccess::new(AddressSpace::Global, 4),
        )
    {
        return Err(reject(
            "vecadd store must write the f32 sum through the exact disjoint output pointer with aligned non-volatile global access",
        ));
    }

    let first_condition = find_compare(&compares, read_index, first_length, "first input")?;
    let second_condition = find_compare(&compares, read_index, second_length, "second input")?;
    let expected_compares = [
        (
            output_condition,
            ComparePredicate::LessThan,
            thread_index,
            output_length,
        ),
        (
            first_condition,
            ComparePredicate::LessThan,
            read_index,
            first_length,
        ),
        (
            second_condition,
            ComparePredicate::LessThan,
            read_index,
            second_length,
        ),
    ];
    let compare_set = compares.iter().copied().collect::<BTreeSet<_>>();
    if compares.len() != 3 || compare_set != expected_compares.into_iter().collect() {
        return Err(reject(
            "vecadd requires exactly the output admission check and both input bounds checks",
        ));
    }

    let expected_conditions = expected_compares
        .iter()
        .map(|(condition, ..)| *condition)
        .collect::<BTreeSet<_>>();
    let mut condition_targets = BTreeMap::new();
    let mut return_blocks = BTreeSet::new();
    let mut unreachable_blocks = BTreeSet::new();
    for block in &body.blocks {
        match block.terminator.as_ref().expect("verified terminator") {
            Terminator::ConditionalBranch {
                condition,
                then_target,
                else_target,
                ..
            } => {
                if condition_targets
                    .insert(*condition, (*then_target, *else_target))
                    .is_some()
                {
                    return Err(reject(format!(
                        "vecadd branches more than once on condition {condition}"
                    )));
                }
            }
            Terminator::Branch { .. } => {}
            Terminator::Return { values } if values.is_empty() => {
                return_blocks.insert(block.id);
            }
            Terminator::Unreachable => {
                unreachable_blocks.insert(block.id);
            }
            terminator => {
                return Err(reject(format!(
                    "vecadd contains unsupported terminator {terminator:?}; no alternate codegen route was attempted"
                )));
            }
        }
    }
    if condition_targets.len() != 3
        || condition_targets.keys().copied().collect::<BTreeSet<_>>() != expected_conditions
        || return_blocks.len() != 1
        || unreachable_blocks.is_empty()
    {
        return Err(reject(format!(
            "vecadd control flow must branch once on output admission and once per input bound, with one return and at least one trap; found conditions {:?}, {} returns, and {} traps",
            condition_targets.keys().collect::<Vec<_>>(),
            return_blocks.len(),
            unreachable_blocks.len()
        )));
    }
    let return_block = *return_blocks.iter().next().expect("one return checked");
    let first_bounds_block = require_map_value(
        &result_blocks,
        first_condition,
        "first bounds operation block",
    )?;
    let first_load_block =
        require_map_value(&result_blocks, first_load, "first load operation block")?;
    let second_bounds_block = require_map_value(
        &result_blocks,
        second_condition,
        "second bounds operation block",
    )?;
    let second_load_block =
        require_map_value(&result_blocks, second_load, "second load operation block")?;
    let store_block = store_block.expect("one store checked");
    let output_targets = require_map_value(
        &condition_targets,
        output_condition,
        "output branch targets",
    )?;
    let first_targets = require_map_value(
        &condition_targets,
        first_condition,
        "first bounds branch targets",
    )?;
    let second_targets = require_map_value(
        &condition_targets,
        second_condition,
        "second bounds branch targets",
    )?;
    if output_targets != (first_bounds_block, return_block)
        || first_targets.0 != first_load_block
        || !unreachable_blocks.contains(&first_targets.1)
        || first_load_block != second_bounds_block
        || second_targets.0 != second_load_block
        || !unreachable_blocks.contains(&second_targets.1)
        || second_load_block != store_block
        || !matches!(
            block(body, store_block).terminator.as_ref(),
            Some(Terminator::Branch { target, arguments })
                if *target == return_block && arguments.is_empty()
        )
    {
        return Err(reject(
            "vecadd control-flow edges do not match output admission, ordered input bounds checks, compute, trap, and return",
        ));
    }
    Ok(())
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn block(body: &FunctionBody, id: BlockId) -> &BasicBlock {
    body.blocks
        .iter()
        .find(|block| block.id == id)
        .expect("verified branch target")
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn single_result(operation: &Operation, expected_type: &Type) -> Result<ValueId, EmitError> {
    let [result] = operation.results.as_slice() else {
        return Err(reject(format!(
            "vecadd operation {:?} must have exactly one result",
            operation.kind
        )));
    };
    if &result.ty != expected_type {
        return Err(reject(format!(
            "vecadd operation {:?} has result type {:?}; expected {expected_type:?}",
            operation.kind, result.ty
        )));
    }
    Ok(result.id)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn insert_unique(
    values: &mut BTreeMap<ValueId, ValueId>,
    key: ValueId,
    value: ValueId,
    label: &str,
) -> Result<(), EmitError> {
    if values.insert(key, value).is_some() {
        return Err(reject(format!(
            "vecadd contains duplicate {label} operations for {key}"
        )));
    }
    Ok(())
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn require_map_value<T: Copy>(
    values: &BTreeMap<ValueId, T>,
    key: ValueId,
    label: &str,
) -> Result<T, EmitError> {
    values
        .get(&key)
        .copied()
        .ok_or_else(|| reject(format!("vecadd is missing {label} for {key}")))
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn require_gep(
    geps: &BTreeMap<ValueId, (ValueId, ValueId, Type)>,
    base: ValueId,
    offset: ValueId,
    ty: &Type,
    label: &str,
) -> Result<ValueId, EmitError> {
    let matches = geps
        .iter()
        .filter(|(_, candidate)| candidate == &&(base, offset, ty.clone()))
        .map(|(result, _)| *result)
        .collect::<Vec<_>>();
    let [result] = matches.as_slice() else {
        return Err(reject(format!(
            "vecadd requires exactly one {label} element pointer at the trusted read/write index; found {}",
            matches.len()
        )));
    };
    Ok(*result)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn require_load(
    loads: &BTreeMap<ValueId, (ValueId, MemoryAccess)>,
    pointer: ValueId,
    access: MemoryAccess,
    label: &str,
) -> Result<ValueId, EmitError> {
    let matches = loads
        .iter()
        .filter(|(_, candidate)| candidate == &&(pointer, access))
        .map(|(result, _)| *result)
        .collect::<Vec<_>>();
    let [result] = matches.as_slice() else {
        return Err(reject(format!(
            "vecadd requires exactly one aligned non-volatile global load from the {label}; found {}",
            matches.len()
        )));
    };
    Ok(*result)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn find_compare(
    compares: &[(ValueId, ComparePredicate, ValueId, ValueId)],
    lhs: ValueId,
    rhs: ValueId,
    label: &str,
) -> Result<ValueId, EmitError> {
    let matches = compares
        .iter()
        .filter(|(_, predicate, candidate_lhs, candidate_rhs)| {
            *predicate == ComparePredicate::LessThan
                && *candidate_lhs == lhs
                && *candidate_rhs == rhs
        })
        .map(|(result, ..)| *result)
        .collect::<Vec<_>>();
    let [result] = matches.as_slice() else {
        return Err(reject(format!(
            "vecadd requires exactly one {label} bounds comparison; found {}",
            matches.len()
        )));
    };
    Ok(*result)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn legalize_option_switches(
    body: &mut FunctionBody,
    kernel_name: &str,
    option_conditions: &BTreeSet<ValueId>,
) -> Result<(), EmitError> {
    let unreachable_blocks = body
        .blocks
        .iter()
        .filter(|block| {
            block.parameters.is_empty()
                && block.operations.is_empty()
                && matches!(block.terminator, Some(Terminator::Unreachable))
        })
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let mut option_switches = 0usize;
    for block in &mut body.blocks {
        let Some(Terminator::Switch {
            selector,
            cases,
            default_target,
            default_arguments,
        }) = block.terminator.as_ref()
        else {
            continue;
        };
        if !option_conditions.contains(selector) {
            return Err(reject(format!(
                "{kernel_name} contains unsupported non-Option switch in {}",
                block.id
            )));
        }
        if cases.len() != 2
            || cases.iter().any(|case| !case.arguments.is_empty())
            || !default_arguments.is_empty()
            || !unreachable_blocks.contains(default_target)
        {
            return Err(reject(format!(
                "{kernel_name} Option switch in {} must have cases 0 and 1 with an unreachable default and no block arguments",
                block.id
            )));
        }
        let false_target = cases
            .iter()
            .find(|case| case.value == 0)
            .map(|case| case.target);
        let true_target = cases
            .iter()
            .find(|case| case.value == 1)
            .map(|case| case.target);
        let (Some(false_target), Some(true_target)) = (false_target, true_target) else {
            return Err(reject(format!(
                "{kernel_name} Option switch in {} must contain exactly discriminants 0 and 1",
                block.id
            )));
        };
        block.terminator = Some(Terminator::ConditionalBranch {
            condition: *selector,
            then_target: true_target,
            then_arguments: Vec::new(),
            else_target: false_target,
            else_arguments: Vec::new(),
        });
        option_switches += 1;
    }
    if option_switches != 1 {
        return Err(reject(format!(
            "{kernel_name} legalization requires exactly one Option switch; found {option_switches}"
        )));
    }
    Ok(())
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn collect_value_types(body: &FunctionBody, parameters: &[Type]) -> BTreeMap<ValueId, Type> {
    let mut types = body
        .parameters
        .iter()
        .copied()
        .zip(parameters.iter().cloned())
        .collect::<BTreeMap<_, _>>();
    for block in &body.blocks {
        for value in block.parameters.iter().chain(
            block
                .operations
                .iter()
                .flat_map(|operation| &operation.results),
        ) {
            types.insert(value.id, value.ty.clone());
        }
    }
    types
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn require_call_shape(
    name: &str,
    operation: &Operation,
    arguments: &[ValueId],
    expected_arguments: &[Type],
    expected_results: &[Type],
    value_types: &BTreeMap<ValueId, Type>,
) -> Result<(), EmitError> {
    let argument_types = arguments
        .iter()
        .map(|argument| value_types.get(argument).cloned())
        .collect::<Option<Vec<_>>>();
    let result_types = operation
        .results
        .iter()
        .map(|result| result.ty.clone())
        .collect::<Vec<_>>();
    if argument_types.as_deref() != Some(expected_arguments) || result_types != expected_results {
        return Err(reject(format!(
            "trusted {name} call has unsupported kernel IR signature {:?} -> {result_types:?}; expected {expected_arguments:?} -> {expected_results:?}",
            argument_types.unwrap_or_default()
        )));
    }
    Ok(())
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn fresh_value(next: &mut u32, ty: Type) -> Result<ValueDef, EmitError> {
    let value = ValueDef::new(ValueId(*next), ty);
    *next = next
        .checked_add(1)
        .ok_or_else(|| reject("kernel exhausted kernel IR value identities"))?;
    Ok(value)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn readonly_f32_slice() -> Type {
    Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn writable_f32_slice() -> Type {
    Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn readonly_f32_pointer() -> Type {
    Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn writable_f32_pointer() -> Type {
    Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite)
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
fn reject(reason: impl Into<String>) -> EmitError {
    EmitError::Preflight {
        reason: format!(
            "{QUALIFICATION_ORACLE_ENV}=kernel-ir-v1 production path rejected input: {}",
            reason.into()
        ),
    }
}

#[cfg(all(test, feature = "qualification-oracles-test-only"))]
mod tests {
    use super::*;
    use fe2o3_compiler_ffi::CompilerDescriptorSourceV1;
    use fe2o3_kernel_descriptor::{
        BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CodeObjectVersion,
        CompilerIdentityV1, DeviceDescriptorTableV1, DeviceLayoutDescriptorV1,
        DeviceLayoutRecordV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
        KernelAbiLayoutV1, KernelDescriptorV1, KernelId as DescriptorKernelId, LaunchConstraintsV1,
        LogicalArgumentV1, ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1,
        SourceTypeRecordV1, Text, ValidName,
    };
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Constant, Function, FunctionId, LaunchDomain, LaunchExtent,
        MemoryAccess, Signature, SwitchCase,
    };

    const FILL_KERNEL: &str = "fill";
    const VECADD_KERNEL: &str = "vecadd";

    fn descriptor_source(entry: &str, descriptor: &str) -> CompilerDescriptorSourceV1 {
        let source_type =
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
        let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
        let argument = LogicalArgumentV1::scalar(
            0,
            ValidName::new("value").unwrap(),
            &source_type,
            &layout,
            0,
        )
        .unwrap();
        let evidence = |byte| {
            BuildEvidenceV1::new(
                EvidenceIdentity::from_opaque_bytes([byte; 32]),
                EvidenceDigest::from_sha256_bytes([byte.wrapping_add(1); 32]),
            )
        };
        let kernel = KernelDescriptorV1::new(
            DescriptorKernelId::from_bytes([0x11; 32]),
            ValidName::new(entry).unwrap(),
            ValidName::new(entry).unwrap(),
            ValidName::new(descriptor).unwrap(),
            evidence(0x21),
            evidence(0x31),
            vec![],
            KernelAbiLayoutV1::new(4, 4, 4).unwrap(),
            LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
                DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
                256,
                0,
                0,
            )
            .unwrap(),
            vec![argument],
        )
        .unwrap();
        let table = DeviceDescriptorTableV1::new(
            CanonicalCodeObjectDigest::from_bytes([0; 32]),
            CodeObjectVersion::V6,
            CompilerIdentityV1::new(
                Text::new("rustc-codegen-fe2o3").unwrap(),
                Text::new("test").unwrap(),
                [0x41; 20],
            ),
            ProducerIdentityV1::new(
                Text::new("rustc-codegen-fe2o3").unwrap(),
                Text::new("test").unwrap(),
            ),
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            vec![source_type],
            vec![layout],
            vec![kernel],
        )
        .unwrap();
        CompilerDescriptorSourceV1::new(table).unwrap()
    }

    fn embedded_descriptor_bytes(llvm_ir: &str) -> Vec<u8> {
        llvm_ir
            .lines()
            .filter_map(|line| {
                line.strip_prefix("module asm \".byte ")
                    .and_then(|line| line.strip_suffix('"'))
            })
            .flat_map(|line| line.split(", "))
            .map(|byte| u8::from_str_radix(byte.strip_prefix("0x").unwrap(), 16).unwrap())
            .collect()
    }

    fn translated_fill() -> Module {
        let slice = writable_f32_slice();
        let pointer = writable_f32_pointer();

        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(1), Type::INDEX),
            OperationKind::Call {
                callee: FunctionId::new(TrustedDeviceItem::ThreadIndex1d.canonical_path()),
                arguments: vec![],
            },
        ));
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });

        let mut get_mut = BasicBlock::new(BlockId(1));
        get_mut.operations.push(Operation::new(
            vec![
                ValueDef::new(ValueId(2), Type::INDEX),
                ValueDef::new(ValueId(3), pointer.clone()),
            ],
            OperationKind::Call {
                callee: FunctionId::new(TrustedDeviceItem::DisjointSliceGetMut.canonical_path()),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ));
        get_mut.terminator = Some(Terminator::Branch {
            target: BlockId(2),
            arguments: vec![],
        });

        let mut select = BasicBlock::new(BlockId(2));
        select.terminator = Some(Terminator::Switch {
            selector: ValueId(2),
            cases: vec![
                SwitchCase {
                    value: 0,
                    target: BlockId(4),
                    arguments: vec![],
                },
                SwitchCase {
                    value: 1,
                    target: BlockId(3),
                    arguments: vec![],
                },
            ],
            default_target: BlockId(5),
            default_arguments: vec![],
        });

        let mut store = BasicBlock::new(BlockId(3));
        store.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(4), Type::F32),
            OperationKind::Constant(Constant::F32Bits(42.5f32.to_bits())),
        ));
        store.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        store.terminator = Some(Terminator::Branch {
            target: BlockId(4),
            arguments: vec![],
        });

        let mut exit = BasicBlock::new(BlockId(4));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        let mut unreachable = BasicBlock::new(BlockId(5));
        unreachable.terminator = Some(Terminator::Unreachable);

        let function = Function::kernel_entry(
            "fill_impl",
            Signature::new(vec![slice.clone()], vec![]),
            vec![ValueId(0)],
            vec![entry, get_mut, select, store, exit, unreachable],
        );
        let mut module = Module::new("tests::translated_fill");
        module.functions.push(function);
        module.functions.push(Function::declaration(
            TrustedDeviceItem::ThreadIndex1d.canonical_path(),
            Signature::new(vec![], vec![Type::INDEX]),
        ));
        module.functions.push(Function::declaration(
            TrustedDeviceItem::DisjointSliceGetMut.canonical_path(),
            Signature::new(vec![slice, Type::INDEX], vec![Type::INDEX, pointer]),
        ));
        module.kernels.push(fe2o3_kernel_ir::Kernel::new(
            FILL_KERNEL,
            "fill_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        module
    }

    fn reject_verified_fill(module: Module, case: &str) -> String {
        verify_module(&module)
            .unwrap_or_else(|errors| panic!("{case} must remain verified kernel IR: {errors}"));
        prepare_fill_collection(module, &[FILL_KERNEL.to_string()])
            .expect_err(case)
            .to_string()
    }

    fn translated_vecadd() -> Module {
        let input = readonly_f32_slice();
        let output = writable_f32_slice();
        let input_pointer = readonly_f32_pointer();
        let output_pointer = writable_f32_pointer();

        let mut index = BasicBlock::new(BlockId(0));
        index.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::Call {
                callee: FunctionId::new(TrustedDeviceItem::ThreadIndex1d.canonical_path()),
                arguments: vec![],
            },
        ));
        index.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });

        let mut read_index = BasicBlock::new(BlockId(1));
        read_index.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::Call {
                callee: FunctionId::new(TrustedDeviceItem::ThreadIndexGet.canonical_path()),
                arguments: vec![ValueId(3)],
            },
        ));
        read_index.terminator = Some(Terminator::Branch {
            target: BlockId(2),
            arguments: vec![],
        });

        let mut get_mut = BasicBlock::new(BlockId(2));
        get_mut.operations.push(Operation::new(
            vec![
                ValueDef::new(ValueId(5), Type::INDEX),
                ValueDef::new(ValueId(6), output_pointer.clone()),
            ],
            OperationKind::Call {
                callee: FunctionId::new(TrustedDeviceItem::DisjointSliceGetMut.canonical_path()),
                arguments: vec![ValueId(2), ValueId(3)],
            },
        ));
        get_mut.terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        });

        let mut select = BasicBlock::new(BlockId(3));
        select.terminator = Some(Terminator::Switch {
            selector: ValueId(5),
            cases: vec![
                SwitchCase {
                    value: 0,
                    target: BlockId(7),
                    arguments: vec![],
                },
                SwitchCase {
                    value: 1,
                    target: BlockId(4),
                    arguments: vec![],
                },
            ],
            default_target: BlockId(8),
            default_arguments: vec![],
        });

        let mut first_bounds = BasicBlock::new(BlockId(4));
        first_bounds.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(7), Type::INDEX),
                OperationKind::SliceLength { slice: ValueId(0) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(8), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(4),
                    rhs: ValueId(7),
                },
            ),
        ];
        first_bounds.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(8),
            then_target: BlockId(5),
            then_arguments: vec![],
            else_target: BlockId(9),
            else_arguments: vec![],
        });

        let mut second_bounds = BasicBlock::new(BlockId(5));
        second_bounds.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(9), input_pointer.clone()),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(10), input_pointer.clone()),
                OperationKind::GetElementPointer {
                    base: ValueId(9),
                    offset: ValueId(4),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(11), Type::F32),
                OperationKind::Load {
                    pointer: ValueId(10),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(12), Type::INDEX),
                OperationKind::SliceLength { slice: ValueId(1) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(13), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(4),
                    rhs: ValueId(12),
                },
            ),
        ];
        second_bounds.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(13),
            then_target: BlockId(6),
            then_arguments: vec![],
            else_target: BlockId(9),
            else_arguments: vec![],
        });

        let mut compute = BasicBlock::new(BlockId(6));
        compute.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(14), input_pointer.clone()),
                OperationKind::SliceData { slice: ValueId(1) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(15), input_pointer),
                OperationKind::GetElementPointer {
                    base: ValueId(14),
                    offset: ValueId(4),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(16), Type::F32),
                OperationKind::Load {
                    pointer: ValueId(15),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(17), Type::F32),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(11),
                    rhs: ValueId(16),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(6),
                    value: ValueId(17),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ];
        compute.terminator = Some(Terminator::Branch {
            target: BlockId(7),
            arguments: vec![],
        });

        let mut exit = BasicBlock::new(BlockId(7));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        let mut option_trap = BasicBlock::new(BlockId(8));
        option_trap.terminator = Some(Terminator::Unreachable);
        let mut bounds_trap = BasicBlock::new(BlockId(9));
        bounds_trap.terminator = Some(Terminator::Unreachable);

        let function = Function::kernel_entry(
            "vecadd_impl",
            Signature::new(vec![input.clone(), input.clone(), output.clone()], vec![]),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![
                index,
                read_index,
                get_mut,
                select,
                first_bounds,
                second_bounds,
                compute,
                exit,
                option_trap,
                bounds_trap,
            ],
        );
        let mut module = Module::new("tests::translated_vecadd");
        module.functions.push(function);
        module.functions.push(Function::declaration(
            TrustedDeviceItem::ThreadIndex1d.canonical_path(),
            Signature::new(vec![], vec![Type::INDEX]),
        ));
        module.functions.push(Function::declaration(
            TrustedDeviceItem::ThreadIndexGet.canonical_path(),
            Signature::new(vec![Type::INDEX], vec![Type::INDEX]),
        ));
        module.functions.push(Function::declaration(
            TrustedDeviceItem::DisjointSliceGetMut.canonical_path(),
            Signature::new(vec![output, Type::INDEX], vec![Type::INDEX, output_pointer]),
        ));
        module.kernels.push(fe2o3_kernel_ir::Kernel::new(
            VECADD_KERNEL,
            "vecadd_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        module
    }

    #[test]
    fn verified_fill_uses_g1_deterministically() {
        let first = prepare_fill_collection(translated_fill(), &[FILL_KERNEL.to_string()])
            .expect("supported fill");
        let second = prepare_fill_collection(translated_fill(), &[FILL_KERNEL.to_string()])
            .expect("supported fill");

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].name, FILL_KERNEL);
        assert_eq!(first[0].llvm_ir, second[0].llvm_ir);
        assert!(first[0].llvm_ir.contains("define amdgpu_kernel void @fill"));
        assert!(first[0].llvm_ir.contains("mul i64 %v1.group, 256"));
        assert!(first[0].llvm_ir.contains("!reqd_work_group_size !0"));
        assert!(!first[0].llvm_ir.contains("fe2o3_device"));
    }

    #[test]
    fn verified_fill_uses_the_authenticated_arbitrary_export_identity() {
        const EXPORT: &str = "renamed_genuine_fill";
        let mut first_module = translated_fill();
        first_module.kernels[0].id = KernelId::new(EXPORT);
        let mut second_module = translated_fill();
        second_module.kernels[0].id = KernelId::new(EXPORT);

        let first = prepare_fill_collection(first_module, &[EXPORT.to_string()])
            .expect("renamed verified fill profile");
        let second = prepare_fill_collection(second_module, &[EXPORT.to_string()])
            .expect("deterministic renamed verified fill profile");

        assert_eq!(first[0].name, EXPORT);
        assert_eq!(first[0].llvm_ir, second[0].llvm_ir);
        assert!(
            first[0]
                .llvm_ir
                .contains("define amdgpu_kernel void @renamed_genuine_fill")
        );
        assert!(
            !first[0]
                .llvm_ir
                .contains("define amdgpu_kernel void @fill(")
        );
    }

    #[test]
    fn collected_and_translated_kernel_identities_must_match_without_fallback() {
        let error = prepare_fill_collection(translated_fill(), &["saxpy".to_string()])
            .expect_err("collected identity must authenticate the translated KernelId");

        let text = error.to_string();
        assert!(text.contains(
            "translated kernel identity \"fill\" does not match collected kernel identity \"saxpy\""
        ));
        assert!(text.contains("production compilation does not fall back"));
    }

    #[test]
    fn authenticated_kernel_export_identity_is_bounded_before_profile_selection() {
        let oversized = "x".repeat(MAX_COMPILER_MODULE_SYMBOL_BYTES + 1);
        let mut module = translated_fill();
        module.kernels[0].id = KernelId::new(oversized.clone());

        let error = prepare_fill_collection(module, &[oversized])
            .expect_err("oversized authenticated identity must fail closed");
        let text = error.to_string();
        assert!(text.contains("authenticated kernel export identity exceeds the 256-byte"));
        assert!(text.contains("production compilation does not fall back"));
    }

    #[test]
    fn fill_rejects_missing_extra_and_wrong_stores() {
        let mut missing = translated_fill();
        missing.functions[0].body.as_mut().expect("body").blocks[3]
            .operations
            .remove(1);
        let error = reject_verified_fill(missing, "missing fill store must fail closed");
        assert!(error.contains("fill requires exactly one disjoint f32 store; found 0"));

        let mut extra = translated_fill();
        let store = extra.functions[0].body.as_ref().expect("body").blocks[3].operations[1].clone();
        extra.functions[0].body.as_mut().expect("body").blocks[3]
            .operations
            .push(store);
        let error = reject_verified_fill(extra, "extra fill store must fail closed");
        assert!(error.contains("fill requires exactly one disjoint f32 store; found 2"));

        let mut wrong_access = translated_fill();
        let OperationKind::Store { access, .. } = &mut wrong_access.functions[0]
            .body
            .as_mut()
            .expect("body")
            .blocks[3]
            .operations[1]
            .kind
        else {
            panic!("fill store")
        };
        *access = MemoryAccess::new(AddressSpace::Global, 8);
        let error = reject_verified_fill(wrong_access, "wrong fill store access must fail closed");
        assert!(error.contains("fill store must write the exact reviewed f32 value"));
    }

    #[test]
    fn fill_rejects_wrong_value_and_pointer_dataflow() {
        let mut wrong_value = translated_fill();
        let OperationKind::Constant(Constant::F32Bits(bits)) =
            &mut wrong_value.functions[0].body.as_mut().expect("body").blocks[3].operations[0].kind
        else {
            panic!("fill value")
        };
        *bits = 7.0f32.to_bits();
        let error = reject_verified_fill(wrong_value, "wrong fill value must fail closed");
        assert!(error.contains("fill value must remain the exact reviewed 42.5f32 bit pattern"));

        let mut wrong_pointer = translated_fill();
        let body = wrong_pointer.functions[0].body.as_mut().expect("body");
        body.blocks[3].operations.insert(
            1,
            Operation::effect_free(
                ValueDef::new(ValueId(10), writable_f32_pointer()),
                OperationKind::GetElementPointer {
                    base: ValueId(3),
                    offset: ValueId(1),
                },
            ),
        );
        let OperationKind::Store { pointer, .. } = &mut body.blocks[3].operations[2].kind else {
            panic!("fill store")
        };
        *pointer = ValueId(10);
        let error = reject_verified_fill(wrong_pointer, "wrong fill pointer must fail closed");
        assert!(error.contains("fill requires exactly one output element pointer; found 2"));
    }

    #[test]
    fn fill_rejects_extra_non_call_operations() {
        let mut extra_load = translated_fill();
        extra_load.functions[0].body.as_mut().expect("body").blocks[3]
            .operations
            .insert(
                1,
                Operation::effect_free(
                    ValueDef::new(ValueId(10), Type::F32),
                    OperationKind::Load {
                        pointer: ValueId(3),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                ),
            );
        let error = reject_verified_fill(extra_load, "extra fill load must fail closed");
        assert!(error.contains("fill contains unsupported operation Load"));

        let mut extra_arithmetic = translated_fill();
        extra_arithmetic.functions[0]
            .body
            .as_mut()
            .expect("body")
            .blocks[3]
            .operations
            .insert(
                1,
                Operation::effect_free(
                    ValueDef::new(ValueId(10), Type::F32),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(4),
                        rhs: ValueId(4),
                    },
                ),
            );
        let error =
            reject_verified_fill(extra_arithmetic, "extra fill arithmetic must fail closed");
        assert!(error.contains("fill contains unsupported operation Binary"));

        let mut extra_intrinsic = translated_fill();
        extra_intrinsic.functions[0]
            .body
            .as_mut()
            .expect("body")
            .blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(ValueId(10), Type::INDEX),
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            ));
        let error = reject_verified_fill(extra_intrinsic, "extra fill intrinsic must fail closed");
        assert!(error.contains("fill requires exactly one legalized global thread intrinsic"));
    }

    #[test]
    fn fill_rejects_control_flow_substitution() {
        let mut module = translated_fill();
        module.functions[0].body.as_mut().expect("body").blocks[3].terminator =
            Some(Terminator::Branch {
                target: BlockId(5),
                arguments: vec![],
            });
        let error = reject_verified_fill(module, "fill branch substitution must fail closed");
        assert!(error.contains("fill control-flow edges do not match"));
    }

    #[test]
    fn verified_vecadd_uses_exact_three_slice_g1_lowering_deterministically() {
        let first = prepare_fill_collection(translated_vecadd(), &[VECADD_KERNEL.to_string()])
            .expect("supported vecadd");
        let second = prepare_fill_collection(translated_vecadd(), &[VECADD_KERNEL.to_string()])
            .expect("supported vecadd");

        let [kernel] = first.as_slice() else {
            panic!("one vecadd kernel")
        };
        assert_eq!(first[0].name, second[0].name);
        assert_eq!(first[0].llvm_ir, second[0].llvm_ir);
        assert_eq!(kernel.name, VECADD_KERNEL);
        assert!(kernel.llvm_ir.contains(
            "@vecadd(ptr addrspace(1) %arg0.data, i64 %arg0.len, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len)"
        ));
        assert_eq!(kernel.llvm_ir.matches("load float").count(), 2);
        assert_eq!(kernel.llvm_ir.matches("store float").count(), 1);
        assert_eq!(kernel.llvm_ir.matches("fadd float").count(), 1);
        assert!(!kernel.llvm_ir.contains("fe2o3_device"));
    }

    #[test]
    fn verified_vecadd_uses_the_authenticated_arbitrary_export_identity() {
        const EXPORT: &str = "renamed_genuine_vecadd";
        let mut module = translated_vecadd();
        module.kernels[0].id = KernelId::new(EXPORT);

        let kernels = prepare_fill_collection(module, &[EXPORT.to_string()])
            .expect("renamed verified vecadd profile");
        let [kernel] = kernels.as_slice() else {
            panic!("one renamed vecadd kernel")
        };
        assert_eq!(kernel.name, EXPORT);
        assert!(kernel.llvm_ir.contains("@renamed_genuine_vecadd("));
        assert_eq!(kernel.llvm_ir.matches("load float").count(), 2);
        assert_eq!(kernel.llvm_ir.matches("fadd float").count(), 1);
        assert_eq!(kernel.llvm_ir.matches("store float").count(), 1);
    }

    #[test]
    fn vecadd_rejects_non_exact_slice_abi() {
        let mut module = translated_vecadd();
        module.functions[0].signature.parameters[0] = writable_f32_slice();
        let body = module.functions[0].body.as_mut().expect("body");
        body.blocks[5].operations[0].results[0].ty = writable_f32_pointer();
        body.blocks[5].operations[1].results[0].ty = writable_f32_pointer();

        let error = prepare_fill_collection(module, &[VECADD_KERNEL.to_string()])
            .expect_err("writable input must be outside the exact vecadd ABI");
        let text = error.to_string();
        assert!(text.contains("does not support kernel export \"vecadd\""));
        assert!(text.contains("bounded terminal profiles require fill"));
        assert!(!text.contains("G1 AMDGPU lowering"));
    }

    #[test]
    fn vecadd_missing_kernel_slice_projection_fails_closed() {
        let mut module = translated_vecadd();
        let body = module.functions[0].body.as_mut().expect("body");
        body.blocks[5]
            .parameters
            .push(ValueDef::new(ValueId(18), readonly_f32_slice()));
        let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
            &mut body.blocks[4].terminator
        else {
            panic!("first bounds branch")
        };
        then_arguments.push(ValueId(1));
        let OperationKind::SliceLength { slice } = &mut body.blocks[5].operations[3].kind else {
            panic!("second input length")
        };
        *slice = ValueId(18);
        let OperationKind::SliceData { slice } = &mut body.blocks[6].operations[0].kind else {
            panic!("second input data")
        };
        *slice = ValueId(18);
        verify_module(&module).expect("block-parameter slice fixture must remain verified");

        let error = prepare_fill_collection(module, &[VECADD_KERNEL.to_string()])
            .expect_err("non-kernel projection key must fail closed instead of panicking");
        assert!(
            error
                .to_string()
                .contains("missing second input length for %1")
        );
    }

    #[test]
    fn vecadd_rejects_mismatched_disjoint_write_witness() {
        let mut module = translated_vecadd();
        let body = module.functions[0].body.as_mut().expect("body");
        body.blocks[2].operations.insert(
            0,
            Operation::effect_free(
                ValueDef::new(ValueId(18), Type::INDEX),
                OperationKind::Constant(Constant::Index(0)),
            ),
        );
        let OperationKind::Call { arguments, .. } = &mut body.blocks[2].operations[1].kind else {
            panic!("get_mut call")
        };
        arguments[1] = ValueId(18);

        let error = prepare_fill_collection(module, &[VECADD_KERNEL.to_string()])
            .expect_err("constant output index must not inherit the disjoint witness");
        assert!(
            error
                .to_string()
                .contains("must consume the exact trusted global thread index")
        );
    }

    #[test]
    fn vecadd_rejects_wrong_input_index_and_arithmetic() {
        let mut wrong_index = translated_vecadd();
        let body = wrong_index.functions[0].body.as_mut().expect("body");
        let OperationKind::GetElementPointer { offset, .. } =
            &mut body.blocks[5].operations[1].kind
        else {
            panic!("first input GEP")
        };
        *offset = ValueId(3);
        let error = prepare_fill_collection(wrong_index, &[VECADD_KERNEL.to_string()])
            .expect_err("input loads must use ThreadIndex::get");
        assert!(error.to_string().contains("first input element pointer"));

        let mut multiply = translated_vecadd();
        let body = multiply.functions[0].body.as_mut().expect("body");
        let OperationKind::Binary { op, .. } = &mut body.blocks[6].operations[3].kind else {
            panic!("f32 add")
        };
        *op = BinaryOp::Multiply;
        let error = prepare_fill_collection(multiply, &[VECADD_KERNEL.to_string()])
            .expect_err("vecadd must not silently broaden to other arithmetic");
        let text = error.to_string();
        assert!(text.contains("unsupported operation"));
        assert!(text.contains("no alternate codegen route"));
    }

    #[test]
    fn vecadd_rejects_inverted_bounds_control_flow() {
        let mut module = translated_vecadd();
        let body = module.functions[0].body.as_mut().expect("body");
        let Some(Terminator::ConditionalBranch {
            then_target,
            else_target,
            ..
        }) = &mut body.blocks[4].terminator
        else {
            panic!("first bounds branch")
        };
        std::mem::swap(then_target, else_target);

        let error = prepare_fill_collection(module, &[VECADD_KERNEL.to_string()])
            .expect_err("inverted bounds edge must not reach emission");
        assert!(
            error
                .to_string()
                .contains("control-flow edges do not match")
        );
    }

    #[test]
    fn vecadd_rejects_unsupported_calls_without_fallback() {
        let mut module = translated_vecadd();
        let body = module.functions[0].body.as_mut().expect("body");
        let OperationKind::Call { callee, .. } = &mut body.blocks[1].operations[0].kind else {
            panic!("ThreadIndex::get call")
        };
        *callee = FunctionId::new("tests::unsupported_index_projection");
        module.functions.push(Function::declaration(
            "tests::unsupported_index_projection",
            Signature::new(vec![Type::INDEX], vec![Type::INDEX]),
        ));

        let error = prepare_fill_collection(module, &[VECADD_KERNEL.to_string()])
            .expect_err("unknown helper must fail closed");
        let text = error.to_string();
        assert!(text.contains("does not support call"));
        assert!(text.contains("no alternate codegen route"));
    }

    #[test]
    fn unsupported_trusted_helper_is_rejected_before_g1() {
        const EXPORT: &str = "renamed_wrong_fill_body";
        let mut module = translated_fill();
        module.kernels[0].id = KernelId::new(EXPORT);
        let function = &mut module.functions[0];
        let body = function.body.as_mut().expect("body");
        let OperationKind::Call { callee, .. } = &mut body.blocks[1].operations[0].kind else {
            panic!("get_mut call")
        };
        *callee = FunctionId::new(TrustedDeviceItem::DisjointSliceGetMutAt.canonical_path());
        let signature = module.functions[2].signature.clone();
        module.functions.push(Function::declaration(
            TrustedDeviceItem::DisjointSliceGetMutAt.canonical_path(),
            signature,
        ));

        let error = prepare_fill_collection(module, &[EXPORT.to_string()])
            .expect_err("a renamed export cannot admit the wrong fill body");
        assert!(error.to_string().contains("does not support call"));
        assert!(error.to_string().contains("no alternate codegen route"));
    }

    #[test]
    fn get_mut_must_use_the_trusted_global_thread_index() {
        let mut module = translated_fill();
        let body = module.functions[0].body.as_mut().expect("body");
        body.blocks[1].operations.insert(
            0,
            Operation::effect_free(
                ValueDef::new(ValueId(10), Type::INDEX),
                OperationKind::Constant(Constant::Index(0)),
            ),
        );
        let OperationKind::Call { arguments, .. } = &mut body.blocks[1].operations[1].kind else {
            panic!("get_mut call")
        };
        arguments[1] = ValueId(10);

        let error = prepare_fill_collection(module, &[FILL_KERNEL.to_string()])
            .expect_err("a constant write index is outside the initial fill subset");
        assert!(
            error
                .to_string()
                .contains("must use the exact trusted global thread index")
        );
    }

    fn inert_compiler_module_fixture() -> Module {
        let mut entry_block = BasicBlock::new(BlockId(0));
        entry_block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "entry_impl",
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry_block],
        );

        let mut helper_block = BasicBlock::new(BlockId(0));
        helper_block.terminator = Some(Terminator::Return { values: vec![] });
        let mut helper = Function::device_ffi_export(
            "visible_helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![helper_block],
        );
        helper
            .required_capabilities
            .insert(TargetCapability::WaveWidth(
                fe2o3_kernel_ir::WaveWidth::Wave64,
            ));
        let declaration = Function::declaration("external_import", Signature::new(vec![], vec![]));

        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "entry",
            "entry_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        kernel
            .required_capabilities
            .insert(TargetCapability::WaveWidth(
                fe2o3_kernel_ir::WaveWidth::Wave64,
            ));

        let mut module = Module::new("tests::inert_compiler_module");
        module.functions = vec![helper, entry, declaration];
        module.kernels.push(kernel);
        module
    }

    fn float_compiler_module_fixture(float: FloatOperation) -> Module {
        let result = ValueId(float.operands().len() as u32);
        let parameter_types = float.parameter_types();
        let parameters = (0..parameter_types.len())
            .map(|index| ValueId(index as u32))
            .collect::<Vec<_>>();
        let declaration = float.declaration();
        let mut entry_block = BasicBlock::new(BlockId(0));
        entry_block.operations.push(float.operation(result));
        entry_block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "float_entry",
            Signature::new(parameter_types, vec![]),
            parameters,
            vec![entry_block],
        );
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "float_kernel",
            "float_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        kernel
            .required_capabilities
            .insert(TargetCapability::WaveWidth(
                fe2o3_kernel_ir::WaveWidth::Wave64,
            ));

        let mut module = Module::new("tests::float_compiler_module");
        module.required_capabilities = declaration.required_capabilities.clone();
        module
            .required_capabilities
            .insert(TargetCapability::WaveWidth(
                fe2o3_kernel_ir::WaveWidth::Wave64,
            ));
        module.functions = vec![entry, declaration];
        module.kernels.push(kernel);
        module
    }

    #[test]
    fn inert_compiler_module_wrapper_is_descriptive_and_deterministic() {
        let module = inert_compiler_module_fixture();
        let first = construct_inert_compiler_module_text_v1(&module).expect("bounded module");
        let second = construct_inert_compiler_module_text_v1(&module).expect("bounded module");

        assert_eq!(first, second);
        assert_eq!(first.kernel_entries(), &["entry"]);
        assert_eq!(first.device_definitions(), &["visible_helper"]);
        assert!(first.internal_helpers().is_empty());
        assert_eq!(first.device_ffi_exports(), &["visible_helper"]);
        assert_eq!(first.external_declarations(), &["external_import"]);
        assert!(first.llvm_ir().contains("define amdgpu_kernel void @entry"));
        assert!(first.llvm_ir().contains("define void @visible_helper"));
        assert!(first.llvm_ir().contains("declare void @external_import"));
        assert!(!first.llvm_ir().contains("bitcode"));
        assert_eq!(first.descriptor_source_identity(), None);
    }

    #[test]
    fn source_debug_injection_preserves_the_compiler_module_text_bound() {
        let mut injected = "x".repeat(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES);
        enforce_source_debug_text_bound(&injected).expect("exact text limit must remain valid");

        injected.push('\n');
        let actual = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES + 1;
        let error = enforce_source_debug_text_bound(&injected)
            .expect_err("one injected byte over the text limit must fail closed");
        assert_eq!(
            error,
            CompilerModuleConstructionError::LimitExceeded {
                field: "source-debug-injected compiler-module LLVM text bytes",
                actual,
                max: dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES,
            }
        );
        assert_eq!(
            error.to_string(),
            format!(
                "source-debug-injected compiler-module LLVM text bytes count/size {actual} exceeds limit {}",
                dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
            )
        );
    }

    #[test]
    fn gfx942_compiler_module_preserves_float_contracts_and_real_link_imports() {
        let sin = FloatOperation::F32Math {
            function: F32MathFunction::Sin,
            implementation: F32MathImplementation::OcmlAbiV1,
            arguments: vec![ValueId(0)],
        };
        let module = float_compiler_module_fixture(sin);

        assert!(matches!(
            construct_inert_compiler_module_text_for_target_v1(&module, None),
            Err(CompilerModuleConstructionError::UnsupportedFloatTarget(target))
                if target == "<unbound>"
        ));
        assert!(matches!(
            construct_inert_compiler_module_text_for_target_v1(
                &module,
                Some(DeviceTargetV1::parse("gfx1100").unwrap())
            ),
            Err(CompilerModuleConstructionError::UnsupportedFloatTarget(target))
                if target == "gfx1100"
        ));

        let compiled = construct_inert_compiler_module_text_for_target_v1(
            &module,
            Some(DeviceTargetV1::parse("gfx942").unwrap()),
        )
        .expect("gfx942 float compiler module");
        assert!(compiled.llvm_ir().contains("\"target-cpu\"=\"gfx942\""));
        assert!(
            compiled
                .llvm_ir()
                .contains("declare float @__ocml_sin_f32(float)")
        );
        assert!(compiled.llvm_ir().contains("call float @__ocml_sin_f32"));
        assert_eq!(compiled.external_declarations(), &["__ocml_sin_f32"]);
        assert!(
            !compiled
                .external_declarations()
                .iter()
                .any(|symbol| symbol.starts_with("__fe2o3_ir_float_v1_"))
        );
    }

    #[test]
    fn exact_target_bound_module_cannot_lower_through_a_processor_only_alias() {
        let mut module = inert_compiler_module_fixture();
        let exact_target = TargetCapability::Extension {
            namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.to_owned(),
        };
        let entry = module.kernels[0].entry.clone();
        module.required_capabilities.insert(exact_target.clone());
        module.kernels[0]
            .required_capabilities
            .insert(exact_target.clone());
        module
            .functions
            .iter_mut()
            .find(|function| function.id == entry)
            .expect("fixture kernel entry")
            .required_capabilities
            .insert(exact_target);

        let exact = construct_inert_compiler_module_text_for_target_v1(
            &module,
            Some(DeviceTargetV1::parse("gfx942:xnack-").unwrap()),
        )
        .expect("exact target-bound module");
        assert!(exact.llvm_ir().contains("\"target-cpu\"=\"gfx942\""));

        for wrong in [
            "gfx942",
            "gfx942:xnack+",
            "gfx942:sramecc+:xnack-",
            "gfx942:sramecc-:xnack-",
            "gfx950:xnack-",
        ] {
            assert!(matches!(
                construct_inert_compiler_module_text_for_target_v1(
                    &module,
                    Some(DeviceTargetV1::parse(wrong).unwrap())
                ),
                Err(CompilerModuleConstructionError::UnsupportedTargetBinding(target))
                    if target == wrong
            ));
        }
    }

    #[test]
    fn descriptor_source_is_embedded_exactly_once_with_non_alloc_section_directives() {
        let source = descriptor_source("entry", "entry.kd");
        let module = construct_inert_compiler_module_text_v1(&inert_compiler_module_fixture())
            .expect("bounded module");
        let bound = bind_compiler_descriptor_source_v1(module, &source).expect("matching closure");

        assert_eq!(bound.descriptor_source_identity(), Some(source.identity()));
        assert!(
            bound
                .llvm_ir()
                .contains("module asm \".section .fe2o3.kd.v1,\\22\\22,@progbits\"")
        );
        assert!(bound.llvm_ir().contains("module asm \".balign 8\""));
        assert_eq!(
            embedded_descriptor_bytes(bound.llvm_ir()),
            source.canonical_bytes()
        );

        assert!(matches!(
            bind_compiler_descriptor_source_v1(bound, &source),
            Err(CompilerModuleConstructionError::DescriptorSourceAlreadyBound)
        ));
    }

    #[test]
    fn descriptor_source_kernel_and_symbol_substitution_fail_closed() {
        let module = construct_inert_compiler_module_text_v1(&inert_compiler_module_fixture())
            .expect("bounded module");
        assert!(matches!(
            bind_compiler_descriptor_source_v1(
                module.clone(),
                &descriptor_source("other", "other.kd")
            ),
            Err(CompilerModuleConstructionError::DescriptorKernelEntryClosureMismatch)
        ));
        assert!(matches!(
            bind_compiler_descriptor_source_v1(
                module,
                &descriptor_source("entry", "other_descriptor")
            ),
            Err(CompilerModuleConstructionError::DescriptorSymbolClosureMismatch)
        ));
    }

    #[test]
    fn compiler_module_limits_run_before_graph_verification() {
        let mut oversized_id = Module::new("x".repeat(MAX_COMPILER_MODULE_ID_BYTES + 1));
        oversized_id.functions.push(Function::declaration(
            "duplicate",
            Signature::new(vec![], vec![]),
        ));
        oversized_id.functions.push(Function::declaration(
            "duplicate",
            Signature::new(vec![], vec![]),
        ));
        let error = construct_inert_compiler_module_text_v1(&oversized_id).unwrap_err();
        assert!(matches!(
            error,
            CompilerModuleConstructionError::LimitExceeded {
                field: "compiler-module ID bytes",
                ..
            }
        ));

        let mut too_many_functions = inert_compiler_module_fixture();
        let declaration = Function::declaration("f", Signature::new(vec![], vec![]));
        too_many_functions.functions = vec![declaration; MAX_COMPILER_MODULE_FUNCTIONS + 1];
        let error = construct_inert_compiler_module_text_v1(&too_many_functions).unwrap_err();
        assert!(matches!(
            error,
            CompilerModuleConstructionError::LimitExceeded {
                field: "compiler-module functions",
                ..
            }
        ));
    }

    #[test]
    fn compiler_module_bounds_cover_call_fanout_and_nested_types() {
        let mut wide_call = inert_compiler_module_fixture();
        let entry = wide_call
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "entry_impl")
            .unwrap();
        entry.body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "external_import".into(),
                    arguments: vec![ValueId(999); MAX_COMPILER_MODULE_CALL_ARGUMENTS + 1],
                },
            ));
        let error = construct_inert_compiler_module_text_v1(&wide_call).unwrap_err();
        assert!(matches!(
            error,
            CompilerModuleConstructionError::LimitExceeded {
                field: "compiler-module call arguments",
                ..
            }
        ));

        let mut nested = inert_compiler_module_fixture();
        let mut ty = Type::F32;
        for _ in 0..=MAX_COMPILER_MODULE_TYPE_DEPTH {
            ty = Type::pointer(ty, AddressSpace::Global, AccessMode::ReadOnly);
        }
        nested.functions.push(Function::declaration(
            "nested_import",
            Signature::new(vec![ty], vec![]),
        ));
        let error = construct_inert_compiler_module_text_v1(&nested).unwrap_err();
        assert!(matches!(
            error,
            CompilerModuleConstructionError::LimitExceeded {
                field: "compiler-module type nesting",
                ..
            }
        ));
    }

    #[test]
    fn unsupported_compiler_module_input_returns_no_partial_text_value() {
        let mut module = inert_compiler_module_fixture();
        module.functions.push(Function::declaration(
            "unsupported_slice_import",
            Signature::new(vec![readonly_f32_slice()], vec![]),
        ));
        let result = construct_inert_compiler_module_text_v1(&module);
        assert!(matches!(
            result,
            Err(CompilerModuleConstructionError::Lowering(_))
        ));
    }

    #[test]
    fn exact_row_softmax_profile_remains_stable_after_generic_float_lowering() {
        let module = crate::collected_row_softmax_v1::canonical_row_softmax_v1_module();
        let generic =
            dialect_amdgcn::lower_device_module_to_gfx942_xnack_minus_llvm_ir(&module).unwrap();
        let profile =
            dialect_amdgcn::authenticate_gfx942_row_softmax_lowering_profile_v1(&module).unwrap();
        let dedicated =
            dialect_amdgcn::lower_authenticated_row_softmax_module_to_gfx942_xnack_minus_llvm_ir_v1(
                &module,
                &profile,
            )
            .unwrap();
        assert_eq!(generic, dedicated);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(dedicated.as_bytes())),
            REVIEWED_ROW_SOFTMAX_LEGACY_LLVM_SHA256,
        );
        let layout_measurement = reviewed_row_softmax_upstream_llvm_layout_v1();
        assert_eq!(
            layout_measurement.llvm_build_identity,
            fe2o3_hsaco_finalize::ROW_SOFTMAX_V1_UPSTREAM_LLVM_BUILD_IDENTITY_V1,
        );
        assert!(
            validate_row_softmax_upstream_llvm_layout_v1(&dedicated, layout_measurement).is_err(),
            "legacy producer layout must not satisfy the reviewed LLVM 22.1.8 measurement",
        );
        let expected =
            bind_reviewed_row_softmax_upstream_llvm_layout_v1(dedicated, layout_measurement)
                .unwrap();
        let first = construct_inert_row_softmax_v1_module_text(&module).unwrap();
        let second = construct_inert_row_softmax_v1_module_text(&module).unwrap();
        assert_eq!(expected, first.llvm_ir());
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(first.llvm_ir().as_bytes())),
            REVIEWED_ROW_SOFTMAX_V1_LLVM_SHA256,
        );
        assert_eq!(first, second);
        assert_eq!(first.kernel_entries(), &["row_softmax_v1"]);
        assert_eq!(first.external_declarations(), &["__ocml_exp_f32"]);
        assert!(first.internal_helpers().is_empty());
        assert!(first.device_ffi_exports().is_empty());
        assert_eq!(
            first
                .llvm_ir()
                .matches("declare float @__ocml_exp_f32(float)")
                .count(),
            1
        );
        assert_eq!(
            first
                .llvm_ir()
                .matches("call float @__ocml_exp_f32(float ")
                .count(),
            2
        );
        assert_eq!(first.llvm_ir().matches("fcmp ogt float ").count(), 1);
        assert_eq!(first.llvm_ir().matches("fcmp ").count(), 1);
        assert_eq!(first.llvm_ir().matches("fdiv float ").count(), 1);
        assert!(!first.llvm_ir().contains("fcmp une float "));
        assert!(!first.llvm_ir().contains("fdiv fast "));

        let mut substituted_predicate = module.clone();
        let predicate = substituted_predicate
            .functions
            .iter_mut()
            .filter_map(|function| function.body.as_mut())
            .flat_map(|body| &mut body.blocks)
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::Compare { predicate, .. }
                    if *predicate == ComparePredicate::GreaterThan =>
                {
                    Some(predicate)
                }
                _ => None,
            })
            .expect("canonical row comparison");
        *predicate = ComparePredicate::NotEqual;
        assert!(
            dialect_amdgcn::authenticate_gfx942_row_softmax_lowering_profile_v1(
                &substituted_predicate
            )
            .is_err()
        );
        assert!(
            dialect_amdgcn::lower_authenticated_row_softmax_module_to_gfx942_xnack_minus_llvm_ir_v1(
                &substituted_predicate,
                &profile,
            )
            .is_err()
        );

        let mut renamed = module;
        renamed.id = "fe2o3::row_softmax_v1_substitution".into();
        assert!(matches!(
            construct_inert_row_softmax_v1_module_text(&renamed),
            Err(CompilerModuleConstructionError::RowSoftmaxLowering(_))
        ));
    }

    #[test]
    fn row_softmax_upstream_layout_header_rejects_stale_missing_duplicate_and_reordered_fields() {
        let module = crate::collected_row_softmax_v1::canonical_row_softmax_v1_module();
        let profile =
            dialect_amdgcn::authenticate_gfx942_row_softmax_lowering_profile_v1(&module).unwrap();
        let legacy =
            dialect_amdgcn::lower_authenticated_row_softmax_module_to_gfx942_xnack_minus_llvm_ir_v1(
                &module,
                &profile,
            )
            .unwrap();
        let measurement = reviewed_row_softmax_upstream_llvm_layout_v1();
        let exact =
            bind_reviewed_row_softmax_upstream_llvm_layout_v1(legacy.clone(), measurement).unwrap();
        validate_row_softmax_upstream_llvm_layout_v1(&exact, measurement).unwrap();

        let exact_layout_line = format!(
            "target datalayout = \"{}\"\n",
            ROW_SOFTMAX_UPSTREAM_LLVM_DATA_LAYOUT_V1,
        );
        let stale_layout_line = format!(
            "target datalayout = \"{}\"\n",
            dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT,
        );
        let triple_line = format!(
            "target triple = \"{}\"\n",
            ROW_SOFTMAX_UPSTREAM_LLVM_TARGET_TRIPLE_V1,
        );
        let stale = exact.replacen(&exact_layout_line, &stale_layout_line, 1);
        let missing = exact.replacen(&exact_layout_line, "", 1);
        let duplicate = exact.replacen(
            &exact_layout_line,
            &format!("{exact_layout_line}{exact_layout_line}"),
            1,
        );
        let reordered = exact.replacen(
            &format!("{triple_line}{exact_layout_line}"),
            &format!("{exact_layout_line}{triple_line}"),
            1,
        );
        for hostile in [stale, missing, duplicate, reordered] {
            assert!(
                validate_row_softmax_upstream_llvm_layout_v1(&hostile, measurement).is_err(),
                "hostile row-softmax target header was admitted",
            );
        }

        let mut unreviewed_legacy = legacy;
        unreviewed_legacy.push('\n');
        assert!(matches!(
            bind_reviewed_row_softmax_upstream_llvm_layout_v1(unreviewed_legacy, measurement),
            Err(CompilerModuleConstructionError::RowSoftmaxLowering(detail))
                if detail.contains("complete legacy lowering")
        ));
    }

    fn require_row_softmax_layout_probe(release_gate: bool, configured: bool) -> bool {
        assert!(
            !release_gate || configured,
            "row-softmax release gate requires FE2O3_TEST_ROW_SOFTMAX_LLVM22_LAYOUT_PROBE",
        );
        configured
    }

    #[test]
    fn row_softmax_release_gate_fails_closed_without_layout_probe() {
        assert!(!require_row_softmax_layout_probe(false, false));
        assert!(
            std::panic::catch_unwind(|| require_row_softmax_layout_probe(true, false)).is_err()
        );
    }

    #[test]
    fn configured_upstream_llvm22_target_machine_matches_reviewed_row_softmax_layout() {
        const PROBE_ENV: &str = "FE2O3_TEST_ROW_SOFTMAX_LLVM22_LAYOUT_PROBE";
        let release_gate = match std::env::var("FE2O3_ROW_SOFTMAX_RELEASE_GATE") {
            Ok(value) => {
                assert_eq!(
                    value, "1",
                    "FE2O3_ROW_SOFTMAX_RELEASE_GATE must be exactly 1 when present",
                );
                true
            }
            Err(std::env::VarError::NotPresent) => false,
            Err(error) => panic!("read FE2O3_ROW_SOFTMAX_RELEASE_GATE: {error}"),
        };
        let Some(probe) = std::env::var_os(PROBE_ENV) else {
            require_row_softmax_layout_probe(release_gate, false);
            eprintln!(
                "skipping configured upstream LLVM layout observation: {PROBE_ENV} is absent"
            );
            return;
        };
        require_row_softmax_layout_probe(release_gate, true);
        let mut command = std::process::Command::new(&probe);
        let output =
            crate::process_execution::capture_output(&mut command).unwrap_or_else(|error| {
                panic!("execute configured upstream LLVM layout probe: {error}")
            });
        assert!(
            output.status.success(),
            "configured upstream LLVM layout probe failed: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert!(
            output.stderr.is_empty(),
            "configured upstream LLVM layout probe wrote stderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(
            String::from_utf8(output.stdout).expect("layout probe stdout is UTF-8"),
            format!(
                "llvm-build-identity={ROW_SOFTMAX_UPSTREAM_LLVM_BUILD_IDENTITY_V1}\n\
                 target-triple={ROW_SOFTMAX_UPSTREAM_LLVM_TARGET_TRIPLE_V1}\n\
                 target-cpu=gfx942\n\
                 target-features=-xnack\n\
                 relocation-model=pic\n\
                 code-model=small\n\
                 codegen-opt-level=none\n\
                 data-layout={ROW_SOFTMAX_UPSTREAM_LLVM_DATA_LAYOUT_V1}\n"
            ),
            "reviewed Rust constant differs from the configured upstream LLVM TargetMachine observation",
        );
    }

    #[test]
    fn row_softmax_authority_transcript_is_bounded_and_commitment_checked() {
        let module = crate::collected_row_softmax_v1::canonical_row_softmax_v1_module();
        let compiler_module = construct_inert_row_softmax_v1_module_text(&module).unwrap();
        let transcript = b"canonical-test-authority-transcript";
        let commitment = Sha256::digest(transcript).into();
        let bound = bind_row_softmax_frontend_authority_v1(
            compiler_module.clone(),
            transcript,
            commitment,
            [0x55; 32],
        )
        .unwrap();
        assert!(
            bound
                .llvm_ir()
                .contains(".fe2o3.row-softmax-authority-transcript.v1")
        );
        assert!(
            bind_row_softmax_frontend_authority_v1(
                compiler_module.clone(),
                transcript,
                [0; 32],
                [0x55; 32],
            )
            .is_err()
        );
        let oversized = vec![
            0;
            crate::collected_row_softmax_v1::MAX_ROW_SOFTMAX_AUTHORITY_TRANSCRIPT_BYTES_V1
                + 1
        ];
        assert!(
            bind_row_softmax_frontend_authority_v1(
                compiler_module,
                &oversized,
                Sha256::digest(&oversized).into(),
                [0x55; 32],
            )
            .is_err()
        );
    }

    #[test]
    fn row_softmax_module_role_closure_rejects_import_substitutions() {
        let module = crate::collected_row_softmax_v1::canonical_row_softmax_v1_module();
        let lowered = construct_inert_row_softmax_v1_module_text(&module).unwrap();
        let descriptor = crate::compiler_descriptor::row_softmax_v1_descriptor_source_for_test();
        let exact = bind_compiler_descriptor_source_v1(lowered, &descriptor).unwrap();
        crate::worker_v2_producer::validate_exact_row_softmax_module_closure(&exact).unwrap();

        let mut omitted = exact.clone();
        omitted.external_declarations.clear();
        assert!(matches!(
            crate::worker_v2_producer::validate_exact_row_softmax_module_closure(&omitted),
            Err(crate::worker_v2_producer::WorkerV2ProducerError::RowSoftmaxClosureMismatch)
        ));

        let mut substituted = exact;
        substituted.external_declarations = vec!["__ocml_exp2_f32".to_owned()];
        assert!(matches!(
            crate::worker_v2_producer::validate_exact_row_softmax_module_closure(&substituted),
            Err(crate::worker_v2_producer::WorkerV2ProducerError::RowSoftmaxClosureMismatch)
        ));
    }

    #[test]
    fn count_valid_twenty_megabyte_module_is_rejected_without_partial_text() {
        const CALLS: usize = 25_000;
        let parameter_type = Type::Scalar(fe2o3_kernel_ir::ScalarType::I32);
        let parameter_types = vec![parameter_type; MAX_COMPILER_MODULE_PARAMETERS];
        let parameter_values = (0..MAX_COMPILER_MODULE_PARAMETERS)
            .map(|index| ValueId(index as u32))
            .collect::<Vec<_>>();
        let import_name = "x".repeat(240);
        let call_arguments = parameter_values.clone();
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = (0..CALLS)
            .map(|_| {
                Operation::new(
                    vec![],
                    OperationKind::Call {
                        callee: import_name.clone().into(),
                        arguments: call_arguments.clone(),
                    },
                )
            })
            .collect();
        block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "large_entry",
            Signature::new(parameter_types.clone(), vec![]),
            parameter_values,
            vec![block],
        );
        let import =
            Function::external_import(import_name.clone(), Signature::new(parameter_types, vec![]));
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "large_kernel",
            "large_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        kernel
            .required_capabilities
            .insert(TargetCapability::WaveWidth(
                fe2o3_kernel_ir::WaveWidth::Wave64,
            ));
        let mut module = Module::new("tests::twenty_megabyte_adversary");
        module.functions = vec![entry, import];
        module.kernels.push(kernel);

        let conservative_text_bytes =
            CALLS * (import_name.len() + MAX_COMPILER_MODULE_PARAMETERS * "i32 %arg00, ".len());
        assert!(conservative_text_bytes > 20 * 1024 * 1024);
        enforce_compiler_module_bounds(&module).expect("adversary is structurally count-valid");
        let error = construct_inert_compiler_module_text_v1(&module).unwrap_err();
        let CompilerModuleConstructionError::Lowering(error) = error else {
            panic!("text limit must be enforced by the capacity-limited lowerer")
        };
        assert!(error.contains(dialect_amdgcn::LoweringDiagnosticCode::ResourceLimit));
        assert!(error.to_string().contains("textual LLVM attempted"));
        assert!(error.to_string().contains("maximum is 16777216"));
    }
}
