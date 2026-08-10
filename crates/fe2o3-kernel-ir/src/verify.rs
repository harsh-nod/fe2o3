use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, AssemblyConstraint, AssemblyEffect, AssemblyOperandKind,
    AssemblyOption, Atomic, AtomicKind, Barrier, BasicBlock, BinaryOp, BlockId, ComparePredicate,
    Fence, FloatOperation, Function, FunctionId, FunctionRole, InlineAssembly, Kernel, KernelId,
    LaunchExtent, MatrixVerificationIssueKind, MemoryOrdering, Module, ModuleId, Operation,
    OperationKind, ScalarType, SemanticOperationIssueKind, SemanticOperationVerificationContext,
    SynchronizationScope, TargetCapability, Terminator, Type, UnaryOp, ValueId, WaveOperation,
    WaveOperationKind, WorkgroupBarrier, WorkgroupMemory, WorkgroupMemoryExtent, pointer_for,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticCode {
    InvalidIdentity,
    DuplicateFunction,
    ConflictingFunctionRole,
    DuplicateKernel,
    DuplicateBlock,
    DuplicateValue,
    UnknownKernelEntry,
    KernelEntryDeclaration,
    InvalidFunctionRole,
    KernelReturnsValue,
    InvalidLaunchDomain,
    InvalidWorkgroupSize,
    InvalidCapability,
    UnsupportedCapability,
    EmptyFunction,
    SignatureMismatch,
    MissingTerminator,
    InvalidBranchTarget,
    BranchArgumentCount,
    BranchArgumentType,
    DuplicateSwitchCase,
    UnsortedSwitchCase,
    UndefinedValue,
    NonDominatingUse,
    UnknownCallee,
    ResultArity,
    TypeMismatch,
    InvalidOperandType,
    InvalidSemanticOperation,
    InvalidMemoryAccess,
    InvalidAlignment,
    InvalidBarrier,
    InvalidAtomic,
    InvalidFence,
    InvalidConvergence,
    InvalidWorkgroupMemory,
    InvalidWaveOperation,
    InvalidFloatOperation,
    InvalidInlineAssembly,
    InvalidTerminator,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticLocation {
    pub module: ModuleId,
    pub function: Option<FunctionId>,
    pub kernel: Option<KernelId>,
    pub block: Option<BlockId>,
    pub operation: Option<usize>,
}

impl DiagnosticLocation {
    fn module(module: &Module) -> Self {
        Self {
            module: module.id.clone(),
            function: None,
            kernel: None,
            block: None,
            operation: None,
        }
    }

    fn function(module: &Module, function: &Function) -> Self {
        Self {
            function: Some(function.id.clone()),
            ..Self::module(module)
        }
    }

    fn kernel(module: &Module, kernel: &Kernel) -> Self {
        Self {
            kernel: Some(kernel.id.clone()),
            ..Self::module(module)
        }
    }

    fn at_block(mut self, block: BlockId) -> Self {
        self.block = Some(block);
        self
    }

    fn at_operation(mut self, operation: usize) -> Self {
        self.operation = Some(operation);
        self
    }
}

impl fmt::Display for DiagnosticLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "module {}", self.module)?;
        if let Some(function) = &self.function {
            write!(formatter, ", function {function}")?;
        }
        if let Some(kernel) = &self.kernel {
            write!(formatter, ", kernel {kernel}")?;
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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Diagnostic {
    pub location: DiagnosticLocation,
    pub code: DiagnosticCode,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {:?}: {}",
            self.location, self.code, self.message
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationErrors {
    diagnostics: Vec<Diagnostic>,
}

impl VerificationErrors {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    pub fn contains(&self, code: DiagnosticCode) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)
    }
}

impl fmt::Display for VerificationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "kernel IR verification failed with {} diagnostic(s)",
            self.diagnostics.len()
        )?;
        for diagnostic in &self.diagnostics {
            writeln!(formatter, "  {diagnostic}")?;
        }
        Ok(())
    }
}

impl Error for VerificationErrors {}

/// Verifies structural and local semantic invariants of a complete module.
///
/// All diagnostics are collected and sorted, making the result deterministic
/// regardless of map implementation details in the verifier.
pub fn verify_module(module: &Module) -> Result<(), VerificationErrors> {
    verify_module_impl(module, None)
}

/// Verifies a module and rejects requirements outside a target capability set.
pub fn verify_module_with_capabilities(
    module: &Module,
    supported_capabilities: &BTreeSet<TargetCapability>,
) -> Result<(), VerificationErrors> {
    verify_module_impl(module, Some(supported_capabilities))
}

fn verify_module_impl(
    module: &Module,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
) -> Result<(), VerificationErrors> {
    let mut verifier = ModuleVerifier {
        module,
        diagnostics: Vec::new(),
        functions: BTreeMap::new(),
        supported_capabilities: supported_capabilities.cloned(),
    };
    verifier.verify();
    verifier.diagnostics.sort();
    if verifier.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(VerificationErrors {
            diagnostics: verifier.diagnostics,
        })
    }
}

struct ModuleVerifier<'module> {
    module: &'module Module,
    diagnostics: Vec<Diagnostic>,
    functions: BTreeMap<&'module FunctionId, &'module Function>,
    supported_capabilities: Option<BTreeSet<TargetCapability>>,
}

impl<'module> ModuleVerifier<'module> {
    fn verify(&mut self) {
        if self.module.id.as_str().is_empty() {
            self.emit(
                DiagnosticLocation::module(self.module),
                DiagnosticCode::InvalidIdentity,
                "module identity must not be empty",
            );
        }

        if let Some(supported) = self.supported_capabilities.clone() {
            self.verify_capabilities(&supported, DiagnosticLocation::module(self.module));
        }

        self.verify_capabilities(
            &self.module.required_capabilities,
            DiagnosticLocation::module(self.module),
        );

        for function in &self.module.functions {
            if function.id.as_str().is_empty() {
                self.emit(
                    DiagnosticLocation::function(self.module, function),
                    DiagnosticCode::InvalidIdentity,
                    "function identity must not be empty",
                );
            }
            if let Some(previous) = self.functions.insert(&function.id, function) {
                self.emit(
                    DiagnosticLocation::function(self.module, function),
                    DiagnosticCode::DuplicateFunction,
                    format!("function {} is defined more than once", function.id),
                );
                if previous.role != function.role {
                    self.emit(
                        DiagnosticLocation::function(self.module, function),
                        DiagnosticCode::ConflictingFunctionRole,
                        format!(
                            "function {} has conflicting roles {:?} and {:?}",
                            function.id, previous.role, function.role
                        ),
                    );
                }
            }
        }

        for function in &self.module.functions {
            self.verify_function(function);
        }

        let mut kernels = BTreeSet::new();
        let mut referenced_entries = BTreeSet::new();
        for kernel in &self.module.kernels {
            if kernel.id.as_str().is_empty() {
                self.emit(
                    DiagnosticLocation::kernel(self.module, kernel),
                    DiagnosticCode::InvalidIdentity,
                    "kernel identity must not be empty",
                );
            }
            if !kernels.insert(&kernel.id) {
                self.emit(
                    DiagnosticLocation::kernel(self.module, kernel),
                    DiagnosticCode::DuplicateKernel,
                    format!("kernel {} is declared more than once", kernel.id),
                );
            }
            referenced_entries.insert(&kernel.entry);
            self.verify_kernel(kernel);
        }
        for function in &self.module.functions {
            if function.role == FunctionRole::KernelEntry
                && !referenced_entries.contains(&function.id)
            {
                self.emit(
                    DiagnosticLocation::function(self.module, function),
                    DiagnosticCode::InvalidFunctionRole,
                    "KernelEntry function is not referenced by any kernel record",
                );
            }
        }
    }

