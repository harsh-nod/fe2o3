//! Bounded executable integer semantics for a closed Kernel IR V5 subset.
//!
//! The supported subset is intentionally the one exercised by the canonical
//! scalar GEMM V1 graph. Numeric values carried by every supported scalar
//! type, including `f32`-typed KIR values, are interpreted as mathematical
//! integers represented by `i128`. This module does not model IEEE-754
//! arithmetic and its results are not evidence about floating-point behavior.
//! Invocations execute in increasing X order as a deterministic differential
//! oracle; that order is not a GPU scheduling model or a data-race proof.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, BlockId, CastKind, ComparePredicate,
    Constant, Function, FunctionBody, FunctionRole, IndexKind, IntrinsicKind, KernelId,
    KernelIrDecodeError, LaunchDomain, LaunchExtent, Module, Operation, OperationKind,
    SCALAR_GEMM_V1_KERNEL_ID, ScalarGemmTargetRequirementsV1, ScalarGemmV1Error, ScalarType,
    TargetCapability, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrErrorV5,
    VerifiedCanonicalKernelIrV5, WaveWidth, decode_module_v5, gfx942_xnack_minus_target_capability,
    verify_scalar_gemm_v1_module,
};

pub const DEFAULT_INTEGER_ORACLE_MAX_CANONICAL_BYTES_V1: usize = 64 * 1024;
pub const DEFAULT_INTEGER_ORACLE_MAX_INVOCATIONS_V1: u64 = 1 << 20;
pub const DEFAULT_INTEGER_ORACLE_MAX_STEPS_V1: u64 = 1 << 26;
pub const DEFAULT_INTEGER_ORACLE_MAX_BUFFER_ELEMENTS_V1: usize = 1 << 24;
pub const DEFAULT_INTEGER_ORACLE_MAX_TOTAL_BUFFER_ELEMENTS_V1: usize = 1 << 25;
pub const DEFAULT_INTEGER_ORACLE_MAX_SSA_VALUES_V1: usize = 4096;

/// Explicit resource limits for one oracle execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegerSemanticOracleLimitsV1 {
    pub max_canonical_bytes: usize,
    pub max_invocations: u64,
    pub max_steps: u64,
    pub max_buffer_elements: usize,
    pub max_total_buffer_elements: usize,
    pub max_ssa_values: usize,
}

impl Default for IntegerSemanticOracleLimitsV1 {
    fn default() -> Self {
        Self {
            max_canonical_bytes: DEFAULT_INTEGER_ORACLE_MAX_CANONICAL_BYTES_V1,
            max_invocations: DEFAULT_INTEGER_ORACLE_MAX_INVOCATIONS_V1,
            max_steps: DEFAULT_INTEGER_ORACLE_MAX_STEPS_V1,
            max_buffer_elements: DEFAULT_INTEGER_ORACLE_MAX_BUFFER_ELEMENTS_V1,
            max_total_buffer_elements: DEFAULT_INTEGER_ORACLE_MAX_TOTAL_BUFFER_ELEMENTS_V1,
            max_ssa_values: DEFAULT_INTEGER_ORACLE_MAX_SSA_VALUES_V1,
        }
    }
}

/// One owned argument supplied to a Kernel IR entry function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntegerSemanticOracleArgumentV1 {
    Bool(bool),
    Integer(i128),
    Buffer(Vec<i128>),
}

/// A launch request for the general executable subset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerSemanticOracleRequestV1 {
    pub kernel: KernelId,
    pub arguments: Vec<IntegerSemanticOracleArgumentV1>,
    pub global_size: [u64; 3],
}

impl IntegerSemanticOracleRequestV1 {
    pub fn new(
        kernel: impl Into<KernelId>,
        arguments: Vec<IntegerSemanticOracleArgumentV1>,
        global_size: [u64; 3],
    ) -> Self {
        Self {
            kernel: kernel.into(),
            arguments,
            global_size,
        }
    }
}

/// Final mutable arguments and deterministic execution counters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerSemanticOracleExecutionV1 {
    arguments: Vec<IntegerSemanticOracleArgumentV1>,
    invocations_executed: u64,
    steps_executed: u64,
}

impl IntegerSemanticOracleExecutionV1 {
    pub fn arguments(&self) -> &[IntegerSemanticOracleArgumentV1] {
        &self.arguments
    }

    pub const fn invocations_executed(&self) -> u64 {
        self.invocations_executed
    }

    pub const fn steps_executed(&self) -> u64 {
        self.steps_executed
    }

    pub fn buffer(&self, argument: usize) -> Option<&[i128]> {
        match self.arguments.get(argument) {
            Some(IntegerSemanticOracleArgumentV1::Buffer(elements)) => Some(elements),
            _ => None,
        }
    }

    pub fn into_arguments(self) -> Vec<IntegerSemanticOracleArgumentV1> {
        self.arguments
    }

    pub const fn is_verus_proof(&self) -> bool {
        false
    }

    pub const fn models_ieee_f32(&self) -> bool {
        false
    }

    pub const fn proves_race_freedom(&self) -> bool {
        false
    }

    pub const fn grants_compiler_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Owned inputs for the exact scalar GEMM V1 convenience entry point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmIntegerOracleInputV1 {
    pub a: Vec<i128>,
    pub b: Vec<i128>,
    pub c: Vec<i128>,
    pub m: u32,
    pub n: u32,
    pub k: u32,
    /// The number of global X invocations. Values beyond `m * n` exercise the
    /// graph's inactive-invocation guard.
    pub global_invocations: u64,
}

impl ScalarGemmIntegerOracleInputV1 {
    pub fn new(a: Vec<i128>, b: Vec<i128>, c: Vec<i128>, m: u32, n: u32, k: u32) -> Self {
        Self {
            a,
            b,
            c,
            m,
            n,
            k,
            global_invocations: u64::from(m) * u64::from(n),
        }
    }
}

/// Integer-model output from the exact scalar GEMM V1 graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmIntegerOracleExecutionV1 {
    pub c: Vec<i128>,
    pub invocations_executed: u64,
    pub steps_executed: u64,
}

