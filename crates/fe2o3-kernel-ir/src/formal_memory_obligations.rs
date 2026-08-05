use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    AccessMode, AddressSpace, Axis, BinaryOp, ByteExpression, Constant, Function, FunctionId,
    FunctionOperationLocation, IndexKind, IntrinsicKind, InvocationRange1d, KernelId, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Type, ValueId,
    VerificationErrors, verify_module,
};

/// A concrete one-dimensional launch extent supplied to formal extraction.
///
/// `Unknown` is accepted so pipeline stages can fail closed without inventing
/// a launch size. It can never produce a complete analysis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExplicitLaunchExtent1d {
    Exact(u64),
    Unknown,
}

/// A compiler-derived identity for one pointer or slice kernel parameter.
///
/// This identity names a formal parameter, not a runtime allocation. There is
/// deliberately no public constructor and no conversion to `AllocationId`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalAllocationIdentity {
    parameter_index: u32,
}

impl FormalAllocationIdentity {
    pub const fn parameter_index(self) -> u32 {
        self.parameter_index
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalParameterKind {
    Pointer,
    Slice,
}

/// Static information about one allocation-bearing kernel parameter.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalAllocationParameter {
    identity: FormalAllocationIdentity,
    value: ValueId,
    kind: FormalParameterKind,
    address_space: AddressSpace,
    access: AccessMode,
}

impl FormalAllocationParameter {
    pub const fn identity(&self) -> FormalAllocationIdentity {
        self.identity
    }

    pub const fn value(&self) -> ValueId {
        self.value
    }

    pub const fn kind(&self) -> FormalParameterKind {
        self.kind
    }

    pub const fn address_space(&self) -> AddressSpace {
        self.address_space
    }

    pub const fn access(&self) -> AccessMode {
        self.access
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalMemoryAccessKind {
    Read,
    Write,
}

/// A compiler-derived, per-invocation byte region rooted at a formal kernel
/// parameter.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalMemoryAccess {
    location: FunctionOperationLocation,
    allocation: FormalAllocationIdentity,
    kind: FormalMemoryAccessKind,
    address_space: AddressSpace,
    byte_offset: ByteExpression,
    byte_width: u64,
    alignment: u64,
    invocations: InvocationRange1d,
}

impl FormalMemoryAccess {
    pub const fn location(&self) -> FunctionOperationLocation {
        self.location
    }

    pub const fn allocation(&self) -> FormalAllocationIdentity {
        self.allocation
    }

    pub const fn kind(&self) -> FormalMemoryAccessKind {
        self.kind
    }

    pub const fn address_space(&self) -> AddressSpace {
        self.address_space
    }

    pub const fn byte_offset(&self) -> ByteExpression {
        self.byte_offset
    }

    pub const fn byte_width(&self) -> u64 {
        self.byte_width
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    pub const fn invocations(&self) -> InvocationRange1d {
        self.invocations
    }
}

/// Runtime allocation size needed for one compiler-derived access family.
///
/// This is an obligation for a later authenticated host binding. It does not
/// assert that the runtime argument has this size.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalBoundsRequirement {
    location: FunctionOperationLocation,
    allocation: FormalAllocationIdentity,
    minimum_byte_len: u64,
}

impl FormalBoundsRequirement {
    pub const fn location(self) -> FunctionOperationLocation {
        self.location
    }

    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.allocation
    }

    pub const fn minimum_byte_len(self) -> u64 {
        self.minimum_byte_len
    }

    /// Descriptive check only; the caller remains responsible for
    /// authenticating which runtime allocation and extent are being checked.
    pub const fn is_met_by_untrusted_byte_len(self, byte_len: u64) -> bool {
        byte_len >= self.minimum_byte_len
    }
}

/// A half-open byte range relative to a formal parameter's runtime base.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalByteRange {
    start: u64,
    end_exclusive: u64,
}

impl FormalByteRange {
    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end_exclusive(self) -> u64 {
        self.end_exclusive
    }

