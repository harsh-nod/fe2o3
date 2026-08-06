//! Exact-kernel legalization for the opt-in production kernel-IR pipeline.
//!
//! Helper names are matched here only after `mir_import` classified their rustc `DefId` and
//! `translate_and_verify` produced this in-memory module. This module must not be used to grant
//! the same authority to decoded or caller-constructed kernel IR.

use crate::CODEGEN_PIPELINE_ENV;
use crate::amdgpu_llvm::{EmitError, PreparedDeviceKernel};
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant,
    FunctionBody, IntrinsicOperation, KernelId, MemoryAccess, Module, Operation, OperationKind,
    TargetCapability, Terminator, Type, ValueDef, ValueId, WorkgroupSize, verify_module,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const FILL_KERNEL: &str = "fill";
const VECADD_KERNEL: &str = "vecadd";
const WORKGROUP_X: u32 = 256;

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
const MAX_COMPILER_MODULE_TEXT_BYTES: usize = 16 * 1024 * 1024;

/// One inert, deterministic textual LLVM AMDGPU module.
///
/// This value is not LLVM bitcode, a link result, a code object, compiler provenance, or load
/// authority. The API is intentionally not connected to rustc collection yet.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InertCompilerModuleTextV1 {
    llvm_ir: String,
    kernel_entries: Vec<String>,
    device_definitions: Vec<String>,
    external_declarations: Vec<String>,
}

#[allow(dead_code)]
impl InertCompilerModuleTextV1 {
    pub(crate) fn llvm_ir(&self) -> &str {
        &self.llvm_ir
    }

    pub(crate) fn kernel_entries(&self) -> &[String] {
        &self.kernel_entries
    }

    pub(crate) fn device_definitions(&self) -> &[String] {
        &self.device_definitions
    }

    pub(crate) fn external_declarations(&self) -> &[String] {
        &self.external_declarations
    }
}

/// Fail-closed compiler-module construction error.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CompilerModuleConstructionError {
    LimitExceeded {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    Lowering(dialect_amdgcn::LoweringErrors),
}

