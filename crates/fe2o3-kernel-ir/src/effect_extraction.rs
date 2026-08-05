use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    AddressSpace, AllocationIdentity, AtomicEffect, BlockId, ByteExpression,
    ConflictIndeterminateReason, ConflictReason, EffectConflict, Function, FunctionId,
    InvocationPairing, InvocationRange1d, MemoryAccess, MemoryRegion, Module, NoConflictReason,
    Operation, OperationKind, RegionAnalysisError, RegionEffect, RegionEffectKind,
    RegionValidationError, ScalarType, SynchronizationEpoch, Type, ValueId, VerificationErrors,
    analyze_effect_conflict, verify_module,
};

/// Stable location of an operation within one function body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionOperationLocation {
    pub block: BlockId,
    pub operation_index: usize,
}

impl FunctionOperationLocation {
    pub const fn new(block: BlockId, operation_index: usize) -> Self {
        Self {
            block,
            operation_index,
        }
    }
}

/// Untrusted caller-supplied facts relating SSA pointers to dynamic invocations.
///
/// These facts are analysis inputs only. Supplying them does not create proof,
/// verification, or safe-launch authority.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FunctionEffectBindings {
    pointer_regions: BTreeMap<ValueId, MemoryRegion>,
    invocations: BTreeMap<FunctionOperationLocation, InvocationRange1d>,
    epochs: BTreeMap<FunctionOperationLocation, SynchronizationEpoch>,
}

impl FunctionEffectBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind_pointer_region(
        &mut self,
        pointer: ValueId,
        region: MemoryRegion,
    ) -> Option<MemoryRegion> {
        self.pointer_regions.insert(pointer, region)
    }

    pub fn bind_invocations(
        &mut self,
        location: FunctionOperationLocation,
        invocations: InvocationRange1d,
    ) -> Option<InvocationRange1d> {
        self.invocations.insert(location, invocations)
    }

    pub fn bind_epoch(
        &mut self,
        location: FunctionOperationLocation,
        epoch: SynchronizationEpoch,
    ) -> Option<SynchronizationEpoch> {
        self.epochs.insert(location, epoch)
    }

    pub fn pointer_region(&self, pointer: ValueId) -> Option<&MemoryRegion> {
        self.pointer_regions.get(&pointer)
    }

    pub fn invocation_range(
        &self,
        location: FunctionOperationLocation,
    ) -> Option<InvocationRange1d> {
        self.invocations.get(&location).copied()
    }

    pub fn epoch(&self, location: FunctionOperationLocation) -> SynchronizationEpoch {
        self.epochs
            .get(&location)
            .copied()
            .unwrap_or(SynchronizationEpoch::INITIAL)
    }
}

/// One extracted effect tied back to the Kernel IR operation that produced it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatedRegionEffect {
    pub location: FunctionOperationLocation,
    pub pointer: ValueId,
    pub invocations: Option<InvocationRange1d>,
    pub effect: RegionEffect,
}

/// Outcome of the byte-bounds obligation for one memory operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoundsObligationOutcome {
    /// The access is in bounds if the caller-supplied bindings are accurate.
    EstablishedUnderSuppliedBindings,
    Violated(BoundsViolation),
    Indeterminate(BoundsIndeterminateReason),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoundsViolation {
    Region(RegionValidationError),
    AddressSpaceMismatch {
        region: AddressSpace,
        access: AddressSpace,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoundsIndeterminateReason {
    MissingPointerRegion { pointer: ValueId },
    MissingInvocationMapping,
    UnknownAllocation,
    UnboundedRegion,
    AccessWidthUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundsObligation {
    pub location: FunctionOperationLocation,
    pub pointer: ValueId,
    pub access_width: Option<u64>,
    pub outcome: BoundsObligationOutcome,
}

/// Outcome of comparing two operations across distinct dynamic invocations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RaceObligationOutcome {
    /// No conflict exists if the caller-supplied bindings are accurate.
    NoConflictUnderSuppliedBindings(NoConflictReason),
    Conflict(ConflictReason),
    Indeterminate(RaceIndeterminateReason),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RaceIndeterminateReason {
    EffectUnavailable { location: FunctionOperationLocation },
    MissingInvocationMapping { location: FunctionOperationLocation },
    Conflict(ConflictIndeterminateReason),
    Analysis(RegionAnalysisError),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RaceObligation {
    pub left: FunctionOperationLocation,
    pub right: FunctionOperationLocation,
    pub outcome: RaceObligationOutcome,
}

/// Conditions that prevent the function report from being a complete summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectExtractionIssue {
    FunctionDeclaration,
    CallEffectsUnavailable {
        location: FunctionOperationLocation,
        callee: FunctionId,
    },
}

/// Describes the non-authoritative facts on which this report is based.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionEffectAnalysisBasis {
    /// Allocation identities, regions, invocation ranges, and epochs came from
    /// an untrusted caller and have not been tied to launch or provenance facts.
    UntrustedCallerSuppliedBindings,
}

/// Whether extraction accounted for every possible effect in the function.
///
/// Completeness does not establish that the supplied bindings are true or that
/// any bounds or race obligation is satisfied.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EffectExtractionCompleteness {
    CompleteUnderSuppliedBindings,
    Incomplete,
}

/// Failure to select a function from a structurally verified module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionRegionEffectExtractionError {
    InvalidModule(VerificationErrors),
    MissingFunction { function: FunctionId },
}

