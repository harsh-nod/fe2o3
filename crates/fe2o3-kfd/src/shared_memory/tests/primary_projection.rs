use super::preparation::PreparationMemoryFixtureV1;
use super::*;
use crate::shared_memory::transitions::{
    self as adapter, ProjectionFaultV1 as Fault, TransitionStageV1 as Stage,
};
use std::any::TypeId;

#[derive(Clone, Copy, Debug)]
pub(crate) struct PrimaryProjectionCaseV1 {
    stage: Stage,
    pub(crate) panic: bool,
}

impl PrimaryProjectionCaseV1 {
    pub(crate) fn cases(allocation: bool) -> Vec<Self> {
        let stages: &[Stage] = if allocation {
            &[
                Stage::AllocationEvidence,
                Stage::AllocationProjection,
                Stage::AllocationCommit,
            ]
        } else {
            &[
                Stage::MappingEvidence,
                Stage::Map,
                Stage::MapProjection,
                Stage::MapCommit,
            ]
        };
        stages
            .iter()
            .flat_map(|&stage| [false, true].map(|panic| Self { stage, panic }))
            .collect()
    }

    pub(crate) fn assert_panic(self, payload: &(dyn std::any::Any + Send)) {
        assert!(self.panic);
        assert_eq!(
            payload.downcast_ref::<(&str, Stage)>(),
            Some(&("session projection", self.stage))
        );
    }
}

impl PreparationMemoryFixtureV1 {
    pub(crate) fn primary_terminal_allocation_v1(&self) -> bool {
        self.fixture
            .engine
            .terminal_transition
            .as_ref()
            .is_some_and(|t| {
                matches!(
                    t.stage,
                    Stage::AllocationEvidence
                        | Stage::AllocationProjection
                        | Stage::AllocationCommit
                )
            })
    }

    pub(crate) fn primary_arm_projection_v1(&mut self, case: PrimaryProjectionCaseV1) {
        assert!(self.projection_fault.is_none());
        self.projection_fault = Some(case);
    }

    pub(super) fn take_primary_projection_v1(&mut self) -> Option<(Stage, Fault)> {
        let case = self.projection_fault.take()?;
        assert!(self.projection_observation.is_none());
        self.projection_observation = Some((case, self.fixture.foundation.memory().clone()));
        Some((
            case.stage,
            if case.panic {
                Fault::Panic
            } else {
                Fault::Error
            },
        ))
    }

    pub(crate) fn primary_assert_projection_v1<P: GttProfileV1>(&self) {
        let (case, before) = self
            .projection_observation
            .as_ref()
            .expect("selected real projection was entered");
        assert!(self.projection_fault.is_none());
        assert_eq!(
            self.fixture.foundation.memory(),
            before,
            "failed projection cannot commit model state"
        );
        let e = &self.fixture.engine;
        assert_eq!(e.phase, SharedMemorySessionPhaseV1::Quarantined);
        assert!(e.pending_allocation.is_none());
        let terminal = e.terminal_transition.as_ref().unwrap();
        assert_eq!(terminal.stage, case.stage);
        let before_map = matches!(case.stage, Stage::MappingEvidence | Stage::Map);
        assert_eq!(terminal.input.is_some(), before_map);
        assert_eq!(terminal.output.is_some(), !before_map);
        let token = terminal
            .input
            .as_ref()
            .or(terminal.output.as_ref())
            .unwrap();
        let record = e.allocations.iter().find(|r| r.id == token.id).unwrap();
        assert_eq!(token.session_id, e.session_id);
        assert_eq!(token.generation, record.generation);
        assert_eq!(token.layout, record.layout);
        assert_eq!(token.profile_type, TypeId::of::<P>());
        assert_eq!(token.profile, P::PROFILE);
        assert_eq!(token.flags, P::FLAGS.bits());
        assert_eq!(token.userptr, P::IS_USERPTR);
        let state = match record.phase {
            SharedAllocationPhaseV1::CpuWritable => TypeId::of::<GttCpuWritableV1>(),
            SharedAllocationPhaseV1::ExecutableImmutable => {
                TypeId::of::<GttExecutableImmutableV1>()
            }
            SharedAllocationPhaseV1::GpuAccessibleMutable => {
                TypeId::of::<GttGpuAccessibleMutableV1>()
            }
            SharedAllocationPhaseV1::GpuAccessibleExecutable => {
                TypeId::of::<GttGpuAccessibleExecutableV1>()
            }
            SharedAllocationPhaseV1::Released => panic!("possibly live token released"),
        };
        assert_eq!(token.state_type, state);
        let mapped = matches!(case.stage, Stage::MapProjection | Stage::MapCommit);
        assert_eq!(
            terminal.progress,
            adapter::NativeTransitionProgressV1 {
                attempted: mapped,
                returned_success: mapped.then_some(true),
                returned_map_prefix: mapped.then_some(1),
            }
        );
    }
}