    fn verify_function(&mut self, function: &Function) {
        let location = DiagnosticLocation::function(self.module, function);
        self.verify_capabilities(&function.required_capabilities, location.clone());

        if function.id.as_str().starts_with("__fe2o3_ir_float_v1_") {
            let valid = FloatOperation::from_intrinsic_id(&function.id).is_some_and(|float| {
                let expected = float.declaration();
                function.role == expected.role
                    && function.body.is_none()
                    && function.signature == expected.signature
                    && function.required_capabilities == expected.required_capabilities
            });
            if !valid {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidFloatOperation,
                    format!(
                        "reserved float intrinsic {} must have its exact canonical declaration",
                        function.id
                    ),
                );
            }
        }

        let role_requires_body = function.role != FunctionRole::ExternalImport;
        if role_requires_body != function.body.is_some() {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidFunctionRole,
                format!(
                    "function role {:?} is incompatible with a {} body",
                    function.role,
                    if function.body.is_some() {
                        "present"
                    } else {
                        "missing"
                    }
                ),
            );
        }

        let Some(body) = &function.body else {
            return;
        };

        if body.parameters.len() != function.signature.parameters.len() {
            self.emit(
                location.clone(),
                DiagnosticCode::SignatureMismatch,
                format!(
                    "body defines {} parameter values but signature has {} parameters",
                    body.parameters.len(),
                    function.signature.parameters.len()
                ),
            );
        }
        if body.blocks.is_empty() {
            self.emit(
                location,
                DiagnosticCode::EmptyFunction,
                "defined function must contain an entry block",
            );
            return;
        }

        let mut function_verifier = FunctionVerifier::new(
            self.module,
            function,
            &self.functions,
            self.supported_capabilities.as_ref(),
            &mut self.diagnostics,
        );
        function_verifier.verify();
    }

    fn verify_kernel(&mut self, kernel: &Kernel) {
        let location = DiagnosticLocation::kernel(self.module, kernel);
        self.verify_capabilities(&kernel.required_capabilities, location.clone());

        for extent in kernel.domain.extents() {
            if matches!(extent, LaunchExtent::Static(0)) {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidLaunchDomain,
                    "static launch extents must be non-zero",
                );
            }
        }

        if let Some(size) = kernel.workgroup_size {
            if size.x == 0 || size.y == 0 || size.z == 0 {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidWorkgroupSize,
                    "workgroup dimensions must be non-zero",
                );
            }
            if (kernel.domain.rank() == 1 && (size.y != 1 || size.z != 1))
                || (kernel.domain.rank() == 2 && size.z != 1)
            {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidWorkgroupSize,
                    "inactive workgroup dimensions must be one",
                );
            }
        }

        let Some(entry) = self.functions.get(&kernel.entry).copied() else {
            self.emit(
                location,
                DiagnosticCode::UnknownKernelEntry,
                format!("entry function {} is not in the module", kernel.entry),
            );
            return;
        };
        if entry.role != FunctionRole::KernelEntry {
            self.emit(
                location.clone(),
                DiagnosticCode::ConflictingFunctionRole,
                format!(
                    "kernel {} references function {} with role {:?}, expected KernelEntry",
                    kernel.id, kernel.entry, entry.role
                ),
            );
        }
        if entry.body.is_none() {
            self.emit(
                location,
                DiagnosticCode::KernelEntryDeclaration,
                format!("entry function {} has no body", kernel.entry),
            );
            return;
        }
        if !entry.signature.results.is_empty() {
            self.emit(
                DiagnosticLocation::kernel(self.module, kernel),
                DiagnosticCode::KernelReturnsValue,
                "kernel entry functions must not return values",
            );
        }

        self.verify_reachable_intrinsic_axes(kernel, entry);
    }

    fn verify_reachable_intrinsic_axes(&mut self, kernel: &Kernel, entry: &'module Function) {
        let mut pending = vec![entry];
        let mut visited = BTreeSet::new();

        while let Some(function) = pending.pop() {
            if !visited.insert(&function.id) {
                continue;
            }
            let Some(body) = &function.body else {
                continue;
            };

            for block in &body.blocks {
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    if let OperationKind::Intrinsic(intrinsic) = &operation.kind
                        && !kernel.domain.contains_axis(intrinsic.kind.axis())
                    {
                        let axis = intrinsic.kind.axis();
                        let mut location = DiagnosticLocation::function(self.module, function);
                        location.kernel = Some(kernel.id.clone());
                        self.emit(
                            location.at_block(block.id).at_operation(operation_index),
                            DiagnosticCode::InvalidLaunchDomain,
                            format!(
                                "axis {axis:?} is outside the {}D launch domain of kernel {}",
                                kernel.domain.rank(),
                                kernel.id
                            ),
                        );
                    }

                    if let OperationKind::Call { callee, .. } = &operation.kind
                        && let Some(callee) = self.functions.get(callee).copied()
                    {
                        pending.push(callee);
                    }
                }
            }
        }
    }

    fn verify_capabilities(
        &mut self,
        capabilities: &BTreeSet<TargetCapability>,
        location: DiagnosticLocation,
    ) {
        if capabilities.contains(&TargetCapability::DynamicWorkgroupMemory)
            && !capabilities.contains(&TargetCapability::WorkgroupMemory)
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidCapability,
                "dynamic workgroup memory requires the base workgroup-memory capability",
            );
        }
        let wave_widths = capabilities
            .iter()
            .filter_map(|capability| match capability {
                TargetCapability::WaveWidth(width) => Some(*width),
                _ => None,
            })
            .collect::<Vec<_>>();
        if wave_widths.len() > 1 {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidCapability,
                format!("conflicting exact wave-width requirements: {wave_widths:?}"),
            );
        }
        if let Some(wave_width) = wave_widths.first()
            && capabilities.iter().any(|capability| {
                matches!(capability, TargetCapability::SubgroupSize(size) if *size != wave_width.lanes())
            })
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidCapability,
                format!(
                    "wave width {} conflicts with the declared subgroup size",
                    wave_width.lanes()
                ),
            );
        }

        for capability in capabilities {
            let invalid = match capability {
                TargetCapability::SubgroupSize(size) => *size == 0 || !size.is_power_of_two(),
                TargetCapability::Atomic {
                    width_bits,
                    address_space,
                    max_scope,
                } => {
                    !matches!(*width_bits, 8 | 16 | 32 | 64)
                        || !matches!(
                            address_space,
                            AddressSpace::Workgroup | AddressSpace::Global | AddressSpace::Generic
                        )
                        || *max_scope == SynchronizationScope::Invocation
                        || (*address_space == AddressSpace::Workgroup
                            && max_scope.rank() > SynchronizationScope::Workgroup.rank())
                }
                TargetCapability::Extension { namespace, name } => {
                    namespace.is_empty() || name.is_empty()
                }
                _ => false,
            };
            if invalid {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidCapability,
                    format!("malformed target capability: {capability:?}"),
                );
            } else if self
                .supported_capabilities
                .as_ref()
                .is_some_and(|supported| !capability_is_supported(capability, supported))
            {
                self.emit(
                    location.clone(),
                    DiagnosticCode::UnsupportedCapability,
                    format!("target does not support required capability {capability:?}"),
                );
            }
        }
    }

    fn emit(
        &mut self,
        location: DiagnosticLocation,
        code: DiagnosticCode,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic {
            location,
            code,
            message: message.into(),
        });
    }
}

