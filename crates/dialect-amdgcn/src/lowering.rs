use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use fe2o3_kernel_ir::{
    AddressSpace as KernelAddressSpace, Atomic, AtomicKind, Axis, BasicBlock, BinaryOp, BlockId,
    CastKind, ComparePredicate, Constant, DiagnosticCode as VerificationDiagnosticCode, Function,
    FunctionId, IndexKind, IntrinsicKind, Kernel, KernelId, LaunchDomain, LaunchExtent,
    MemoryOrdering, Module, ModuleId, Operation, OperationKind, ScalarType, SynchronizationScope,
    TargetCapability, Terminator, Type, ValueId, VerificationErrors, WaveWidth,
    WorkgroupMemoryExtent, WorkgroupSize, verify_module,
};

use crate::{AMDGPU_TRIPLE, AmdgcnIntrinsic, Dim};

const MAX_G1_WORKGROUP_SIZE: u32 = 1024;

/// Stable rejection categories for the first target-neutral AMDGPU lowering slice.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LoweringDiagnosticCode {
    InputVerification(VerificationDiagnosticCode),
    MissingKernel,
    AmbiguousKernel,
    UnsafeSymbolName,
    UnsupportedLaunchDomain,
    MissingWorkgroupSize,
    UnsupportedWorkgroupSize,
    UnsupportedCapability,
    KernelEntryDeclaration,
    UnsupportedResults,
    UnsupportedParameter,
    UnsupportedType,
    UnsupportedAddressSpace,
    UnsupportedBlockArguments,
    UnsupportedOperation,
    UnsupportedAtomic,
    UnsupportedBarrier,
    UnsupportedWorkgroupMemory,
    UnsupportedCast,
    UnsupportedConstant,
    UnsupportedTerminator,
}

/// A deterministic source location in the kernel IR.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LoweringLocation {
    pub module: ModuleId,
    pub kernel: Option<KernelId>,
    pub function: Option<FunctionId>,
    pub block: Option<BlockId>,
    pub operation: Option<usize>,
}

impl LoweringLocation {
    fn module(module: &Module) -> Self {
        Self {
            module: module.id.clone(),
            kernel: None,
            function: None,
            block: None,
            operation: None,
        }
    }

    fn kernel(module: &Module, kernel: &Kernel) -> Self {
        Self {
            kernel: Some(kernel.id.clone()),
            ..Self::module(module)
        }
    }

    fn function(module: &Module, kernel: &Kernel, function: &Function) -> Self {
        Self {
            function: Some(function.id.clone()),
            ..Self::kernel(module, kernel)
        }
    }

    fn block(module: &Module, kernel: &Kernel, function: &Function, block: BlockId) -> Self {
        Self {
            block: Some(block),
            ..Self::function(module, kernel, function)
        }
    }

    fn operation(
        module: &Module,
        kernel: &Kernel,
        function: &Function,
        block: BlockId,
        operation: usize,
    ) -> Self {
        Self {
            operation: Some(operation),
            ..Self::block(module, kernel, function, block)
        }
    }
}

impl fmt::Display for LoweringLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "module {}", self.module)?;
        if let Some(kernel) = &self.kernel {
            write!(formatter, ", kernel {kernel}")?;
        }
        if let Some(function) = &self.function {
            write!(formatter, ", function {function}")?;
        }
        if let Some(block) = self.block {
            write!(formatter, ", {block}")?;
        }
        if let Some(operation) = self.operation {
            write!(formatter, ", op {operation}")?;
        }
        Ok(())
    }
}

/// One stable, located lowering diagnostic.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LoweringDiagnostic {
    pub location: LoweringLocation,
    pub code: LoweringDiagnosticCode,
    pub message: String,
}

impl fmt::Display for LoweringDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {:?}: {}",
            self.location, self.code, self.message
        )
    }
}

/// A deterministic set of errors produced before textual emission succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweringErrors {
    diagnostics: Vec<LoweringDiagnostic>,
}

impl LoweringErrors {
    pub fn diagnostics(&self) -> &[LoweringDiagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<LoweringDiagnostic> {
        self.diagnostics
    }

    pub fn contains(&self, code: LoweringDiagnosticCode) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)
    }

    fn one(
        location: LoweringLocation,
        code: LoweringDiagnosticCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            diagnostics: vec![LoweringDiagnostic {
                location,
                code,
                message: message.into(),
            }],
        }
    }

    fn verification(errors: VerificationErrors) -> Self {
        let diagnostics = errors
            .into_diagnostics()
            .into_iter()
            .map(|diagnostic| LoweringDiagnostic {
                location: LoweringLocation {
                    module: diagnostic.location.module,
                    kernel: diagnostic.location.kernel,
                    function: diagnostic.location.function,
                    block: diagnostic.location.block,
                    operation: diagnostic.location.operation,
                },
                code: LoweringDiagnosticCode::InputVerification(diagnostic.code),
                message: diagnostic.message,
            })
            .collect();
        Self { diagnostics }
    }
}

impl fmt::Display for LoweringErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "AMDGPU lowering failed with {} diagnostic(s)",
            self.diagnostics.len()
        )?;
        for diagnostic in &self.diagnostics {
            writeln!(formatter, "  {diagnostic}")?;
        }
        Ok(())
    }
}

impl Error for LoweringErrors {}

/// Lowers one exact kernel to deterministic textual AMDGPU LLVM IR.
///
/// This is a deliberately small, non-production lowering seam. It always calls
/// [`verify_module`] before selecting the kernel, then rejects every valid IR construct outside
/// the documented G1 subset. The launch extent remains a host contract; LLVM IR records the
/// required workgroup size, while launch-grid selection remains outside this API.
pub fn lower_kernel_to_llvm_ir(
    module: &Module,
    kernel_id: &KernelId,
) -> Result<String, LoweringErrors> {
    verify_module(module).map_err(LoweringErrors::verification)?;

    let matches = module
        .kernels
        .iter()
        .filter(|kernel| &kernel.id == kernel_id)
        .collect::<Vec<_>>();
    let kernel = match matches.as_slice() {
        [] => {
            return Err(LoweringErrors::one(
                LoweringLocation::module(module),
                LoweringDiagnosticCode::MissingKernel,
                format!("kernel {kernel_id} is not in the module"),
            ));
        }
        [kernel] => *kernel,
        _ => {
            return Err(LoweringErrors::one(
                LoweringLocation::module(module),
                LoweringDiagnosticCode::AmbiguousKernel,
                format!("kernel identity {kernel_id} is ambiguous"),
            ));
        }
    };

    if !is_safe_symbol(kernel.id.as_str()) {
        return Err(LoweringErrors::one(
            LoweringLocation::kernel(module, kernel),
            LoweringDiagnosticCode::UnsafeSymbolName,
            "kernel identity is not a safe unquoted LLVM symbol",
        ));
    }

    let module_wave = validate_capabilities(
        LoweringLocation::module(module),
        &module.required_capabilities,
        "module",
    )?;
    let kernel_wave = validate_capabilities(
        LoweringLocation::kernel(module, kernel),
        &kernel.required_capabilities,
        "kernel",
    )?;

    let workgroup_size = validate_launch(module, kernel)?;
    let entry = module
        .functions
        .iter()
        .find(|function| function.id == kernel.entry)
        .expect("verify_module established the kernel entry");
    let body = entry.body.as_ref().ok_or_else(|| {
        LoweringErrors::one(
            LoweringLocation::function(module, kernel, entry),
            LoweringDiagnosticCode::KernelEntryDeclaration,
            "kernel entry must be a definition",
        )
    })?;

    let function_wave = validate_capabilities(
        LoweringLocation::function(module, kernel, entry),
        &entry.required_capabilities,
        "entry function",
    )?;
    let wave_widths = [module_wave, kernel_wave, function_wave]
        .into_iter()
        .flatten()
        .collect::<BTreeSet<_>>();
    if wave_widths.len() > 1 {
        return Err(LoweringErrors::one(
            LoweringLocation::function(module, kernel, entry),
            LoweringDiagnosticCode::UnsupportedCapability,
            format!("conflicting exact wave-width requirements: {wave_widths:?}"),
        ));
    }
    let wave_width = wave_widths.first().copied();
    if !entry.signature.results.is_empty() {
        return Err(LoweringErrors::one(
            LoweringLocation::function(module, kernel, entry),
            LoweringDiagnosticCode::UnsupportedResults,
            "G1 kernel entries must return void",
        ));
    }

    let mut lowerer = FunctionLowerer::new(module, kernel, entry, workgroup_size.x, wave_width);
    lowerer.validate_parameters()?;
    for block in &body.blocks {
        lowerer.validate_block(block)?;
    }
    lowerer.validate_block_arguments()?;
    lowerer.emit()
}

