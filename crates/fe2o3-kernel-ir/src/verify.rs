use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, AssemblyConstraint, AssemblyEffect,
    AssemblyOperandKind, AssemblyOption, AsyncCopyCompletionV1, Atomic, AtomicKind, Barrier,
    BasicBlock, BinaryOp, BlockId, CastKind, CheckedBinaryOperator,
    CollectiveCapabilityOperationV1, ComparePredicate, Constant, ControlFlowError,
    ExecutionCapabilityOpV1, ExecutionCapabilityRequirementV1, ExecutionCapabilityRoleV1, Fence,
    FloatOperation, Function, FunctionId, FunctionRole, Gfx950LdsTransposeFormatV1,
    Gfx950LdsTransposeOperationKindV1, Gfx950LdsTransposeOperationV1, GlobalCapabilityRoleV1,
    IndexedControlFlow, InlineAssembly, Kernel, KernelId, LaunchExtent, MatrixOperation,
    MatrixOperationKind, MatrixVerificationIssueKind, MemoryOrdering, Module, ModuleId, Operation,
    OperationKind, ResourceCapabilityRequirementV1, ScalarType, SemanticOperationIssueKind,
    SemanticOperationVerificationContext, SynchronizationScope, TargetCapability, Terminator, Type,
    UnaryOp, ValueId, WaveOperation, WaveOperationKind, WorkgroupBarrier, WorkgroupMemory,
    WorkgroupMemoryExtent, analyze_control_flow, pointer_for,
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
    InvalidCast,
    InvalidSemanticOperation,
    InvalidMemoryAccess,
    InvalidAlignment,
    InvalidBarrier,
    InvalidAtomic,
    InvalidFence,
    InvalidConvergence,
    ResourceLimit,
    InvalidWorkgroupMemory,
    InvalidGfx950LdsTranspose,
    InvalidWaveOperation,
    InvalidFloatOperation,
    InvalidAmdGpuDiagnosticOperation,
    InvalidInlineAssembly,
    InvalidKernelContext,
    InvalidGlobalCapability,
    InvalidExecutionCapability,
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
    verify_module_ref(module).map(|_| ())
}

/// A borrow of a module whose structural and local semantic invariants were
/// checked by this crate.
///
/// The private field prevents analysis crates from bypassing verification when
/// several passes need to share one verified module traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedKernelIrModuleV1<'module> {
    module: &'module Module,
}

impl<'module> VerifiedKernelIrModuleV1<'module> {
    pub const fn module(self) -> &'module Module {
        self.module
    }
}

