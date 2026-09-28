//! Shared read-only capability-state adapter. No admission or state mutation.
use super::{
    ProjectedCapabilityOriginV1, ProjectedCapabilityStateV1, ProjectedCapabilityValueV1,
    SemanticOperandV1, simple_operand_local,
};

pub(super) trait CapabilityStateReadV1 {
    fn capability_value_v1(&self, local: usize) -> Option<ProjectedCapabilityValueV1>;
}
impl CapabilityStateReadV1 for ProjectedCapabilityStateV1 {
    fn capability_value_v1(&self, local: usize) -> Option<ProjectedCapabilityValueV1> {
        self.get(&local).copied()
    }
}

pub(super) fn capability_known_origin_read_v1(
    state: &(impl CapabilityStateReadV1 + ?Sized),
    operand: &SemanticOperandV1,
) -> Option<ProjectedCapabilityOriginV1> {
    let local = simple_operand_local(operand)?.index() as usize;
    match state.capability_value_v1(local) {
        Some(ProjectedCapabilityValueV1::Known(origin)) => Some(origin),
        Some(ProjectedCapabilityValueV1::ConstructedEnum(_))
        | Some(ProjectedCapabilityValueV1::Invalid)
        | None => None,
    }
}