impl fmt::Display for FunctionRegionEffectExtractionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModule(errors) => errors.fmt(formatter),
            Self::MissingFunction { function } => {
                write!(
                    formatter,
                    "function {function} is not present in the module"
                )
            }
        }
    }
}

impl Error for FunctionRegionEffectExtractionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidModule(errors) => Some(errors),
            Self::MissingFunction { .. } => None,
        }
    }
}

/// Descriptive extraction and obligation results for one function.
///
/// This report intentionally has no conversion to `Verified` or launch
/// authority. Consumers must treat every conflict, violation, indeterminate
/// outcome, and extraction issue as an unsatisfied obligation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionRegionEffectReport {
    function: FunctionId,
    effects: Vec<LocatedRegionEffect>,
    bounds_obligations: Vec<BoundsObligation>,
    race_obligations: Vec<RaceObligation>,
    extraction_issues: Vec<EffectExtractionIssue>,
    completeness: EffectExtractionCompleteness,
}

impl FunctionRegionEffectReport {
    pub fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn analysis_basis(&self) -> FunctionEffectAnalysisBasis {
        FunctionEffectAnalysisBasis::UntrustedCallerSuppliedBindings
    }

    pub fn effects(&self) -> &[LocatedRegionEffect] {
        &self.effects
    }

    pub fn bounds_obligations(&self) -> &[BoundsObligation] {
        &self.bounds_obligations
    }

    pub fn race_obligations(&self) -> &[RaceObligation] {
        &self.race_obligations
    }

    pub fn extraction_issues(&self) -> &[EffectExtractionIssue] {
        &self.extraction_issues
    }

    pub const fn completeness(&self) -> EffectExtractionCompleteness {
        self.completeness
    }
}