fn is_safe_symbol(symbol: &str) -> bool {
    let mut bytes = symbol.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn validate_capabilities(
    location: LoweringLocation,
    capabilities: &BTreeSet<TargetCapability>,
    owner: &str,
) -> Result<Option<WaveWidth>, LoweringErrors> {
    let mut wave_width = None;
    for capability in capabilities {
        match capability {
            TargetCapability::WorkgroupMemory
            | TargetCapability::WorkgroupBarrier
            | TargetCapability::DynamicWorkgroupMemory => {}
            TargetCapability::WaveWidth(width) => wave_width = Some(*width),
            TargetCapability::Atomic {
                width_bits,
                address_space,
                max_scope,
            } if supported_atomic_capability(*width_bits, *address_space, *max_scope) => {}
            _ => {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedCapability,
                    format!("G1 does not lower {owner} capability {capability:?}"),
                ));
            }
        }
    }
    Ok(wave_width)
}

fn validate_launch(module: &Module, kernel: &Kernel) -> Result<WorkgroupSize, LoweringErrors> {
    match kernel.domain {
        LaunchDomain::D1 {
            x: LaunchExtent::Static(_) | LaunchExtent::Dynamic,
        } => {}
        LaunchDomain::D2 { .. } | LaunchDomain::D3 { .. } => {
            return Err(LoweringErrors::one(
                LoweringLocation::kernel(module, kernel),
                LoweringDiagnosticCode::UnsupportedLaunchDomain,
                "G1 supports only a 1D launch domain",
            ));
        }
    }

    let Some(size) = kernel.workgroup_size else {
        return Err(LoweringErrors::one(
            LoweringLocation::kernel(module, kernel),
            LoweringDiagnosticCode::MissingWorkgroupSize,
            "G1 requires a statically declared workgroup size",
        ));
    };
    if size.x > MAX_G1_WORKGROUP_SIZE || size.y != 1 || size.z != 1 {
        return Err(LoweringErrors::one(
            LoweringLocation::kernel(module, kernel),
            LoweringDiagnosticCode::UnsupportedWorkgroupSize,
            format!(
                "G1 requires workgroup dimensions (x, 1, 1) with x at most {MAX_G1_WORKGROUP_SIZE}"
            ),
        ));
    }
    Ok(size)
}

#[derive(Clone)]
enum ValueBinding {
    Value {
        llvm_name: String,
        ty: Type,
    },
    Slice {
        data_name: String,
        length_name: String,
        ty: Type,
    },
}

impl ValueBinding {
    fn value(&self) -> Option<(&str, &Type)> {
        match self {
            Self::Value { llvm_name, ty } => Some((llvm_name, ty)),
            Self::Slice { .. } => None,
        }
    }
}

struct FunctionLowerer<'a> {
    module: &'a Module,
    kernel: &'a Kernel,
    function: &'a Function,
    workgroup_x: u32,
    wave_width: Option<WaveWidth>,
    bindings: BTreeMap<ValueId, ValueBinding>,
}

impl<'a> FunctionLowerer<'a> {
    fn new(
        module: &'a Module,
        kernel: &'a Kernel,
        function: &'a Function,
        workgroup_x: u32,
        wave_width: Option<WaveWidth>,
    ) -> Self {
        Self {
            module,
            kernel,
            function,
            workgroup_x,
            wave_width,
            bindings: BTreeMap::new(),
        }
    }