impl ScalarGemmIntegerOracleExecutionV1 {
    pub const fn is_verus_proof(&self) -> bool {
        false
    }

    pub const fn models_ieee_f32(&self) -> bool {
        false
    }

    pub const fn proves_race_freedom(&self) -> bool {
        false
    }

    pub const fn grants_compiler_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Executes the supported subset of one semantically valid canonical V5
/// module using default limits.
pub fn execute_kernel_ir_v5_integer_semantic_oracle_v1(
    canonical_v5: &[u8],
    request: IntegerSemanticOracleRequestV1,
) -> Result<IntegerSemanticOracleExecutionV1, IntegerSemanticOracleErrorV1> {
    execute_kernel_ir_v5_integer_semantic_oracle_with_limits_v1(
        canonical_v5,
        request,
        &IntegerSemanticOracleLimitsV1::default(),
    )
}

/// Executes the supported subset of one semantically valid canonical V5
/// module using caller-selected limits.
pub fn execute_kernel_ir_v5_integer_semantic_oracle_with_limits_v1(
    canonical_v5: &[u8],
    request: IntegerSemanticOracleRequestV1,
    limits: &IntegerSemanticOracleLimitsV1,
) -> Result<IntegerSemanticOracleExecutionV1, IntegerSemanticOracleErrorV1> {
    let module = load_canonical_module(canonical_v5, limits)?;
    execute_module(module, request, limits)
}

/// Executes only the exact canonical scalar GEMM V1 graph using default
/// limits. Any semantically valid graph mutation is rejected before execution.
pub fn execute_scalar_gemm_v1_integer_semantic_oracle_v1(
    canonical_v5: &[u8],
    input: ScalarGemmIntegerOracleInputV1,
) -> Result<ScalarGemmIntegerOracleExecutionV1, IntegerSemanticOracleErrorV1> {
    execute_scalar_gemm_v1_integer_semantic_oracle_with_limits_v1(
        canonical_v5,
        input,
        &IntegerSemanticOracleLimitsV1::default(),
    )
}

/// Executes only the exact canonical scalar GEMM V1 graph using caller-selected
/// limits.
pub fn execute_scalar_gemm_v1_integer_semantic_oracle_with_limits_v1(
    canonical_v5: &[u8],
    input: ScalarGemmIntegerOracleInputV1,
    limits: &IntegerSemanticOracleLimitsV1,
) -> Result<ScalarGemmIntegerOracleExecutionV1, IntegerSemanticOracleErrorV1> {
    let module = load_canonical_module(canonical_v5, limits)?;
    verify_scalar_gemm_v1_module(
        &module,
        ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6(),
    )
    .map_err(IntegerSemanticOracleErrorV1::ScalarGemmProfile)?;

    let request = IntegerSemanticOracleRequestV1::new(
        SCALAR_GEMM_V1_KERNEL_ID,
        vec![
            IntegerSemanticOracleArgumentV1::Buffer(input.a),
            IntegerSemanticOracleArgumentV1::Buffer(input.b),
            IntegerSemanticOracleArgumentV1::Buffer(input.c),
            IntegerSemanticOracleArgumentV1::Integer(i128::from(input.m)),
            IntegerSemanticOracleArgumentV1::Integer(i128::from(input.n)),
            IntegerSemanticOracleArgumentV1::Integer(i128::from(input.k)),
        ],
        [input.global_invocations, 1, 1],
    );
    let execution = execute_module(module, request, limits)?;
    let invocations_executed = execution.invocations_executed;
    let steps_executed = execution.steps_executed;
    let mut arguments = execution.into_arguments();
    let c = match arguments.get_mut(2) {
        Some(IntegerSemanticOracleArgumentV1::Buffer(elements)) => std::mem::take(elements),
        _ => return Err(IntegerSemanticOracleErrorV1::InternalResultShape),
    };
    Ok(ScalarGemmIntegerOracleExecutionV1 {
        c,
        invocations_executed,
        steps_executed,
    })
}

fn load_canonical_module(
    canonical_v5: &[u8],
    limits: &IntegerSemanticOracleLimitsV1,
) -> Result<Module, IntegerSemanticOracleErrorV1> {
    check_usize_limit(
        "canonical bytes",
        canonical_v5.len(),
        limits.max_canonical_bytes,
    )?;
    let owner = VerifiedCanonicalKernelIrV5::from_canonical_bytes(canonical_v5.to_vec())
        .map_err(IntegerSemanticOracleErrorV1::CanonicalKernelIr)?;
    owner
        .revalidate()
        .map_err(IntegerSemanticOracleErrorV1::CanonicalKernelIr)?;
    decode_module_v5(owner.canonical_bytes()).map_err(IntegerSemanticOracleErrorV1::CanonicalDecode)
}

fn execute_module(
    module: Module,
    mut request: IntegerSemanticOracleRequestV1,
    limits: &IntegerSemanticOracleLimitsV1,
) -> Result<IntegerSemanticOracleExecutionV1, IntegerSemanticOracleErrorV1> {
    preflight_capabilities(&module)?;
    validate_buffers(&request.arguments, limits)?;

    let kernel = module
        .kernels
        .iter()
        .find(|kernel| kernel.id == request.kernel)
        .ok_or_else(|| IntegerSemanticOracleErrorV1::UnknownKernel(request.kernel.clone()))?;
    let function = module
        .function(&kernel.entry)
        .ok_or_else(|| IntegerSemanticOracleErrorV1::UnknownEntry(kernel.entry.clone()))?;
    if function.role != FunctionRole::KernelEntry {
        return Err(IntegerSemanticOracleErrorV1::UnsupportedFunctionRole(
            function.role,
        ));
    }
    let body = function
        .body
        .as_ref()
        .ok_or(IntegerSemanticOracleErrorV1::MissingFunctionBody)?;

    let invocation_count = validate_launch(&kernel.domain, request.global_size)?;
    if invocation_count > limits.max_invocations {
        return Err(IntegerSemanticOracleErrorV1::InvocationLimitExceeded {
            actual: invocation_count,
            limit: limits.max_invocations,
        });
    }
    preflight_function(function, body, limits)?;
    let parameter_values = bind_function_arguments(function, body, &request.arguments)?;

    let mut fuel = Fuel {
        consumed: 0,
        limit: limits.max_steps,
    };
    for global_x in 0..invocation_count {
        execute_invocation(
            body,
            &parameter_values,
            &mut request.arguments,
            global_x,
            limits,
            &mut fuel,
        )?;
    }

    Ok(IntegerSemanticOracleExecutionV1 {
        arguments: request.arguments,
        invocations_executed: invocation_count,
        steps_executed: fuel.consumed,
    })
}

fn preflight_capabilities(module: &Module) -> Result<(), IntegerSemanticOracleErrorV1> {
    let supported = BTreeSet::from([
        gfx942_xnack_minus_target_capability(),
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ]);
    ensure_supported_capabilities(&module.required_capabilities, &supported)?;
    for function in &module.functions {
        ensure_supported_capabilities(&function.required_capabilities, &supported)?;
        ensure_supported_capabilities(&function.derived_capabilities(), &supported)?;
    }
    for kernel in &module.kernels {
        ensure_supported_capabilities(&kernel.required_capabilities, &supported)?;
    }
    Ok(())
}

fn ensure_supported_capabilities(
    capabilities: &BTreeSet<TargetCapability>,
    supported: &BTreeSet<TargetCapability>,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    if let Some(capability) = capabilities
        .iter()
        .find(|capability| !supported.contains(*capability))
    {
        Err(IntegerSemanticOracleErrorV1::UnsupportedCapability(
            capability.clone(),
        ))
    } else {
        Ok(())
    }
}

fn validate_buffers(
    arguments: &[IntegerSemanticOracleArgumentV1],
    limits: &IntegerSemanticOracleLimitsV1,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    let mut total = 0usize;
    for (argument, value) in arguments.iter().enumerate() {
        let IntegerSemanticOracleArgumentV1::Buffer(elements) = value else {
            continue;
        };
        if elements.len() > limits.max_buffer_elements {
            return Err(IntegerSemanticOracleErrorV1::BufferElementLimitExceeded {
                argument,
                actual: elements.len(),
                limit: limits.max_buffer_elements,
            });
        }
        total = total.checked_add(elements.len()).ok_or(
            IntegerSemanticOracleErrorV1::TotalBufferElementLimitExceeded {
                actual: usize::MAX,
                limit: limits.max_total_buffer_elements,
            },
        )?;
        if total > limits.max_total_buffer_elements {
            return Err(
                IntegerSemanticOracleErrorV1::TotalBufferElementLimitExceeded {
                    actual: total,
                    limit: limits.max_total_buffer_elements,
                },
            );
        }
    }
    Ok(())
}

fn validate_launch(
    domain: &LaunchDomain,
    global_size: [u64; 3],
) -> Result<u64, IntegerSemanticOracleErrorV1> {
    let LaunchDomain::D1 { x } = domain else {
        return Err(IntegerSemanticOracleErrorV1::UnsupportedLaunchDomain);
    };
    if global_size[1] != 1 || global_size[2] != 1 {
        return Err(IntegerSemanticOracleErrorV1::InvalidLaunchSize { global_size });
    }
    if let LaunchExtent::Static(expected) = x
        && global_size[0] != u64::from(*expected)
    {
        return Err(IntegerSemanticOracleErrorV1::InvalidLaunchSize { global_size });
    }
    Ok(global_size[0])
}

fn preflight_function(
    function: &Function,
    body: &FunctionBody,
    limits: &IntegerSemanticOracleLimitsV1,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    for ty in function
        .signature
        .parameters
        .iter()
        .chain(&function.signature.results)
        .chain(
            body.blocks
                .iter()
                .flat_map(|block| block.parameters.iter().map(|parameter| &parameter.ty)),
        )
        .chain(body.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .flat_map(|operation| operation.results.iter().map(|result| &result.ty))
        }))
    {
        ensure_supported_type(ty)?;
    }

    let static_values = body
        .blocks
        .iter()
        .fold(body.parameters.len(), |total, block| {
            block.operations.iter().fold(
                total.saturating_add(block.parameters.len()),
                |total, operation| total.saturating_add(operation.results.len()),
            )
        });
    if static_values > limits.max_ssa_values {
        return Err(IntegerSemanticOracleErrorV1::SsaValueLimitExceeded {
            actual: static_values,
            limit: limits.max_ssa_values,
        });
    }

    for block in &body.blocks {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            preflight_operation(block.id, operation_index, operation)?;
        }
        preflight_terminator(block.id, block.terminator.as_ref())?;
    }
    Ok(())
}