    const fn overlaps(self, other: Self) -> bool {
        self.start < other.end_exclusive && other.start < self.end_exclusive
    }
}

/// A host-side requirement needed because distinct formal parameters may name
/// overlapping ranges of the same runtime allocation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeAliasRequirement {
    left: FormalAllocationIdentity,
    right: FormalAllocationIdentity,
    left_accessed_bytes: FormalByteRange,
    right_accessed_bytes: FormalByteRange,
}

impl RuntimeAliasRequirement {
    pub const fn left(self) -> FormalAllocationIdentity {
        self.left
    }

    pub const fn right(self) -> FormalAllocationIdentity {
        self.right
    }

    pub const fn left_accessed_bytes(self) -> FormalByteRange {
        self.left_accessed_bytes
    }

    pub const fn right_accessed_bytes(self) -> FormalByteRange {
        self.right_accessed_bytes
    }
}

/// A compiler-derived possible race within one formal allocation.
///
/// This is an unsatisfied obligation, not a statement about runtime behavior.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterInvocationConflictRequirement {
    left: FunctionOperationLocation,
    right: FunctionOperationLocation,
    allocation: FormalAllocationIdentity,
}

impl InterInvocationConflictRequirement {
    pub const fn left(self) -> FunctionOperationLocation {
        self.left
    }

    pub const fn right(self) -> FunctionOperationLocation {
        self.right
    }

    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.allocation
    }
}

/// Why compiler-derived formal extraction could not account for all memory
/// behavior of the selected kernel launch.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalMemoryIncompleteReason {
    LaunchExtentUnknown,
    LaunchExtentZero,
    LaunchRankUnsupported {
        rank: u8,
    },
    StaticLaunchExtentMismatch {
        expected: u32,
        actual: u64,
    },
    CallEffectsUnavailable {
        location: FunctionOperationLocation,
        callee: FunctionId,
    },
    UnsupportedMemoryEffect {
        location: FunctionOperationLocation,
    },
    UnsupportedPointerDerivation {
        location: FunctionOperationLocation,
        pointer: ValueId,
    },
    UnsupportedIndexExpression {
        location: FunctionOperationLocation,
        index: ValueId,
    },
    ElementWidthUnavailable {
        location: FunctionOperationLocation,
        pointer: ValueId,
    },
    AddressArithmeticOverflow {
        location: FunctionOperationLocation,
    },
}

/// Describes exactly what the analysis result establishes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalMemoryAnalysisBasis {
    /// Regions and identities were derived from verified IR, but no runtime
    /// pointer, allocation extent, or alias relationship was authenticated.
    CompilerDerivedFormalParametersUnauthenticatedAtRuntime,
}

/// Partial or complete formal facts for one kernel launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalMemoryObligations {
    kernel: KernelId,
    entry: FunctionId,
    invocations: Option<InvocationRange1d>,
    allocations: Vec<FormalAllocationParameter>,
    accesses: Vec<FormalMemoryAccess>,
    bounds_requirements: Vec<FormalBoundsRequirement>,
    runtime_alias_requirements: Vec<RuntimeAliasRequirement>,
    inter_invocation_conflicts: Vec<InterInvocationConflictRequirement>,
}

impl FormalMemoryObligations {
    pub fn kernel(&self) -> &KernelId {
        &self.kernel
    }

    pub fn entry(&self) -> &FunctionId {
        &self.entry
    }

    pub const fn analysis_basis(&self) -> FormalMemoryAnalysisBasis {
        FormalMemoryAnalysisBasis::CompilerDerivedFormalParametersUnauthenticatedAtRuntime
    }

    pub const fn invocations(&self) -> Option<InvocationRange1d> {
        self.invocations
    }

    pub fn allocations(&self) -> &[FormalAllocationParameter] {
        &self.allocations
    }

    pub fn accesses(&self) -> &[FormalMemoryAccess] {
        &self.accesses
    }