    fn validate_parameters(&mut self) -> Result<(), LoweringErrors> {
        let body = self.function.body.as_ref().expect("definition required");
        for (index, (value, ty)) in body
            .parameters
            .iter()
            .copied()
            .zip(&self.function.signature.parameters)
            .enumerate()
        {
            let location = LoweringLocation::function(self.module, self.kernel, self.function);
            match ty {
                Type::Scalar(scalar) => {
                    if !supported_scalar(*scalar) {
                        return Err(LoweringErrors::one(
                            location,
                            LoweringDiagnosticCode::UnsupportedType,
                            format!("unsupported kernel parameter {index}: {ty:?}"),
                        ));
                    }
                    self.bindings.insert(
                        value,
                        ValueBinding::Value {
                            llvm_name: format!("%arg{index}"),
                            ty: ty.clone(),
                        },
                    );
                }
                Type::Slice(slice)
                    if slice.address_space == KernelAddressSpace::Global
                        && supported_memory_type(&slice.element) =>
                {
                    self.bindings.insert(
                        value,
                        ValueBinding::Slice {
                            data_name: format!("%arg{index}.data"),
                            length_name: format!("%arg{index}.len"),
                            ty: ty.clone(),
                        },
                    );
                }
                Type::Slice(slice) => {
                    let code = if slice.address_space != KernelAddressSpace::Global {
                        LoweringDiagnosticCode::UnsupportedAddressSpace
                    } else {
                        LoweringDiagnosticCode::UnsupportedType
                    };
                    return Err(LoweringErrors::one(
                        location,
                        code,
                        format!("unsupported kernel parameter {index}: {ty:?}"),
                    ));
                }
                _ => {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedParameter,
                        format!("unsupported kernel parameter {index}: {ty:?}"),
                    ));
                }
            }
        }
        for block in &body.blocks {
            for parameter in &block.parameters {
                let location =
                    LoweringLocation::block(self.module, self.kernel, self.function, block.id);
                match &parameter.ty {
                    Type::Scalar(scalar) if supported_scalar(*scalar) => {
                        self.bindings.insert(
                            parameter.id,
                            ValueBinding::Value {
                                llvm_name: value_name(parameter.id),
                                ty: parameter.ty.clone(),
                            },
                        );
                    }
                    Type::Pointer(_) => {
                        validate_pointer(&parameter.ty, &location)?;
                        self.bindings.insert(
                            parameter.id,
                            ValueBinding::Value {
                                llvm_name: value_name(parameter.id),
                                ty: parameter.ty.clone(),
                            },
                        );
                    }
                    Type::Slice(slice)
                        if slice.address_space == KernelAddressSpace::Global
                            && supported_memory_type(&slice.element) =>
                    {
                        self.bindings.insert(
                            parameter.id,
                            ValueBinding::Slice {
                                data_name: format!("{}.data", value_name(parameter.id)),
                                length_name: format!("{}.len", value_name(parameter.id)),
                                ty: parameter.ty.clone(),
                            },
                        );
                    }
                    Type::Slice(slice) => {
                        let code = if slice.address_space != KernelAddressSpace::Global {
                            LoweringDiagnosticCode::UnsupportedAddressSpace
                        } else {
                            LoweringDiagnosticCode::UnsupportedType
                        };
                        return Err(LoweringErrors::one(
                            location,
                            code,
                            format!(
                                "unsupported block parameter {}: {:?}",
                                parameter.id, parameter.ty
                            ),
                        ));
                    }
                    _ => {
                        return Err(LoweringErrors::one(
                            location,
                            LoweringDiagnosticCode::UnsupportedType,
                            format!(
                                "unsupported block parameter {}: {:?}",
                                parameter.id, parameter.ty
                            ),
                        ));
                    }
                }
            }
        }
        for block in &body.blocks {
            for operation in &block.operations {
                for result in &operation.results {
                    let llvm_name = match &operation.kind {
                        OperationKind::Constant(constant) => {
                            constant_value(constant).unwrap_or_else(|| value_name(result.id))
                        }
                        _ => value_name(result.id),
                    };
                    self.bindings.insert(
                        result.id,
                        ValueBinding::Value {
                            llvm_name,
                            ty: result.ty.clone(),
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_block(&mut self, block: &BasicBlock) -> Result<(), LoweringErrors> {
        for (index, operation) in block.operations.iter().enumerate() {
            self.validate_operation(block.id, index, operation)?;
        }
        self.validate_terminator(
            block.id,
            block
                .terminator
                .as_ref()
                .expect("verify_module required it"),
        )
    }

    fn validate_block_arguments(&self) -> Result<(), LoweringErrors> {
        let body = self.function.body.as_ref().expect("definition required");
        let entry = body
            .blocks
            .first()
            .expect("verify_module required an entry block");
        for block in body
            .blocks
            .iter()
            .filter(|block| !block.parameters.is_empty())
        {
            let location =
                LoweringLocation::block(self.module, self.kernel, self.function, block.id);
            if block.id == entry.id {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedBlockArguments,
                    "G1 cannot materialize entry-block parameters because the initial entry edge has no SSA arguments",
                ));
            }

            let incomings = self.incoming_edges(block.id);
            if incomings.is_empty() {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedBlockArguments,
                    "G1 cannot materialize block parameters without an incoming CFG edge",
                ));
            }

            let mut predecessors = BTreeSet::new();
            for (predecessor, _) in incomings {
                if !predecessors.insert(predecessor) {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedBlockArguments,
                        format!(
                            "G1 cannot materialize block parameters for multiple edges from {predecessor}; LLVM phi nodes require one incoming value per predecessor"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_operation(
        &mut self,
        block: BlockId,
        index: usize,
        operation: &Operation,
    ) -> Result<(), LoweringErrors> {
        let location =
            LoweringLocation::operation(self.module, self.kernel, self.function, block, index);
        match &operation.kind {
            OperationKind::Constant(constant) => {
                validate_constant(constant).map_err(|message| {
                    LoweringErrors::one(
                        location.clone(),
                        LoweringDiagnosticCode::UnsupportedConstant,
                        message,
                    )
                })?;
            }
            OperationKind::Intrinsic(intrinsic)
                if intrinsic.kind
                    == (IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Global,
                        axis: Axis::X,
                    }) => {}
            OperationKind::Binary { op, lhs, .. } => {
                let ty = self.value_type(*lhs);
                if !supported_binary(*op, ty) {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedOperation,
                        format!("G1 does not lower {op:?} for {ty:?}"),
                    ));
                }
            }
            OperationKind::Compare { lhs, .. } => {
                let ty = self.value_type(*lhs);
                if !ty
                    .as_scalar()
                    .is_some_and(|scalar| scalar == ScalarType::Bool || supported_integer(scalar))
                {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedOperation,
                        "G1 lowers only integer and boolean comparisons",
                    ));
                }
            }
            OperationKind::Cast { kind, value, to } => {
                let from = self.value_type(*value);
                validate_cast(*kind, from, to).map_err(|message| {
                    LoweringErrors::one(
                        location.clone(),
                        LoweringDiagnosticCode::UnsupportedCast,
                        message,
                    )
                })?;
            }
            OperationKind::SliceLength { slice } => {
                let binding = self.bindings.get(slice).expect("verified operand");
                let ValueBinding::Slice { .. } = binding else {
                    unreachable!("verify_module checked slice_length")
                };
            }
            OperationKind::SliceData { slice } => {
                let binding = self.bindings.get(slice).expect("verified operand");
                let ValueBinding::Slice { .. } = binding else {
                    unreachable!("verify_module checked slice_data")
                };
            }
            OperationKind::GetElementPointer { base, .. } => {
                validate_pointer(self.value_type(*base), &location)?;
            }
            OperationKind::Load { pointer, access } => {
                validate_memory_access(self.value_type(*pointer), access.address_space, &location)?;
            }
            OperationKind::Store {
                pointer, access, ..
            } => {
                validate_memory_access(self.value_type(*pointer), access.address_space, &location)?;
            }
            OperationKind::Fence(_) | OperationKind::WorkgroupBarrier(_) => {}
            OperationKind::WorkgroupMemory(memory) => {
                if !supported_memory_type(&memory.element) {
                    return Err(LoweringErrors::one(
                        location,
                        LoweringDiagnosticCode::UnsupportedWorkgroupMemory,
                        format!(
                            "AMDGPU LDS lowering does not support element type {:?}",
                            memory.element
                        ),
                    ));
                }
            }
            OperationKind::Atomic(atomic) => self.validate_atomic(atomic, &location)?,
            OperationKind::Barrier(_) => {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedBarrier,
                    "legacy barriers lack the convergence evidence required by AMDGPU lowering",
                ));
            }
            OperationKind::Alloca {
                address_space: KernelAddressSpace::Workgroup,
                ..
            } => {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedWorkgroupMemory,
                    "workgroup Alloca is ambiguous; use explicit WorkgroupMemory",
                ));
            }
            OperationKind::Intrinsic(_)
            | OperationKind::Unary { .. }
            | OperationKind::Select { .. }
            | OperationKind::Call { .. }
            | OperationKind::Alloca { .. } => {
                return Err(LoweringErrors::one(
                    location,
                    LoweringDiagnosticCode::UnsupportedOperation,
                    format!("G1 does not lower {:?}", operation.kind),
                ));
            }
        }
        Ok(())
    }

    fn validate_terminator(
        &self,
        block: BlockId,
        terminator: &Terminator,
    ) -> Result<(), LoweringErrors> {
        let location = LoweringLocation::block(self.module, self.kernel, self.function, block);
        match terminator {
            Terminator::Branch { .. } | Terminator::ConditionalBranch { .. } => Ok(()),
            Terminator::Return { values } if values.is_empty() => Ok(()),
            Terminator::Unreachable => Ok(()),
            Terminator::Switch { .. }
            | Terminator::IntegerSwitch { .. }
            | Terminator::Return { .. } => Err(LoweringErrors::one(
                location,
                LoweringDiagnosticCode::UnsupportedTerminator,
                format!("G1 does not lower {terminator:?}"),
            )),
        }
    }

    fn incoming_edges(&self, target: BlockId) -> Vec<(BlockId, &[ValueId])> {
        let body = self.function.body.as_ref().expect("definition required");
        let mut incomings = Vec::new();
        for block in &body.blocks {
            match block.terminator.as_ref().expect("verified terminator") {
                Terminator::Branch {
                    target: edge_target,
                    arguments,
                } if *edge_target == target => incomings.push((block.id, arguments.as_slice())),
                Terminator::ConditionalBranch {
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                    ..
                } => {
                    if *then_target == target {
                        incomings.push((block.id, then_arguments.as_slice()));
                    }
                    if *else_target == target {
                        incomings.push((block.id, else_arguments.as_slice()));
                    }
                }
                Terminator::Branch { .. }
                | Terminator::Switch { .. }
                | Terminator::IntegerSwitch { .. }
                | Terminator::Return { .. }
                | Terminator::Unreachable => {}
            }
        }
        incomings
    }

    fn value_type(&self, value: ValueId) -> &Type {
        match self
            .bindings
            .get(&value)
            .expect("verify_module checked value")
        {
            ValueBinding::Value { ty, .. } | ValueBinding::Slice { ty, .. } => ty,
        }
    }

    fn value(&self, value: ValueId) -> (&str, &Type) {
        self.bindings
            .get(&value)
            .and_then(ValueBinding::value)
            .expect("validated scalar or pointer value")
    }

    fn emit(&self) -> Result<String, LoweringErrors> {
        let mut output = String::new();
        writeln!(output, "target triple = \"{AMDGPU_TRIPLE}\"\n").unwrap();
        let has_workgroup_barrier = self.has_workgroup_barrier();
        if self.emit_workgroup_memory_declarations(&mut output) {
            writeln!(output).unwrap();
        }
        writeln!(
            output,
            "declare i32 @{}() #1",
            AmdgcnIntrinsic::WorkItemId(Dim::X).llvm_name()
        )
        .unwrap();
        writeln!(
            output,
            "declare i32 @{}() #1",
            AmdgcnIntrinsic::WorkGroupId(Dim::X).llvm_name()
        )
        .unwrap();
        if has_workgroup_barrier {
            writeln!(
                output,
                "declare void @{}() #2",
                AmdgcnIntrinsic::SBarrier.llvm_name()
            )
            .unwrap();
        }
        writeln!(output).unwrap();

        write!(
            output,
            "define amdgpu_kernel void @{}(",
            self.kernel.id.as_str()
        )
        .unwrap();
        let parameters = self.llvm_parameters()?;
        write!(output, "{}", parameters.join(", ")).unwrap();
        writeln!(output, ") #0 !reqd_work_group_size !0 {{").unwrap();

        let body = self.function.body.as_ref().expect("definition required");
        for block in &body.blocks {
            writeln!(output, "{}:", block_label(block.id)).unwrap();
            self.emit_block_parameters(&mut output, block);
            for operation in &block.operations {
                self.emit_operation(&mut output, operation)?;
            }
            self.emit_terminator(
                &mut output,
                block.terminator.as_ref().expect("verified terminator"),
            );
        }
        writeln!(output, "}}\n").unwrap();
        let wave_attribute = self.wave_width.map_or("", |width| match width {
            WaveWidth::Wave32 => " \"target-features\"=\"+wavefrontsize32,-wavefrontsize64\"",
            WaveWidth::Wave64 => " \"target-features\"=\"-wavefrontsize32,+wavefrontsize64\"",
        });
        writeln!(
            output,
            "attributes #0 = {{ nounwind \"amdgpu-flat-work-group-size\"=\"{0},{0}\"{wave_attribute} }}",
            self.workgroup_x
        )
        .unwrap();
        writeln!(
            output,
            "attributes #1 = {{ nounwind readnone speculatable willreturn }}"
        )
        .unwrap();
        if has_workgroup_barrier {
            writeln!(output, "attributes #2 = {{ convergent nounwind }}").unwrap();
        }
        writeln!(output).unwrap();
        writeln!(output, "!0 = !{{i32 {}, i32 1, i32 1}}", self.workgroup_x).unwrap();
        Ok(output)
    }

    fn has_workgroup_barrier(&self) -> bool {
        self.function
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(&operation.kind, OperationKind::WorkgroupBarrier(_)))
    }

    fn emit_workgroup_memory_declarations(&self, output: &mut String) -> bool {
        let mut emitted = false;
        let body = self.function.body.as_ref().expect("definition required");
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            let OperationKind::WorkgroupMemory(memory) = &operation.kind else {
                continue;
            };
            emitted = true;
            let result = operation.results.first().expect("verified LDS result");
            let symbol = lds_symbol(self.kernel, result.id);
            let element = llvm_type(&memory.element);
            match memory.extent {
                WorkgroupMemoryExtent::Static(elements) => writeln!(
                    output,
                    "{symbol} = internal addrspace(3) global [{elements} x {element}] undef, align {}",
                    memory.alignment
                )
                .unwrap(),
                WorkgroupMemoryExtent::Dynamic => writeln!(
                    output,
                    "{symbol} = external addrspace(3) global [0 x {element}], align {}",
                    memory.alignment
                )
                .unwrap(),
            }
        }
        emitted
    }

    fn emit_block_parameters(&self, output: &mut String, block: &BasicBlock) {
        let incomings = self.incoming_edges(block.id);
        for (parameter_index, parameter) in block.parameters.iter().enumerate() {
            match self
                .bindings
                .get(&parameter.id)
                .expect("validated block parameter")
            {
                ValueBinding::Value { llvm_name, ty } => {
                    let values = incomings
                        .iter()
                        .map(|(predecessor, arguments)| {
                            let (argument, _) = self.value(arguments[parameter_index]);
                            format!("[ {argument}, %{} ]", block_label(*predecessor))
                        })
                        .collect::<Vec<_>>();
                    writeln!(
                        output,
                        "  {llvm_name} = phi {} {}",
                        llvm_type(ty),
                        values.join(", ")
                    )
                    .unwrap();
                }
                ValueBinding::Slice {
                    data_name,
                    length_name,
                    ..
                } => {
                    let data_values = incomings
                        .iter()
                        .map(|(predecessor, arguments)| {
                            let ValueBinding::Slice {
                                data_name: argument,
                                ..
                            } = self
                                .bindings
                                .get(&arguments[parameter_index])
                                .expect("verified branch argument")
                            else {
                                unreachable!("verify_module checked branch argument types")
                            };
                            format!("[ {argument}, %{} ]", block_label(*predecessor))
                        })
                        .collect::<Vec<_>>();
                    let length_values = incomings
                        .iter()
                        .map(|(predecessor, arguments)| {
                            let ValueBinding::Slice {
                                length_name: argument,
                                ..
                            } = self
                                .bindings
                                .get(&arguments[parameter_index])
                                .expect("verified branch argument")
                            else {
                                unreachable!("verify_module checked branch argument types")
                            };
                            format!("[ {argument}, %{} ]", block_label(*predecessor))
                        })
                        .collect::<Vec<_>>();
                    writeln!(
                        output,
                        "  {data_name} = phi ptr addrspace(1) {}",
                        data_values.join(", ")
                    )
                    .unwrap();
                    writeln!(
                        output,
                        "  {length_name} = phi i64 {}",
                        length_values.join(", ")
                    )
                    .unwrap();
                }
            }
        }
    }

    fn llvm_parameters(&self) -> Result<Vec<String>, LoweringErrors> {
        self.function
            .signature
            .parameters
            .iter()
            .enumerate()
            .map(|(index, ty)| match ty {
                Type::Scalar(scalar) => Ok(format!("{} %arg{index}", llvm_scalar(*scalar))),
                Type::Slice(_) => Ok(format!(
                    "ptr addrspace(1) %arg{index}.data, i64 %arg{index}.len"
                )),
                _ => Err(LoweringErrors::one(
                    LoweringLocation::function(self.module, self.kernel, self.function),
                    LoweringDiagnosticCode::UnsupportedParameter,
                    format!("unsupported kernel parameter {index}: {ty:?}"),
                )),
            })
            .collect()
    }

    fn emit_operation(
        &self,
        output: &mut String,
        operation: &Operation,
    ) -> Result<(), LoweringErrors> {
        let result_name = operation
            .results
            .first()
            .map(|result| value_name(result.id));
        match &operation.kind {
            OperationKind::Constant(_) => {}
            OperationKind::SliceLength { slice } => {
                let ValueBinding::Slice { length_name, .. } =
                    self.bindings.get(slice).expect("validated slice binding")
                else {
                    unreachable!()
                };
                writeln!(
                    output,
                    "  {} = add i64 {}, 0",
                    result_name.expect("validated result"),
                    length_name
                )
                .unwrap();
            }
            OperationKind::SliceData { slice } => {
                let ValueBinding::Slice { data_name, .. } =
                    self.bindings.get(slice).expect("validated slice binding")
                else {
                    unreachable!()
                };
                writeln!(
                    output,
                    "  {} = getelementptr i8, ptr addrspace(1) {}, i64 0",
                    result_name.expect("validated result"),
                    data_name
                )
                .unwrap();
            }
            OperationKind::Intrinsic(_) => {
                let result = result_name.expect("validated result");
                writeln!(
                    output,
                    "  {result}.local.i32 = call i32 @{}()",
                    AmdgcnIntrinsic::WorkItemId(Dim::X).llvm_name()
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.group.i32 = call i32 @{}()",
                    AmdgcnIntrinsic::WorkGroupId(Dim::X).llvm_name()
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.local = zext i32 {result}.local.i32 to i64"
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.group = zext i32 {result}.group.i32 to i64"
                )
                .unwrap();
                writeln!(
                    output,
                    "  {result}.base = mul i64 {result}.group, {}",
                    self.workgroup_x
                )
                .unwrap();
                writeln!(output, "  {result} = add i64 {result}.base, {result}.local").unwrap();
            }
            OperationKind::Binary { op, lhs, rhs } => {
                let (lhs_name, lhs_ty) = self.value(*lhs);
                let (rhs_name, _) = self.value(*rhs);
                writeln!(
                    output,
                    "  {} = {} {} {}, {}",
                    result_name.expect("validated result"),
                    binary_opcode(*op, lhs_ty),
                    llvm_type(lhs_ty),
                    lhs_name,
                    rhs_name
                )
                .unwrap();
            }
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } => {
                let (lhs_name, lhs_ty) = self.value(*lhs);
                let (rhs_name, _) = self.value(*rhs);
                writeln!(
                    output,
                    "  {} = icmp {} {} {}, {}",
                    result_name.expect("validated result"),
                    compare_predicate(*predicate, lhs_ty),
                    llvm_type(lhs_ty),
                    lhs_name,
                    rhs_name
                )
                .unwrap();
            }
            OperationKind::Cast { kind, value, to } => {
                let (value_name, from) = self.value(*value);
                writeln!(
                    output,
                    "  {} = {} {} {} to {}",
                    result_name.expect("validated result"),
                    cast_opcode(*kind, from, to),
                    llvm_type(from),
                    value_name,
                    llvm_type(to)
                )
                .unwrap();
            }
            OperationKind::GetElementPointer { base, offset } => {
                let (base_name, base_ty) = self.value(*base);
                let (offset_name, offset_ty) = self.value(*offset);
                let Type::Pointer(pointer) = base_ty else {
                    unreachable!()
                };
                let address_space = llvm_address_space(pointer.address_space);
                writeln!(
                    output,
                    "  {} = getelementptr {}, ptr addrspace({}) {}, {} {}",
                    result_name.expect("validated result"),
                    llvm_type(&pointer.pointee),
                    address_space,
                    base_name,
                    llvm_type(offset_ty),
                    offset_name
                )
                .unwrap();
            }
            OperationKind::Load { pointer, access } => {
                let (pointer_name, pointer_ty) = self.value(*pointer);
                let Type::Pointer(pointer_ty) = pointer_ty else {
                    unreachable!()
                };
                let address_space = llvm_address_space(pointer_ty.address_space);
                let volatile = if access.volatile { " volatile" } else { "" };
                writeln!(
                    output,
                    "  {} = load{} {}, ptr addrspace({}) {}, align {}",
                    result_name.expect("validated result"),
                    volatile,
                    llvm_type(&pointer_ty.pointee),
                    address_space,
                    pointer_name,
                    access.alignment
                )
                .unwrap();
            }
            OperationKind::Store {
                pointer,
                value,
                access,
            } => {
                let (pointer_name, pointer_ty) = self.value(*pointer);
                let (value_name, _) = self.value(*value);
                let Type::Pointer(pointer_ty) = pointer_ty else {
                    unreachable!()
                };
                let address_space = llvm_address_space(pointer_ty.address_space);
                let volatile = if access.volatile { " volatile" } else { "" };
                writeln!(
                    output,
                    "  store{} {} {}, ptr addrspace({}) {}, align {}",
                    volatile,
                    llvm_type(&pointer_ty.pointee),
                    value_name,
                    address_space,
                    pointer_name,
                    access.alignment
                )
                .unwrap();
            }
            OperationKind::Atomic(atomic) => {
                self.emit_atomic(output, operation, atomic);
            }
            OperationKind::Fence(fence) => {
                emit_fence(output, fence.memory_scope, fence.semantics.ordering);
            }
            OperationKind::WorkgroupBarrier(barrier) => {
                match barrier.semantics.ordering {
                    MemoryOrdering::Acquire => {}
                    MemoryOrdering::Release | MemoryOrdering::AcquireRelease => {
                        emit_fence(output, barrier.memory_scope, MemoryOrdering::Release);
                    }
                    MemoryOrdering::SequentiallyConsistent => {
                        emit_fence(
                            output,
                            barrier.memory_scope,
                            MemoryOrdering::SequentiallyConsistent,
                        );
                    }
                    MemoryOrdering::Relaxed => {
                        unreachable!("kernel IR verification rejected a relaxed barrier")
                    }
                }
                writeln!(
                    output,
                    "  call void @{}()",
                    AmdgcnIntrinsic::SBarrier.llvm_name()
                )
                .unwrap();
                match barrier.semantics.ordering {
                    MemoryOrdering::Release => {}
                    MemoryOrdering::Acquire | MemoryOrdering::AcquireRelease => {
                        emit_fence(output, barrier.memory_scope, MemoryOrdering::Acquire);
                    }
                    MemoryOrdering::SequentiallyConsistent => {
                        emit_fence(
                            output,
                            barrier.memory_scope,
                            MemoryOrdering::SequentiallyConsistent,
                        );
                    }
                    MemoryOrdering::Relaxed => unreachable!(),
                }
            }
            OperationKind::WorkgroupMemory(memory) => {
                let result = operation.results.first().expect("verified LDS result");
                let result_name = result_name.expect("verified LDS result name");
                let elements = match memory.extent {
                    WorkgroupMemoryExtent::Static(elements) => elements,
                    WorkgroupMemoryExtent::Dynamic => 0,
                };
                let element = llvm_type(&memory.element);
                writeln!(
                    output,
                    "  {result_name} = getelementptr [{elements} x {element}], ptr addrspace(3) {}, i32 0, i32 0",
                    lds_symbol(self.kernel, result.id)
                )
                .unwrap();
            }
            _ => unreachable!("preflight rejected unsupported operation"),
        }
        Ok(())
    }

    fn validate_atomic(
        &self,
        atomic: &Atomic,
        location: &LoweringLocation,
    ) -> Result<(), LoweringErrors> {
        let pointer = self.value_type(atomic.pointer);
        validate_memory_access(pointer, atomic.access.address_space, location)?;
        let Type::Pointer(pointer) = pointer else {
            unreachable!("kernel IR verification required an atomic pointer")
        };
        let Some(scalar) = pointer.pointee.as_scalar() else {
            unreachable!("kernel IR verification required a scalar atomic pointee")
        };

        if !matches!(
            scalar,
            ScalarType::I32 | ScalarType::U32 | ScalarType::I64 | ScalarType::U64
        ) {
            return Err(LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedAtomic,
                format!(
                    "AMDGPU atomic lowering supports only 32-bit and 64-bit integers, found {scalar:?}"
                ),
            ));
        }
        if atomic.access.volatile {
            return Err(LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedAtomic,
                "volatile scoped atomics are outside the supported AMDGPU subset",
            ));
        }
        if !supported_atomic_address_scope(atomic.access.address_space, atomic.scope) {
            return Err(LoweringErrors::one(
                location.clone(),
                LoweringDiagnosticCode::UnsupportedAtomic,
                format!(
                    "AMDGPU atomic lowering does not support {:?} memory at {:?} scope",
                    atomic.access.address_space, atomic.scope
                ),
            ));
        }
        Ok(())
    }

    fn emit_atomic(&self, output: &mut String, operation: &Operation, atomic: &Atomic) {
        let (pointer_name, pointer_ty) = self.value(atomic.pointer);
        let Type::Pointer(pointer_ty) = pointer_ty else {
            unreachable!("atomic preflight required a pointer")
        };
        let value_type = llvm_type(&pointer_ty.pointee);
        let address_space = llvm_address_space(pointer_ty.address_space);
        let sync_scope = llvm_atomic_sync_scope(atomic.scope);
        let ordering = llvm_atomic_ordering(atomic.ordering);

        match atomic.kind {
            AtomicKind::Load => {
                let result = operation
                    .results
                    .first()
                    .expect("verified atomic load result");
                writeln!(
                    output,
                    "  {} = load atomic {value_type}, ptr addrspace({address_space}) {pointer_name}{sync_scope} {ordering}, align {}",
                    value_name(result.id),
                    atomic.access.alignment
                )
                .unwrap();
            }
            AtomicKind::Store => {
                let value = self
                    .value(atomic.value.expect("verified atomic store value"))
                    .0;
                writeln!(
                    output,
                    "  store atomic {value_type} {value}, ptr addrspace({address_space}) {pointer_name}{sync_scope} {ordering}, align {}",
                    atomic.access.alignment
                )
                .unwrap();
            }
            AtomicKind::CompareExchange => {
                let [old, succeeded] = operation.results.as_slice() else {
                    unreachable!("verified compare-exchange results")
                };
                let desired = self
                    .value(
                        atomic
                            .value
                            .expect("verified compare-exchange desired value"),
                    )
                    .0;
                let expected = self
                    .value(
                        atomic
                            .compare
                            .expect("verified compare-exchange expected value"),
                    )
                    .0;
                let failure_ordering = llvm_atomic_ordering(
                    atomic
                        .failure_ordering
                        .expect("verified compare-exchange failure ordering"),
                );
                let pair = format!("{}.cmpxchg", value_name(old.id));
                writeln!(
                    output,
                    "  {pair} = cmpxchg ptr addrspace({address_space}) {pointer_name}, {value_type} {expected}, {value_type} {desired}{sync_scope} {ordering} {failure_ordering}, align {}",
                    atomic.access.alignment
                )
                .unwrap();
                writeln!(
                    output,
                    "  {} = extractvalue {{ {value_type}, i1 }} {pair}, 0",
                    value_name(old.id)
                )
                .unwrap();
                writeln!(
                    output,
                    "  {} = extractvalue {{ {value_type}, i1 }} {pair}, 1",
                    value_name(succeeded.id)
                )
                .unwrap();
            }
            kind => {
                let result = operation
                    .results
                    .first()
                    .expect("verified atomic RMW result");
                let value = self
                    .value(atomic.value.expect("verified atomic RMW value"))
                    .0;
                let scalar = pointer_ty
                    .pointee
                    .as_scalar()
                    .expect("atomic preflight required a scalar");
                let opcode = llvm_atomic_rmw_opcode(kind, scalar);
                writeln!(
                    output,
                    "  {} = atomicrmw {opcode} ptr addrspace({address_space}) {pointer_name}, {value_type} {value}{sync_scope} {ordering}, align {}",
                    value_name(result.id),
                    atomic.access.alignment
                )
                .unwrap();
            }
        }
    }

    fn emit_terminator(&self, output: &mut String, terminator: &Terminator) {
        match terminator {
            Terminator::Branch { target, .. } => {
                writeln!(output, "  br label %{}", block_label(*target)).unwrap();
            }
            Terminator::ConditionalBranch {
                condition,
                then_target,
                else_target,
                ..
            } => {
                let (condition, _) = self.value(*condition);
                writeln!(
                    output,
                    "  br i1 {condition}, label %{}, label %{}",
                    block_label(*then_target),
                    block_label(*else_target)
                )
                .unwrap();
            }
            Terminator::Return { .. } => writeln!(output, "  ret void").unwrap(),
            Terminator::Unreachable => writeln!(output, "  unreachable").unwrap(),
            _ => unreachable!("preflight rejected unsupported terminator"),
        }
    }
}