fn ensure_supported_type(ty: &Type) -> Result<(), IntegerSemanticOracleErrorV1> {
    let supported = match ty {
        Type::Scalar(ScalarType::Bool | ScalarType::U32 | ScalarType::Index | ScalarType::F32) => {
            true
        }
        Type::Slice(slice) => {
            slice.element.as_ref() == &Type::F32
                && slice.address_space == AddressSpace::Global
                && matches!(slice.access, AccessMode::ReadOnly | AccessMode::ReadWrite)
        }
        Type::Pointer(pointer) => {
            pointer.pointee.as_ref() == &Type::F32
                && pointer.address_space == AddressSpace::Global
                && matches!(pointer.access, AccessMode::ReadOnly | AccessMode::ReadWrite)
        }
        _ => false,
    };
    if supported {
        Ok(())
    } else {
        Err(IntegerSemanticOracleErrorV1::UnsupportedType(ty.clone()))
    }
}

fn preflight_operation(
    block: BlockId,
    operation_index: usize,
    operation: &Operation,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    let supported = match &operation.kind {
        OperationKind::Constant(Constant::U32(_))
        | OperationKind::Constant(Constant::F32Bits(0))
        | OperationKind::Intrinsic(crate::IntrinsicOperation {
            kind:
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                },
            result_type: Type::Scalar(ScalarType::Index),
        })
        | OperationKind::Binary {
            op: BinaryOp::Add | BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Remainder,
            ..
        }
        | OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            ..
        }
        | OperationKind::Cast {
            kind: CastKind::ZeroExtend,
            to: Type::Scalar(ScalarType::Index),
            ..
        }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. } => true,
        OperationKind::Load { access, .. } | OperationKind::Store { access, .. } => {
            access.address_space == AddressSpace::Global
                && access.alignment == 4
                && !access.volatile
        }
        _ => false,
    };
    let expected_results = if matches!(operation.kind, OperationKind::Store { .. }) {
        0
    } else {
        1
    };
    if !supported || operation.results.len() != expected_results {
        return Err(IntegerSemanticOracleErrorV1::UnsupportedOperation {
            block,
            operation: operation_index,
            kind: operation_name(&operation.kind),
        });
    }
    Ok(())
}