    pub fn bounds_requirements(&self) -> &[FormalBoundsRequirement] {
        &self.bounds_requirements
    }

    pub fn runtime_alias_requirements(&self) -> &[RuntimeAliasRequirement] {
        &self.runtime_alias_requirements
    }

    pub fn inter_invocation_conflicts(&self) -> &[InterInvocationConflictRequirement] {
        &self.inter_invocation_conflicts
    }
}

/// Completeness of compiler-derived formal extraction.
///
/// `Complete` means all modeled IR effects were translated into obligations.
/// It does not mean those obligations hold for a runtime launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormalMemoryObligationAnalysis {
    Complete(FormalMemoryObligations),
    Incomplete {
        partial: FormalMemoryObligations,
        reasons: Vec<FormalMemoryIncompleteReason>,
    },
}

impl FormalMemoryObligationAnalysis {
    pub const fn obligations(&self) -> &FormalMemoryObligations {
        match self {
            Self::Complete(obligations) => obligations,
            Self::Incomplete { partial, .. } => partial,
        }
    }

    pub fn incomplete_reasons(&self) -> &[FormalMemoryIncompleteReason] {
        match self {
            Self::Complete(_) => &[],
            Self::Incomplete { reasons, .. } => reasons,
        }
    }

    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormalMemoryObligationError {
    InvalidModule(VerificationErrors),
    MissingKernel { kernel: KernelId },
}

impl fmt::Display for FormalMemoryObligationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModule(errors) => errors.fmt(formatter),
            Self::MissingKernel { kernel } => {
                write!(formatter, "kernel {kernel} is not present in the module")
            }
        }
    }
}

impl Error for FormalMemoryObligationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidModule(errors) => Some(errors),
            Self::MissingKernel { .. } => None,
        }
    }
}

/// Derives formal memory obligations from a structurally verified Kernel IR
/// module and a concrete one-dimensional launch extent.
///
/// No caller-selected allocation identity is accepted. The resulting formal
/// identities still require runtime authentication before any launch-safety or
/// race-freedom claim can be made.
pub fn derive_kernel_memory_obligations(
    module: &Module,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent1d,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    verify_module(module).map_err(FormalMemoryObligationError::InvalidModule)?;
    let kernel = module
        .kernels
        .iter()
        .find(|kernel| &kernel.id == kernel_id)
        .ok_or_else(|| FormalMemoryObligationError::MissingKernel {
            kernel: kernel_id.clone(),
        })?;
    let function = module
        .function(&kernel.entry)
        .expect("verified kernel entry must exist");
    let body = function
        .body
        .as_ref()
        .expect("verified kernel entry must be a definition");

    let mut reasons = BTreeSet::new();
    let invocations = resolve_invocations(&kernel.domain, launch_extent, &mut reasons);
    let allocations = formal_allocations(function);
    let allocation_by_value: BTreeMap<_, _> = allocations
        .iter()
        .map(|allocation| (allocation.value, allocation.identity))
        .collect();
    let definitions = collect_definitions(function);
    let value_types = collect_types(function);
    let context = AccessDerivationContext {
        definitions: &definitions,
        value_types: &value_types,
        allocation_by_value: &allocation_by_value,
    };
    let mut accesses = Vec::new();

    for block in &body.blocks {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            match &operation.kind {
                OperationKind::Call { callee, .. } => {
                    reasons.insert(FormalMemoryIncompleteReason::CallEffectsUnavailable {
                        location,
                        callee: callee.clone(),
                    });
                }
                OperationKind::Load { pointer, access } => {
                    if let Some(invocations) = invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Read,
                            *access,
                            invocations,
                            &context,
                        ) {
                            Ok(access) => accesses.push(access),
                            Err(reason) => {
                                reasons.insert(reason);
                            }
                        }
                    }
                }
                OperationKind::Store {
                    pointer, access, ..
                } => {
                    if let Some(invocations) = invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Write,
                            *access,
                            invocations,
                            &context,
                        ) {
                            Ok(access) => accesses.push(access),
                            Err(reason) => {
                                reasons.insert(reason);
                            }
                        }
                    }
                }
                OperationKind::Atomic(_) => {
                    reasons
                        .insert(FormalMemoryIncompleteReason::UnsupportedMemoryEffect { location });
                }
                _ => {}
            }
        }
    }

    let bounds_requirements = derive_bounds_requirements(&accesses, &mut reasons);
    let runtime_alias_requirements = derive_alias_requirements(&accesses);
    let inter_invocation_conflicts = derive_inter_invocation_conflicts(&accesses);
    let obligations = FormalMemoryObligations {
        kernel: kernel.id.clone(),
        entry: kernel.entry.clone(),
        invocations,
        allocations,
        accesses,
        bounds_requirements,
        runtime_alias_requirements,
        inter_invocation_conflicts,
    };

    if reasons.is_empty() {
        Ok(FormalMemoryObligationAnalysis::Complete(obligations))
    } else {
        Ok(FormalMemoryObligationAnalysis::Incomplete {
            partial: obligations,
            reasons: reasons.into_iter().collect(),
        })
    }
}