impl fmt::Display for CompilerModuleConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded { field, actual, max } => {
                write!(formatter, "{field} count/size {actual} exceeds limit {max}")
            }
            Self::Lowering(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for CompilerModuleConstructionError {}

/// Constructs one bounded canonical textual module without invoking or wiring LLVM.
///
/// Structural bounds are checked before kernel-IR verification. The dialect lowerer then
/// preflights every kernel, helper, declaration, call, attribute, and metadata record before its
/// private emission pass. An error returns no partially constructed module.
#[allow(dead_code)]
pub(crate) fn construct_inert_compiler_module_text_v1(
    module: &Module,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    enforce_compiler_module_bounds(module)?;
    let llvm_ir = dialect_amdgcn::lower_compiler_module_to_llvm_ir(module)
        .map_err(CompilerModuleConstructionError::Lowering)?;
    check_compiler_module_limit(
        "compiler-module textual LLVM bytes",
        llvm_ir.len(),
        MAX_COMPILER_MODULE_TEXT_BYTES,
    )?;

    let entry_functions = module
        .kernels
        .iter()
        .map(|kernel| &kernel.entry)
        .collect::<BTreeSet<_>>();
    let mut kernel_entries = module
        .kernels
        .iter()
        .map(|kernel| kernel.id.as_str().to_string())
        .collect::<Vec<_>>();
    let mut device_definitions = module
        .functions
        .iter()
        .filter(|function| function.body.is_some() && !entry_functions.contains(&function.id))
        .map(|function| function.id.as_str().to_string())
        .collect::<Vec<_>>();
    let mut external_declarations = module
        .functions
        .iter()
        .filter(|function| function.body.is_none() && !entry_functions.contains(&function.id))
        .map(|function| function.id.as_str().to_string())
        .collect::<Vec<_>>();
    kernel_entries.sort();
    device_definitions.sort();
    external_declarations.sort();

    Ok(InertCompilerModuleTextV1 {
        llvm_ir,
        kernel_entries,
        device_definitions,
        external_declarations,
    })
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

pub(crate) fn prepare_fill_collection(
    mut module: Module,
    expected_kernel_names: &[String],
) -> Result<Vec<PreparedDeviceKernel>, EmitError> {
    verify_module(&module).map_err(|errors| {
        reject(format!(
            "received invalid verified kernel IR before fill legalization: {errors}"
        ))
    })?;

    let mut expected = expected_kernel_names.to_vec();
    expected.sort();
    let [kernel_name] = expected.as_slice() else {
        return Err(reject(format!(
            "supports exactly one kernel export from {FILL_KERNEL:?} or {VECADD_KERNEL:?}; collected {expected:?}; unset {CODEGEN_PIPELINE_ENV} to use the default legacy-v1 pipeline"
        )));
    };
    if !matches!(kernel_name.as_str(), FILL_KERNEL | VECADD_KERNEL) {
        return Err(reject(format!(
            "does not support kernel export {kernel_name:?}; expected {FILL_KERNEL:?} or {VECADD_KERNEL:?}; unset {CODEGEN_PIPELINE_ENV} to use the default legacy-v1 pipeline"
        )));
    }

    let mut translated = module
        .kernels
        .iter()
        .map(|kernel| kernel.id.as_str().to_string())
        .collect::<Vec<_>>();
    translated.sort();
    if translated != expected {
        return Err(reject(format!(
            "translated kernel identities {translated:?} do not match collected kernel identities {expected:?}"
        )));
    }

    let kernel = module
        .kernels
        .iter_mut()
        .find(|kernel| kernel.id.as_str() == kernel_name)
        .expect("identity equality established the selected kernel");
    kernel.workgroup_size = Some(WorkgroupSize::new(WORKGROUP_X, 1, 1));
    let entry = kernel.entry.clone();

    let function = module
        .functions
        .iter_mut()
        .find(|function| function.id == entry)
        .expect("initial verification established the kernel entry");
    let expected_parameters = match kernel_name.as_str() {
        FILL_KERNEL => vec![writable_f32_slice()],
        VECADD_KERNEL => vec![
            readonly_f32_slice(),
            readonly_f32_slice(),
            writable_f32_slice(),
        ],
        _ => unreachable!("kernel admission checked the selected identity"),
    };
    if function.signature.parameters != expected_parameters
        || !function.signature.results.is_empty()
    {
        return Err(reject(format!(
            "`{kernel_name}` must have exact kernel IR signature {expected_parameters:?} -> (); found {:?} -> {:?}",
            function.signature.parameters, function.signature.results
        )));
    }
    let body = function.body.as_mut().expect("verified kernel entry body");
    match kernel_name.as_str() {
        FILL_KERNEL => legalize_fill_body(body, &function.signature.parameters)?,
        VECADD_KERNEL => legalize_vecadd_body(body, &function.signature.parameters)?,
        _ => unreachable!("kernel admission checked the selected identity"),
    }

    verify_module(&module).map_err(|errors| {
        reject(format!(
            "{kernel_name} legalization produced invalid kernel IR and was not emitted: {errors}"
        ))
    })?;
    let llvm_ir = dialect_amdgcn::lower_kernel_to_llvm_ir(&module, &KernelId::new(kernel_name))
        .map_err(|errors| {
            reject(format!(
                "G1 AMDGPU lowering rejected `{kernel_name}`: {errors}"
            ))
        })?;

    Ok(vec![PreparedDeviceKernel {
        name: kernel_name.to_string(),
        llvm_ir,
    }])
}

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
                "fill legalization does not support call `{callee}`; no legacy fallback was attempted"
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

    legalize_option_switches(body, FILL_KERNEL, &option_conditions)?;
    Ok(())
}

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
                "vecadd legalization does not support call `{callee}`; no legacy fallback was attempted"
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

    legalize_option_switches(body, VECADD_KERNEL, &option_conditions)?;
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
                        "vecadd contains unsupported operation {other:?}; no legacy fallback was attempted"
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
                    "vecadd contains unsupported terminator {terminator:?}; no legacy fallback was attempted"
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

fn block(body: &FunctionBody, id: BlockId) -> &BasicBlock {
    body.blocks
        .iter()
        .find(|block| block.id == id)
        .expect("verified branch target")
}

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

fn fresh_value(next: &mut u32, ty: Type) -> Result<ValueDef, EmitError> {
    let value = ValueDef::new(ValueId(*next), ty);
    *next = next
        .checked_add(1)
        .ok_or_else(|| reject("kernel exhausted kernel IR value identities"))?;
    Ok(value)
}

fn readonly_f32_slice() -> Type {
    Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly)
}