fn preflight_terminator(
    block: BlockId,
    terminator: Option<&Terminator>,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    let supported = match terminator {
        Some(Terminator::Branch { .. }) | Some(Terminator::ConditionalBranch { .. }) => true,
        Some(Terminator::Return { values }) => values.is_empty(),
        _ => false,
    };
    if supported {
        Ok(())
    } else {
        Err(IntegerSemanticOracleErrorV1::UnsupportedTerminator { block })
    }
}

fn operation_name(kind: &OperationKind) -> &'static str {
    match kind {
        OperationKind::Constant(_) => "constant",
        OperationKind::Intrinsic(_) => "intrinsic",
        OperationKind::MemoryIntrinsic(_) => "memory-intrinsic",
        OperationKind::Unary { .. } => "unary",
        OperationKind::Binary { .. } => "binary",
        OperationKind::Compare { .. } => "compare",
        OperationKind::Cast { .. } => "cast",
        OperationKind::Select { .. } => "select",
        OperationKind::Call { .. } => "call",
        OperationKind::Alloca { .. } => "alloca",
        OperationKind::SliceLength { .. } => "slice-length",
        OperationKind::SliceData { .. } => "slice-data",
        OperationKind::GetElementPointer { .. } => "get-element-pointer",
        OperationKind::Load { .. } => "load",
        OperationKind::Store { .. } => "store",
        OperationKind::Barrier(_) => "barrier",
        OperationKind::Atomic(_) => "atomic",
        OperationKind::Fence(_) => "fence",
        OperationKind::WorkgroupBarrier(_) => "workgroup-barrier",
        OperationKind::WorkgroupMemory(_) => "workgroup-memory",
        OperationKind::Matrix(_) => "matrix",
        OperationKind::Wave(_) => "wave",
        OperationKind::InlineAssembly(_) => "inline-assembly",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuntimeValue {
    Bool(bool),
    Integer {
        value: i128,
        ty: ScalarType,
    },
    Slice {
        argument: usize,
        element: Type,
        address_space: AddressSpace,
        access: AccessMode,
    },
    Pointer {
        argument: usize,
        element: Type,
        address_space: AddressSpace,
        access: AccessMode,
        offset: usize,
    },
}

impl RuntimeValue {
    fn ty(&self) -> Type {
        match self {
            Self::Bool(_) => Type::BOOL,
            Self::Integer { ty, .. } => Type::Scalar(*ty),
            Self::Slice {
                element,
                address_space,
                access,
                ..
            } => Type::slice(element.clone(), *address_space, *access),
            Self::Pointer {
                element,
                address_space,
                access,
                ..
            } => Type::pointer(element.clone(), *address_space, *access),
        }
    }

    fn integer(
        &self,
        value_id: ValueId,
    ) -> Result<(i128, ScalarType), IntegerSemanticOracleErrorV1> {
        match self {
            Self::Integer { value, ty } => Ok((*value, *ty)),
            _ => Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                value: value_id,
                expected: "integer-model numeric value",
            }),
        }
    }

    fn boolean(&self, value_id: ValueId) -> Result<bool, IntegerSemanticOracleErrorV1> {
        match self {
            Self::Bool(value) => Ok(*value),
            _ => Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                value: value_id,
                expected: "boolean",
            }),
        }
    }
}