#[derive(Clone, Copy)]
enum DefSite {
    FunctionParameter,
    BlockParameter(BlockId),
    Operation(BlockId, usize),
}

#[derive(Clone)]
struct DefInfo {
    ty: Type,
    site: DefSite,
}

struct FunctionVerifier<'a, 'module> {
    module: &'module Module,
    function: &'module Function,
    functions: &'a BTreeMap<&'module FunctionId, &'module Function>,
    supported_capabilities: Option<&'a BTreeSet<TargetCapability>>,
    diagnostics: &'a mut Vec<Diagnostic>,
    definitions: BTreeMap<ValueId, DefInfo>,
    blocks: BTreeMap<BlockId, &'module BasicBlock>,
    dominators: BTreeMap<BlockId, BTreeSet<BlockId>>,
    dynamic_workgroup_memory_declarations: usize,
}

impl<'a, 'module> FunctionVerifier<'a, 'module> {
    fn new(
        module: &'module Module,
        function: &'module Function,
        functions: &'a BTreeMap<&'module FunctionId, &'module Function>,
        supported_capabilities: Option<&'a BTreeSet<TargetCapability>>,
        diagnostics: &'a mut Vec<Diagnostic>,
    ) -> Self {
        Self {
            module,
            function,
            functions,
            supported_capabilities,
            diagnostics,
            definitions: BTreeMap::new(),
            blocks: BTreeMap::new(),
            dominators: BTreeMap::new(),
            dynamic_workgroup_memory_declarations: 0,
        }
    }