fn supported_scalar(scalar: ScalarType) -> bool {
    scalar == ScalarType::Bool || supported_integer(scalar) || scalar == ScalarType::F32
}

fn supported_integer(scalar: ScalarType) -> bool {
    matches!(
        scalar,
        ScalarType::I8
            | ScalarType::I16
            | ScalarType::I32
            | ScalarType::I64
            | ScalarType::U8
            | ScalarType::U16
            | ScalarType::U32
            | ScalarType::U64
            | ScalarType::Index
    )
}

fn supported_memory_type(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(scalar) if supported_integer(*scalar) || *scalar == ScalarType::F32)
}

fn supported_atomic_capability(
    width_bits: u16,
    address_space: KernelAddressSpace,
    max_scope: SynchronizationScope,
) -> bool {
    matches!(width_bits, 32 | 64) && supported_atomic_address_scope(address_space, max_scope)
}

fn supported_atomic_address_scope(
    address_space: KernelAddressSpace,
    scope: SynchronizationScope,
) -> bool {
    match address_space {
        KernelAddressSpace::Workgroup => scope == SynchronizationScope::Workgroup,
        KernelAddressSpace::Global => matches!(
            scope,
            SynchronizationScope::Workgroup
                | SynchronizationScope::Device
                | SynchronizationScope::System
        ),
        KernelAddressSpace::Generic
        | KernelAddressSpace::Private
        | KernelAddressSpace::Constant => false,
    }
}