fn bind_function_arguments(
    function: &Function,
    body: &FunctionBody,
    arguments: &[IntegerSemanticOracleArgumentV1],
) -> Result<Vec<(ValueId, RuntimeValue)>, IntegerSemanticOracleErrorV1> {
    if arguments.len() != function.signature.parameters.len()
        || body.parameters.len() != function.signature.parameters.len()
    {
        return Err(IntegerSemanticOracleErrorV1::ArgumentCount {
            expected: function.signature.parameters.len(),
            actual: arguments.len(),
        });
    }
    body.parameters
        .iter()
        .copied()
        .zip(&function.signature.parameters)
        .zip(arguments)
        .enumerate()
        .map(|(argument, ((value_id, ty), supplied))| {
            runtime_argument(argument, ty, supplied).map(|value| (value_id, value))
        })
        .collect()
}

fn runtime_argument(
    argument: usize,
    ty: &Type,
    supplied: &IntegerSemanticOracleArgumentV1,
) -> Result<RuntimeValue, IntegerSemanticOracleErrorV1> {
    match (ty, supplied) {
        (Type::Scalar(ScalarType::Bool), IntegerSemanticOracleArgumentV1::Bool(value)) => {
            Ok(RuntimeValue::Bool(*value))
        }
        (
            Type::Scalar(scalar @ (ScalarType::U32 | ScalarType::Index | ScalarType::F32)),
            IntegerSemanticOracleArgumentV1::Integer(value),
        ) => checked_integer(*value, *scalar),
        (Type::Slice(slice), IntegerSemanticOracleArgumentV1::Buffer(_)) => {
            Ok(RuntimeValue::Slice {
                argument,
                element: slice.element.as_ref().clone(),
                address_space: slice.address_space,
                access: slice.access,
            })
        }
        _ => Err(IntegerSemanticOracleErrorV1::ArgumentType { argument }),
    }
}

fn execute_invocation(
    body: &FunctionBody,
    parameters: &[(ValueId, RuntimeValue)],
    arguments: &mut [IntegerSemanticOracleArgumentV1],
    global_x: u64,
    limits: &IntegerSemanticOracleLimitsV1,
    fuel: &mut Fuel,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    let block_indices = body
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.id, index))
        .collect::<BTreeMap<_, _>>();
    let entry = body
        .blocks
        .first()
        .ok_or(IntegerSemanticOracleErrorV1::MissingEntryBlock)?;
    let mut values = parameters.iter().cloned().collect::<BTreeMap<_, _>>();
    let mut current = entry.id;
    let mut incoming = Vec::new();

    loop {
        let block_index = *block_indices
            .get(&current)
            .ok_or(IntegerSemanticOracleErrorV1::UnknownBlock(current))?;
        let block = &body.blocks[block_index];
        bind_block_parameters(block, &incoming, &mut values, limits)?;

        for (operation_index, operation) in block.operations.iter().enumerate() {
            fuel.consume()?;
            let results = execute_operation(
                operation,
                arguments,
                &values,
                global_x,
                block.id,
                operation_index,
            )?;
            bind_results(operation, results, &mut values, limits)?;
        }

        fuel.consume()?;
        let terminator = block
            .terminator
            .as_ref()
            .ok_or(IntegerSemanticOracleErrorV1::UnsupportedTerminator { block: block.id })?;
        match terminator {
            Terminator::Branch { target, arguments } => {
                incoming = resolve_values(arguments, &values)?;
                current = *target;
            }
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                let take_then = value(&values, *condition)?.boolean(*condition)?;
                let (target, arguments) = if take_then {
                    (then_target, then_arguments)
                } else {
                    (else_target, else_arguments)
                };
                incoming = resolve_values(arguments, &values)?;
                current = *target;
            }
            Terminator::Return { values: returned } if returned.is_empty() => return Ok(()),
            _ => {
                return Err(IntegerSemanticOracleErrorV1::UnsupportedTerminator {
                    block: block.id,
                });
            }
        }
    }
}

fn bind_block_parameters(
    block: &BasicBlock,
    incoming: &[RuntimeValue],
    values: &mut BTreeMap<ValueId, RuntimeValue>,
    limits: &IntegerSemanticOracleLimitsV1,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    if block.parameters.len() != incoming.len() {
        return Err(IntegerSemanticOracleErrorV1::BlockArgumentCount {
            block: block.id,
            expected: block.parameters.len(),
            actual: incoming.len(),
        });
    }
    for (parameter, incoming) in block.parameters.iter().zip(incoming) {
        bind_value(parameter, incoming.clone(), values, limits)?;
    }
    Ok(())
}

fn bind_results(
    operation: &Operation,
    results: Vec<RuntimeValue>,
    values: &mut BTreeMap<ValueId, RuntimeValue>,
    limits: &IntegerSemanticOracleLimitsV1,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    if operation.results.len() != results.len() {
        return Err(IntegerSemanticOracleErrorV1::RuntimeResultArity);
    }
    for (definition, result) in operation.results.iter().zip(results) {
        bind_value(definition, result, values, limits)?;
    }
    Ok(())
}

fn bind_value(
    definition: &ValueDef,
    runtime: RuntimeValue,
    values: &mut BTreeMap<ValueId, RuntimeValue>,
    limits: &IntegerSemanticOracleLimitsV1,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    if runtime.ty() != definition.ty {
        return Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
            value: definition.id,
            expected: "declared SSA result type",
        });
    }
    if !values.contains_key(&definition.id) && values.len() == limits.max_ssa_values {
        return Err(IntegerSemanticOracleErrorV1::SsaValueLimitExceeded {
            actual: values.len().saturating_add(1),
            limit: limits.max_ssa_values,
        });
    }
    values.insert(definition.id, runtime);
    Ok(())
}