    fn verify(&mut self) {
        let body = self.function.body.as_ref().expect("definition required");
        let base_location = DiagnosticLocation::function(self.module, self.function);

        for (index, value) in body.parameters.iter().copied().enumerate() {
            let Some(ty) = self.function.signature.parameters.get(index) else {
                break;
            };
            self.define(
                value,
                ty.clone(),
                DefSite::FunctionParameter,
                base_location.clone(),
            );
        }

        for block in &body.blocks {
            let location = base_location.clone().at_block(block.id);
            if self.blocks.insert(block.id, block).is_some() {
                self.emit(
                    location.clone(),
                    DiagnosticCode::DuplicateBlock,
                    format!("block {} is defined more than once", block.id),
                );
            }
            for parameter in &block.parameters {
                self.define(
                    parameter.id,
                    parameter.ty.clone(),
                    DefSite::BlockParameter(block.id),
                    location.clone(),
                );
            }
            for (operation_index, operation) in block.operations.iter().enumerate() {
                for result in &operation.results {
                    self.define(
                        result.id,
                        result.ty.clone(),
                        DefSite::Operation(block.id, operation_index),
                        location.clone().at_operation(operation_index),
                    );
                }
            }
            if block.terminator.is_none() {
                self.emit(
                    location,
                    DiagnosticCode::MissingTerminator,
                    "basic block has no terminator",
                );
            }
        }

        self.dominators = self.compute_dominators();

        for block in &body.blocks {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let location = base_location
                    .clone()
                    .at_block(block.id)
                    .at_operation(operation_index);
                for operand in operation.kind.operands() {
                    self.verify_use(operand, block.id, Some(operation_index), location.clone());
                }
                self.verify_operation(operation, location);
            }
            if let Some(terminator) = &block.terminator {
                let location = base_location.clone().at_block(block.id);
                for operand in terminator.operands() {
                    self.verify_use(operand, block.id, None, location.clone());
                }
                self.verify_terminator(block, terminator, location);
            }
        }
    }

    fn define(&mut self, value: ValueId, ty: Type, site: DefSite, location: DiagnosticLocation) {
        if self
            .definitions
            .insert(value, DefInfo { ty, site })
            .is_some()
        {
            self.emit(
                location,
                DiagnosticCode::DuplicateValue,
                format!("SSA value {value} is defined more than once"),
            );
        }
    }

    fn verify_use(
        &mut self,
        value: ValueId,
        use_block: BlockId,
        use_operation: Option<usize>,
        location: DiagnosticLocation,
    ) {
        let Some(definition) = self.definitions.get(&value) else {
            self.emit(
                location,
                DiagnosticCode::UndefinedValue,
                format!("SSA value {value} is not defined in this function"),
            );
            return;
        };

        let dominates = match definition.site {
            DefSite::FunctionParameter => true,
            DefSite::BlockParameter(def_block) => {
                def_block == use_block || self.block_dominates(def_block, use_block)
            }
            DefSite::Operation(def_block, def_operation) if def_block == use_block => {
                use_operation.is_none_or(|use_operation| def_operation < use_operation)
            }
            DefSite::Operation(def_block, _) => self.block_dominates(def_block, use_block),
        };
        if !dominates {
            self.emit(
                location,
                DiagnosticCode::NonDominatingUse,
                format!("definition of {value} does not dominate this use"),
            );
        }
    }

    fn block_dominates(&self, definition: BlockId, use_block: BlockId) -> bool {
        self.dominators
            .get(&use_block)
            .is_some_and(|dominators| dominators.contains(&definition))
    }

    fn verify_operation(&mut self, operation: &Operation, location: DiagnosticLocation) {
        if let Some(supported) = self.supported_capabilities {
            for capability in operation.required_capabilities() {
                if !capability_is_supported(&capability, supported) {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::UnsupportedCapability,
                        format!("target does not support required capability {capability:?}"),
                    );
                }
            }
        }

        if let Some(semantic) = operation.kind.semantic_operation() {
            let operands = operation.kind.operands();
            let operand_types = operands
                .iter()
                .map(|operand| self.ty(*operand).cloned())
                .collect::<Vec<_>>();
            let issues = semantic.verify(SemanticOperationVerificationContext {
                operands: &operands,
                results: &operation.results,
                operand_types: &operand_types,
            });
            for issue in issues {
                let code = match issue.kind {
                    SemanticOperationIssueKind::InvalidStructure => {
                        DiagnosticCode::InvalidSemanticOperation
                    }
                    SemanticOperationIssueKind::InvalidOperandType => {
                        DiagnosticCode::InvalidOperandType
                    }
                    SemanticOperationIssueKind::ResultArity => DiagnosticCode::ResultArity,
                    SemanticOperationIssueKind::TypeMismatch => DiagnosticCode::TypeMismatch,
                };
                self.emit(location.clone(), code, issue.message);
            }
            return;
        }

        match &operation.kind {
            OperationKind::Constant(constant) => {
                self.expect_results(operation, &[constant.ty()], location);
            }
            OperationKind::Intrinsic(_) | OperationKind::MemoryIntrinsic(_) => {
                unreachable!("semantic operations return before legacy operation verification")
            }
            OperationKind::Unary { op, operand } => {
                let Some(ty) = self.ty(*operand).cloned() else {
                    return;
                };
                let valid = match (op, ty.as_scalar()) {
                    (UnaryOp::Negate, Some(scalar)) => {
                        scalar.is_signed_integer() || scalar.is_float()
                    }
                    (UnaryOp::Not, Some(ScalarType::Bool)) => true,
                    (UnaryOp::Not, Some(scalar)) => scalar.is_integer(),
                    _ => false,
                };
                if !valid {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidOperandType,
                        format!("unary {op:?} does not accept {ty:?}"),
                    );
                }
                self.expect_results(operation, &[ty], location);
            }
            OperationKind::Binary { op, lhs, rhs } => {
                self.verify_binary(operation, *op, *lhs, *rhs, location);
            }
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } => self.verify_compare(operation, *predicate, *lhs, *rhs, location),
            OperationKind::Cast { value, to, .. } => {
                let Some(from) = self.ty(*value).cloned() else {
                    return;
                };
                if from.as_scalar().is_none() || to.as_scalar().is_none() {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidOperandType,
                        format!("casts require scalar types, found {from:?} to {to:?}"),
                    );
                }
                self.expect_results(operation, std::slice::from_ref(to), location);
            }
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => {
                self.expect_type(*condition, &Type::BOOL, location.clone());
                let (Some(true_ty), Some(false_ty)) = (
                    self.ty(*true_value).cloned(),
                    self.ty(*false_value).cloned(),
                ) else {
                    return;
                };
                if true_ty != false_ty {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::TypeMismatch,
                        format!("select alternatives differ: {true_ty:?} and {false_ty:?}"),
                    );
                }
                self.expect_results(operation, &[true_ty], location);
            }
            OperationKind::Call { callee, arguments } => {
                if callee.as_str().starts_with("__fe2o3_ir_float_v1_")
                    && FloatOperation::from_intrinsic_call(callee, arguments).is_none()
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidFloatOperation,
                        format!(
                            "reserved float intrinsic call {callee} must use its exact canonical contract"
                        ),
                    );
                }
                let Some(callee) = self.functions.get(callee).copied() else {
                    self.emit(
                        location,
                        DiagnosticCode::UnknownCallee,
                        format!("callee {callee} is not in the module"),
                    );
                    return;
                };
                self.verify_argument_list(
                    arguments,
                    &callee.signature.parameters,
                    location.clone(),
                );
                self.expect_results(operation, &callee.signature.results, location);
            }
            OperationKind::Alloca {
                element,
                count,
                address_space,
                alignment,
            } => {
                if !element.is_storable()
                    || !matches!(
                        address_space,
                        AddressSpace::Private | AddressSpace::Workgroup
                    )
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidMemoryAccess,
                        "alloca requires a storable type in private or workgroup memory",
                    );
                }
                if let Some(count) = count {
                    self.expect_integer(*count, location.clone());
                }
                self.verify_alignment(*alignment, location.clone());
                let result = pointer_for(element.clone(), *address_space, AccessMode::ReadWrite);
                self.expect_results(operation, &[result], location);
            }
            OperationKind::SliceLength { slice } => {
                if !matches!(self.ty(*slice), Some(Type::Slice(_))) {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidOperandType,
                        "slice_length operand must have slice type",
                    );
                }
                self.expect_results(operation, &[Type::INDEX], location);
            }
            OperationKind::SliceData { slice } => {
                let Some(Type::Slice(slice_ty)) = self.ty(*slice) else {
                    self.emit(
                        location,
                        DiagnosticCode::InvalidOperandType,
                        "slice_data operand must have slice type",
                    );
                    return;
                };
                let result = pointer_for(
                    (*slice_ty.element).clone(),
                    slice_ty.address_space,
                    slice_ty.access,
                );
                self.expect_results(operation, &[result], location);
            }
            OperationKind::GetElementPointer { base, offset } => {
                let Some(base_ty) = self.ty(*base).cloned() else {
                    return;
                };
                if !matches!(base_ty, Type::Pointer(_)) {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidOperandType,
                        "get_element_pointer base must have pointer type",
                    );
                }
                self.expect_integer(*offset, location.clone());
                self.expect_results(operation, &[base_ty], location);
            }
            OperationKind::Load { pointer, access } => {
                let Some(pointee) = self.verify_pointer_access(*pointer, *access, false, &location)
                else {
                    return;
                };
                self.expect_results(operation, &[pointee], location);
            }
            OperationKind::Store {
                pointer,
                value,
                access,
            } => {
                self.expect_results(operation, &[], location.clone());
                let Some(pointee) = self.verify_pointer_access(*pointer, *access, true, &location)
                else {
                    return;
                };
                self.expect_type(*value, &pointee, location);
            }
            OperationKind::Barrier(barrier) => {
                self.expect_results(operation, &[], location.clone());
                self.verify_barrier(barrier, location);
            }
            OperationKind::Atomic(atomic) => self.verify_atomic(operation, atomic, location),
            OperationKind::Fence(fence) => {
                self.expect_results(operation, &[], location.clone());
                self.verify_fence(fence, location);
            }
            OperationKind::WorkgroupBarrier(barrier) => {
                self.expect_results(operation, &[], location.clone());
                self.verify_workgroup_barrier(barrier, location);
            }
            OperationKind::WorkgroupMemory(memory) => {
                self.verify_workgroup_memory(operation, memory, location);
            }
            OperationKind::Matrix(matrix) => {
                let operand_types = matrix
                    .operands()
                    .iter()
                    .map(|operand| self.ty(*operand).cloned())
                    .collect::<Vec<_>>();
                for issue in matrix.verify(&operand_types, &operation.results) {
                    let code = match issue.kind {
                        MatrixVerificationIssueKind::InvalidStructure => {
                            DiagnosticCode::InvalidSemanticOperation
                        }
                        MatrixVerificationIssueKind::InvalidOperandType => {
                            DiagnosticCode::InvalidOperandType
                        }
                        MatrixVerificationIssueKind::InvalidResult => DiagnosticCode::TypeMismatch,
                    };
                    self.emit(location.clone(), code, issue.message);
                }
            }
            OperationKind::Wave(wave) => self.verify_wave(operation, wave, location),
            OperationKind::InlineAssembly(assembly) => {
                self.verify_inline_assembly(operation, assembly, location)
            }
        }
    }

    fn verify_inline_assembly(
        &mut self,
        operation: &Operation,
        assembly: &InlineAssembly,
        location: DiagnosticLocation,
    ) {
        if !assembly.source.is_complete() {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidInlineAssembly,
                "inline assembly requires nonzero frontend-unit, function, contract, and statement identities",
            );
        }
        if assembly.mnemonic.is_empty()
            || !assembly
                .mnemonic
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidInlineAssembly,
                "inline assembly mnemonic must be nonempty canonical lowercase ASCII",
            );
        }
        if assembly.operands.is_empty() {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidInlineAssembly,
                "inline assembly requires at least one exact operand",
            );
        }

        let mut referenced_results = BTreeSet::new();
        for (operand_index, operand) in assembly.operands.iter().enumerate() {
            let value = match operand.kind {
                AssemblyOperandKind::Input(value) => Some(value),
                AssemblyOperandKind::InOut {
                    input,
                    result_index,
                } => {
                    self.verify_assembly_result(
                        operation,
                        result_index,
                        operand.constraint,
                        operand_index,
                        &mut referenced_results,
                        location.clone(),
                    );
                    Some(input)
                }
                AssemblyOperandKind::Output { result_index } => {
                    self.verify_assembly_result(
                        operation,
                        result_index,
                        operand.constraint,
                        operand_index,
                        &mut referenced_results,
                        location.clone(),
                    );
                    None
                }
                AssemblyOperandKind::ImmediateI32(_) => {
                    if operand.constraint != AssemblyConstraint::ImmediateI32 {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::InvalidInlineAssembly,
                            format!(
                                "inline assembly immediate operand {operand_index} requires ImmediateI32 constraint"
                            ),
                        );
                    }
                    None
                }
            };
            if let Some(value) = value {
                if operand.constraint == AssemblyConstraint::ImmediateI32 {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidInlineAssembly,
                        format!(
                            "inline assembly SSA operand {operand_index} cannot use an immediate constraint"
                        ),
                    );
                }
                if let Some(ty) = self.ty(value).cloned()
                    && !is_assembly_register_type(&ty)
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidOperandType,
                        format!(
                            "inline assembly register operand {operand_index} requires i32 or u32, found {ty:?}"
                        ),
                    );
                }
            }
        }
        if referenced_results.len() != operation.results.len() {
            self.emit(
                location.clone(),
                DiagnosticCode::ResultArity,
                "every inline assembly result must be referenced exactly once by an output or inout operand",
            );
        }

        let no_memory = assembly.options.contains(&AssemblyOption::NoMemory);
        let read_only = assembly.options.contains(&AssemblyOption::ReadOnly);
        if no_memory && read_only {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidInlineAssembly,
                "NoMemory and ReadOnly assembly options are mutually exclusive",
            );
        }
        if assembly.options.contains(&AssemblyOption::Pure) && !(no_memory || read_only) {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidInlineAssembly,
                "Pure inline assembly requires NoMemory or ReadOnly",
            );
        }
        let has_memory_effect = assembly
            .declared_effects
            .iter()
            .any(|effect| !matches!(effect, AssemblyEffect::ControlFlow));
        if no_memory && has_memory_effect {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidInlineAssembly,
                "NoMemory inline assembly cannot declare memory, atomic, or barrier effects",
            );
        }
        let has_write = assembly.declared_effects.iter().any(|effect| {
            matches!(
                effect,
                AssemblyEffect::WriteGlobal
                    | AssemblyEffect::WriteWorkgroup
                    | AssemblyEffect::Atomic
            )
        });
        if read_only && has_write {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidInlineAssembly,
                "ReadOnly inline assembly cannot declare write or atomic effects",
            );
        }
        if assembly.options.contains(&AssemblyOption::Pure)
            && assembly
                .declared_effects
                .contains(&AssemblyEffect::ControlFlow)
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidInlineAssembly,
                "Pure inline assembly cannot declare control-flow effects",
            );
        }
        if assembly.declared_effects.is_empty() && !no_memory {
            self.emit(
                location,
                DiagnosticCode::InvalidInlineAssembly,
                "effect-free inline assembly requires an explicit NoMemory option",
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn verify_assembly_result(
        &mut self,
        operation: &Operation,
        result_index: u32,
        constraint: AssemblyConstraint,
        operand_index: usize,
        referenced_results: &mut BTreeSet<u32>,
        location: DiagnosticLocation,
    ) {
        let Some(result) = usize::try_from(result_index)
            .ok()
            .and_then(|index| operation.results.get(index))
        else {
            self.emit(
                location,
                DiagnosticCode::ResultArity,
                format!(
                    "inline assembly operand {operand_index} references missing result {result_index}"
                ),
            );
            return;
        };
        if !referenced_results.insert(result_index) {
            self.emit(
                location.clone(),
                DiagnosticCode::DuplicateValue,
                format!("inline assembly result {result_index} is referenced more than once"),
            );
        }
        if constraint == AssemblyConstraint::ImmediateI32 {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidInlineAssembly,
                format!("inline assembly output operand {operand_index} cannot be immediate"),
            );
        }
        if !is_assembly_register_type(&result.ty) {
            self.emit(
                location,
                DiagnosticCode::InvalidOperandType,
                format!(
                    "inline assembly result operand {operand_index} requires i32 or u32, found {:?}",
                    result.ty
                ),
            );
        }
    }

    fn verify_wave(
        &mut self,
        operation: &Operation,
        wave: &WaveOperation,
        location: DiagnosticLocation,
    ) {
        if wave.active_lanes != wave.width.lanes() {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidWaveOperation,
                format!(
                    "the first wave-operation subset requires all {} lanes active, found {}",
                    wave.width.lanes(),
                    wave.active_lanes
                ),
            );
        }
        if wave.convergence.scope() != SynchronizationScope::Subgroup {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidConvergence,
                "wave operation requires a uniform subgroup convergence claim",
            );
        }

        match wave.kind {
            WaveOperationKind::LaneId => {
                self.expect_results(operation, &[Type::Scalar(ScalarType::U32)], location)
            }
            WaveOperationKind::Ballot { predicate } => {
                self.expect_type(predicate, &Type::BOOL, location.clone());
                let result = match wave.width {
                    crate::WaveWidth::Wave32 => Type::Scalar(ScalarType::U32),
                    crate::WaveWidth::Wave64 => Type::Scalar(ScalarType::U64),
                };
                self.expect_results(operation, &[result], location);
            }
            WaveOperationKind::Any { predicate } | WaveOperationKind::All { predicate } => {
                self.expect_type(predicate, &Type::BOOL, location.clone());
                self.expect_results(operation, &[Type::BOOL], location);
            }
            WaveOperationKind::ShuffleIndex {
                value,
                source_lane,
                tile_width,
            } => {
                let value_ty = self.ty(value).cloned();
                if !matches!(
                    value_ty,
                    Some(Type::Scalar(ScalarType::I32 | ScalarType::U32))
                ) {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidOperandType,
                        "wave shuffle supports only i32 and u32 values",
                    );
                }
                self.expect_type(
                    source_lane,
                    &Type::Scalar(ScalarType::U32),
                    location.clone(),
                );
                if tile_width == 0
                    || !tile_width.is_power_of_two()
                    || tile_width > wave.width.lanes()
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidWaveOperation,
                        format!(
                            "shuffle tile width {tile_width} must be a non-zero power of two no larger than {}",
                            wave.width.lanes()
                        ),
                    );
                }
                if let Some(value_ty) = value_ty {
                    self.expect_results(operation, &[value_ty], location);
                }
            }
        }
    }

    fn verify_binary(
        &mut self,
        operation: &Operation,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        location: DiagnosticLocation,
    ) {
        let (Some(lhs_ty), Some(rhs_ty)) = (self.ty(lhs).cloned(), self.ty(rhs).cloned()) else {
            return;
        };
        let lhs_scalar = lhs_ty.as_scalar();
        let rhs_scalar = rhs_ty.as_scalar();
        let valid = match op {
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                lhs_scalar.is_some_and(ScalarType::is_integer)
                    && rhs_scalar.is_some_and(ScalarType::is_integer)
            }
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                lhs_ty == rhs_ty
                    && lhs_scalar
                        .is_some_and(|scalar| scalar == ScalarType::Bool || scalar.is_integer())
            }
            _ => lhs_ty == rhs_ty && lhs_scalar.is_some_and(ScalarType::is_numeric),
        };
        if !valid {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidOperandType,
                format!("binary {op:?} does not accept {lhs_ty:?} and {rhs_ty:?}"),
            );
        }
        self.expect_results(operation, &[lhs_ty], location);
    }

    fn verify_compare(
        &mut self,
        operation: &Operation,
        predicate: ComparePredicate,
        lhs: ValueId,
        rhs: ValueId,
        location: DiagnosticLocation,
    ) {
        let (Some(lhs_ty), Some(rhs_ty)) = (self.ty(lhs).cloned(), self.ty(rhs).cloned()) else {
            return;
        };
        let comparable = lhs_ty == rhs_ty
            && lhs_ty.as_scalar().is_some_and(|scalar| {
                scalar.is_numeric()
                    || (scalar == ScalarType::Bool
                        && matches!(
                            predicate,
                            ComparePredicate::Equal | ComparePredicate::NotEqual
                        ))
            });
        if !comparable {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidOperandType,
                format!("comparison does not accept {lhs_ty:?} and {rhs_ty:?}"),
            );
        }
        self.expect_results(operation, &[Type::BOOL], location);
    }

    fn verify_pointer_access(
        &mut self,
        pointer: ValueId,
        access: crate::MemoryAccess,
        write: bool,
        location: &DiagnosticLocation,
    ) -> Option<Type> {
        self.verify_alignment(access.alignment, location.clone());
        let pointer_ty = self.ty(pointer).cloned()?;
        let Type::Pointer(pointer_ty) = pointer_ty else {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidOperandType,
                format!("memory operand {pointer} does not have pointer type"),
            );
            return None;
        };
        if pointer_ty.address_space != access.address_space {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidMemoryAccess,
                format!(
                    "access names {:?} memory but pointer is in {:?} memory",
                    access.address_space, pointer_ty.address_space
                ),
            );
        }
        if write
            && (pointer_ty.access != AccessMode::ReadWrite
                || pointer_ty.address_space == AddressSpace::Constant)
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidMemoryAccess,
                "write requires a writable pointer outside constant memory",
            );
        }
        if !pointer_ty.pointee.is_storable() {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidMemoryAccess,
                "memory operation pointee is not storable",
            );
        }
        Some(*pointer_ty.pointee)
    }

    fn verify_barrier(&mut self, barrier: &Barrier, location: DiagnosticLocation) {
        let invalid_execution_scope = !matches!(
            barrier.execution_scope,
            SynchronizationScope::Subgroup | SynchronizationScope::Workgroup
        );
        let invalid_memory_scope = barrier.memory_scope.rank() < barrier.execution_scope.rank();
        let invalid_semantics = !valid_synchronization_semantics(
            barrier.memory_scope,
            barrier.semantics.ordering,
            &barrier.semantics.address_spaces,
        );
        if invalid_execution_scope || invalid_memory_scope || invalid_semantics {
            self.emit(
                location,
                DiagnosticCode::InvalidBarrier,
                "barrier requires subgroup/workgroup execution, a non-narrower legal memory scope, non-relaxed ordering, and shared writable memory",
            );
        }
    }

    fn verify_fence(&mut self, fence: &Fence, location: DiagnosticLocation) {
        if fence.memory_scope == SynchronizationScope::Invocation
            || !valid_synchronization_semantics(
                fence.memory_scope,
                fence.semantics.ordering,
                &fence.semantics.address_spaces,
            )
        {
            self.emit(
                location,
                DiagnosticCode::InvalidFence,
                "fence requires a scope wider than invocation, non-relaxed ordering, and memory visible at that scope",
            );
        }
    }

    fn verify_workgroup_barrier(
        &mut self,
        barrier: &WorkgroupBarrier,
        location: DiagnosticLocation,
    ) {
        if barrier.convergence.scope() != SynchronizationScope::Workgroup {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidConvergence,
                "workgroup barrier requires a uniform workgroup convergence claim",
            );
        }
        if barrier.memory_scope.rank() < SynchronizationScope::Workgroup.rank()
            || !valid_synchronization_semantics(
                barrier.memory_scope,
                barrier.semantics.ordering,
                &barrier.semantics.address_spaces,
            )
        {
            self.emit(
                location,
                DiagnosticCode::InvalidBarrier,
                "workgroup barrier requires workgroup-or-wider legal memory semantics",
            );
        }
    }

    fn verify_workgroup_memory(
        &mut self,
        operation: &Operation,
        memory: &WorkgroupMemory,
        location: DiagnosticLocation,
    ) {
        if !memory.element.is_storable() {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidWorkgroupMemory,
                "workgroup memory element type must be storable",
            );
        }
        if matches!(memory.extent, WorkgroupMemoryExtent::Static(0)) {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidWorkgroupMemory,
                "static workgroup memory extent must be non-zero",
            );
        }
        if memory.extent == WorkgroupMemoryExtent::Dynamic {
            self.dynamic_workgroup_memory_declarations += 1;
            if self.dynamic_workgroup_memory_declarations > 1 {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidWorkgroupMemory,
                    "a function may declare at most one dynamic workgroup-memory base",
                );
            }
        }
        self.verify_alignment(memory.alignment, location.clone());
        self.expect_results(
            operation,
            &[pointer_for(
                memory.element.clone(),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            )],
            location,
        );
    }

    fn verify_atomic(
        &mut self,
        operation: &Operation,
        atomic: &Atomic,
        location: DiagnosticLocation,
    ) {
        let write = atomic.kind != AtomicKind::Load;
        let pointee = self.verify_pointer_access(atomic.pointer, atomic.access, write, &location);
        let valid_space = matches!(
            atomic.access.address_space,
            AddressSpace::Workgroup | AddressSpace::Global | AddressSpace::Generic
        );
        if !valid_space
            || atomic.scope == SynchronizationScope::Invocation
            || !scope_can_observe_address_space(atomic.scope, atomic.access.address_space)
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidAtomic,
                "atomic scope cannot observe the selected address space",
            );
        }

        let Some(pointee) = pointee else {
            return;
        };
        let Some(scalar) = pointee.as_scalar() else {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidAtomic,
                "atomic pointee must be a scalar",
            );
            return;
        };

        let Some(width) = scalar.bit_width() else {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidAtomic,
                "atomic pointee must have a fixed physical width",
            );
            return;
        };

        if atomic.access.alignment < u32::from(width.div_ceil(8)) {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidAtomic,
                "atomic alignment is smaller than the scalar width",
            );
        }

        let scalar_class_is_valid = scalar != ScalarType::Bool
            && scalar != ScalarType::Index
            && match atomic.kind {
                AtomicKind::Min
                | AtomicKind::Max
                | AtomicKind::BitAnd
                | AtomicKind::BitOr
                | AtomicKind::BitXor => scalar.is_integer(),
                _ => scalar.is_integer() || scalar.is_float(),
            };
        if !scalar_class_is_valid {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidAtomic,
                format!("{:?} does not support {scalar:?}", atomic.kind),
            );
        }

        if valid_space
            && atomic.scope != SynchronizationScope::Invocation
            && scope_can_observe_address_space(atomic.scope, atomic.access.address_space)
            && let Some(supported) = self.supported_capabilities
        {
            let required = TargetCapability::Atomic {
                width_bits: width,
                address_space: atomic.access.address_space,
                max_scope: atomic.scope,
            };
            if !capability_is_supported(&required, supported) {
                self.emit(
                    location.clone(),
                    DiagnosticCode::UnsupportedCapability,
                    format!("target does not support required capability {required:?}"),
                );
            }
        }

        let expected_results: Vec<Type> = match atomic.kind {
            AtomicKind::Store => Vec::new(),
            AtomicKind::CompareExchange => vec![pointee.clone(), Type::BOOL],
            _ => vec![pointee.clone()],
        };
        self.expect_results(operation, &expected_results, location.clone());

        let valid_metadata = match atomic.kind {
            AtomicKind::Load => {
                atomic.value.is_none()
                    && atomic.compare.is_none()
                    && atomic.failure_ordering.is_none()
                    && matches!(
                        atomic.ordering,
                        MemoryOrdering::Relaxed
                            | MemoryOrdering::Acquire
                            | MemoryOrdering::SequentiallyConsistent
                    )
            }
            AtomicKind::Store => {
                atomic.value.is_some()
                    && atomic.compare.is_none()
                    && atomic.failure_ordering.is_none()
                    && matches!(
                        atomic.ordering,
                        MemoryOrdering::Relaxed
                            | MemoryOrdering::Release
                            | MemoryOrdering::SequentiallyConsistent
                    )
            }
            AtomicKind::CompareExchange => {
                atomic.value.is_some()
                    && atomic.compare.is_some()
                    && atomic
                        .failure_ordering
                        .is_some_and(|failure| valid_failure_ordering(atomic.ordering, failure))
            }
            _ => {
                atomic.value.is_some()
                    && atomic.compare.is_none()
                    && atomic.failure_ordering.is_none()
            }
        };
        if !valid_metadata {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidAtomic,
                format!("malformed {:?} operands or orderings", atomic.kind),
            );
        }

        if let Some(value) = atomic.value {
            self.expect_type(value, &pointee, location.clone());
        }
        if let Some(compare) = atomic.compare {
            self.expect_type(compare, &pointee, location);
        }
    }

    fn verify_terminator(
        &mut self,
        _block: &BasicBlock,
        terminator: &Terminator,
        location: DiagnosticLocation,
    ) {
        match terminator {
            Terminator::Branch { target, arguments } => {
                self.verify_edge(*target, arguments, location);
            }
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                self.expect_type(*condition, &Type::BOOL, location.clone());
                self.verify_edge(*then_target, then_arguments, location.clone());
                self.verify_edge(*else_target, else_arguments, location);
            }
            Terminator::Switch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                self.expect_integer(*selector, location.clone());
                let mut values = BTreeSet::new();
                for case in cases {
                    if !values.insert(case.value) {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::DuplicateSwitchCase,
                            format!("switch case {} appears more than once", case.value),
                        );
                    }
                    self.verify_edge(case.target, &case.arguments, location.clone());
                }
                self.verify_edge(*default_target, default_arguments, location);
            }
            Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                let selector_ty = self.ty(*selector).cloned();
                self.expect_integer(*selector, location.clone());
                let mut previous: Option<&crate::Constant> = None;
                for case in cases {
                    if let Some(previous) = previous {
                        match previous.cmp(&case.value) {
                            std::cmp::Ordering::Equal => self.emit(
                                location.clone(),
                                DiagnosticCode::DuplicateSwitchCase,
                                format!(
                                    "integer switch case {:?} appears more than once",
                                    case.value
                                ),
                            ),
                            std::cmp::Ordering::Greater => self.emit(
                                location.clone(),
                                DiagnosticCode::UnsortedSwitchCase,
                                format!(
                                    "integer switch case {:?} is not greater than previous case {previous:?}",
                                    case.value
                                ),
                            ),
                            std::cmp::Ordering::Less => {}
                        }
                    }
                    previous = Some(&case.value);

                    let case_ty = case.value.ty();
                    if !case_ty.as_scalar().is_some_and(ScalarType::is_integer) {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::InvalidOperandType,
                            format!(
                                "integer switch case {:?} must have integer or index type",
                                case.value
                            ),
                        );
                    }
                    if let Some(selector_ty) = selector_ty.as_ref()
                        && selector_ty != &case_ty
                    {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::TypeMismatch,
                            format!(
                                "integer switch case {:?} has type {case_ty:?}, expected selector type {selector_ty:?}",
                                case.value
                            ),
                        );
                    }
                    self.verify_edge(case.target, &case.arguments, location.clone());
                }
                self.verify_edge(*default_target, default_arguments, location);
            }
            Terminator::Return { values } => {
                self.verify_argument_list(values, &self.function.signature.results, location);
            }
            Terminator::Unreachable => {}
        }
    }

    fn verify_edge(
        &mut self,
        target: BlockId,
        arguments: &[ValueId],
        location: DiagnosticLocation,
    ) {
        let Some(target_block) = self.blocks.get(&target).copied() else {
            self.emit(
                location,
                DiagnosticCode::InvalidBranchTarget,
                format!("branch target {target} is not defined"),
            );
            return;
        };
        if arguments.len() != target_block.parameters.len() {
            self.emit(
                location.clone(),
                DiagnosticCode::BranchArgumentCount,
                format!(
                    "branch to {target} supplies {} arguments for {} block parameters",
                    arguments.len(),
                    target_block.parameters.len()
                ),
            );
        }
        for (argument, parameter) in arguments.iter().zip(&target_block.parameters) {
            let Some(argument_ty) = self.ty(*argument) else {
                continue;
            };
            if argument_ty != &parameter.ty {
                self.emit(
                    location.clone(),
                    DiagnosticCode::BranchArgumentType,
                    format!(
                        "branch argument {argument} has type {argument_ty:?}, expected {:?}",
                        parameter.ty
                    ),
                );
            }
        }
    }

    fn verify_argument_list(
        &mut self,
        values: &[ValueId],
        expected: &[Type],
        location: DiagnosticLocation,
    ) {
        if values.len() != expected.len() {
            self.emit(
                location.clone(),
                DiagnosticCode::SignatureMismatch,
                format!(
                    "found {} values where {} are required",
                    values.len(),
                    expected.len()
                ),
            );
        }
        for (value, expected_ty) in values.iter().zip(expected) {
            self.expect_type(*value, expected_ty, location.clone());
        }
    }

    fn expect_results(
        &mut self,
        operation: &Operation,
        expected: &[Type],
        location: DiagnosticLocation,
    ) {
        if operation.results.len() != expected.len() {
            self.emit(
                location.clone(),
                DiagnosticCode::ResultArity,
                format!(
                    "operation defines {} results but {} are required",
                    operation.results.len(),
                    expected.len()
                ),
            );
        }
        for (result, expected_ty) in operation.results.iter().zip(expected) {
            if &result.ty != expected_ty {
                self.emit(
                    location.clone(),
                    DiagnosticCode::TypeMismatch,
                    format!(
                        "result {} has type {:?}, expected {expected_ty:?}",
                        result.id, result.ty
                    ),
                );
            }
        }
    }

    fn expect_type(&mut self, value: ValueId, expected: &Type, location: DiagnosticLocation) {
        let Some(actual) = self.ty(value) else {
            return;
        };
        if actual != expected {
            self.emit(
                location,
                DiagnosticCode::TypeMismatch,
                format!("value {value} has type {actual:?}, expected {expected:?}"),
            );
        }
    }

    fn expect_integer(&mut self, value: ValueId, location: DiagnosticLocation) {
        let valid = self
            .ty(value)
            .and_then(Type::as_scalar)
            .is_some_and(ScalarType::is_integer);
        if !valid && self.ty(value).is_some() {
            self.emit(
                location,
                DiagnosticCode::InvalidOperandType,
                format!("value {value} must have integer or index type"),
            );
        }
    }

    fn verify_alignment(&mut self, alignment: u32, location: DiagnosticLocation) {
        if alignment == 0 || !alignment.is_power_of_two() {
            self.emit(
                location,
                DiagnosticCode::InvalidAlignment,
                format!("alignment {alignment} is not a non-zero power of two"),
            );
        }
    }

    fn ty(&self, value: ValueId) -> Option<&Type> {
        self.definitions
            .get(&value)
            .map(|definition| &definition.ty)
    }

    fn compute_dominators(&self) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
        let body = self.function.body.as_ref().expect("definition required");
        let entry = body.blocks[0].id;
        let all_blocks: BTreeSet<_> = self.blocks.keys().copied().collect();
        let mut predecessors: BTreeMap<BlockId, BTreeSet<BlockId>> = self
            .blocks
            .keys()
            .copied()
            .map(|block| (block, BTreeSet::new()))
            .collect();
        for block in self.blocks.values() {
            let Some(terminator) = &block.terminator else {
                continue;
            };
            for successor in terminator.successors() {
                if let Some(predecessors) = predecessors.get_mut(&successor) {
                    predecessors.insert(block.id);
                }
            }
        }

        let mut reachable = BTreeSet::from([entry]);
        let mut frontier = vec![entry];
        while let Some(block) = frontier.pop() {
            let Some(terminator) = self
                .blocks
                .get(&block)
                .and_then(|block| block.terminator.as_ref())
            else {
                continue;
            };
            for successor in terminator.successors() {
                if self.blocks.contains_key(&successor) && reachable.insert(successor) {
                    frontier.push(successor);
                }
            }
        }

        let mut dominators = BTreeMap::new();
        for block in &all_blocks {
            let initial = if *block == entry {
                BTreeSet::from([entry])
            } else if reachable.contains(block) {
                reachable.clone()
            } else {
                BTreeSet::from([*block])
            };
            dominators.insert(*block, initial);
        }

        loop {
            let mut changed = false;
            for block in reachable.iter().copied().filter(|block| *block != entry) {
                let reachable_predecessors: Vec<_> = predecessors[&block]
                    .iter()
                    .copied()
                    .filter(|predecessor| reachable.contains(predecessor))
                    .collect();
                let mut next = if let Some(first) = reachable_predecessors.first() {
                    dominators[first].clone()
                } else {
                    BTreeSet::new()
                };
                for predecessor in reachable_predecessors.iter().skip(1) {
                    next = next
                        .intersection(&dominators[predecessor])
                        .copied()
                        .collect();
                }
                next.insert(block);
                if dominators[&block] != next {
                    dominators.insert(block, next);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        dominators
    }

    fn emit(
        &mut self,
        location: DiagnosticLocation,
        code: DiagnosticCode,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic {
            location,
            code,
            message: message.into(),
        });
    }
}