/// Extracts region effects from Kernel IR memory operations and reports the
/// bounds and cross-invocation race obligations that this bounded model can
/// classify.
pub fn extract_function_region_effects(
    module: &Module,
    function: &FunctionId,
    bindings: &FunctionEffectBindings,
) -> Result<FunctionRegionEffectReport, FunctionRegionEffectExtractionError> {
    verify_module(module).map_err(FunctionRegionEffectExtractionError::InvalidModule)?;
    let function = module.function(function).ok_or_else(|| {
        FunctionRegionEffectExtractionError::MissingFunction {
            function: function.clone(),
        }
    })?;

    let Some(body) = &function.body else {
        return Ok(FunctionRegionEffectReport {
            function: function.id.clone(),
            effects: Vec::new(),
            bounds_obligations: Vec::new(),
            race_obligations: Vec::new(),
            extraction_issues: vec![EffectExtractionIssue::FunctionDeclaration],
            completeness: EffectExtractionCompleteness::Incomplete,
        });
    };

    let value_types = collect_value_types(function);
    let mut effects = Vec::new();
    let mut bounds_obligations = Vec::new();
    let mut extraction_issues = Vec::new();
    let mut candidates = Vec::new();

    for block in &body.blocks {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            if let OperationKind::Call { callee, .. } = &operation.kind {
                extraction_issues.push(EffectExtractionIssue::CallEffectsUnavailable {
                    location,
                    callee: callee.clone(),
                });
            }

            let Some(memory_operation) = MemoryOperation::from_ir(operation, &value_types) else {
                continue;
            };
            let pointer = memory_operation.pointer;
            let invocations = bindings.invocation_range(location);
            let access_width = memory_operation.access_width;
            let region = bindings.pointer_region(pointer).cloned().or_else(|| {
                access_width.map(|width| {
                    MemoryRegion::new(
                        AllocationIdentity::Unknown,
                        memory_operation.access.address_space,
                        ByteExpression::constant(0),
                        ByteExpression::constant(width),
                    )
                })
            });
            let effect = region.zip(access_width).map(|(region, access_width)| {
                RegionEffect::new(
                    memory_operation.kind,
                    region,
                    access_width,
                    u64::from(memory_operation.access.alignment),
                    bindings.epoch(location),
                )
            });

            let outcome = bounds_outcome(
                pointer,
                access_width,
                effect.as_ref(),
                invocations,
                bindings.pointer_region(pointer).is_some(),
                memory_operation.access.address_space,
            );
            bounds_obligations.push(BoundsObligation {
                location,
                pointer,
                access_width,
                outcome,
            });

            let effect_index = effect.map(|effect| {
                let index = effects.len();
                effects.push(LocatedRegionEffect {
                    location,
                    pointer,
                    invocations,
                    effect,
                });
                index
            });
            candidates.push(RaceCandidate {
                location,
                effect_index,
            });
        }
    }

    let race_obligations = race_obligations(&candidates, &effects);
    let completeness = if extraction_issues.is_empty()
        && candidates
            .iter()
            .all(|candidate| candidate.effect_index.is_some())
    {
        EffectExtractionCompleteness::CompleteUnderSuppliedBindings
    } else {
        EffectExtractionCompleteness::Incomplete
    };
    Ok(FunctionRegionEffectReport {
        function: function.id.clone(),
        effects,
        bounds_obligations,
        race_obligations,
        extraction_issues,
        completeness,
    })
}

#[derive(Clone, Copy)]
struct MemoryOperation {
    pointer: ValueId,
    access: MemoryAccess,
    access_width: Option<u64>,
    kind: RegionEffectKind,
}

impl MemoryOperation {
    fn from_ir(operation: &Operation, value_types: &ValueTypes) -> Option<Self> {
        match &operation.kind {
            OperationKind::Load { pointer, access } => Some(Self {
                pointer: *pointer,
                access: *access,
                access_width: operation
                    .results
                    .first()
                    .and_then(|result| byte_width(&result.ty)),
                kind: RegionEffectKind::Read,
            }),
            OperationKind::Store {
                pointer,
                value,
                access,
            } => Some(Self {
                pointer: *pointer,
                access: *access,
                access_width: value_types.get(*value).and_then(byte_width),
                kind: RegionEffectKind::Write,
            }),
            OperationKind::Atomic(atomic) => {
                let value_type = value_types
                    .get(atomic.pointer)
                    .and_then(pointer_scalar_type);
                Some(Self {
                    pointer: atomic.pointer,
                    access: atomic.access,
                    access_width: value_type.and_then(scalar_byte_width),
                    kind: RegionEffectKind::Atomic(AtomicEffect {
                        kind: atomic.kind,
                        value_type: value_type.unwrap_or(ScalarType::Index),
                        scope: atomic.scope,
                        ordering: atomic.ordering,
                    }),
                })
            }
            _ => None,
        }
    }
}

fn bounds_outcome(
    pointer: ValueId,
    access_width: Option<u64>,
    effect: Option<&RegionEffect>,
    invocations: Option<InvocationRange1d>,
    has_pointer_region: bool,
    access_address_space: AddressSpace,
) -> BoundsObligationOutcome {
    let Some(_) = access_width else {
        return BoundsObligationOutcome::Indeterminate(
            BoundsIndeterminateReason::AccessWidthUnavailable,
        );
    };
    if !has_pointer_region {
        return BoundsObligationOutcome::Indeterminate(
            BoundsIndeterminateReason::MissingPointerRegion { pointer },
        );
    }
    let Some(invocations) = invocations else {
        return BoundsObligationOutcome::Indeterminate(
            BoundsIndeterminateReason::MissingInvocationMapping,
        );
    };
    let effect = effect.expect("known width and region produce an effect");
    if effect.region.address_space != access_address_space {
        return BoundsObligationOutcome::Violated(BoundsViolation::AddressSpaceMismatch {
            region: effect.region.address_space,
            access: access_address_space,
        });
    }
    match effect.validate(invocations) {
        Err(RegionValidationError::UnboundedExpression) => {
            BoundsObligationOutcome::Indeterminate(BoundsIndeterminateReason::UnboundedRegion)
        }
        Err(error) => BoundsObligationOutcome::Violated(BoundsViolation::Region(error)),
        Ok(()) if matches!(effect.region.allocation, AllocationIdentity::Unknown) => {
            BoundsObligationOutcome::Indeterminate(BoundsIndeterminateReason::UnknownAllocation)
        }
        Ok(()) => BoundsObligationOutcome::EstablishedUnderSuppliedBindings,
    }
}