fn resolve_values(
    ids: &[ValueId],
    values: &BTreeMap<ValueId, RuntimeValue>,
) -> Result<Vec<RuntimeValue>, IntegerSemanticOracleErrorV1> {
    ids.iter().map(|id| value(values, *id).cloned()).collect()
}

fn value(
    values: &BTreeMap<ValueId, RuntimeValue>,
    id: ValueId,
) -> Result<&RuntimeValue, IntegerSemanticOracleErrorV1> {
    values
        .get(&id)
        .ok_or(IntegerSemanticOracleErrorV1::UndefinedRuntimeValue(id))
}

fn execute_operation(
    operation: &Operation,
    arguments: &mut [IntegerSemanticOracleArgumentV1],
    values: &BTreeMap<ValueId, RuntimeValue>,
    global_x: u64,
    block: BlockId,
    operation_index: usize,
) -> Result<Vec<RuntimeValue>, IntegerSemanticOracleErrorV1> {
    let one = |runtime| Ok(vec![runtime]);
    match &operation.kind {
        OperationKind::Constant(Constant::U32(value)) => one(RuntimeValue::Integer {
            value: i128::from(*value),
            ty: ScalarType::U32,
        }),
        OperationKind::Constant(Constant::F32Bits(0)) => one(RuntimeValue::Integer {
            value: 0,
            ty: ScalarType::F32,
        }),
        OperationKind::Intrinsic(crate::IntrinsicOperation {
            kind:
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                },
            ..
        }) => one(RuntimeValue::Integer {
            value: i128::from(global_x),
            ty: ScalarType::Index,
        }),
        OperationKind::Binary { op, lhs, rhs } => {
            let (lhs_value, lhs_ty) = value(values, *lhs)?.integer(*lhs)?;
            let (rhs_value, rhs_ty) = value(values, *rhs)?.integer(*rhs)?;
            if lhs_ty != rhs_ty {
                return Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                    value: *rhs,
                    expected: "matching binary operand type",
                });
            }
            let computed = match op {
                BinaryOp::Add => lhs_value.checked_add(rhs_value),
                BinaryOp::Multiply => lhs_value.checked_mul(rhs_value),
                BinaryOp::Divide => {
                    if rhs_value == 0 {
                        return Err(IntegerSemanticOracleErrorV1::DivisionByZero {
                            block,
                            operation: operation_index,
                        });
                    }
                    lhs_value.checked_div(rhs_value)
                }
                BinaryOp::Remainder => {
                    if rhs_value == 0 {
                        return Err(IntegerSemanticOracleErrorV1::DivisionByZero {
                            block,
                            operation: operation_index,
                        });
                    }
                    lhs_value.checked_rem(rhs_value)
                }
                _ => None,
            }
            .ok_or(IntegerSemanticOracleErrorV1::ArithmeticOverflow {
                block,
                operation: operation_index,
            })?;
            one(checked_integer(computed, lhs_ty)?)
        }
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } => {
            let (lhs_value, lhs_ty) = value(values, *lhs)?.integer(*lhs)?;
            let (rhs_value, rhs_ty) = value(values, *rhs)?.integer(*rhs)?;
            if lhs_ty != rhs_ty {
                return Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                    value: *rhs,
                    expected: "matching comparison operand type",
                });
            }
            one(RuntimeValue::Bool(lhs_value < rhs_value))
        }
        OperationKind::Cast {
            kind: CastKind::ZeroExtend,
            value: source_id,
            to: Type::Scalar(ScalarType::Index),
        } => {
            let (source, source_ty) = value(values, *source_id)?.integer(*source_id)?;
            if source_ty != ScalarType::U32 {
                return Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                    value: *source_id,
                    expected: "u32 zero-extension source",
                });
            }
            one(checked_integer(source, ScalarType::Index)?)
        }
        OperationKind::SliceData { slice } => match value(values, *slice)? {
            RuntimeValue::Slice {
                argument,
                element,
                address_space,
                access,
            } => one(RuntimeValue::Pointer {
                argument: *argument,
                element: element.clone(),
                address_space: *address_space,
                access: *access,
                offset: 0,
            }),
            _ => Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                value: *slice,
                expected: "slice",
            }),
        },
        OperationKind::GetElementPointer { base, offset } => {
            let (relative, _) = value(values, *offset)?.integer(*offset)?;
            let relative = usize::try_from(relative).map_err(|_| {
                IntegerSemanticOracleErrorV1::PointerOffsetOutOfRange { value: *offset }
            })?;
            match value(values, *base)? {
                RuntimeValue::Pointer {
                    argument,
                    element,
                    address_space,
                    access,
                    offset,
                } => one(RuntimeValue::Pointer {
                    argument: *argument,
                    element: element.clone(),
                    address_space: *address_space,
                    access: *access,
                    offset: offset.checked_add(relative).ok_or(
                        IntegerSemanticOracleErrorV1::PointerOffsetOutOfRange { value: *base },
                    )?,
                }),
                _ => Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                    value: *base,
                    expected: "pointer",
                }),
            }
        }
        OperationKind::Load { pointer, access } => {
            let RuntimeValue::Pointer {
                argument,
                element,
                address_space,
                offset,
                ..
            } = value(values, *pointer)?
            else {
                return Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                    value: *pointer,
                    expected: "pointer",
                });
            };
            ensure_memory_access(*address_space, access.address_space, *pointer)?;
            let elements = buffer(arguments, *argument)?;
            let loaded =
                *elements
                    .get(*offset)
                    .ok_or(IntegerSemanticOracleErrorV1::MemoryOutOfBounds {
                        argument: *argument,
                        index: *offset,
                        length: elements.len(),
                    })?;
            let Type::Scalar(scalar) = element else {
                return Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                    value: *pointer,
                    expected: "scalar element pointer",
                });
            };
            one(checked_integer(loaded, *scalar)?)
        }
        OperationKind::Store {
            pointer,
            value: stored,
            access: memory_access,
        } => {
            let RuntimeValue::Pointer {
                argument,
                element,
                address_space,
                access,
                offset,
            } = value(values, *pointer)?
            else {
                return Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                    value: *pointer,
                    expected: "pointer",
                });
            };
            ensure_memory_access(*address_space, memory_access.address_space, *pointer)?;
            if *access != AccessMode::ReadWrite {
                return Err(IntegerSemanticOracleErrorV1::WriteThroughReadOnlyPointer {
                    value: *pointer,
                });
            }
            let (stored_value, stored_ty) = value(values, *stored)?.integer(*stored)?;
            if &Type::Scalar(stored_ty) != element {
                return Err(IntegerSemanticOracleErrorV1::RuntimeTypeMismatch {
                    value: *stored,
                    expected: "pointer element type",
                });
            }
            let elements = buffer_mut(arguments, *argument)?;
            let length = elements.len();
            let slot = elements.get_mut(*offset).ok_or(
                IntegerSemanticOracleErrorV1::MemoryOutOfBounds {
                    argument: *argument,
                    index: *offset,
                    length,
                },
            )?;
            *slot = stored_value;
            Ok(Vec::new())
        }
        _ => Err(IntegerSemanticOracleErrorV1::UnsupportedOperation {
            block,
            operation: operation_index,
            kind: operation_name(&operation.kind),
        }),
    }
}