fn supported_binary(op: BinaryOp, ty: &Type) -> bool {
    matches!(op, BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply)
        && ty
            .as_scalar()
            .is_some_and(|scalar| supported_integer(scalar) || scalar == ScalarType::F32)
}

fn validate_pointer(ty: &Type, location: &LoweringLocation) -> Result<(), LoweringErrors> {
    let Type::Pointer(pointer) = ty else {
        unreachable!("verify_module checked GEP base")
    };
    if !matches!(
        pointer.address_space,
        KernelAddressSpace::Global | KernelAddressSpace::Workgroup
    ) {
        return Err(LoweringErrors::one(
            location.clone(),
            LoweringDiagnosticCode::UnsupportedAddressSpace,
            format!(
                "G1 supports only global or workgroup pointers, found {:?}",
                pointer.address_space
            ),
        ));
    }
    if !supported_memory_type(&pointer.pointee) {
        return Err(LoweringErrors::one(
            location.clone(),
            LoweringDiagnosticCode::UnsupportedType,
            format!("unsupported memory pointee type {:?}", pointer.pointee),
        ));
    }
    Ok(())
}

fn validate_memory_access(
    pointer: &Type,
    access_space: KernelAddressSpace,
    location: &LoweringLocation,
) -> Result<(), LoweringErrors> {
    validate_pointer(pointer, location)?;
    let Type::Pointer(pointer) = pointer else {
        unreachable!("validate_pointer required a pointer")
    };
    if access_space != pointer.address_space {
        return Err(LoweringErrors::one(
            location.clone(),
            LoweringDiagnosticCode::UnsupportedAddressSpace,
            format!(
                "memory access names {access_space:?} but pointer uses {:?}",
                pointer.address_space
            ),
        ));
    }
    Ok(())
}