#[derive(Clone, Copy)]
struct RaceCandidate {
    location: FunctionOperationLocation,
    effect_index: Option<usize>,
}

fn race_obligations(
    candidates: &[RaceCandidate],
    effects: &[LocatedRegionEffect],
) -> Vec<RaceObligation> {
    let mut obligations = Vec::new();
    for (left_index, left) in candidates.iter().enumerate() {
        for right in &candidates[left_index..] {
            let outcome = match (left.effect_index, right.effect_index) {
                (Some(left_effect), Some(right_effect)) => {
                    race_outcome(&effects[left_effect], &effects[right_effect])
                }
                (None, _) => RaceObligationOutcome::Indeterminate(
                    RaceIndeterminateReason::EffectUnavailable {
                        location: left.location,
                    },
                ),
                (_, None) => RaceObligationOutcome::Indeterminate(
                    RaceIndeterminateReason::EffectUnavailable {
                        location: right.location,
                    },
                ),
            };
            obligations.push(RaceObligation {
                left: left.location,
                right: right.location,
                outcome,
            });
        }
    }
    obligations
}

fn race_outcome(left: &LocatedRegionEffect, right: &LocatedRegionEffect) -> RaceObligationOutcome {
    let Some(left_invocations) = left.invocations else {
        return RaceObligationOutcome::Indeterminate(
            RaceIndeterminateReason::MissingInvocationMapping {
                location: left.location,
            },
        );
    };
    let Some(right_invocations) = right.invocations else {
        return RaceObligationOutcome::Indeterminate(
            RaceIndeterminateReason::MissingInvocationMapping {
                location: right.location,
            },
        );
    };

    match analyze_effect_conflict(
        &left.effect,
        left_invocations,
        &right.effect,
        right_invocations,
        InvocationPairing::DistinctInvocations,
    ) {
        Ok(EffectConflict::NoConflict(reason)) => {
            RaceObligationOutcome::NoConflictUnderSuppliedBindings(reason)
        }
        Ok(EffectConflict::Conflict(reason)) => RaceObligationOutcome::Conflict(reason),
        Ok(EffectConflict::Indeterminate(reason)) => {
            RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::Conflict(reason))
        }
        Err(error) => {
            RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::Analysis(error))
        }
    }
}

struct ValueTypes {
    values: BTreeMap<ValueId, Type>,
    ambiguous: BTreeSet<ValueId>,
}

impl ValueTypes {
    fn insert(&mut self, value: ValueId, ty: Type) {
        if self.values.insert(value, ty).is_some() {
            self.ambiguous.insert(value);
        }
    }

    fn get(&self, value: ValueId) -> Option<&Type> {
        (!self.ambiguous.contains(&value))
            .then(|| self.values.get(&value))
            .flatten()
    }
}

fn collect_value_types(function: &Function) -> ValueTypes {
    let mut types = ValueTypes {
        values: BTreeMap::new(),
        ambiguous: BTreeSet::new(),
    };
    let Some(body) = &function.body else {
        return types;
    };
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

fn byte_width(ty: &Type) -> Option<u64> {
    ty.as_scalar().and_then(scalar_byte_width)
}

fn scalar_byte_width(scalar: ScalarType) -> Option<u64> {
    let bits = scalar.bit_width()?;
    (bits % 8 == 0).then_some(u64::from(bits / 8))
}

fn pointer_scalar_type(ty: &Type) -> Option<ScalarType> {
    match ty {
        Type::Pointer(pointer) => pointer.pointee.as_scalar(),
        _ => None,
    }
}