fn checked_integer(
    value: i128,
    ty: ScalarType,
) -> Result<RuntimeValue, IntegerSemanticOracleErrorV1> {
    let in_range = match ty {
        ScalarType::U32 => (0..=i128::from(u32::MAX)).contains(&value),
        ScalarType::Index => (0..=i128::from(u64::MAX)).contains(&value),
        ScalarType::F32 => true,
        _ => false,
    };
    if !in_range {
        return Err(IntegerSemanticOracleErrorV1::IntegerOutOfRange { value, ty });
    }
    Ok(RuntimeValue::Integer { value, ty })
}

fn ensure_memory_access(
    pointer: AddressSpace,
    access: AddressSpace,
    value: ValueId,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    if pointer == AddressSpace::Global && access == pointer {
        Ok(())
    } else {
        Err(IntegerSemanticOracleErrorV1::MemoryAddressSpaceMismatch { value })
    }
}

fn buffer(
    arguments: &[IntegerSemanticOracleArgumentV1],
    argument: usize,
) -> Result<&[i128], IntegerSemanticOracleErrorV1> {
    match arguments.get(argument) {
        Some(IntegerSemanticOracleArgumentV1::Buffer(elements)) => Ok(elements),
        _ => Err(IntegerSemanticOracleErrorV1::ArgumentType { argument }),
    }
}

fn buffer_mut(
    arguments: &mut [IntegerSemanticOracleArgumentV1],
    argument: usize,
) -> Result<&mut [i128], IntegerSemanticOracleErrorV1> {
    match arguments.get_mut(argument) {
        Some(IntegerSemanticOracleArgumentV1::Buffer(elements)) => Ok(elements),
        _ => Err(IntegerSemanticOracleErrorV1::ArgumentType { argument }),
    }
}

fn check_usize_limit(
    resource: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), IntegerSemanticOracleErrorV1> {
    if actual <= limit {
        Ok(())
    } else {
        Err(IntegerSemanticOracleErrorV1::ResourceLimitExceeded {
            resource,
            actual,
            limit,
        })
    }
}

struct Fuel {
    consumed: u64,
    limit: u64,
}