fn resolve_invocations(
    domain: &LaunchDomain,
    launch_extent: ExplicitLaunchExtent1d,
    reasons: &mut BTreeSet<FormalMemoryIncompleteReason>,
) -> Option<InvocationRange1d> {
    let LaunchDomain::D1 { x } = domain else {
        reasons.insert(FormalMemoryIncompleteReason::LaunchRankUnsupported {
            rank: domain.rank(),
        });
        return None;
    };
    let ExplicitLaunchExtent1d::Exact(actual) = launch_extent else {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentUnknown);
        return None;
    };
    if actual == 0 {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentZero);
        return None;
    }
    if let LaunchExtent::Static(expected) = x
        && u64::from(*expected) != actual
    {
        reasons.insert(FormalMemoryIncompleteReason::StaticLaunchExtentMismatch {
            expected: *expected,
            actual,
        });
        return None;
    }
    Some(InvocationRange1d::from_count(actual).expect("nonzero launch extent is a valid range"))
}

fn formal_allocations(function: &Function) -> Vec<FormalAllocationParameter> {
    let body = function
        .body
        .as_ref()
        .expect("verified kernel entry is defined");
    body.parameters
        .iter()
        .copied()
        .zip(&function.signature.parameters)
        .enumerate()
        .filter_map(|(parameter_index, (value, ty))| {
            let (kind, address_space, access) = match ty {
                Type::Pointer(pointer) => (
                    FormalParameterKind::Pointer,
                    pointer.address_space,
                    pointer.access,
                ),
                Type::Slice(slice) => (
                    FormalParameterKind::Slice,
                    slice.address_space,
                    slice.access,
                ),
                _ => return None,
            };
            Some(FormalAllocationParameter {
                identity: FormalAllocationIdentity {
                    parameter_index: u32::try_from(parameter_index)
                        .expect("verified body length fits ValueId space"),
                },
                value,
                kind,
                address_space,
                access,
            })
        })
        .collect()
}

struct Definitions<'module> {
    operations: BTreeMap<ValueId, (&'module Operation, FunctionOperationLocation)>,
}

fn collect_definitions(function: &Function) -> Definitions<'_> {
    let mut operations = BTreeMap::new();
    let body = function
        .body
        .as_ref()
        .expect("verified function is defined");
    for block in &body.blocks {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            for result in &operation.results {
                operations.insert(result.id, (operation, location));
            }
        }
    }
    Definitions { operations }
}

fn collect_types(function: &Function) -> BTreeMap<ValueId, Type> {
    let mut types = BTreeMap::new();
    let body = function
        .body
        .as_ref()
        .expect("verified function is defined");
    for (value, ty) in body.parameters.iter().zip(&function.signature.parameters) {
        types.insert(*value, ty.clone());
    }
    for block in &body.blocks {
        for parameter in &block.parameters {
            types.insert(parameter.id, parameter.ty.clone());
        }
        for operation in &block.operations {
            for result in &operation.results {
                types.insert(result.id, result.ty.clone());
            }
        }
    }
    types
}