fn validate_constant(constant: &Constant) -> Result<(), String> {
    match constant {
        Constant::Bool(_)
        | Constant::I8(_)
        | Constant::I16(_)
        | Constant::I32(_)
        | Constant::I64(_)
        | Constant::U8(_)
        | Constant::U16(_)
        | Constant::U32(_)
        | Constant::U64(_)
        | Constant::Index(_) => Ok(()),
        Constant::F32Bits(bits) if !f32::from_bits(*bits).is_nan() => Ok(()),
        Constant::F32Bits(_) => Err("G1 rejects NaN f32 constants because LLVM's widened hexadecimal spelling does not preserve every payload".to_string()),
        _ => Err(format!("G1 does not lower constant {constant:?}")),
    }
}

fn validate_cast(kind: CastKind, from: &Type, to: &Type) -> Result<(), String> {
    let (Some(from_scalar), Some(to_scalar)) = (from.as_scalar(), to.as_scalar()) else {
        return Err(format!(
            "G1 casts require scalar types, found {from:?} to {to:?}"
        ));
    };
    if !supported_scalar(from_scalar) || !supported_scalar(to_scalar) {
        return Err(format!("G1 does not lower cast types {from:?} to {to:?}"));
    }
    let from_width = llvm_width(from_scalar);
    let to_width = llvm_width(to_scalar);
    let valid = match kind {
        CastKind::Truncate => {
            supported_integer(from_scalar) && supported_integer(to_scalar) && from_width > to_width
        }
        CastKind::ZeroExtend => {
            (from_scalar == ScalarType::Bool
                || (!from_scalar.is_signed_integer() && supported_integer(from_scalar)))
                && supported_integer(to_scalar)
                && from_width < to_width
        }
        CastKind::SignExtend => {
            from_scalar.is_signed_integer() && supported_integer(to_scalar) && from_width < to_width
        }
        CastKind::IntegerToFloat => supported_integer(from_scalar) && to_scalar == ScalarType::F32,
        CastKind::FloatToInteger => from_scalar == ScalarType::F32 && supported_integer(to_scalar),
        CastKind::Bitcast => {
            from_width == to_width && llvm_scalar(from_scalar) != llvm_scalar(to_scalar)
        }
        CastKind::FloatExtend | CastKind::FloatTruncate => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!("unsupported {kind:?} cast from {from:?} to {to:?}"))
    }
}

