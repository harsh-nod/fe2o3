use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    AddressSpace, BlockId, CastKind, FunctionId, KernelId, MemoryOrdering, Module, ModuleId,
    ScalarType, SynchronizationScope, TargetCapability, Type,
    verify_public_module_with_shared_engine_v1,
};

#[cfg(test)]
use crate::Function;

#[cfg(test)]
#[path = "verification_definition_borrow_tests.rs"]
mod definition_borrow_tests;

#[cfg(test)]
#[path = "verification_capability_borrow_tests.rs"]
mod capability_borrow_tests;

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
    InvalidVectorOperation,
    InvalidAmdGpuDiagnosticOperation,
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
    #[cfg(test)]
    pub(crate) fn borrowed_v1(&self) -> crate::VerificationDiagnosticLocationV1<'_> {
        crate::VerificationDiagnosticLocationV1 {
            module: &self.module,
            function: self.function.as_ref(),
            kernel: self.kernel.as_ref(),
            block: self.block,
            operation: self.operation,
        }
    }

    pub(crate) fn module(module: &Module) -> Self {
        Self {
            module: module.id.clone(),
            function: None,
            kernel: None,
            block: None,
            operation: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn function(module: &Module, function: &Function) -> Self {
        Self {
            function: Some(function.id.clone()),
            ..Self::module(module)
        }
    }

    #[cfg(test)]
    pub(crate) fn at_block(mut self, block: BlockId) -> Self {
        self.block = Some(block);
        self
    }

    #[cfg(test)]
    pub(crate) fn at_operation(mut self, operation: usize) -> Self {
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
    pub(crate) fn from_sorted_diagnostics_v1(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }
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
/// regardless of map implementation details in the verifier. Recursive types
/// deeper than the canonical wire limit are rejected before semantic checks.
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
    pub(crate) const fn new_verified_v1(module: &'module Module) -> Self {
        Self { module }
    }

    pub const fn module(self) -> &'module Module {
        self.module
    }
}

/// Verifies a module and returns a non-owning token reusable by later analyses.
/// In-memory recursive types must fit the canonical V12 depth limit.
pub fn verify_module_ref(
    module: &Module,
) -> Result<VerifiedKernelIrModuleV1<'_>, VerificationErrors> {
    verify_public_module_with_shared_engine_v1(module, None)
}

/// Verifies a module and rejects requirements outside a target capability set.
/// In-memory recursive types must fit the canonical V12 depth limit.
pub fn verify_module_with_capabilities(
    module: &Module,
    supported_capabilities: &BTreeSet<TargetCapability>,
) -> Result<(), VerificationErrors> {
    verify_public_module_with_shared_engine_v1(module, Some(supported_capabilities)).map(|_| ())
}

pub(crate) fn valid_scalar_cast(kind: CastKind, from: ScalarType, to: ScalarType) -> bool {
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

pub(crate) fn valid_failure_ordering(success: MemoryOrdering, failure: MemoryOrdering) -> bool {
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

pub(crate) fn valid_synchronization_semantics(
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

pub(crate) fn scope_can_observe_address_space(
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

pub(crate) fn is_assembly_register_type(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(ScalarType::I32 | ScalarType::U32))
}