struct AccessDerivationContext<'analysis, 'module> {
    definitions: &'analysis Definitions<'module>,
    value_types: &'analysis BTreeMap<ValueId, Type>,
    allocation_by_value: &'analysis BTreeMap<ValueId, FormalAllocationIdentity>,
}

fn derive_access(
    location: FunctionOperationLocation,
    pointer: ValueId,
    kind: FormalMemoryAccessKind,
    access: MemoryAccess,
    invocations: InvocationRange1d,
    context: &AccessDerivationContext<'_, '_>,
) -> Result<FormalMemoryAccess, FormalMemoryIncompleteReason> {
    let byte_width = context
        .value_types
        .get(&pointer)
        .and_then(pointer_byte_width)
        .ok_or(FormalMemoryIncompleteReason::ElementWidthUnavailable { location, pointer })?;
    let pointer_expression = derive_pointer_expression(
        pointer,
        context.definitions,
        context.value_types,
        context.allocation_by_value,
        location,
    )?;
    Ok(FormalMemoryAccess {
        location,
        allocation: pointer_expression.allocation,
        kind,
        address_space: access.address_space,
        byte_offset: pointer_expression.byte_offset.into_byte_expression(),
        byte_width,
        alignment: u64::from(access.alignment),
        invocations,
    })
}

#[derive(Clone, Copy)]
struct PointerExpression {
    allocation: FormalAllocationIdentity,
    byte_offset: AffineExpression,
}

fn derive_pointer_expression(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    access_location: FunctionOperationLocation,
) -> Result<PointerExpression, FormalMemoryIncompleteReason> {
    if let Some(allocation) = allocation_by_value.get(&pointer).copied()
        && matches!(value_types.get(&pointer), Some(Type::Pointer(_)))
    {
        return Ok(PointerExpression {
            allocation,
            byte_offset: AffineExpression::ZERO,
        });
    }
    let Some((operation, definition_location)) = definitions.operations.get(&pointer) else {
        return Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            location: access_location,
            pointer,
        });
    };
    match &operation.kind {
        OperationKind::SliceData { slice } => allocation_by_value
            .get(slice)
            .copied()
            .map(|allocation| PointerExpression {
                allocation,
                byte_offset: AffineExpression::ZERO,
            })
            .ok_or(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                location: *definition_location,
                pointer,
            }),
        OperationKind::GetElementPointer { base, offset } => {
            let base_expression = derive_pointer_expression(
                *base,
                definitions,
                value_types,
                allocation_by_value,
                access_location,
            )?;
            let element_width = value_types.get(base).and_then(pointer_byte_width).ok_or(
                FormalMemoryIncompleteReason::ElementWidthUnavailable {
                    location: *definition_location,
                    pointer: *base,
                },
            )?;
            let index = derive_affine_index(*offset, definitions).map_err(|error| match error {
                IndexExpressionError::Unsupported => {
                    FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                        location: *definition_location,
                        index: *offset,
                    }
                }
                IndexExpressionError::Overflow => {
                    FormalMemoryIncompleteReason::AddressArithmeticOverflow {
                        location: *definition_location,
                    }
                }
            })?;
            let byte_delta = index.checked_multiply_constant(element_width).ok_or(
                FormalMemoryIncompleteReason::AddressArithmeticOverflow {
                    location: *definition_location,
                },
            )?;
            let byte_offset = base_expression.byte_offset.checked_add(byte_delta).ok_or(
                FormalMemoryIncompleteReason::AddressArithmeticOverflow {
                    location: *definition_location,
                },
            )?;
            Ok(PointerExpression {
                allocation: base_expression.allocation,
                byte_offset,
            })
        }
        _ => Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            location: *definition_location,
            pointer,
        }),
    }
}