fn value_name(value: ValueId) -> String {
    format!("%v{}", value.0)
}

fn lds_symbol(kernel: &Kernel, value: ValueId) -> String {
    format!("@__fe2o3_lds_{}_{}", kernel.id.as_str(), value.0)
}

fn emit_fence(
    output: &mut String,
    scope: fe2o3_kernel_ir::SynchronizationScope,
    ordering: MemoryOrdering,
) {
    let ordering = match ordering {
        MemoryOrdering::Acquire => "acquire",
        MemoryOrdering::Release => "release",
        MemoryOrdering::AcquireRelease => "acq_rel",
        MemoryOrdering::SequentiallyConsistent => "seq_cst",
        MemoryOrdering::Relaxed => unreachable!("verification rejected a relaxed fence"),
    };
    match scope {
        fe2o3_kernel_ir::SynchronizationScope::Subgroup => {
            writeln!(output, "  fence syncscope(\"wavefront\") {ordering}").unwrap();
        }
        fe2o3_kernel_ir::SynchronizationScope::Workgroup => {
            writeln!(output, "  fence syncscope(\"workgroup\") {ordering}").unwrap();
        }
        fe2o3_kernel_ir::SynchronizationScope::Device => {
            writeln!(output, "  fence syncscope(\"agent\") {ordering}").unwrap();
        }
        fe2o3_kernel_ir::SynchronizationScope::System => {
            writeln!(output, "  fence {ordering}").unwrap();
        }
        fe2o3_kernel_ir::SynchronizationScope::Invocation => {
            unreachable!("verification rejected invocation-scoped synchronization")
        }
    }
}

