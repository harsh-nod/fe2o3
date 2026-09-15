//! Inspection-only coordinates from the exact graph admitted before erasure.
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrVersionV1 as Version, PhaseOperationSourceV1, ReusablePhaseOperationV1 as Phase, ReusablePhaseCheckLimitsV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationPhaseFamilyV1 { OwnerConvert, Begin, Bind, CloseStorage, Seal, RelayClosure, RelayDrop, End }
impl SimulationPhaseFamilyV1 {
    fn of(operation: &Phase) -> Self {
        match operation {
            Phase::OwnerConvert { .. } => Self::OwnerConvert, Phase::Begin { .. } => Self::Begin,
            Phase::Bind { .. } => Self::Bind, Phase::CloseStorage { .. } => Self::CloseStorage,
            Phase::Seal { .. } => Self::Seal, Phase::RelayClosure { .. } => Self::RelayClosure,
            Phase::RelayDrop { .. } => Self::RelayDrop, Phase::End { .. } => Self::End,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationPhaseCoordinateV1 {
    function: u32, block: BlockId, operation: u32, value: ValueId,
    coordinate_kind: SimulationCapabilityCoordinateKindV13,
    family: SimulationPhaseFamilyV1, source: PhaseOperationSourceV1,
}
impl SimulationPhaseCoordinateV1 {
    pub const fn function(&self) -> u32 { self.function }
    pub const fn block(&self) -> BlockId { self.block }
    pub const fn operation(&self) -> u32 { self.operation }
    pub const fn value(&self) -> ValueId { self.value }
    pub const fn coordinate_kind(&self) -> SimulationCapabilityCoordinateKindV13 { self.coordinate_kind }
    pub const fn family(&self) -> SimulationPhaseFamilyV1 { self.family }
    pub const fn source(&self) -> &PhaseOperationSourceV1 { &self.source }
}

/// V13 exposes only its old exact receipt. The V14 variant retains both the
/// old-shaped execution coordinates and all typed phase definition/use sites.
#[derive(Debug, Eq, PartialEq)]
pub enum SimulationCapabilityProjectionReceiptV1 {
    V13(SimulationCapabilityProjectionReceiptV13),
    V14 { execution: SimulationCapabilityProjectionReceiptV13, phases: Vec<SimulationPhaseCoordinateV1>, work: usize },
}
impl SimulationCapabilityProjectionReceiptV1 {
    pub const fn version(&self) -> Version { match self { Self::V13(_) => Version::V13, Self::V14 { .. } => Version::V14 } }
    pub const fn as_v13(&self) -> Option<&SimulationCapabilityProjectionReceiptV13> { match self { Self::V13(r) => Some(r), Self::V14 { .. } => None } }
    pub fn execution_coordinates(&self) -> &[SimulationCapabilityCoordinateV13] {
        match self { Self::V13(r) | Self::V14 { execution:r, .. } => r.coordinates() }
    }
    pub fn phase_coordinates(&self) -> &[SimulationPhaseCoordinateV1] {
        match self { Self::V13(_) => &[], Self::V14 { phases, .. } => phases }
    }
    pub(crate) fn retained_heap_bytes(&self) -> Option<usize> {
        match self { Self::V13(r) => r.retained_heap_bytes(), Self::V14 { execution, phases, .. } =>
            execution.retained_heap_bytes()?.checked_add(phases.capacity().checked_mul(size_of::<SimulationPhaseCoordinateV1>())?) }
    }
    pub(crate) fn phase_work(&self) -> usize { match self { Self::V13(_) => 0, Self::V14 {work, ..} => *work } }
}

pub(crate) fn record_projection_coordinates(module: &Module, version: Version)
    -> Result<(SimulationCapabilityProjectionReceiptV1, usize), ExecutionCapabilityProjectionErrorV13> {
    if version == Version::V13 {
        let (receipt, scratch) = record_projection_coordinates_v13(module)?;
        return Ok((SimulationCapabilityProjectionReceiptV1::V13(receipt), scratch));
    }
    let error = || ExecutionCapabilityProjectionErrorV13::Invalid("declared phase coordinate budget or ordinal exceeded");
    let (execution, scratch) = record_projection_coordinates_base(module)?;
    let mut count = 0usize;
    let mut work = 0usize;
    for f in &module.functions {
        if let Some(body) = &f.body { for b in &body.blocks { for op in &b.operations {
            work = work.checked_add(1).ok_or_else(error)?;
            if work > ReusablePhaseCheckLimitsV1::DEFAULT.work { return Err(error()); }
            if let OperationKind::ReusablePhase(p) = &op.kind {
                count = count.checked_add(op.results.len()).and_then(|n| n.checked_add(p.operands.len())).ok_or_else(error)?;
            }
        } } }
    }
    let bytes = count.checked_mul(size_of::<SimulationPhaseCoordinateV1>()).ok_or_else(error)?;
    if bytes > ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes { return Err(error()); }
    let mut phases = Vec::new();
    phases.try_reserve_exact(count).map_err(|_| ExecutionCapabilityProjectionErrorV13::AllocationFailure)?;
    if phases.capacity().checked_mul(size_of::<SimulationPhaseCoordinateV1>()).ok_or_else(error)?
        > ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes { return Err(error()); }
    for (function, f) in module.functions.iter().enumerate() {
        let function = u32::try_from(function).map_err(|_| error())?;
        if let Some(body) = &f.body { for b in &body.blocks { for (ordinal, op) in b.operations.iter().enumerate() {
            work = work.checked_add(1).ok_or_else(error)?;
            if work > ReusablePhaseCheckLimitsV1::DEFAULT.work { return Err(error()); }
            let OperationKind::ReusablePhase(p) = &op.kind else { continue; };
            let operation = u32::try_from(ordinal).map_err(|_| error())?;
            work = work.checked_add((op.results.len() + p.operands.len()).checked_mul(size_of::<SimulationPhaseCoordinateV1>()).ok_or_else(error)?).ok_or_else(error)?;
            if work > ReusablePhaseCheckLimitsV1::DEFAULT.work { return Err(error()); }
            for (result, r) in op.results.iter().enumerate() {
                phases.push(SimulationPhaseCoordinateV1 { function, block:b.id, operation, value:r.id,
                    coordinate_kind: SimulationCapabilityCoordinateKindV13::OperationResultDefinition { result:u16::try_from(result).map_err(|_| error())? },
                    family: SimulationPhaseFamilyV1::of(&p.operation), source:p.source.clone() });
            }
            for (operand, value) in p.operands.iter().enumerate() {
                phases.push(SimulationPhaseCoordinateV1 { function, block:b.id, operation, value:*value,
                    coordinate_kind: SimulationCapabilityCoordinateKindV13::OperationOperandUse { operand:u16::try_from(operand).map_err(|_| error())? },
                    family: SimulationPhaseFamilyV1::of(&p.operation), source:p.source.clone() });
            }
        } } }
    }
    if phases.len() != count { return Err(error()); }
    Ok((SimulationCapabilityProjectionReceiptV1::V14 {execution, phases, work}, scratch))
}