fn valid_failure_ordering(success: MemoryOrdering, failure: MemoryOrdering) -> bool {
    match success {
        MemoryOrdering::Relaxed => failure == MemoryOrdering::Relaxed,
        MemoryOrdering::Acquire => {
            matches!(failure, MemoryOrdering::Relaxed | MemoryOrdering::Acquire)
        }
        MemoryOrdering::Release => failure == MemoryOrdering::Relaxed,
        MemoryOrdering::AcquireRelease => {
            matches!(failure, MemoryOrdering::Relaxed | MemoryOrdering::Acquire)
        }
        MemoryOrdering::SequentiallyConsistent => matches!(
            failure,
            MemoryOrdering::Relaxed
                | MemoryOrdering::Acquire
                | MemoryOrdering::SequentiallyConsistent
        ),
    }
}

fn valid_synchronization_semantics(
    scope: SynchronizationScope,
    ordering: MemoryOrdering,
    address_spaces: &BTreeSet<AddressSpace>,
) -> bool {
    ordering != MemoryOrdering::Relaxed
        && !address_spaces.is_empty()
        && address_spaces
            .iter()
            .all(|address_space| scope_can_observe_address_space(scope, *address_space))
}

fn scope_can_observe_address_space(
    scope: SynchronizationScope,
    address_space: AddressSpace,
) -> bool {
    match address_space {
        AddressSpace::Workgroup => matches!(
            scope,
            SynchronizationScope::Subgroup | SynchronizationScope::Workgroup
        ),
        AddressSpace::Global | AddressSpace::Generic => scope != SynchronizationScope::Invocation,
        AddressSpace::Private | AddressSpace::Constant => false,
    }
}

fn is_assembly_register_type(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(ScalarType::I32 | ScalarType::U32))
}

fn capability_is_supported(
    required: &TargetCapability,
    supported: &BTreeSet<TargetCapability>,
) -> bool {
    match required {
        TargetCapability::Atomic {
            width_bits,
            address_space,
            max_scope,
        } => supported.iter().any(|capability| {
            matches!(
                capability,
                TargetCapability::Atomic {
                    width_bits: supported_width,
                    address_space: supported_space,
                    max_scope: supported_scope,
                } if supported_width == width_bits
                    && supported_space == address_space
                    && supported_scope.rank() >= max_scope.rank()
            )
        }),
        _ => supported.contains(required),
    }
}