fn llvm_atomic_sync_scope(scope: SynchronizationScope) -> &'static str {
    match scope {
        SynchronizationScope::Workgroup => " syncscope(\"workgroup\")",
        SynchronizationScope::Device => " syncscope(\"agent\")",
        SynchronizationScope::System => "",
        SynchronizationScope::Invocation | SynchronizationScope::Subgroup => {
            unreachable!("atomic preflight rejected unsupported synchronization scope")
        }
    }
}

fn llvm_atomic_ordering(ordering: MemoryOrdering) -> &'static str {
    match ordering {
        MemoryOrdering::Relaxed => "monotonic",
        MemoryOrdering::Acquire => "acquire",
        MemoryOrdering::Release => "release",
        MemoryOrdering::AcquireRelease => "acq_rel",
        MemoryOrdering::SequentiallyConsistent => "seq_cst",
    }
}

fn llvm_atomic_rmw_opcode(kind: AtomicKind, scalar: ScalarType) -> &'static str {
    match kind {
        AtomicKind::Exchange => "xchg",
        AtomicKind::Add => "add",
        AtomicKind::Subtract => "sub",
        AtomicKind::Min if scalar.is_signed_integer() => "min",
        AtomicKind::Min => "umin",
        AtomicKind::Max if scalar.is_signed_integer() => "max",
        AtomicKind::Max => "umax",
        AtomicKind::BitAnd => "and",
        AtomicKind::BitOr => "or",
        AtomicKind::BitXor => "xor",
        AtomicKind::Load | AtomicKind::Store | AtomicKind::CompareExchange => {
            unreachable!("non-RMW atomic kind")
        }
    }
}

fn block_label(block: BlockId) -> String {
    format!("bb{}", block.0)
}

fn llvm_type(ty: &Type) -> &'static str {
    match ty {
        Type::Scalar(scalar) => llvm_scalar(*scalar),
        Type::Pointer(pointer) if pointer.address_space == KernelAddressSpace::Global => {
            "ptr addrspace(1)"
        }
        Type::Pointer(pointer) if pointer.address_space == KernelAddressSpace::Workgroup => {
            "ptr addrspace(3)"
        }
        Type::Pointer(_) => unreachable!("preflight rejected unsupported address space"),
        Type::Unit | Type::Slice(_) => unreachable!("type is not a first-class G1 LLVM value"),
    }
}

fn llvm_address_space(address_space: KernelAddressSpace) -> u32 {
    match address_space {
        KernelAddressSpace::Global => 1,
        KernelAddressSpace::Workgroup => 3,
        _ => unreachable!("preflight rejected unsupported address space"),
    }
}

fn llvm_scalar(scalar: ScalarType) -> &'static str {
    match scalar {
        ScalarType::Bool => "i1",
        ScalarType::I8 | ScalarType::U8 => "i8",
        ScalarType::I16 | ScalarType::U16 => "i16",
        ScalarType::I32 | ScalarType::U32 => "i32",
        ScalarType::I64 | ScalarType::U64 | ScalarType::Index => "i64",
        ScalarType::F32 => "float",
        ScalarType::F16 | ScalarType::Bf16 | ScalarType::F64 => {
            unreachable!("preflight rejected unsupported scalar")
        }
    }
}

fn llvm_width(scalar: ScalarType) -> u16 {
    scalar.bit_width().unwrap_or(64)
}

fn constant_value(constant: &Constant) -> Option<String> {
    match constant {
        Constant::Bool(value) => Some(value.to_string()),
        Constant::I8(value) => Some(value.to_string()),
        Constant::I16(value) => Some(value.to_string()),
        Constant::I32(value) => Some(value.to_string()),
        Constant::I64(value) => Some(value.to_string()),
        Constant::U8(value) => Some(value.to_string()),
        Constant::U16(value) => Some(value.to_string()),
        Constant::U32(value) => Some(value.to_string()),
        Constant::U64(value) => Some(value.to_string()),
        Constant::Index(value) => Some(value.to_string()),
        Constant::F32Bits(bits) if !f32::from_bits(*bits).is_nan() => Some(format!(
            "0x{:016X}",
            f64::from(f32::from_bits(*bits)).to_bits()
        )),
        _ => None,
    }
}

fn binary_opcode(op: BinaryOp, ty: &Type) -> &'static str {
    let floating = ty.as_scalar() == Some(ScalarType::F32);
    match (op, floating) {
        (BinaryOp::Add, false) => "add",
        (BinaryOp::Subtract, false) => "sub",
        (BinaryOp::Multiply, false) => "mul",
        (BinaryOp::Add, true) => "fadd",
        (BinaryOp::Subtract, true) => "fsub",
        (BinaryOp::Multiply, true) => "fmul",
        _ => unreachable!("preflight rejected unsupported binary operation"),
    }
}

fn compare_predicate(predicate: ComparePredicate, ty: &Type) -> &'static str {
    let scalar = ty.as_scalar().expect("validated scalar comparison");
    let signed = scalar.is_signed_integer();
    match predicate {
        ComparePredicate::Equal => "eq",
        ComparePredicate::NotEqual => "ne",
        ComparePredicate::LessThan if signed => "slt",
        ComparePredicate::LessThan => "ult",
        ComparePredicate::LessThanOrEqual if signed => "sle",
        ComparePredicate::LessThanOrEqual => "ule",
        ComparePredicate::GreaterThan if signed => "sgt",
        ComparePredicate::GreaterThan => "ugt",
        ComparePredicate::GreaterThanOrEqual if signed => "sge",
        ComparePredicate::GreaterThanOrEqual => "uge",
    }
}

fn cast_opcode(kind: CastKind, from: &Type, to: &Type) -> &'static str {
    match kind {
        CastKind::Truncate => "trunc",
        CastKind::ZeroExtend => "zext",
        CastKind::SignExtend => "sext",
        CastKind::IntegerToFloat if from.as_scalar().is_some_and(ScalarType::is_signed_integer) => {
            "sitofp"
        }
        CastKind::IntegerToFloat => "uitofp",
        CastKind::FloatToInteger if to.as_scalar().is_some_and(ScalarType::is_signed_integer) => {
            "fptosi"
        }
        CastKind::FloatToInteger => "fptoui",
        CastKind::Bitcast => "bitcast",
        CastKind::FloatExtend | CastKind::FloatTruncate => {
            unreachable!("preflight rejected unsupported cast")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_symbols_are_intentionally_conservative() {
        for symbol in ["fill", "_fill_2", "Fill42"] {
            assert!(is_safe_symbol(symbol), "{symbol}");
        }
        for symbol in ["", "42fill", "fill.kernel", "fill-kernel", "fill\nret void"] {
            assert!(!is_safe_symbol(symbol), "{symbol}");
        }
    }

    #[test]
    fn constants_and_signedness_are_stable() {
        assert_eq!(constant_value(&Constant::I32(-7)).unwrap(), "-7");
        assert_eq!(
            constant_value(&Constant::F32Bits(1.0f32.to_bits())).unwrap(),
            "0x3FF0000000000000"
        );
        assert_eq!(
            compare_predicate(ComparePredicate::LessThan, &Type::Scalar(ScalarType::I32)),
            "slt"
        );
        assert_eq!(
            compare_predicate(ComparePredicate::LessThan, &Type::INDEX),
            "ult"
        );
    }
}