#[derive(Clone, Copy)]
struct AffineExpression {
    constant: u64,
    invocation_coefficient: u64,
}

impl AffineExpression {
    const ZERO: Self = Self {
        constant: 0,
        invocation_coefficient: 0,
    };

    const INVOCATION: Self = Self {
        constant: 0,
        invocation_coefficient: 1,
    };

    const fn constant(value: u64) -> Self {
        Self {
            constant: value,
            invocation_coefficient: 0,
        }
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            constant: self.constant.checked_add(other.constant)?,
            invocation_coefficient: self
                .invocation_coefficient
                .checked_add(other.invocation_coefficient)?,
        })
    }

    fn checked_multiply_constant(self, multiplier: u64) -> Option<Self> {
        Some(Self {
            constant: self.constant.checked_mul(multiplier)?,
            invocation_coefficient: self.invocation_coefficient.checked_mul(multiplier)?,
        })
    }

    const fn into_byte_expression(self) -> ByteExpression {
        ByteExpression::invocation_affine(self.constant, self.invocation_coefficient)
    }

    fn evaluate(self, invocation: u64) -> Option<u64> {
        self.invocation_coefficient
            .checked_mul(invocation)
            .and_then(|offset| self.constant.checked_add(offset))
    }
}

#[derive(Clone, Copy)]
enum IndexExpressionError {
    Unsupported,
    Overflow,
}

fn derive_affine_index(
    value: ValueId,
    definitions: &Definitions<'_>,
) -> Result<AffineExpression, IndexExpressionError> {
    let (operation, _) = definitions
        .operations
        .get(&value)
        .ok_or(IndexExpressionError::Unsupported)?;
    if !matches!(operation.results.as_slice(), [result] if result.ty == Type::INDEX) {
        return Err(IndexExpressionError::Unsupported);
    }
    match &operation.kind {
        OperationKind::Constant(Constant::Index(value)) => Ok(AffineExpression::constant(*value)),
        OperationKind::Intrinsic(intrinsic)
            if intrinsic.kind
                == (IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                }) =>
        {
            Ok(AffineExpression::INVOCATION)
        }
        OperationKind::Binary { op, lhs, rhs } => {
            let lhs = derive_affine_index(*lhs, definitions)?;
            let rhs = derive_affine_index(*rhs, definitions)?;
            match op {
                BinaryOp::Add => lhs.checked_add(rhs).ok_or(IndexExpressionError::Overflow),
                BinaryOp::Multiply if lhs.invocation_coefficient == 0 => rhs
                    .checked_multiply_constant(lhs.constant)
                    .ok_or(IndexExpressionError::Overflow),
                BinaryOp::Multiply if rhs.invocation_coefficient == 0 => lhs
                    .checked_multiply_constant(rhs.constant)
                    .ok_or(IndexExpressionError::Overflow),
                BinaryOp::Multiply => Err(IndexExpressionError::Unsupported),
                _ => Err(IndexExpressionError::Unsupported),
            }
        }
        _ => Err(IndexExpressionError::Unsupported),
    }
}

fn pointer_byte_width(ty: &Type) -> Option<u64> {
    let Type::Pointer(pointer) = ty else {
        return None;
    };
    scalar_byte_width(pointer.pointee.as_scalar()?)
}

fn scalar_byte_width(scalar: ScalarType) -> Option<u64> {
    let bits = scalar.bit_width()?;
    (bits % 8 == 0).then_some(u64::from(bits / 8))
}

fn derive_bounds_requirements(
    accesses: &[FormalMemoryAccess],
    reasons: &mut BTreeSet<FormalMemoryIncompleteReason>,
) -> Vec<FormalBoundsRequirement> {
    accesses
        .iter()
        .filter_map(|access| {
            let range = access_envelope(access).or_else(|| {
                reasons.insert(FormalMemoryIncompleteReason::AddressArithmeticOverflow {
                    location: access.location,
                });
                None
            })?;
            Some(FormalBoundsRequirement {
                location: access.location,
                allocation: access.allocation,
                minimum_byte_len: range.end_exclusive,
            })
        })
        .collect()
}