impl Fuel {
    fn consume(&mut self) -> Result<(), IntegerSemanticOracleErrorV1> {
        if self.consumed == self.limit {
            return Err(IntegerSemanticOracleErrorV1::FuelExhausted {
                consumed: self.consumed,
                limit: self.limit,
            });
        }
        self.consumed += 1;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IntegerSemanticOracleErrorV1 {
    CanonicalKernelIr(VerifiedCanonicalKernelIrErrorV5),
    CanonicalDecode(KernelIrDecodeError),
    ScalarGemmProfile(ScalarGemmV1Error),
    ResourceLimitExceeded {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    InvocationLimitExceeded {
        actual: u64,
        limit: u64,
    },
    BufferElementLimitExceeded {
        argument: usize,
        actual: usize,
        limit: usize,
    },
    TotalBufferElementLimitExceeded {
        actual: usize,
        limit: usize,
    },
    SsaValueLimitExceeded {
        actual: usize,
        limit: usize,
    },
    FuelExhausted {
        consumed: u64,
        limit: u64,
    },
    UnknownKernel(KernelId),
    UnknownEntry(crate::FunctionId),
    UnsupportedFunctionRole(FunctionRole),
    MissingFunctionBody,
    MissingEntryBlock,
    UnsupportedLaunchDomain,
    InvalidLaunchSize {
        global_size: [u64; 3],
    },
    UnsupportedCapability(TargetCapability),
    UnsupportedType(Type),
    UnsupportedOperation {
        block: BlockId,
        operation: usize,
        kind: &'static str,
    },
    UnsupportedTerminator {
        block: BlockId,
    },
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    ArgumentType {
        argument: usize,
    },
    UnknownBlock(BlockId),
    BlockArgumentCount {
        block: BlockId,
        expected: usize,
        actual: usize,
    },
    UndefinedRuntimeValue(ValueId),
    RuntimeTypeMismatch {
        value: ValueId,
        expected: &'static str,
    },
    RuntimeResultArity,
    IntegerOutOfRange {
        value: i128,
        ty: ScalarType,
    },
    ArithmeticOverflow {
        block: BlockId,
        operation: usize,
    },
    DivisionByZero {
        block: BlockId,
        operation: usize,
    },
    PointerOffsetOutOfRange {
        value: ValueId,
    },
    MemoryOutOfBounds {
        argument: usize,
        index: usize,
        length: usize,
    },
    MemoryAddressSpaceMismatch {
        value: ValueId,
    },
    WriteThroughReadOnlyPointer {
        value: ValueId,
    },
    InternalResultShape,
}

impl fmt::Display for IntegerSemanticOracleErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CanonicalKernelIr(error) => {
                write!(formatter, "invalid canonical KIR V5: {error}")
            }
            Self::CanonicalDecode(error) => {
                write!(formatter, "revalidated KIR V5 failed to decode: {error}")
            }
            Self::ScalarGemmProfile(error) => {
                write!(formatter, "not exact scalar GEMM V1: {error}")
            }
            Self::ResourceLimitExceeded {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "{resource} resource limit exceeded: {actual} > {limit}"
            ),
            Self::InvocationLimitExceeded { actual, limit } => {
                write!(formatter, "invocation limit exceeded: {actual} > {limit}")
            }
            Self::BufferElementLimitExceeded {
                argument,
                actual,
                limit,
            } => write!(
                formatter,
                "argument {argument} buffer limit exceeded: {actual} > {limit}"
            ),
            Self::TotalBufferElementLimitExceeded { actual, limit } => {
                write!(formatter, "total buffer limit exceeded: {actual} > {limit}")
            }
            Self::SsaValueLimitExceeded { actual, limit } => {
                write!(formatter, "SSA value limit exceeded: {actual} > {limit}")
            }
            Self::FuelExhausted { consumed, limit } => write!(
                formatter,
                "execution fuel exhausted after {consumed} of {limit} steps"
            ),
            Self::UnknownKernel(kernel) => write!(formatter, "unknown kernel {kernel}"),
            Self::UnknownEntry(function) => write!(formatter, "unknown kernel entry {function}"),
            Self::UnsupportedFunctionRole(role) => {
                write!(formatter, "unsupported function role {role:?}")
            }
            Self::MissingFunctionBody => formatter.write_str("selected kernel entry has no body"),
            Self::MissingEntryBlock => {
                formatter.write_str("selected kernel entry has no entry block")
            }
            Self::UnsupportedLaunchDomain => {
                formatter.write_str("only one-dimensional launches are supported")
            }
            Self::InvalidLaunchSize { global_size } => {
                write!(formatter, "invalid launch size {global_size:?}")
            }
            Self::UnsupportedCapability(capability) => {
                write!(formatter, "unsupported capability {capability:?}")
            }
            Self::UnsupportedType(ty) => write!(formatter, "unsupported KIR type {ty:?}"),
            Self::UnsupportedOperation {
                block,
                operation,
                kind,
            } => write!(
                formatter,
                "unsupported {kind} operation at {block}, op {operation}"
            ),
            Self::UnsupportedTerminator { block } => {
                write!(formatter, "unsupported terminator in {block}")
            }
            Self::ArgumentCount { expected, actual } => write!(
                formatter,
                "argument count mismatch: expected {expected}, found {actual}"
            ),
            Self::ArgumentType { argument } => {
                write!(formatter, "argument {argument} has the wrong runtime kind")
            }
            Self::UnknownBlock(block) => write!(formatter, "unknown runtime block {block}"),
            Self::BlockArgumentCount {
                block,
                expected,
                actual,
            } => write!(
                formatter,
                "block argument mismatch at {block}: expected {expected}, found {actual}"
            ),
            Self::UndefinedRuntimeValue(value) => {
                write!(formatter, "undefined runtime value {value}")
            }
            Self::RuntimeTypeMismatch { value, expected } => {
                write!(formatter, "runtime value {value} is not {expected}")
            }
            Self::RuntimeResultArity => formatter.write_str("runtime result arity mismatch"),
            Self::IntegerOutOfRange { value, ty } => {
                write!(formatter, "integer {value} is outside {ty:?}")
            }
            Self::ArithmeticOverflow { block, operation } => write!(
                formatter,
                "integer arithmetic overflow at {block}, op {operation}"
            ),
            Self::DivisionByZero { block, operation } => write!(
                formatter,
                "integer division by zero at {block}, op {operation}"
            ),
            Self::PointerOffsetOutOfRange { value } => {
                write!(formatter, "pointer offset {value} is out of range")
            }
            Self::MemoryOutOfBounds {
                argument,
                index,
                length,
            } => write!(
                formatter,
                "argument {argument} memory access {index} is outside length {length}"
            ),
            Self::MemoryAddressSpaceMismatch { value } => write!(
                formatter,
                "pointer {value} has an unsupported address space"
            ),
            Self::WriteThroughReadOnlyPointer { value } => {
                write!(formatter, "cannot write through read-only pointer {value}")
            }
            Self::InternalResultShape => {
                formatter.write_str("oracle produced an invalid result shape")
            }
        }
    }
}

impl Error for IntegerSemanticOracleErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CanonicalKernelIr(error) => Some(error),
            Self::CanonicalDecode(error) => Some(error),
            Self::ScalarGemmProfile(error) => Some(error),
            _ => None,
        }
    }
}