/// Verifies a module and returns a non-owning token reusable by later analyses.
pub fn verify_module_ref(
    module: &Module,
) -> Result<VerifiedKernelIrModuleV1<'_>, VerificationErrors> {
    verify_module_impl(module, None)?;
    Ok(VerifiedKernelIrModuleV1 { module })
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
        self.verify_kernel_context_signature(function, location.clone());

        if function
            .id
            .as_str()
            .starts_with("__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_")
        {
            let valid = AmdGpuDiagnosticOperation::from_intrinsic_id(&function.id).is_some_and(
                |diagnostic| {
                    let expected = diagnostic.declaration();
                    let legacy = diagnostic.legacy_gfx942_declaration();
                    function.role == expected.role
                        && function.body.is_none()
                        && function.signature == expected.signature
                        && (function.required_capabilities == expected.required_capabilities
                            || function.required_capabilities == legacy.required_capabilities)
                },
            );
            if !valid {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidAmdGpuDiagnosticOperation,
                    format!(
                        "reserved AMDGPU diagnostic intrinsic {} must have its exact canonical declaration",
                        function.id
                    ),
                );
            }
        }

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

        let control_flow = match analyze_control_flow(function) {
            Ok(control_flow) => Some(control_flow),
            Err(
                error @ (ControlFlowError::ResourceLimit { .. }
                | ControlFlowError::ArithmeticOverflow(_)),
            ) => {
                self.emit(location, DiagnosticCode::ResourceLimit, error.to_string());
                return;
            }
            Err(_) => None,
        };
        let mut function_verifier = FunctionVerifier::new(
            self.module,
            function,
            &self.functions,
            self.supported_capabilities.as_ref(),
            &mut self.diagnostics,
            control_flow,
        );
        function_verifier.verify();
    }

    fn verify_kernel_context_signature(
        &mut self,
        function: &Function,
        location: DiagnosticLocation,
    ) {
        let mut direct_contexts = 0_usize;
        for (index, ty) in function.signature.parameters.iter().enumerate() {
            let is_direct_context = matches!(ty, Type::KernelContext(_));
            direct_contexts += usize::from(is_direct_context);
            if ty.contains_kernel_context()
                && (function.role != FunctionRole::InternalHelper || !is_direct_context)
            {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidKernelContext,
                    format!(
                        "parameter {index} places a logical kernel context in the {:?} ABI",
                        function.role
                    ),
                );
            }
            if is_direct_context && index != 0 {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidKernelContext,
                    format!(
                        "logical kernel context must be parameter 0 of an internal helper, found parameter {index}"
                    ),
                );
            }
            if let Type::KernelContext(context) = ty
                && !context.is_complete()
            {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidKernelContext,
                    format!("parameter {index} has an incomplete kernel-context brand"),
                );
            }
            if let Type::GlobalCapability(capability) = ty {
                if function.role != FunctionRole::InternalHelper {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGlobalCapability,
                        format!(
                            "parameter {index} places branded global authority in the {:?} ABI",
                            function.role
                        ),
                    );
                }
                if !capability.is_complete() {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGlobalCapability,
                        format!("parameter {index} has an incomplete global-capability type"),
                    );
                }
            }
            if let Type::ExecutionCapability(capability) = ty {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidExecutionCapability,
                    format!(
                        "parameter {index} places execution authority in the {:?} ABI instead of a verified V13 defining graph",
                        function.role
                    ),
                );
                if !capability.is_complete() {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidExecutionCapability,
                        format!("parameter {index} has an incomplete execution-capability type"),
                    );
                }
            }
        }
        if direct_contexts > 1 {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidKernelContext,
                "an internal helper may accept at most one logical kernel context",
            );
        }
        for (index, ty) in function.signature.results.iter().enumerate() {
            if ty.contains_logical_capability() {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidGlobalCapability,
                    format!("result {index} would let logical capability authority escape"),
                );
            }
        }
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
        self.verify_reachable_kernel_context_flow(kernel, entry);
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

    fn verify_reachable_kernel_context_flow(&mut self, kernel: &Kernel, entry: &'module Function) {
        let issued = entry
            .body
            .iter()
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(
                |operation| match (&operation.kind, operation.results.as_slice()) {
                    (OperationKind::KernelContextIssue(_), [result]) => match &result.ty {
                        Type::KernelContext(context) => Some(context.clone()),
                        _ => None,
                    },
                    _ => None,
                },
            )
            .collect::<Vec<_>>();
        let [expected] = issued.as_slice() else {
            // Context-free older modules retain their established validation.
            // Local verification rejects malformed or duplicate V12 issuances.
            return;
        };

        let mut pending = vec![entry.id.clone()];
        let mut visited = BTreeSet::new();
        while let Some(function_id) = pending.pop() {
            if !visited.insert(function_id.clone()) {
                continue;
            }
            let Some(function) = self.functions.get(&function_id).copied() else {
                continue;
            };
            let helper_context = function
                .signature
                .parameters
                .iter()
                .find_map(|ty| match ty {
                    Type::KernelContext(context) => Some(context),
                    Type::GlobalCapability(capability) => Some(capability.context()),
                    _ => None,
                });
            let carries_context = function.id == entry.id
                || helper_context.is_some_and(|context| context == expected);
            let Some(body) = &function.body else {
                continue;
            };

            for block in &body.blocks {
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    let mut location = DiagnosticLocation::function(self.module, function);
                    location.kernel = Some(kernel.id.clone());
                    location = location.at_block(block.id).at_operation(operation_index);
                    if operation_requires_kernel_context(operation) && !carries_context {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::InvalidKernelContext,
                            format!(
                                "context-enabled kernel {} reaches a capability operation through helper {} without its exact context brand",
                                kernel.id, function.id
                            ),
                        );
                    }

                    let OperationKind::Call { callee, .. } = &operation.kind else {
                        continue;
                    };
                    let Some(callee) = self.functions.get(callee).copied() else {
                        continue;
                    };
                    if callee.role != FunctionRole::ExternalImport {
                        pending.push(callee.id.clone());
                    }
                    let callee_context =
                        callee.signature.parameters.first().and_then(|ty| match ty {
                            Type::KernelContext(context) => Some(context),
                            _ => None,
                        });
                    if let Some(callee_context) = callee_context
                        && (!carries_context || callee_context != expected)
                    {
                        self.emit(
                            location,
                            DiagnosticCode::InvalidKernelContext,
                            format!(
                                "call from {} to {} substitutes or omits the exact context brand for kernel {}",
                                function.id, callee.id, kernel.id
                            ),
                        );
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
                TargetCapability::Execution(requirement) => {
                    !valid_execution_capability_requirement(requirement)
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
    control_flow: Option<IndexedControlFlow>,
    dynamic_workgroup_memory_declarations: usize,
    gfx950_lds_transpose_currents: BTreeSet<Gfx950LdsTransposeFormatV1>,
    kernel_context_issuances: usize,
    kernel_context_authority: Option<DefInfo>,
    bound_physical_globals: BTreeSet<ValueId>,
    execution_source_locations: BTreeSet<([u8; 32], u32)>,
}

impl<'a, 'module> FunctionVerifier<'a, 'module> {
    fn new(
        module: &'module Module,
        function: &'module Function,
        functions: &'a BTreeMap<&'module FunctionId, &'module Function>,
        supported_capabilities: Option<&'a BTreeSet<TargetCapability>>,
        diagnostics: &'a mut Vec<Diagnostic>,
        control_flow: Option<IndexedControlFlow>,
    ) -> Self {
        Self {
            module,
            function,
            functions,
            supported_capabilities,
            diagnostics,
            definitions: BTreeMap::new(),
            blocks: BTreeMap::new(),
            control_flow,
            dynamic_workgroup_memory_declarations: 0,
            gfx950_lds_transpose_currents: BTreeSet::new(),
            kernel_context_issuances: 0,
            kernel_context_authority: None,
            bound_physical_globals: BTreeSet::new(),
            execution_source_locations: BTreeSet::new(),
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

        for (block_index, block) in body.blocks.iter().enumerate() {
            let location = base_location.clone().at_block(block.id);
            if self.blocks.insert(block.id, block).is_some() {
                self.emit(
                    location.clone(),
                    DiagnosticCode::DuplicateBlock,
                    format!("block {} is defined more than once", block.id),
                );
            }
            for parameter in &block.parameters {
                if let Type::KernelContext(context) = &parameter.ty {
                    if block_index == 0 {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::InvalidKernelContext,
                            format!(
                                "entry block parameter {} cannot introduce a logical kernel context",
                                parameter.id
                            ),
                        );
                    }
                    if !context.is_complete() {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::InvalidKernelContext,
                            format!(
                                "block parameter {} has an incomplete kernel-context brand",
                                parameter.id
                            ),
                        );
                    }
                }
                if parameter.ty.contains_kernel_context()
                    && !matches!(parameter.ty, Type::KernelContext(_))
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidKernelContext,
                        format!(
                            "block parameter {} nests a logical kernel context in another type",
                            parameter.id
                        ),
                    );
                }
                if let Type::GlobalCapability(capability) = &parameter.ty {
                    if block_index == 0 || !capability.is_complete() {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::InvalidGlobalCapability,
                            format!(
                                "block parameter {} introduces invalid global-capability authority",
                                parameter.id
                            ),
                        );
                    }
                }
                if let Type::ExecutionCapability(capability) = &parameter.ty
                    && (block_index == 0 || !capability.is_complete())
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidExecutionCapability,
                        format!(
                            "block parameter {} introduces invalid execution authority",
                            parameter.id
                        ),
                    );
                }
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

        let issued_authorities = body
            .blocks
            .iter()
            .flat_map(|block| {
                block.operations.iter().enumerate().filter_map(
                    move |(operation_index, operation)| {
                        if matches!(operation.kind, OperationKind::KernelContextIssue(_)) {
                            operation.results.first().map(|result| DefInfo {
                                ty: result.ty.clone(),
                                site: DefSite::Operation(block.id, operation_index),
                            })
                        } else {
                            None
                        }
                    },
                )
            })
            .collect::<Vec<_>>();
        if let [authority] = issued_authorities.as_slice() {
            self.kernel_context_authority = Some(authority.clone());
        }

        let bound_physical_globals = body
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation.kind {
                OperationKind::GlobalCapabilityBind(bind) => Some(bind.physical),
                _ => None,
            })
            .collect::<BTreeSet<_>>();

        for block in &body.blocks {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let location = base_location
                    .clone()
                    .at_block(block.id)
                    .at_operation(operation_index);
                if matches!(
                    &operation.kind,
                    OperationKind::Call { callee, arguments }
                        if AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
                            .is_some_and(|diagnostic| diagnostic.is_terminating())
                ) && (operation_index + 1 != block.operations.len()
                    || !matches!(block.terminator, Some(Terminator::Unreachable)))
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidAmdGpuDiagnosticOperation,
                        "terminating AMDGPU diagnostic must be the final operation of a block terminated by unreachable",
                    );
                }
                if !matches!(operation.kind, OperationKind::GlobalCapabilityBind(_))
                    && operation
                        .kind
                        .operands()
                        .iter()
                        .any(|operand| bound_physical_globals.contains(operand))
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGlobalCapability,
                        "a bound physical global slice may be used only through its logical capability",
                    );
                }
                let capability_operand = operation
                    .kind
                    .operands()
                    .into_iter()
                    .any(|operand| matches!(self.ty(operand), Some(Type::GlobalCapability(_))));
                if capability_operand
                    && !matches!(
                        operation.kind,
                        OperationKind::SliceLength { .. }
                            | OperationKind::SliceData { .. }
                            | OperationKind::GlobalCapabilityIndex(_)
                            | OperationKind::Call { .. }
                    )
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGlobalCapability,
                        "global-capability authority is used by an operation outside its closed projection surface",
                    );
                }
                let execution_operand =
                    operation.kind.operands().into_iter().any(|operand| {
                        matches!(self.ty(operand), Some(Type::ExecutionCapability(_)))
                    });
                if execution_operand
                    && !matches!(
                        operation.kind,
                        OperationKind::ExecutionCapability(_) | OperationKind::Call { .. }
                    )
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidExecutionCapability,
                        "execution authority is used outside the closed V13 operation surface",
                    );
                }
                for operand in operation.kind.operands() {
                    self.verify_use(operand, block.id, Some(operation_index), location.clone());
                }
                self.verify_kernel_context_authority(
                    operation,
                    block.id,
                    operation_index,
                    location.clone(),
                );
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
        self.control_flow
            .as_ref()
            .is_some_and(|control_flow| control_flow.dominates(definition, use_block))
    }

    fn verify_kernel_context_authority(
        &mut self,
        operation: &Operation,
        block: BlockId,
        operation_index: usize,
        location: DiagnosticLocation,
    ) {
        for operand in operation.kind.operands() {
            if self
                .definitions
                .get(&operand)
                .is_some_and(|definition| definition.ty.contains_kernel_context())
                && !matches!(
                    operation.kind,
                    OperationKind::Call { .. }
                        | OperationKind::GlobalCapabilityBind(_)
                        | OperationKind::ExecutionCapability(_)
                )
            {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidKernelContext,
                    "logical kernel-context authority may flow only through typed helper calls, V13 execution operations, and control-flow block arguments",
                );
            }
        }

        if !operation_requires_kernel_context(operation)
            || self.function.role != FunctionRole::KernelEntry
        {
            return;
        }
        let Some(authority) = self.kernel_context_authority.clone() else {
            return;
        };
        if !self.site_dominates(authority.site, block, Some(operation_index)) {
            self.emit(
                location,
                DiagnosticCode::InvalidKernelContext,
                "kernel-context issuance must dominate every capability operation in its physical root",
            );
        }
    }

    fn site_dominates(
        &self,
        definition: DefSite,
        use_block: BlockId,
        use_operation: Option<usize>,
    ) -> bool {
        match definition {
            DefSite::FunctionParameter => true,
            DefSite::BlockParameter(def_block) => {
                def_block == use_block || self.block_dominates(def_block, use_block)
            }
            DefSite::Operation(def_block, def_operation) if def_block == use_block => {
                use_operation.is_none_or(|use_operation| def_operation < use_operation)
            }
            DefSite::Operation(def_block, _) => self.block_dominates(def_block, use_block),
        }
    }

    fn verify_operation(&mut self, operation: &Operation, location: DiagnosticLocation) {
        if !matches!(&operation.kind, OperationKind::KernelContextIssue(_))
            && operation
                .results
                .iter()
                .any(|result| result.ty.contains_kernel_context())
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidKernelContext,
                "only KernelContextIssue may define a kernel-context SSA value",
            );
        }
        if !matches!(&operation.kind, OperationKind::GlobalCapabilityBind(_))
            && operation
                .results
                .iter()
                .any(|result| matches!(result.ty, Type::GlobalCapability(_)))
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidGlobalCapability,
                "only GlobalCapabilityBind may define global-capability SSA authority",
            );
        }
        if !matches!(&operation.kind, OperationKind::ExecutionCapability(_))
            && operation
                .results
                .iter()
                .any(|result| matches!(result.ty, Type::ExecutionCapability(_)))
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidExecutionCapability,
                "only a V13 execution operation may define execution authority",
            );
        }

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
            OperationKind::Cast { kind, value, to } => {
                let Some(from) = self.ty(*value).cloned() else {
                    return;
                };
                match (kind, &from, to) {
                    (
                        CastKind::RestrictPointerAccess,
                        Type::Pointer(from_pointer),
                        Type::Pointer(to_pointer),
                    ) if from_pointer.pointee == to_pointer.pointee
                        && from_pointer.address_space == to_pointer.address_space
                        && from_pointer.access == AccessMode::ReadWrite
                        && to_pointer.access == AccessMode::ReadOnly => {}
                    (CastKind::RestrictPointerAccess, _, _) => {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::InvalidCast,
                            format!("invalid {kind:?} cast from {from:?} to {to:?}"),
                        );
                    }
                    (_, Type::Scalar(from_scalar), Type::Scalar(to_scalar)) => {
                        if !valid_scalar_cast(*kind, *from_scalar, *to_scalar) {
                            self.emit(
                                location.clone(),
                                DiagnosticCode::InvalidCast,
                                format!("invalid {kind:?} cast from {from:?} to {to:?}"),
                            );
                        }
                    }
                    _ => {
                        self.emit(
                            location.clone(),
                            DiagnosticCode::InvalidOperandType,
                            format!("casts require compatible scalar or pointer types, found {from:?} to {to:?}"),
                        );
                    }
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
                if callee
                    .as_str()
                    .starts_with("__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_")
                    && AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments).is_none()
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidAmdGpuDiagnosticOperation,
                        format!(
                            "reserved AMDGPU diagnostic intrinsic call {callee} must use its exact canonical contract"
                        ),
                    );
                }
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
                if !matches!(
                    self.ty(*slice),
                    Some(Type::Slice(_) | Type::GlobalCapability(_))
                ) {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidOperandType,
                        "slice_length operand must have slice type",
                    );
                }
                self.expect_results(operation, &[Type::INDEX], location);
            }
            OperationKind::SliceData { slice } => {
                let result = match self.ty(*slice) {
                    Some(Type::Slice(slice_ty)) => pointer_for(
                        (*slice_ty.element).clone(),
                        slice_ty.address_space,
                        slice_ty.access,
                    ),
                    Some(Type::GlobalCapability(capability)) => capability.physical_pointer_type(),
                    _ => {
                        self.emit(
                            location,
                            DiagnosticCode::InvalidOperandType,
                            "slice_data operand must have slice or global-capability type",
                        );
                        return;
                    }
                };
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
            OperationKind::GuardedLoad {
                pointer,
                predicate,
                fallback,
                access,
            } => {
                let Some(pointee) = self.verify_pointer_access(*pointer, *access, false, &location)
                else {
                    return;
                };
                self.expect_type(*predicate, &Type::BOOL, location.clone());
                self.expect_type(*fallback, &pointee, location.clone());
                if let Some(Err(message)) =
                    self.verify_guarded_global_capability_access(*pointer, *predicate, false)
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGlobalCapability,
                        message,
                    );
                }
                self.expect_results(operation, &[pointee], location);
            }
            OperationKind::GuardedStore {
                pointer,
                predicate,
                value,
                access,
            } => {
                self.expect_results(operation, &[], location.clone());
                let Some(pointee) = self.verify_pointer_access(*pointer, *access, true, &location)
                else {
                    return;
                };
                self.expect_type(*predicate, &Type::BOOL, location.clone());
                if let Some(Err(message)) =
                    self.verify_guarded_global_capability_access(*pointer, *predicate, true)
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGlobalCapability,
                        message,
                    );
                }
                self.expect_type(*value, &pointee, location);
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
                self.verify_matrix_lds_allocation(matrix, location);
            }
            OperationKind::Gfx950LdsTranspose(transpose) => {
                self.verify_gfx950_lds_transpose(operation, transpose, location)
            }
            OperationKind::Wave(wave) => self.verify_wave(operation, wave, location),
            OperationKind::InlineAssembly(assembly) => {
                self.verify_inline_assembly(operation, assembly, location)
            }
            OperationKind::KernelContextIssue(issue) => {
                self.kernel_context_issuances += 1;
                if self.kernel_context_issuances > 1 {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidKernelContext,
                        "a kernel entry may issue at most one logical kernel context",
                    );
                }
                if self.function.role != FunctionRole::KernelEntry {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidKernelContext,
                        "a logical kernel context may be issued only in a kernel entry",
                    );
                }
                if !issue.source().is_complete() {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidKernelContext,
                        "kernel-context source provenance is incomplete",
                    );
                }
                if operation.results.len() != 1 {
                    self.emit(
                        location,
                        DiagnosticCode::ResultArity,
                        format!(
                            "KernelContextIssue defines {} results but exactly one is required",
                            operation.results.len()
                        ),
                    );
                    return;
                }
                let Type::KernelContext(context) = &operation.results[0].ty else {
                    self.emit(
                        location,
                        DiagnosticCode::TypeMismatch,
                        "KernelContextIssue result must have KernelContext type",
                    );
                    return;
                };
                if !context.is_complete() {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidKernelContext,
                        "issued kernel-context brand is incomplete",
                    );
                }
                if context.root() != &self.function.id {
                    self.emit(
                        location,
                        DiagnosticCode::InvalidKernelContext,
                        format!(
                            "kernel-context root {} does not match issuing function {}",
                            context.root(),
                            self.function.id
                        ),
                    );
                }
            }
            OperationKind::GlobalCapabilityBind(bind) => {
                if self.function.role != FunctionRole::KernelEntry {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGlobalCapability,
                        "global capabilities may be bound only in a physical kernel root",
                    );
                }
                let [result] = operation.results.as_slice() else {
                    self.emit(
                        location,
                        DiagnosticCode::ResultArity,
                        "GlobalCapabilityBind must define exactly one result",
                    );
                    return;
                };
                let Type::GlobalCapability(capability) = &result.ty else {
                    self.emit(
                        location,
                        DiagnosticCode::TypeMismatch,
                        "GlobalCapabilityBind result must have global-capability type",
                    );
                    return;
                };
                if !capability.is_complete() {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGlobalCapability,
                        "global-capability type is incomplete",
                    );
                }
                self.expect_type(
                    bind.context,
                    &Type::KernelContext(capability.context().clone()),
                    location.clone(),
                );
                self.expect_type(
                    bind.physical,
                    &capability.physical_slice_type(),
                    location.clone(),
                );
                if !self.bound_physical_globals.insert(bind.physical) {
                    self.emit(
                        location,
                        DiagnosticCode::InvalidGlobalCapability,
                        "one physical global slice may be bound only once",
                    );
                }
            }
            OperationKind::GlobalCapabilityIndex(index) => {
                let Some(Type::GlobalCapability(capability)) = self.ty(index.capability) else {
                    self.emit(
                        location,
                        DiagnosticCode::InvalidGlobalCapability,
                        "global-capability index projection requires branded authority",
                    );
                    return;
                };
                let role = capability.role();
                self.expect_type(index.index, &Type::INDEX, location.clone());
                let expected = match role {
                    GlobalCapabilityRoleV1::ReadOnly
                    | GlobalCapabilityRoleV1::ExclusiveReadWrite => None,
                    GlobalCapabilityRoleV1::DisjointWrite(index_space) => Some(index_space),
                };
                if index.index_space != expected {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGlobalCapability,
                        "global-capability index projection substituted its access role or index space",
                    );
                }
                self.expect_results(operation, &[Type::INDEX], location);
            }
            OperationKind::ExecutionCapability(contract) => {
                self.verify_execution_capability(operation, contract, location)
            }
        }
    }

    fn verify_execution_capability(
        &mut self,
        operation: &Operation,
        contract: &ExecutionCapabilityOpV1,
        location: DiagnosticLocation,
    ) {
        let current_block = location.block;
        let current_operation = location.operation;
        if !contract.is_complete() || contract.provenance.root != self.function.id {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidExecutionCapability,
                "execution contract is incomplete or names the wrong canonical root",
            );
        }
        if !self
            .execution_source_locations
            .insert((contract.source.operation, contract.source.block))
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidExecutionCapability,
                "execution source operation identity/location was duplicated or replayed",
            );
        }
        if operation.results.len() > crate::MAX_EXECUTION_CAPABILITY_RESULTS_V1 {
            self.emit(
                location.clone(),
                DiagnosticCode::ResourceLimit,
                "execution operation exceeds its bounded result count",
            );
        }
        let declared = contract.operation.required_capabilities();
        for requirement in declared {
            if !self.function.required_capabilities.contains(&requirement)
                || !self.module.required_capabilities.contains(&requirement)
            {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidExecutionCapability,
                    format!("execution operation omits required declaration {requirement:?}"),
                );
            }
        }

        let expected_operands = execution_operand_contract(&contract.operation);
        if contract.operands.len() != expected_operands.len() {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidExecutionCapability,
                format!(
                    "execution operation has {} SSA operands, expected {} in its closed V13 order",
                    contract.operands.len(),
                    expected_operands.len()
                ),
            );
        }
        for (index, (operand, expected)) in
            contract.operands.iter().zip(&expected_operands).enumerate()
        {
            let actual = self.ty(*operand);
            if !execution_operand_type_matches(actual, *expected, contract) {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidExecutionCapability,
                    format!(
                        "SSA operand {index} ({operand}) does not match its exact V13 type/role"
                    ),
                );
            }
        }
        if let crate::ExecutionCapabilityOperationV1::RawMemoryBind { extent, .. } =
            &contract.operation
            && !self.valid_dynamic_extent(contract, *extent)
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidExecutionCapability,
                "dynamic extent ordinal, semantic type, transport type, or upper-bound check is invalid",
            );
        }
        if matches!(
            contract.operation,
            crate::ExecutionCapabilityOperationV1::Atomic { .. }
        ) && !self.valid_atomic_address_role(contract)
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidExecutionCapability,
                "atomic address space does not match its location or memory-view authority",
            );
        }

        let expected_results = execution_result_contract(&contract.operation, contract, |index| {
            contract
                .operands
                .get(index)
                .and_then(|operand| self.ty(*operand))
                .cloned()
        });
        if operation.results.len() != expected_results.len() {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidExecutionCapability,
                format!(
                    "execution operation has {} SSA results, expected {} in its closed V13 order",
                    operation.results.len(),
                    expected_results.len()
                ),
            );
        }
        for (index, (result, expected)) in
            operation.results.iter().zip(&expected_results).enumerate()
        {
            if !execution_result_type_matches(&result.ty, *expected, contract) {
                self.emit(
                    location.clone(),
                    DiagnosticCode::InvalidExecutionCapability,
                    format!(
                        "SSA result {index} ({}) substitutes its exact V13 type, provenance, epoch, or role",
                        result.id
                    ),
                );
            }
        }

        let Some(body) = &self.function.body else {
            return;
        };
        for block in &body.blocks {
            for (operation_index, candidate) in block.operations.iter().enumerate() {
                let OperationKind::ExecutionCapability(transition) = &candidate.kind else {
                    continue;
                };
                let (Some(brand), Some(before), Some(_)) = (
                    transition.workgroup_brand,
                    transition.epoch_before,
                    transition.epoch_after,
                ) else {
                    continue;
                };
                if contract.workgroup_brand == Some(brand)
                    && contract.epoch_before == Some(before)
                    && transition.provenance == contract.provenance
                    && current_block.zip(current_operation).is_some_and(
                        |(current_block, current_operation)| {
                            self.site_dominates(
                                DefSite::Operation(block.id, operation_index),
                                current_block,
                                Some(current_operation),
                            )
                        },
                    )
                {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidExecutionCapability,
                        "execution operation consumes an epoch after a dominating transition",
                    );
                    break;
                }
            }
        }
    }

    fn valid_dynamic_extent(
        &self,
        contract: &ExecutionCapabilityOpV1,
        extent: crate::ExecutionDynamicExtentV1,
    ) -> bool {
        let expected_nonnegative = matches!(
            extent.value_type,
            ScalarType::I8 | ScalarType::I16 | ScalarType::I32 | ScalarType::I64
        )
        .then_some(4);
        if extent.source_argument != 2
            || extent.operand != 2
            || extent.bound_check_operand != 3
            || extent.nonnegative_check_operand != expected_nonnegative
        {
            return false;
        }
        let Some(value) = contract.operands.get(usize::from(extent.operand)).copied() else {
            return false;
        };
        let Some(bound_check) = contract
            .operands
            .get(usize::from(extent.bound_check_operand))
            .copied()
        else {
            return false;
        };
        let nonnegative_check = extent
            .nonnegative_check_operand
            .and_then(|operand| contract.operands.get(usize::from(operand)).copied());
        if contract
            .signature
            .arguments()
            .nth(usize::from(extent.source_argument))
            != Some(extent.source_type)
            || self.ty(value) != Some(&Type::Scalar(extent.value_type))
            || self.ty(bound_check) != Some(&Type::BOOL)
            || extent
                .nonnegative_check_operand
                .is_some_and(|_| nonnegative_check.is_none())
        {
            return false;
        }
        let Some(OperationKind::Compare {
            predicate: crate::ComparePredicate::LessThanOrEqual,
            lhs,
            rhs,
        }) = self
            .defining_operation(bound_check)
            .map(|operation| &operation.kind)
        else {
            return false;
        };
        if *lhs != value
            || self.constant_extent_bound(*rhs, extent.value_type) != Some(extent.upper_bound)
        {
            return false;
        }
        match (extent.value_type, nonnegative_check) {
            (
                ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
                | ScalarType::Index,
                None,
            ) => true,
            (
                ScalarType::I8 | ScalarType::I16 | ScalarType::I32 | ScalarType::I64,
                Some(nonnegative_check),
            ) if self.ty(nonnegative_check) == Some(&Type::BOOL) => matches!(
                self.defining_operation(nonnegative_check)
                    .map(|operation| &operation.kind),
                Some(OperationKind::Compare {
                    predicate: crate::ComparePredicate::GreaterThanOrEqual,
                    lhs,
                    rhs,
                }) if *lhs == value && self.constant_zero(*rhs, extent.value_type)
            ),
            _ => false,
        }
    }

    fn valid_atomic_address_role(&self, contract: &ExecutionCapabilityOpV1) -> bool {
        use crate::{
            ExecutionAtomicKindV1 as Atomic, ExecutionCapabilityOperationV1 as Op,
            ExecutionCapabilityRoleV1 as Role, ExecutionMemoryAccessV1 as Access,
        };
        let Op::Atomic {
            kind,
            element,
            address_space,
            scope,
            ..
        } = &contract.operation
        else {
            return false;
        };
        if *kind == Atomic::BindGlobalView {
            return *address_space == crate::ExecutionMemoryAddressSpaceV1::Global;
        }
        let Some(Type::ExecutionCapability(location)) = contract
            .operands
            .get(1)
            .and_then(|operand| self.ty(*operand))
        else {
            return false;
        };
        match (kind, &location.role) {
            (
                Atomic::BindGlobalLocation,
                Role::MemoryView {
                    element: role_element,
                    space,
                    access: Access::AtomicReadWrite,
                    atomic_scope: Some(role_scope),
                    ..
                },
            ) => {
                *address_space == crate::ExecutionMemoryAddressSpaceV1::Global
                    && space == address_space
                    && role_element == element
                    && role_scope == scope
            }
            (
                Atomic::Load | Atomic::Store | Atomic::FetchAdd | Atomic::CompareExchange,
                Role::ScopedAtomic {
                    element: role_element,
                    space,
                    scope: role_scope,
                },
            ) => role_element == element && space == address_space && role_scope == scope,
            _ => false,
        }
    }

    fn constant_extent_bound(&self, value: ValueId, ty: ScalarType) -> Option<u64> {
        let operation = self.defining_operation(value)?;
        match (&operation.kind, ty) {
            (OperationKind::Constant(Constant::U8(value)), ScalarType::U8) => {
                Some(u64::from(*value))
            }
            (OperationKind::Constant(Constant::U16(value)), ScalarType::U16) => {
                Some(u64::from(*value))
            }
            (OperationKind::Constant(Constant::U32(value)), ScalarType::U32) => {
                Some(u64::from(*value))
            }
            (OperationKind::Constant(Constant::U64(value)), ScalarType::U64)
            | (OperationKind::Constant(Constant::Index(value)), ScalarType::Index) => Some(*value),
            (OperationKind::Constant(Constant::I8(value)), ScalarType::I8) => {
                u64::try_from(*value).ok()
            }
            (OperationKind::Constant(Constant::I16(value)), ScalarType::I16) => {
                u64::try_from(*value).ok()
            }
            (OperationKind::Constant(Constant::I32(value)), ScalarType::I32) => {
                u64::try_from(*value).ok()
            }
            (OperationKind::Constant(Constant::I64(value)), ScalarType::I64) => {
                u64::try_from(*value).ok()
            }
            _ => None,
        }
    }

    fn constant_zero(&self, value: ValueId, ty: ScalarType) -> bool {
        self.constant_extent_bound(value, ty) == Some(0)
    }

    fn verify_matrix_lds_allocation(
        &mut self,
        matrix: &MatrixOperation,
        location: DiagnosticLocation,
    ) {
        let (base, profile) = match &matrix.kind {
            MatrixOperationKind::MultiplyAccumulate { .. }
            | MatrixOperationKind::ScaledMultiplyAccumulate { .. } => return,
            MatrixOperationKind::LdsLoad { base, profile }
            | MatrixOperationKind::LdsStore { base, profile, .. } => (*base, *profile),
        };

        let Some(definition) = self.definitions.get(&base) else {
            return;
        };
        let DefSite::Operation(block_id, operation_index) = definition.site else {
            self.emit(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "matrix LDS base must be the direct result of an authenticated workgroup-memory allocation",
            );
            return;
        };
        let Some(allocation) = self
            .blocks
            .get(&block_id)
            .and_then(|block| block.operations.get(operation_index))
            .and_then(|operation| match &operation.kind {
                OperationKind::WorkgroupMemory(memory) => Some(memory),
                _ => None,
            })
        else {
            self.emit(
                location,
                DiagnosticCode::InvalidMemoryAccess,
                "matrix LDS base must be the direct result of an authenticated workgroup-memory allocation",
            );
            return;
        };

        let extent = allocation.extent;
        let alignment = allocation.alignment;
        let required_elements = profile.required_elements();
        match extent.guaranteed_elements() {
            Some(elements) if elements >= required_elements => {}
            Some(elements) => self.emit(
                location.clone(),
                DiagnosticCode::InvalidMemoryAccess,
                format!(
                    "matrix LDS allocation guarantees {elements} elements but requires at least {required_elements}"
                ),
            ),
            None => self.emit(
                location.clone(),
                DiagnosticCode::InvalidMemoryAccess,
                "matrix LDS operation requires a statically authenticated allocation extent",
            ),
        }

        let required_alignment = profile.required_alignment();
        if alignment < required_alignment {
            self.emit(
                location,
                DiagnosticCode::InvalidAlignment,
                format!(
                    "matrix LDS allocation alignment {alignment} is below the required {required_alignment}"
                ),
            );
        }
    }

    fn verify_gfx950_lds_transpose(
        &mut self,
        operation: &Operation,
        transpose: &Gfx950LdsTransposeOperationV1,
        location: DiagnosticLocation,
    ) {
        let storage_ty = Type::pointer(
            Type::Scalar(ScalarType::U8),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        );
        if transpose.width != crate::WaveWidth::Wave64 || transpose.active_lanes != 64 {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidGfx950LdsTranspose,
                "gfx950 LDS transpose requires one fully active Wave64",
            );
        }
        if transpose.convergence.scope() != SynchronizationScope::Workgroup {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidConvergence,
                "gfx950 LDS transpose requires uniform workgroup convergence",
            );
        }

        match transpose.kind {
            Gfx950LdsTransposeOperationKindV1::Current { format } => {
                self.expect_results(
                    operation,
                    std::slice::from_ref(&storage_ty),
                    location.clone(),
                );
                let wave_private_entry = self.function.role == FunctionRole::KernelEntry
                    && self.module.kernels.iter().any(|kernel| {
                        kernel.entry == self.function.id
                            && kernel.workgroup_size.is_some_and(|workgroup| {
                                workgroup.y == 1
                                    && workgroup.z == 1
                                    && (64..=256).contains(&workgroup.x)
                                    && workgroup.x.is_multiple_of(64)
                            })
                    });
                if !wave_private_entry {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "gfx950 LDS transpose storage requires a kernel entry with one-dimensional workgroup size [64, 1, 1] through [256, 1, 1] in exact Wave64 multiples",
                    );
                }
                if !self.gfx950_lds_transpose_currents.insert(format) {
                    self.emit(
                        location,
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "one kernel entry may declare at most one gfx950 LDS transpose tile per format",
                    );
                }
            }
            Gfx950LdsTransposeOperationKindV1::Stage {
                format,
                storage,
                source_slice,
                offset,
                rows,
                columns,
                stride,
                token_base,
                reduction_base,
            } => {
                self.expect_results(
                    operation,
                    std::slice::from_ref(&storage_ty),
                    location.clone(),
                );
                self.expect_type(storage, &storage_ty, location.clone());
                let valid_source = matches!(
                    self.ty(source_slice),
                    Some(Type::Slice(slice))
                        if *slice.element == Type::Scalar(ScalarType::U8)
                            && slice.address_space == AddressSpace::Global
                            && slice.access == AccessMode::ReadOnly
                );
                if !valid_source {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidOperandType,
                        "gfx950 LDS transpose stage source must be an exact global read-only u8 slice",
                    );
                }
                for value in [offset, rows, columns, stride, token_base, reduction_base] {
                    self.expect_type(value, &Type::INDEX, location.clone());
                }
                if !matches!(
                    self.gfx950_lds_transpose_producer(storage),
                    Some(Gfx950LdsTransposeOperationKindV1::Current { format: producer })
                        if producer == format
                ) {
                    self.emit(
                        location,
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "gfx950 LDS transpose stage must directly consume the matching Current token",
                    );
                }
            }
            Gfx950LdsTransposeOperationKindV1::Publish { format, storage } => {
                self.expect_results(
                    operation,
                    std::slice::from_ref(&storage_ty),
                    location.clone(),
                );
                self.expect_type(storage, &storage_ty, location.clone());
                if !matches!(
                    self.gfx950_lds_transpose_producer(storage),
                    Some(Gfx950LdsTransposeOperationKindV1::Stage { format: producer, .. })
                        if producer == format
                ) {
                    self.emit(
                        location,
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "gfx950 LDS transpose publish must directly consume the matching staged token",
                    );
                }
            }
            Gfx950LdsTransposeOperationKindV1::Read { format, storage } => {
                self.expect_results(
                    operation,
                    &vec![Type::Scalar(ScalarType::U32); 8],
                    location.clone(),
                );
                self.expect_type(storage, &storage_ty, location.clone());
                if !matches!(
                    self.gfx950_lds_transpose_producer(storage),
                    Some(Gfx950LdsTransposeOperationKindV1::Publish { format: producer, .. })
                        if producer == format
                ) {
                    self.emit(
                        location,
                        DiagnosticCode::InvalidGfx950LdsTranspose,
                        "gfx950 LDS transpose read must directly consume the matching dominating Publish token",
                    );
                }
            }
        }
    }

    fn gfx950_lds_transpose_producer(
        &self,
        value: ValueId,
    ) -> Option<Gfx950LdsTransposeOperationKindV1> {
        let definition = self.definitions.get(&value)?;
        let DefSite::Operation(block, operation_index) = definition.site else {
            return None;
        };
        let block = self.blocks.get(&block)?;
        let operation = block.operations.get(operation_index)?;
        let OperationKind::Gfx950LdsTranspose(transpose) = &operation.kind else {
            return None;
        };
        operation
            .results
            .iter()
            .any(|result| result.id == value)
            .then_some(transpose.kind)
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
            WaveOperationKind::ReduceF32 {
                value, tile_width, ..
            } => {
                self.expect_type(value, &Type::Scalar(ScalarType::F32), location.clone());
                self.verify_wave_tile_width(wave, tile_width, location.clone());
                self.expect_results(operation, &[Type::Scalar(ScalarType::F32)], location);
            }
            WaveOperationKind::BroadcastF32 {
                value,
                source_lane,
                tile_width,
            } => {
                self.expect_type(value, &Type::Scalar(ScalarType::F32), location.clone());
                self.expect_type(
                    source_lane,
                    &Type::Scalar(ScalarType::U32),
                    location.clone(),
                );
                self.verify_wave_tile_width(wave, tile_width, location.clone());
                if !self.bounded_u32_source_lane(source_lane, tile_width) {
                    self.emit(
                        location.clone(),
                        DiagnosticCode::InvalidWaveOperation,
                        "wave f32 broadcast requires a statically bounded tile-local source lane",
                    );
                }
                self.expect_results(operation, &[Type::Scalar(ScalarType::F32)], location);
            }
        }
    }

    fn verify_wave_tile_width(
        &mut self,
        wave: &WaveOperation,
        tile_width: u32,
        location: DiagnosticLocation,
    ) {
        if tile_width == 0 || !tile_width.is_power_of_two() || tile_width > wave.width.lanes() {
            self.emit(
                location,
                DiagnosticCode::InvalidWaveOperation,
                format!(
                    "wave tile width {tile_width} must be a non-zero power of two no larger than {}",
                    wave.width.lanes()
                ),
            );
        }
    }

    fn constant_u32(&self, value: ValueId) -> Option<u32> {
        let operation = self.defining_operation(value)?;
        match operation.kind {
            OperationKind::Constant(Constant::U32(value)) => Some(value),
            _ => None,
        }
    }

    fn bounded_u32_source_lane(&self, value: ValueId, width: u32) -> bool {
        if width == 0 || !width.is_power_of_two() || width > 64 {
            return false;
        }
        if self.constant_u32(value).is_some_and(|lane| lane < width) {
            return true;
        }
        let Some(OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs,
            rhs,
        }) = self
            .defining_operation(value)
            .map(|operation| &operation.kind)
        else {
            return false;
        };
        self.constant_u32(*lhs).is_some_and(|mask| mask < width)
            || self.constant_u32(*rhs).is_some_and(|mask| mask < width)
    }

    fn defining_operation(&self, value: ValueId) -> Option<&Operation> {
        let definition = self.definitions.get(&value)?;
        let DefSite::Operation(block, operation_index) = definition.site else {
            return None;
        };
        self.blocks.get(&block)?.operations.get(operation_index)
    }

    fn verify_binary(
        &mut self,
        operation: &Operation,
        op: BinaryOp,
        lhs: ValueId,
        rhs: ValueId,
        location: DiagnosticLocation,
    ) {
        if let BinaryOp::Checked(operator) = op {
            self.verify_checked_binary(operation, operator, lhs, rhs, location);
            return;
        }
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

    fn verify_checked_binary(
        &mut self,
        operation: &Operation,
        operator: CheckedBinaryOperator,
        lhs: ValueId,
        rhs: ValueId,
        location: DiagnosticLocation,
    ) {
        let (Some(lhs_ty), Some(rhs_ty)) = (self.ty(lhs).cloned(), self.ty(rhs).cloned()) else {
            return;
        };
        let valid = lhs_ty == rhs_ty && lhs_ty.as_scalar().is_some_and(ScalarType::is_integer);
        if !valid {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidOperandType,
                format!(
                    "checked {:?} does not accept {lhs_ty:?} and {rhs_ty:?}",
                    operator
                ),
            );
        }
        self.expect_results(operation, &[lhs_ty, Type::BOOL], location);
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
            && (!matches!(
                pointer_ty.access,
                AccessMode::WriteOnly | AccessMode::ReadWrite
            ) || pointer_ty.address_space == AddressSpace::Constant)
        {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidMemoryAccess,
                "write requires a writable pointer outside constant memory",
            );
        }
        if !write && pointer_ty.access == AccessMode::WriteOnly {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidMemoryAccess,
                "read requires a readable pointer",
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
        if matches!(
            memory.extent,
            WorkgroupMemoryExtent::Static(0) | WorkgroupMemoryExtent::DynamicAtLeast(0)
        ) {
            self.emit(
                location.clone(),
                DiagnosticCode::InvalidWorkgroupMemory,
                "authenticated workgroup memory extent must be non-zero",
            );
        }
        if memory.extent.is_dynamic() {
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

    fn verify_guarded_global_capability_access(
        &self,
        pointer: ValueId,
        predicate: ValueId,
        write: bool,
    ) -> Option<Result<(), &'static str>> {
        let OperationKind::GetElementPointer { base, offset } =
            &self.defining_operation(pointer)?.kind
        else {
            return None;
        };
        let OperationKind::SliceData { slice: capability } = self.defining_operation(*base)?.kind
        else {
            return None;
        };
        let Some(Type::GlobalCapability(capability_ty)) = self.ty(capability) else {
            return None;
        };
        let role_matches = matches!(
            (write, capability_ty.role()),
            (false, GlobalCapabilityRoleV1::ReadOnly)
                | (false, GlobalCapabilityRoleV1::ExclusiveReadWrite)
                | (true, GlobalCapabilityRoleV1::ExclusiveReadWrite)
                | (true, GlobalCapabilityRoleV1::DisjointWrite(_))
        );
        if !role_matches {
            return Some(Err(
                "guarded global access does not match the capability initialization/access role",
            ));
        }
        let OperationKind::Select {
            condition,
            true_value,
            false_value,
        } = self.defining_operation(*offset)?.kind
        else {
            return Some(Err(
                "guarded global access must use a predicate-selected safe index",
            ));
        };
        if condition != predicate {
            return Some(Err(
                "guarded global access predicate differs from its safe-index predicate",
            ));
        }
        if !matches!(
            self.defining_operation(false_value)
                .map(|operation| &operation.kind),
            Some(OperationKind::Constant(Constant::Index(0)))
        ) {
            return Some(Err(
                "guarded global access must select index zero on the inactive path",
            ));
        }
        let Some(projected) = self.defining_operation(true_value) else {
            return Some(Err(
                "guarded global access index lacks a capability projection",
            ));
        };
        let OperationKind::GlobalCapabilityIndex(projected) = projected.kind else {
            return Some(Err(
                "guarded global access index lacks a capability projection",
            ));
        };
        if projected.capability != capability {
            return Some(Err(
                "guarded global access substituted a different capability at index projection",
            ));
        }
        if !self.guarded_global_bounds_match(predicate, true_value, capability) {
            return Some(Err(
                "guarded global access predicate lacks the exact projected-index slice bound",
            ));
        }
        Some(Ok(()))
    }

    fn guarded_global_bounds_match(
        &self,
        predicate: ValueId,
        projected: ValueId,
        capability: ValueId,
    ) -> bool {
        let Some(operation) = self.defining_operation(predicate) else {
            return false;
        };
        match operation.kind {
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs,
                rhs,
            } => {
                lhs == projected
                    && matches!(
                        self.defining_operation(rhs).map(|operation| &operation.kind),
                        Some(OperationKind::SliceLength { slice }) if *slice == capability
                    )
            }
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs,
                rhs,
            } => {
                self.guarded_global_bounds_match(lhs, projected, capability)
                    || self.guarded_global_bounds_match(rhs, projected, capability)
            }
            _ => false,
        }
    }

    fn ty(&self, value: ValueId) -> Option<&Type> {
        self.definitions
            .get(&value)
            .map(|definition| &definition.ty)
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

fn valid_scalar_cast(kind: CastKind, from: ScalarType, to: ScalarType) -> bool {
    if (from.is_integer() || from == ScalarType::Bool) && to.is_integer() {
        return crate::plan_integer_cast_v1(from, to) == Some([Some((kind, to)), None]);
    }
    if from == ScalarType::Index || to == ScalarType::Index {
        return false;
    }

    let Some(from_width) = from.bit_width() else {
        return false;
    };
    let Some(to_width) = to.bit_width() else {
        return false;
    };

    match kind {
        CastKind::RestrictPointerAccess => false,
        CastKind::Truncate => from.is_integer() && to.is_integer() && from_width > to_width,
        CastKind::ZeroExtend => {
            (from == ScalarType::Bool || (from.is_integer() && !from.is_signed_integer()))
                && to.is_integer()
                && from_width < to_width
        }
        CastKind::SignExtend => {
            from.is_signed_integer() && to.is_integer() && from_width < to_width
        }
        CastKind::FloatExtend => from.is_float() && to.is_float() && from_width < to_width,
        CastKind::FloatTruncate => from.is_float() && to.is_float() && from_width > to_width,
        CastKind::IntegerToFloat => from.is_integer() && to.is_float(),
        CastKind::FloatToInteger => from.is_float() && to.is_integer(),
        CastKind::Bitcast => {
            from.is_numeric() && to.is_numeric() && from != to && from_width == to_width
        }
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

fn operation_requires_kernel_context(operation: &Operation) -> bool {
    matches!(
        operation.kind,
        OperationKind::Intrinsic(_) | OperationKind::GlobalCapabilityBind(_)
    ) || !operation.required_capabilities().is_empty()
}

#[derive(Clone, Copy)]
enum ExecutionOperandContractV1 {
    KernelContext,
    Capability {
        source: [Option<crate::ExecutionTypeIdentityV1>; 2],
        role: ExecutionRoleContractV1,
    },
    Scalar(ScalarType),
    ElementScalar(crate::ExecutionElementLayoutV1),
    Pointer {
        layout: crate::ExecutionElementLayoutV1,
        space: AddressSpace,
        access: AccessMode,
    },
    Slice {
        scalar: ScalarType,
        space: AddressSpace,
        access: AccessMode,
    },
    BoolProof,
}

#[derive(Clone, Copy)]
enum ExecutionResultContractV1 {
    Capability {
        source: crate::ExecutionTypeIdentityV1,
        role: ExecutionRoleContractV1,
    },
    Scalar(ScalarType),
    ElementScalar(crate::ExecutionElementLayoutV1),
    Bool,
}

#[derive(Clone, Copy)]
enum ExecutionRoleContractV1 {
    Workgroup,
    Subgroup {
        width: u32,
    },
    Lds {
        element: crate::ExecutionTypeIdentityV1,
        layout: crate::ExecutionElementLayoutV1,
        elements: u64,
        state: crate::ExecutionLdsStateV1,
    },
    ScopedAtomic {
        element: crate::ExecutionTypeIdentityV1,
        space: crate::ExecutionMemoryAddressSpaceV1,
        scope: crate::ExecutionMemoryScopeV1,
    },
    Matrix {
        subgroup_brand: [u8; 32],
        width: u32,
    },
    PendingAsyncCopy {
        element: crate::ExecutionTypeIdentityV1,
        layout: crate::ExecutionElementLayoutV1,
        elements: u64,
    },
    MemoryView {
        element: crate::ExecutionTypeIdentityV1,
        layout: crate::ExecutionElementLayoutV1,
        space: crate::ExecutionMemoryAddressSpaceV1,
        access: crate::ExecutionMemoryAccessV1,
        extent: Option<crate::ExecutionMemoryExtentV1>,
        initialization: Option<crate::ExecutionMemoryInitializationV1>,
        index_space: Option<crate::ExecutionTypeIdentityV1>,
        requires_index_space: bool,
        atomic_scope: Option<crate::ExecutionMemoryScopeV1>,
    },
    WorkgroupMemoryIndex,
}

const fn execution_source(
    source: crate::ExecutionTypeIdentityV1,
) -> [Option<crate::ExecutionTypeIdentityV1>; 2] {
    [Some(source), None]
}

const fn execution_sources(
    first: crate::ExecutionTypeIdentityV1,
    second: crate::ExecutionTypeIdentityV1,
) -> [Option<crate::ExecutionTypeIdentityV1>; 2] {
    [Some(first), Some(second)]
}

fn execution_operand_contract(
    operation: &crate::ExecutionCapabilityOperationV1,
) -> Vec<ExecutionOperandContractV1> {
    use crate::{
        ExecutionAtomicKindV1 as Atomic, ExecutionCapabilityOperationV1 as Op,
        ExecutionLdsStateV1 as Lds, ExecutionMemoryAccessV1 as Access,
        ExecutionMemoryAddressSpaceV1 as Space, ExecutionMemoryInitializationV1 as Initialization,
    };

    let capability = |source, role| ExecutionOperandContractV1::Capability { source, role };
    let workgroup =
        |source| capability(execution_source(source), ExecutionRoleContractV1::Workgroup);
    let lds = |source, element, layout, elements, state| {
        capability(
            source,
            ExecutionRoleContractV1::Lds {
                element,
                layout,
                elements,
                state,
            },
        )
    };
    let memory_view = |source,
                       element,
                       layout,
                       space,
                       access,
                       initialization,
                       index_space,
                       requires_index_space,
                       atomic_scope| {
        capability(
            source,
            ExecutionRoleContractV1::MemoryView {
                element,
                layout,
                space,
                access,
                extent: None,
                initialization,
                index_space,
                requires_index_space,
                atomic_scope,
            },
        )
    };

    match operation {
        Op::WorkgroupDerive { .. } => vec![ExecutionOperandContractV1::KernelContext],
        Op::SubgroupDerive {
            workgroup: source, ..
        }
        | Op::LdsAllocate {
            workgroup: source, ..
        }
        | Op::WorkgroupBarrier {
            input_workgroup: source,
            ..
        }
        | Op::WorkgroupFence {
            workgroup: source, ..
        }
        | Op::WorkgroupMemoryIndex {
            workgroup: source, ..
        }
        | Op::WorkgroupMemoryAllocate {
            workgroup: source, ..
        } => vec![workgroup(*source)],
        Op::LdsInitializeByInvocation {
            input_lds,
            workgroup: workgroup_source,
            element,
            layout,
            elements,
            ..
        } => vec![
            lds(
                execution_source(*input_lds),
                *element,
                *layout,
                *elements,
                Lds::Uninitialized,
            ),
            workgroup(*workgroup_source),
            ExecutionOperandContractV1::ElementScalar(*layout),
        ],
        Op::LdsPublish {
            input_workgroup,
            input_lds,
            element,
            layout,
            elements,
            ..
        } => vec![
            workgroup(*input_workgroup),
            lds(
                execution_source(*input_lds),
                *element,
                *layout,
                *elements,
                Lds::InvocationInitialized,
            ),
        ],
        Op::LdsReadPublished {
            lds_reference,
            lds: lds_type,
            workgroup: workgroup_source,
            element,
            layout,
            elements,
            ..
        } => vec![
            lds(
                execution_sources(*lds_reference, *lds_type),
                *element,
                *layout,
                *elements,
                Lds::Published,
            ),
            workgroup(*workgroup_source),
            ExecutionOperandContractV1::Scalar(ScalarType::Index),
        ],
        Op::SubgroupBarrier {
            input_workgroup,
            subgroup,
            width,
            ..
        } => vec![
            workgroup(*input_workgroup),
            capability(
                execution_source(*subgroup),
                ExecutionRoleContractV1::Subgroup { width: *width },
            ),
        ],
        Op::SubgroupFence {
            subgroup_reference,
            subgroup,
            width,
            ..
        } => vec![capability(
            execution_sources(*subgroup_reference, *subgroup),
            ExecutionRoleContractV1::Subgroup { width: *width },
        )],
        Op::Atomic {
            kind,
            authority,
            location_input,
            location,
            element,
            operand: _,
            replacement: _,
            value_type,
            scope,
            ..
        } => match kind {
            Atomic::BindGlobalView => vec![
                ExecutionOperandContractV1::KernelContext,
                ExecutionOperandContractV1::Slice {
                    scalar: *value_type,
                    space: AddressSpace::Global,
                    access: AccessMode::ReadOnly,
                },
            ],
            Atomic::BindGlobalLocation => vec![
                workgroup(*authority),
                memory_view(
                    execution_source(*location_input),
                    *element,
                    execution_scalar_layout(*value_type),
                    Space::Global,
                    Access::AtomicReadWrite,
                    Some(Initialization::FullyInitialized),
                    None,
                    false,
                    Some(*scope),
                ),
                ExecutionOperandContractV1::Scalar(ScalarType::Index),
            ],
            Atomic::Load => vec![
                workgroup(*authority),
                capability(
                    execution_sources(*location_input, *location),
                    ExecutionRoleContractV1::ScopedAtomic {
                        element: *element,
                        space: Space::Global,
                        scope: *scope,
                    },
                ),
            ],
            Atomic::Store | Atomic::FetchAdd => vec![
                workgroup(*authority),
                capability(
                    execution_sources(*location_input, *location),
                    ExecutionRoleContractV1::ScopedAtomic {
                        element: *element,
                        space: Space::Global,
                        scope: *scope,
                    },
                ),
                ExecutionOperandContractV1::Scalar(*value_type),
            ],
            Atomic::CompareExchange => vec![
                workgroup(*authority),
                capability(
                    execution_sources(*location_input, *location),
                    ExecutionRoleContractV1::ScopedAtomic {
                        element: *element,
                        space: Space::Global,
                        scope: *scope,
                    },
                ),
                ExecutionOperandContractV1::Scalar(*value_type),
                ExecutionOperandContractV1::Scalar(*value_type),
            ],
        },
        Op::WorkgroupCollective {
            kind,
            input_workgroup,
            scratch,
            element,
            value_type,
            layout,
            elements,
            ..
        } => {
            match kind {
                crate::ExecutionCollectiveKindV1::ReduceSum
                | crate::ExecutionCollectiveKindV1::InclusiveScanSum
                | crate::ExecutionCollectiveKindV1::ExclusiveScanSum => {}
            }
            vec![
                workgroup(*input_workgroup),
                lds(
                    execution_source(*scratch),
                    *element,
                    *layout,
                    *elements,
                    Lds::Uninitialized,
                ),
                ExecutionOperandContractV1::Scalar(*value_type),
            ]
        }
        Op::SubgroupCollective {
            kind,
            subgroup_reference,
            subgroup,
            element: _,
            value_type,
            width,
            ..
        } => {
            match kind {
                crate::ExecutionCollectiveKindV1::ReduceSum
                | crate::ExecutionCollectiveKindV1::InclusiveScanSum
                | crate::ExecutionCollectiveKindV1::ExclusiveScanSum => {}
            }
            vec![
                capability(
                    execution_sources(*subgroup_reference, *subgroup),
                    ExecutionRoleContractV1::Subgroup { width: *width },
                ),
                ExecutionOperandContractV1::Scalar(*value_type),
            ]
        }
        Op::MatrixAccess {
            subgroup, width, ..
        } => vec![capability(
            execution_source(*subgroup),
            ExecutionRoleContractV1::Subgroup { width: *width },
        )],
        Op::AsyncCopy {
            workgroup: workgroup_source,
            source_reference,
            source,
            destination,
            element,
            layout,
            elements,
            ..
        } => vec![
            workgroup(*workgroup_source),
            memory_view(
                execution_sources(*source_reference, *source),
                *element,
                *layout,
                Space::Global,
                Access::ReadOnly,
                Some(Initialization::FullyInitialized),
                None,
                false,
                None,
            ),
            ExecutionOperandContractV1::Scalar(ScalarType::Index),
            lds(
                execution_source(*destination),
                *element,
                *layout,
                *elements,
                Lds::Uninitialized,
            ),
        ],
        Op::AsyncWait {
            input_workgroup,
            pending,
            element,
            layout,
            elements,
            ..
        } => vec![
            workgroup(*input_workgroup),
            capability(
                execution_source(*pending),
                ExecutionRoleContractV1::PendingAsyncCopy {
                    element: *element,
                    layout: *layout,
                    elements: *elements,
                },
            ),
        ],
        Op::RawMemoryBind {
            authority,
            extent,
            layout,
            space,
            ..
        } => {
            let authority = if matches!(space, Space::Private) {
                ExecutionOperandContractV1::KernelContext
            } else {
                workgroup(*authority)
            };
            let mut operands = vec![
                authority,
                ExecutionOperandContractV1::Pointer {
                    layout: *layout,
                    space: space.address_space(),
                    access: AccessMode::ReadWrite,
                },
                ExecutionOperandContractV1::Scalar(extent.value_type),
                ExecutionOperandContractV1::BoolProof,
            ];
            if extent.nonnegative_check_operand.is_some() {
                operands.push(ExecutionOperandContractV1::BoolProof);
            }
            operands
        }
        Op::PrivateMemoryAllocate { .. } => vec![ExecutionOperandContractV1::KernelContext],
        Op::WorkgroupMemoryPublish {
            input_workgroup,
            input_view,
            element,
            layout,
            ..
        } => vec![
            workgroup(*input_workgroup),
            memory_view(
                execution_source(*input_view),
                *element,
                *layout,
                Space::Workgroup,
                Access::DisjointWrite,
                Some(Initialization::Uninitialized),
                None,
                true,
                None,
            ),
        ],
        Op::MemoryLoad {
            view,
            workgroup: workgroup_source,
            element,
            layout,
            space,
            access,
            ..
        } => {
            let mut operands = vec![memory_view(
                execution_source(*view),
                *element,
                *layout,
                *space,
                *access,
                None,
                None,
                false,
                None,
            )];
            if let Some(source) = workgroup_source {
                operands.push(workgroup(*source));
            }
            operands.push(ExecutionOperandContractV1::Scalar(ScalarType::Index));
            operands
        }
        Op::MemoryStore {
            view,
            workgroup: workgroup_source,
            index,
            element,
            layout,
            space,
            access,
            ..
        } => {
            let disjoint = matches!(access, Access::DisjointWrite);
            let mut operands = vec![memory_view(
                execution_source(*view),
                *element,
                *layout,
                *space,
                *access,
                None,
                None,
                disjoint,
                None,
            )];
            if let Some(source) = workgroup_source {
                operands.push(workgroup(*source));
            }
            if disjoint {
                operands.push(capability(
                    execution_source(*index),
                    ExecutionRoleContractV1::WorkgroupMemoryIndex,
                ));
            } else {
                operands.push(ExecutionOperandContractV1::Scalar(ScalarType::Index));
            }
            operands.push(ExecutionOperandContractV1::ElementScalar(*layout));
            operands
        }
    }
}

fn execution_result_contract(
    operation: &crate::ExecutionCapabilityOperationV1,
    _contract: &ExecutionCapabilityOpV1,
    operand_type: impl Fn(usize) -> Option<Type>,
) -> Vec<ExecutionResultContractV1> {
    use crate::{
        ExecutionAtomicKindV1 as Atomic, ExecutionCapabilityOperationV1 as Op,
        ExecutionLdsStateV1 as Lds, ExecutionMemoryAccessV1 as Access,
        ExecutionMemoryAddressSpaceV1 as Space, ExecutionMemoryExtentV1 as Extent,
        ExecutionMemoryInitializationV1 as Initialization,
    };
    let capability = |source, role| ExecutionResultContractV1::Capability { source, role };
    let workgroup = |source| capability(source, ExecutionRoleContractV1::Workgroup);
    let lds = |source, element, layout, elements, state| {
        capability(
            source,
            ExecutionRoleContractV1::Lds {
                element,
                layout,
                elements,
                state,
            },
        )
    };

    match operation {
        Op::WorkgroupDerive {
            workgroup: output, ..
        } => vec![workgroup(*output)],
        Op::SubgroupDerive {
            subgroup: output,
            width,
            ..
        } => vec![capability(
            *output,
            ExecutionRoleContractV1::Subgroup { width: *width },
        )],
        Op::LdsAllocate {
            lds: output_lds,
            element,
            layout,
            elements,
            ..
        } => vec![lds(
            *output_lds,
            *element,
            *layout,
            *elements,
            Lds::Uninitialized,
        )],
        Op::LdsInitializeByInvocation {
            output_lds,
            element,
            layout,
            elements,
            ..
        } => vec![lds(
            *output_lds,
            *element,
            *layout,
            *elements,
            Lds::InvocationInitialized,
        )],
        Op::LdsPublish {
            output_lds,
            transition,
            element,
            layout,
            elements,
            ..
        } => vec![
            workgroup(*transition),
            lds(*output_lds, *element, *layout, *elements, Lds::Published),
        ],
        Op::LdsReadPublished { layout, .. } | Op::MemoryLoad { layout, .. } => vec![
            ExecutionResultContractV1::ElementScalar(*layout),
            ExecutionResultContractV1::Bool,
        ],
        Op::WorkgroupBarrier {
            output_workgroup, ..
        } => vec![workgroup(*output_workgroup)],
        Op::SubgroupBarrier {
            transition, width, ..
        } => vec![
            workgroup(*transition),
            capability(
                *transition,
                ExecutionRoleContractV1::Subgroup { width: *width },
            ),
        ],
        Op::WorkgroupFence { .. } | Op::SubgroupFence { .. } => Vec::new(),
        Op::Atomic {
            kind,
            location,
            result,
            element,
            value_type,
            scope,
            ..
        } => match kind {
            Atomic::BindGlobalLocation => vec![
                capability(
                    *location,
                    ExecutionRoleContractV1::ScopedAtomic {
                        element: *element,
                        space: Space::Global,
                        scope: *scope,
                    },
                ),
                ExecutionResultContractV1::Bool,
            ],
            Atomic::BindGlobalView => vec![capability(
                *result,
                ExecutionRoleContractV1::MemoryView {
                    element: *element,
                    layout: execution_scalar_layout(*value_type),
                    space: Space::Global,
                    access: Access::AtomicReadWrite,
                    extent: None,
                    initialization: Some(Initialization::FullyInitialized),
                    index_space: None,
                    requires_index_space: false,
                    atomic_scope: Some(*scope),
                },
            )],
            Atomic::Load | Atomic::FetchAdd => {
                vec![ExecutionResultContractV1::Scalar(*value_type)]
            }
            Atomic::Store => Vec::new(),
            Atomic::CompareExchange => vec![
                ExecutionResultContractV1::Scalar(*value_type),
                ExecutionResultContractV1::Bool,
            ],
        },
        Op::WorkgroupCollective {
            transition,
            element,
            value_type,
            layout,
            elements,
            ..
        } => vec![
            workgroup(*transition),
            lds(
                *transition,
                *element,
                *layout,
                *elements,
                Lds::Uninitialized,
            ),
            ExecutionResultContractV1::Scalar(*value_type),
        ],
        Op::SubgroupCollective { value_type, .. } => {
            vec![ExecutionResultContractV1::Scalar(*value_type)]
        }
        Op::MatrixAccess {
            matrix,
            subgroup_brand,
            width,
            ..
        } => vec![capability(
            *matrix,
            ExecutionRoleContractV1::Matrix {
                subgroup_brand: *subgroup_brand,
                width: *width,
            },
        )],
        Op::AsyncCopy {
            pending,
            element,
            layout,
            elements,
            ..
        } => vec![capability(
            *pending,
            ExecutionRoleContractV1::PendingAsyncCopy {
                element: *element,
                layout: *layout,
                elements: *elements,
            },
        )],
        Op::AsyncWait {
            output_lds,
            transition,
            element,
            layout,
            elements,
            ..
        } => vec![
            workgroup(*transition),
            lds(*output_lds, *element, *layout, *elements, Lds::Published),
        ],
        Op::RawMemoryBind {
            view,
            element,
            extent,
            layout,
            space,
            access,
            index_space,
            atomic_scope,
            ..
        } => vec![capability(
            *view,
            ExecutionRoleContractV1::MemoryView {
                element: *element,
                layout: *layout,
                space: *space,
                access: *access,
                extent: Some(Extent::Dynamic(*extent)),
                initialization: Some(Initialization::FullyInitialized),
                index_space: *index_space,
                requires_index_space: index_space.is_some(),
                atomic_scope: *atomic_scope,
            },
        )],
        Op::PrivateMemoryAllocate {
            view,
            element,
            layout,
            elements,
            ..
        } => vec![capability(
            *view,
            ExecutionRoleContractV1::MemoryView {
                element: *element,
                layout: *layout,
                space: Space::Private,
                access: Access::ExclusiveReadWrite,
                extent: Some(Extent::Static(*elements)),
                initialization: Some(Initialization::FullyInitialized),
                index_space: None,
                requires_index_space: false,
                atomic_scope: None,
            },
        )],
        Op::WorkgroupMemoryIndex { witness, .. } => vec![capability(
            *witness,
            ExecutionRoleContractV1::WorkgroupMemoryIndex,
        )],
        Op::WorkgroupMemoryAllocate {
            view,
            element,
            layout,
            elements,
            index_space,
            ..
        } => vec![capability(
            *view,
            ExecutionRoleContractV1::MemoryView {
                element: *element,
                layout: *layout,
                space: Space::Workgroup,
                access: Access::DisjointWrite,
                extent: Some(Extent::Static(*elements)),
                initialization: Some(Initialization::Uninitialized),
                index_space: Some(*index_space),
                requires_index_space: true,
                atomic_scope: None,
            },
        )],
        Op::WorkgroupMemoryPublish {
            output_view,
            transition,
            element,
            layout,
            ..
        } => {
            let extent = operand_type(1).and_then(|ty| match ty {
                Type::ExecutionCapability(capability) => match capability.role {
                    ExecutionCapabilityRoleV1::MemoryView { extent, .. } => Some(extent),
                    _ => None,
                },
                _ => None,
            });
            vec![
                workgroup(*transition),
                capability(
                    *output_view,
                    ExecutionRoleContractV1::MemoryView {
                        element: *element,
                        layout: *layout,
                        space: Space::Workgroup,
                        access: Access::ReadOnly,
                        extent,
                        initialization: Some(Initialization::Published),
                        index_space: None,
                        requires_index_space: false,
                        atomic_scope: None,
                    },
                ),
            ]
        }
        Op::MemoryStore { .. } => vec![ExecutionResultContractV1::Bool],
    }
}

fn execution_operand_type_matches(
    actual: Option<&Type>,
    expected: ExecutionOperandContractV1,
    contract: &ExecutionCapabilityOpV1,
) -> bool {
    match expected {
        ExecutionOperandContractV1::KernelContext => matches!(
            actual,
            Some(Type::KernelContext(context))
                if context.root() == &contract.provenance.root
                    && context.kernel_marker() == &contract.provenance.kernel_marker
                    && context.target() == &contract.provenance.target_brand
                    && context.launch() == &contract.provenance.launch_brand
        ),
        ExecutionOperandContractV1::Capability { source, role } => matches!(
            actual,
            Some(Type::ExecutionCapability(capability))
                if source.into_iter().flatten().any(|source| source == capability.source_type)
                    && capability.provenance == contract.provenance
                    && capability.workgroup_brand == contract.workgroup_brand
                    && capability.epoch == contract.epoch_before
                    && execution_role_matches(&capability.role, role)
        ),
        ExecutionOperandContractV1::Scalar(scalar) => actual == Some(&Type::Scalar(scalar)),
        ExecutionOperandContractV1::ElementScalar(layout) => {
            matches!(actual, Some(Type::Scalar(scalar)) if scalar_matches_execution_layout(*scalar, layout))
        }
        ExecutionOperandContractV1::Pointer {
            layout,
            space,
            access,
        } => matches!(
            actual,
            Some(Type::Pointer(pointer))
                if pointer.address_space == space
                    && pointer.access == access
                    && matches!(pointer.pointee.as_ref(), Type::Scalar(scalar) if scalar_matches_execution_layout(*scalar, layout))
        ),
        ExecutionOperandContractV1::Slice {
            scalar,
            space,
            access,
        } => matches!(
            actual,
            Some(Type::Slice(slice))
                if slice.address_space == space
                    && slice.access == access
                    && slice.element.as_ref() == &Type::Scalar(scalar)
        ),
        ExecutionOperandContractV1::BoolProof => actual == Some(&Type::BOOL),
    }
}

fn execution_result_type_matches(
    actual: &Type,
    expected: ExecutionResultContractV1,
    contract: &ExecutionCapabilityOpV1,
) -> bool {
    match expected {
        ExecutionResultContractV1::Capability { source, role } => matches!(
            actual,
            Type::ExecutionCapability(capability)
                if capability.source_type == source
                    && capability.provenance == contract.provenance
                    && capability.workgroup_brand == contract.workgroup_brand
                    && capability.epoch == contract.epoch_after.or(contract.epoch_before)
                    && execution_role_matches(&capability.role, role)
        ),
        ExecutionResultContractV1::Scalar(scalar) => actual == &Type::Scalar(scalar),
        ExecutionResultContractV1::ElementScalar(layout) => {
            matches!(actual, Type::Scalar(scalar) if scalar_matches_execution_layout(*scalar, layout))
        }
        ExecutionResultContractV1::Bool => actual == &Type::BOOL,
    }
}

fn execution_role_matches(
    actual: &ExecutionCapabilityRoleV1,
    expected: ExecutionRoleContractV1,
) -> bool {
    use crate::ExecutionCapabilityRoleV1 as Role;
    match (actual, expected) {
        (Role::Workgroup, ExecutionRoleContractV1::Workgroup)
        | (Role::WorkgroupMemoryIndex, ExecutionRoleContractV1::WorkgroupMemoryIndex) => true,
        (
            Role::Subgroup { width },
            ExecutionRoleContractV1::Subgroup {
                width: expected_width,
            },
        ) => *width == expected_width,
        (
            Role::Lds {
                element,
                layout,
                elements,
                state,
            },
            ExecutionRoleContractV1::Lds {
                element: expected_element,
                layout: expected_layout,
                elements: expected_elements,
                state: expected_state,
            },
        ) => {
            *element == expected_element
                && *layout == expected_layout
                && *elements == expected_elements
                && *state == expected_state
        }
        (
            Role::ScopedAtomic {
                element,
                space,
                scope,
            },
            ExecutionRoleContractV1::ScopedAtomic {
                element: expected_element,
                space: expected_space,
                scope: expected_scope,
            },
        ) => *element == expected_element && *space == expected_space && *scope == expected_scope,
        (
            Role::Matrix {
                subgroup_brand,
                width,
            },
            ExecutionRoleContractV1::Matrix {
                subgroup_brand: expected_brand,
                width: expected_width,
            },
        ) => *subgroup_brand == expected_brand && *width == expected_width,
        (
            Role::PendingAsyncCopy {
                element,
                layout,
                elements,
            },
            ExecutionRoleContractV1::PendingAsyncCopy {
                element: expected_element,
                layout: expected_layout,
                elements: expected_elements,
            },
        ) => {
            *element == expected_element
                && *layout == expected_layout
                && *elements == expected_elements
        }
        (
            Role::MemoryView {
                element,
                layout,
                space,
                access,
                extent,
                initialization,
                index_space,
                atomic_scope,
            },
            ExecutionRoleContractV1::MemoryView {
                element: expected_element,
                layout: expected_layout,
                space: expected_space,
                access: expected_access,
                extent: expected_extent,
                initialization: expected_initialization,
                index_space: expected_index_space,
                requires_index_space,
                atomic_scope: expected_atomic_scope,
            },
        ) => {
            *element == expected_element
                && *layout == expected_layout
                && *space == expected_space
                && *access == expected_access
                && expected_extent.is_none_or(|expected| *extent == expected)
                && expected_initialization.is_none_or(|expected| *initialization == expected)
                && if requires_index_space {
                    index_space.is_some()
                        && expected_index_space
                            .is_none_or(|expected| *index_space == Some(expected))
                } else {
                    index_space.is_none()
                }
                && *atomic_scope == expected_atomic_scope
        }
        _ => false,
    }
}

fn execution_scalar_layout(scalar: ScalarType) -> crate::ExecutionElementLayoutV1 {
    let bytes = scalar.bit_width().unwrap_or(0).div_ceil(8);
    crate::ExecutionElementLayoutV1 {
        byte_size: u32::from(bytes),
        byte_alignment: bytes,
    }
}

fn scalar_matches_execution_layout(
    scalar: ScalarType,
    layout: crate::ExecutionElementLayoutV1,
) -> bool {
    !matches!(scalar, ScalarType::Bool | ScalarType::Index)
        && execution_scalar_layout(scalar) == layout
}

fn valid_execution_capability_requirement(requirement: &ExecutionCapabilityRequirementV1) -> bool {
    match requirement {
        ExecutionCapabilityRequirementV1::AddressSpace { .. } => true,
        ExecutionCapabilityRequirementV1::Atomic {
            value_type,
            operation,
            ordering,
            failure_ordering,
            scope,
            address_space,
        } => {
            let legal_storage = valid_capability_scalar(*value_type)
                && matches!(
                    address_space,
                    AddressSpace::Workgroup | AddressSpace::Global | AddressSpace::Generic
                )
                && *scope != SynchronizationScope::Invocation
                && (*address_space != AddressSpace::Workgroup
                    || scope.rank() <= SynchronizationScope::Workgroup.rank());
            let legal_ordering = match operation {
                AtomicKind::Load => {
                    failure_ordering.is_none()
                        && !matches!(
                            ordering,
                            MemoryOrdering::Release | MemoryOrdering::AcquireRelease
                        )
                }
                AtomicKind::Store => {
                    failure_ordering.is_none()
                        && !matches!(
                            ordering,
                            MemoryOrdering::Acquire | MemoryOrdering::AcquireRelease
                        )
                }
                AtomicKind::CompareExchange => failure_ordering
                    .is_some_and(|failure| valid_failure_ordering(*ordering, failure)),
                _ => failure_ordering.is_none(),
            };
            legal_storage && legal_ordering
        }
        ExecutionCapabilityRequirementV1::Barrier {
            execution_scope,
            memory_scope,
            ordering,
            address_spaces,
        } => {
            matches!(
                execution_scope,
                SynchronizationScope::Subgroup
                    | SynchronizationScope::Workgroup
                    | SynchronizationScope::Device
            ) && memory_scope.rank() >= execution_scope.rank()
                && valid_synchronization_semantics(*memory_scope, *ordering, address_spaces)
        }
        ExecutionCapabilityRequirementV1::Collective {
            execution_scope,
            operation,
            value_type,
            participants,
        } => {
            matches!(
                execution_scope,
                SynchronizationScope::Subgroup
                    | SynchronizationScope::Workgroup
                    | SynchronizationScope::Device
            ) && match operation {
                CollectiveCapabilityOperationV1::Any | CollectiveCapabilityOperationV1::All => {
                    *value_type == ScalarType::Bool
                }
                CollectiveCapabilityOperationV1::Broadcast => {
                    *value_type == ScalarType::Bool || valid_capability_scalar(*value_type)
                }
                _ => valid_capability_scalar(*value_type),
            } && *participants != 0
        }
        ExecutionCapabilityRequirementV1::Matrix {
            m,
            n,
            k,
            input_type,
            accumulator_type,
        } => {
            *m != 0
                && *n != 0
                && *k != 0
                && valid_capability_scalar(*input_type)
                && valid_capability_scalar(*accumulator_type)
        }
        ExecutionCapabilityRequirementV1::AsyncCopy {
            source,
            destination,
            bytes,
            alignment,
            completion,
        } => {
            let legal_transfer = source != destination
                && matches!(
                    (source, destination),
                    (
                        AddressSpace::Global | AddressSpace::Constant,
                        AddressSpace::Workgroup
                    ) | (AddressSpace::Workgroup, AddressSpace::Global)
                )
                && *bytes != 0
                && *alignment != 0
                && alignment.is_power_of_two();
            let legal_completion = match completion {
                AsyncCopyCompletionV1::ExplicitWaitGroups {
                    maximum_pending_groups,
                } => *maximum_pending_groups != 0,
                AsyncCopyCompletionV1::WorkgroupBarrier => true,
            };
            legal_transfer && legal_completion
        }
        ExecutionCapabilityRequirementV1::Numerical { value_type, mode } => {
            let _ = mode;
            value_type.is_float()
        }
        ExecutionCapabilityRequirementV1::Resource(resource) => match resource {
            ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(value) => *value != 0,
            ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(value)
            | ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(value)
            | ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(value) => {
                *value != 0
            }
        },
    }
}

fn valid_capability_scalar(scalar: ScalarType) -> bool {
    scalar != ScalarType::Bool && scalar != ScalarType::Index && scalar.bit_width().is_some()
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
        TargetCapability::Extension { namespace, name }
            if namespace == AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
                && name == AMDGPU_DIAGNOSTICS_CAPABILITY_NAME =>
        {
            supported.contains(required)
                || supported.contains(&TargetCapability::Extension {
                    namespace: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
                    name: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
                })
        }
        TargetCapability::Extension { namespace, name }
            if namespace == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE
                && name == AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME =>
        {
            supported.contains(required)
                || supported.contains(&TargetCapability::Extension {
                    namespace: AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
                    name: AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
                })
        }
        TargetCapability::Execution(required) => supported.iter().any(|capability| {
            matches!(
                capability,
                TargetCapability::Execution(available)
                    if execution_requirement_is_supported(required, available)
            )
        }),
        _ => supported.contains(required),
    }
}

fn execution_requirement_is_supported(
    required: &ExecutionCapabilityRequirementV1,
    available: &ExecutionCapabilityRequirementV1,
) -> bool {
    match (required, available) {
        (
            ExecutionCapabilityRequirementV1::Resource(required),
            ExecutionCapabilityRequirementV1::Resource(available),
        ) => match (required, available) {
            (
                ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(required),
                ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(available),
            ) => required <= available,
            (
                ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(required),
                ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(available),
            )
            | (
                ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(required),
                ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(available),
            )
            | (
                ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(required),
                ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(available),
            ) => required <= available,
            _ => false,
        },
        _ => required == available,
    }
}