fn access_envelope(access: &FormalMemoryAccess) -> Option<FormalByteRange> {
    let ByteExpression::Affine {
        constant,
        invocation_coefficient,
    } = access.byte_offset
    else {
        return None;
    };
    let expression = AffineExpression {
        constant,
        invocation_coefficient,
    };
    let last = access.invocations.last();
    let start = expression.evaluate(0)?;
    let end_exclusive = expression.evaluate(last)?.checked_add(access.byte_width)?;
    Some(FormalByteRange {
        start,
        end_exclusive,
    })
}

#[derive(Clone, Copy)]
struct AllocationEnvelope {
    range: FormalByteRange,
    writes: bool,
    address_space: AddressSpace,
}

fn derive_alias_requirements(accesses: &[FormalMemoryAccess]) -> Vec<RuntimeAliasRequirement> {
    let mut envelopes = BTreeMap::<FormalAllocationIdentity, AllocationEnvelope>::new();
    for access in accesses {
        let Some(range) = access_envelope(access) else {
            continue;
        };
        envelopes
            .entry(access.allocation)
            .and_modify(|envelope| {
                envelope.range.start = envelope.range.start.min(range.start);
                envelope.range.end_exclusive =
                    envelope.range.end_exclusive.max(range.end_exclusive);
                envelope.writes |= access.kind == FormalMemoryAccessKind::Write;
            })
            .or_insert(AllocationEnvelope {
                range,
                writes: access.kind == FormalMemoryAccessKind::Write,
                address_space: access.address_space,
            });
    }

    let entries: Vec<_> = envelopes.into_iter().collect();
    let mut requirements = Vec::new();
    for (left_index, (left, left_envelope)) in entries.iter().enumerate() {
        for (right, right_envelope) in &entries[left_index + 1..] {
            if address_spaces_may_alias(left_envelope.address_space, right_envelope.address_space)
                && (left_envelope.writes || right_envelope.writes)
            {
                requirements.push(RuntimeAliasRequirement {
                    left: *left,
                    right: *right,
                    left_accessed_bytes: left_envelope.range,
                    right_accessed_bytes: right_envelope.range,
                });
            }
        }
    }
    requirements
}

fn address_spaces_may_alias(left: AddressSpace, right: AddressSpace) -> bool {
    left == right || matches!(left, AddressSpace::Generic) || matches!(right, AddressSpace::Generic)
}

fn derive_inter_invocation_conflicts(
    accesses: &[FormalMemoryAccess],
) -> Vec<InterInvocationConflictRequirement> {
    let mut requirements = Vec::new();
    for (left_index, left) in accesses.iter().enumerate() {
        for right in &accesses[left_index..] {
            if left.allocation != right.allocation
                || (left.kind == FormalMemoryAccessKind::Read
                    && right.kind == FormalMemoryAccessKind::Read)
                || proves_distinct_invocation_disjointness(left, right)
            {
                continue;
            }
            requirements.push(InterInvocationConflictRequirement {
                left: left.location,
                right: right.location,
                allocation: left.allocation,
            });
        }
    }
    requirements
}

fn proves_distinct_invocation_disjointness(
    left: &FormalMemoryAccess,
    right: &FormalMemoryAccess,
) -> bool {
    if left.invocations.last() == 0 || right.invocations.last() == 0 {
        return true;
    }
    if left.byte_offset == right.byte_offset && left.byte_width == right.byte_width {
        let ByteExpression::Affine {
            invocation_coefficient,
            ..
        } = left.byte_offset
        else {
            return false;
        };
        return invocation_coefficient >= left.byte_width;
    }
    match (access_envelope(left), access_envelope(right)) {
        (Some(left), Some(right)) => !left.overlaps(right),
        _ => false,
    }
}
