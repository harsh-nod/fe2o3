//! Workload-neutral compiler-module custody for the production pipeline.
//!
//! The production transaction supplies verified KIR and target-lowered LLVM text. This
//! module bounds that structure, retains its exact symbol closure, and binds the compiler
//! descriptor without selecting a workload or performing another lowering.

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
use fe2o3_kernel_ir::{
    AmdGpuDiagnosticOperation, F32MathFunction, F32MathImplementation, FloatOperation,
    FunctionRole, Module, Operation, OperationKind, TargetCapability, Terminator, Type,
    verify_module,
};
use std::collections::BTreeSet;
use std::fmt;

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
/// authority. The production compiler may place its exact text in an attempt-scoped handoff after
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

impl InertCompilerModuleTextV1 {
    pub(crate) fn llvm_ir(&self) -> &str {
        &self.llvm_ir
    }

    pub(crate) fn kernel_entries(&self) -> &[String] {
        &self.kernel_entries
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
            Self::Verification(error) => write!(formatter, "{error}"),
            #[cfg(test)]
            Self::Lowering(error) => write!(formatter, "{error}"),
        }
    }
}

fn enforce_llvm_text_bound(llvm_ir: &str) -> Result<(), CompilerModuleConstructionError> {
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
            dialect_amdgcn::lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(module)
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
    enforce_llvm_text_bound(&llvm_ir)?;
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
                && AmdGpuDiagnosticOperation::from_intrinsic_id(&function.id).is_none()
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

#[cfg(test)]
mod production_symbol_closure_tests {
    use super::*;
    use fe2o3_kernel_ir::{Function, Signature};

    #[test]
    fn gfx942_diagnostic_intrinsics_are_not_link_imports() {
        let mut module = Module::new("tests::diagnostic_symbol_closure");
        module
            .functions
            .push(AmdGpuDiagnosticOperation::Trap.declaration());
        module.functions.push(Function::external_import(
            "real_device_import",
            Signature::new(vec![], vec![]),
        ));

        let symbols = compiler_module_symbol_closure_v1(&module);
        assert_eq!(symbols.external_declarations, ["real_device_import"]);
    }
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
            | F32MathFunction::RoundTiesEven
            | F32MathFunction::Abs => {
                unreachable!("canonical implementation excludes constrained LLVM from OCML")
            }
        })
    })
}

/// Embeds one exact zero-digest descriptor source in compiler-owned LLVM module assembly.
///
/// The empty ELF flag string is intentional: LLVM and LLD preserve this as a non-allocatable,
/// non-writable, non-executable `SHT_PROGBITS` section. The production compiler-module identity
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