fn writable_f32_slice() -> Type {
    Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite)
}

fn readonly_f32_pointer() -> Type {
    Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly)
}

fn writable_f32_pointer() -> Type {
    Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite)
}

fn reject(reason: impl Into<String>) -> EmitError {
    EmitError::Preflight {
        reason: format!(
            "{CODEGEN_PIPELINE_ENV}=kernel-ir-v1 production path rejected input: {}",
            reason.into()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Constant, Function, FunctionId, LaunchDomain, LaunchExtent,
        MemoryAccess, Signature, SwitchCase,
    };

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

        let function = Function::definition(
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

        let function = Function::definition(
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
    fn kernel_admission_is_exact_and_never_falls_back() {
        let error = prepare_fill_collection(translated_fill(), &["saxpy".to_string()])
            .expect_err("saxpy must remain on legacy-v1");

        let text = error.to_string();
        assert!(text.contains("does not support kernel export \"saxpy\""));
        assert!(text.contains("default legacy-v1 pipeline"));
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
    fn vecadd_rejects_non_exact_slice_abi() {
        let mut module = translated_vecadd();
        module.functions[0].signature.parameters[0] = writable_f32_slice();
        let body = module.functions[0].body.as_mut().expect("body");
        body.blocks[5].operations[0].results[0].ty = writable_f32_pointer();
        body.blocks[5].operations[1].results[0].ty = writable_f32_pointer();

        let error = prepare_fill_collection(module, &[VECADD_KERNEL.to_string()])
            .expect_err("writable input must be outside the exact vecadd ABI");
        let text = error.to_string();
        assert!(text.contains("must have exact kernel IR signature"));
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
        assert!(text.contains("no legacy fallback"));
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
        assert!(text.contains("no legacy fallback"));
    }

    #[test]
    fn unsupported_trusted_helper_is_rejected_before_g1() {
        let mut module = translated_fill();
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

        let error = prepare_fill_collection(module, &[FILL_KERNEL.to_string()])
            .expect_err("get_mut_at is outside the production fill subset");
        assert!(error.to_string().contains("does not support call"));
        assert!(error.to_string().contains("no legacy fallback"));
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
        let entry = Function::definition(
            "entry_impl",
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry_block],
        );

        let mut helper_block = BasicBlock::new(BlockId(0));
        helper_block.terminator = Some(Terminator::Return { values: vec![] });
        let helper = Function::definition(
            "visible_helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![helper_block],
        );
        let declaration = Function::declaration("external_import", Signature::new(vec![], vec![]));

        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "entry",
            "entry_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

        let mut module = Module::new("tests::inert_compiler_module");
        module.functions = vec![helper, entry, declaration];
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
        assert_eq!(first.external_declarations(), &["external_import"]);
        assert!(first.llvm_ir().contains("define amdgpu_kernel void @entry"));
        assert!(first.llvm_ir().contains("define void @visible_helper"));
        assert!(first.llvm_ir().contains("declare void @external_import"));
        assert!(!first.llvm_ir().contains("bitcode"));
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
}
