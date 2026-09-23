//! Pure projection of actual producer facts; no IDs are synthesized from depth.
use crate::protocol_address_space;
use fe2o3_debug_protocol::*;
use fe2o3_kernel_ir::AccessMode;
use fe2o3_kir_debugger::{
    RuntimeAllocationMissingV1, RuntimeFrameMissingV1, RuntimeFrameObservationV1,
    RuntimeObservationCoverageV1, RuntimeObservationCutoffV1, RuntimeOriginMissingV1,
    RuntimeOriginObservationV1,
};
use fe2o3_kir_sim::*;

pub(super) fn number(value: u64) -> ResourceDecimalU64V1 {
    ResourceDecimalU64V1::new(value)
}
pub(super) fn invocation(value: SimulationInvocationV1) -> RuntimeInvocationV1 {
    RuntimeInvocationV1 {
        global: value.global.map(number),
        workgroup: value.workgroup.map(number),
        local: value.local,
        workgroup_size: value.workgroup_size,
        workgroup_count: value.workgroup_count.map(number),
        launch_extent: value.launch_extent.map(number),
    }
}
pub(super) fn site(value: SimulationDebugSiteV1) -> RuntimeOperationSiteV1 {
    RuntimeOperationSiteV1 {
        function_ordinal: number(value.function_ordinal as u64),
        block: value.block.0,
        operation: value.operation,
    }
}
pub(super) fn coverage(
    value: RuntimeObservationCoverageV1,
    retained: usize,
) -> RuntimeMetadataCoverageV1 {
    match value {
        RuntimeObservationCoverageV1::Disabled => RuntimeMetadataCoverageV1::Disabled,
        RuntimeObservationCoverageV1::Complete => RuntimeMetadataCoverageV1::Complete,
        RuntimeObservationCoverageV1::InvalidJoin => RuntimeMetadataCoverageV1::InvalidJoin,
        RuntimeObservationCoverageV1::PrefixTruncated(reason) => {
            RuntimeMetadataCoverageV1::PrefixTruncated {
                retained_records: number(retained as u64),
                reason: match reason {
                    RuntimeObservationCutoffV1::RowLimit => RuntimeMetadataCutoffV1::RowLimit,
                    RuntimeObservationCutoffV1::FrameRowLimit => {
                        RuntimeMetadataCutoffV1::FrameLimit
                    }
                    RuntimeObservationCutoffV1::TransitionLimit => {
                        RuntimeMetadataCutoffV1::TransitionLimit
                    }
                    RuntimeObservationCutoffV1::ByteLimit => RuntimeMetadataCutoffV1::ByteLimit,
                    RuntimeObservationCutoffV1::AllocationFailure => {
                        RuntimeMetadataCutoffV1::AllocationFailure
                    }
                    RuntimeObservationCutoffV1::InvalidCapacity => {
                        RuntimeMetadataCutoffV1::InvalidCapacity
                    }
                    RuntimeObservationCutoffV1::ValidationWorkLimit => {
                        RuntimeMetadataCutoffV1::ValidationWorkLimit
                    }
                },
            }
        }
    }
}
pub(super) fn origin(
    result: Result<RuntimeOriginObservationV1<'_>, RuntimeOriginMissingV1>,
) -> RuntimeOperationObservationV1 {
    match result {
        Ok(value) => RuntimeOperationObservationV1::Available {
            identity: RuntimeOperationIdentityV1 {
                activation: number(value.activation()),
                attempt: number(value.attempt()),
                site: site(value.site()),
            },
        },
        Err(error) => RuntimeOperationObservationV1::Unavailable {
            reason: origin_missing(error),
        },
    }
}
fn origin_missing(error: RuntimeOriginMissingV1) -> RuntimeObservationUnavailableV1 {
    use RuntimeObservationUnavailableV1 as Out;
    match error {
        RuntimeOriginMissingV1::NoSuchRecord | RuntimeOriginMissingV1::NoCurrentRecord => {
            Out::NoSelectedRecord
        }
        RuntimeOriginMissingV1::Disabled => Out::NotRequested,
        RuntimeOriginMissingV1::PrefixTruncated(_) => Out::PrefixTruncated,
        RuntimeOriginMissingV1::InvalidJoin => Out::InvalidJoin,
        RuntimeOriginMissingV1::RuntimeUnavailable(reason) => match reason {
            SimulationDebugOriginUnavailableV1::NotRequested => Out::NotRequested,
            SimulationDebugOriginUnavailableV1::NoMatchingOperation => Out::NoMatchingOperation,
            SimulationDebugOriginUnavailableV1::AggregateRecord => Out::AggregateRecord,
            SimulationDebugOriginUnavailableV1::IdentityInvariant => Out::IdentityInvariant,
        },
    }
}
pub(super) fn frames(
    result: Result<RuntimeFrameObservationV1<'_>, RuntimeFrameMissingV1>,
    maximum: usize,
) -> RuntimeFramesV1 {
    let observation = match result {
        Ok(value) => value,
        Err(error) => {
            return RuntimeFramesV1::Unavailable {
                reason: frame_missing(error),
            };
        }
    };
    if observation.len() > maximum {
        return RuntimeFramesV1::Unavailable {
            reason: RuntimeObservationUnavailableV1::NotCaptured,
        };
    }
    let mut frames = Vec::new();
    if frames.try_reserve_exact(observation.len()).is_err() {
        return RuntimeFramesV1::Unavailable {
            reason: RuntimeObservationUnavailableV1::AllocationFailure,
        };
    }
    for index in 0..observation.len() {
        let Some(value) = observation.get(index) else {
            return RuntimeFramesV1::Unavailable {
                reason: RuntimeObservationUnavailableV1::InvalidJoin,
            };
        };
        let legacy = value.legacy();
        frames.push(RuntimeFrameV1 {
            legacy_depth: legacy.depth,
            function_ordinal: number(legacy.function_ordinal as u64),
            block: legacy.block.0,
            next_operation: legacy.next_operation,
            activation: number(value.activation()),
            operation: match value.operation_state() {
                SimulationDebugFrameOperationV1::Ready => RuntimeFrameOperationV1::Ready,
                SimulationDebugFrameOperationV1::ActiveOperation { attempt, site: at } => {
                    RuntimeFrameOperationV1::ActiveOperation {
                        attempt: number(attempt),
                        site: site(at),
                    }
                }
                SimulationDebugFrameOperationV1::Suspended { attempt, site: at } => {
                    RuntimeFrameOperationV1::Suspended {
                        attempt: number(attempt),
                        site: site(at),
                    }
                }
            },
            parent: match value.parent() {
                SimulationDebugFrameParentV1::Root => RuntimeFrameParentV1::Root,
                SimulationDebugFrameParentV1::Caller {
                    activation,
                    attempt,
                    call_site,
                } => RuntimeFrameParentV1::Caller {
                    activation: number(activation),
                    attempt: number(attempt),
                    call_site: site(call_site),
                },
            },
        });
    }
    RuntimeFramesV1::Captured { frames }
}
fn frame_missing(error: RuntimeFrameMissingV1) -> RuntimeObservationUnavailableV1 {
    use RuntimeObservationUnavailableV1 as Out;
    match error {
        RuntimeFrameMissingV1::NoSuchRecord | RuntimeFrameMissingV1::NoCurrentRecord => {
            Out::NoSelectedRecord
        }
        RuntimeFrameMissingV1::Disabled => Out::NotRequested,
        RuntimeFrameMissingV1::PrefixTruncated(_) => Out::PrefixTruncated,
        RuntimeFrameMissingV1::InvalidJoin => Out::InvalidJoin,
        RuntimeFrameMissingV1::RuntimeUnavailable(reason) => match reason {
            SimulationDebugFrameOriginUnavailableV1::NotRequested => Out::NotRequested,
            SimulationDebugFrameOriginUnavailableV1::NotCheckpoint => Out::NotCheckpoint,
            SimulationDebugFrameOriginUnavailableV1::LegacyStackUnavailable { .. } => {
                Out::LegacyStackUnavailable
            }
            SimulationDebugFrameOriginUnavailableV1::IdentityInvariant => Out::IdentityInvariant,
        },
    }
}
pub(super) fn allocation_missing(
    error: RuntimeAllocationMissingV1,
) -> RuntimeObservationUnavailableV1 {
    use RuntimeObservationUnavailableV1 as Out;
    match error {
        RuntimeAllocationMissingV1::NoSuchRecord | RuntimeAllocationMissingV1::NoCurrentRecord => {
            Out::NoSelectedRecord
        }
        RuntimeAllocationMissingV1::Disabled => Out::NotRequested,
        RuntimeAllocationMissingV1::PrefixTruncated(_) => Out::PrefixTruncated,
        RuntimeAllocationMissingV1::InvalidJoin => Out::InvalidJoin,
        RuntimeAllocationMissingV1::NotLive => Out::NotCaptured,
        RuntimeAllocationMissingV1::WorkLimit => Out::WorkLimit,
        RuntimeAllocationMissingV1::RuntimeUnavailable(watermark) => match watermark {
            SimulationAllocationWatermarkV1::NotRequested => Out::NotRequested,
            SimulationAllocationWatermarkV1::PolicyDisabled => Out::PolicyDisabled,
            SimulationAllocationWatermarkV1::Available { .. } => Out::InvalidJoin,
            SimulationAllocationWatermarkV1::Unavailable { reason } => match reason {
                SimulationAllocationObservationUnavailableV1::IdentityInvariant => {
                    Out::IdentityInvariant
                }
                SimulationAllocationObservationUnavailableV1::SequenceOverflow => {
                    Out::SequenceOverflow
                }
                SimulationAllocationObservationUnavailableV1::ObservationStopped => {
                    Out::ObservationStopped
                }
            },
        },
    }
}
pub(super) fn identity(value: SimulationAllocationStorageIdentityV1) -> ResourceStorageIdentityV2 {
    ResourceStorageIdentityV2 {
        allocation: number(value.allocation()),
        storage_slot: number(value.storage_slot()),
        generation: number(value.generation()),
    }
}
pub(super) fn descriptor(
    value: SimulationAllocationDescriptorV1,
) -> ResourceAllocationDescriptorV2 {
    ResourceAllocationDescriptorV2 {
        identity: identity(value.identity()),
        address_space: protocol_address_space(value.address_space()),
        access: match value.access() {
            AccessMode::ReadOnly => ResourceAllocationAccessV1::ReadOnly,
            AccessMode::WriteOnly => ResourceAllocationAccessV1::WriteOnly,
            AccessMode::ReadWrite => ResourceAllocationAccessV1::ReadWrite,
        },
        alignment: value.alignment(),
        byte_len: number(value.byte_len()),
        owning_scope: match value.scope() {
            SimulationAllocationScopeV1::Dispatch => ResourceAllocationScopeV2::Dispatch,
            SimulationAllocationScopeV1::Invocation(value) => {
                ResourceAllocationScopeV2::Invocation {
                    invocation: invocation(value),
                }
            }
            SimulationAllocationScopeV1::Workgroup {
                coordinate,
                size,
                count,
                launch,
            } => ResourceAllocationScopeV2::Workgroup {
                coordinate: coordinate.map(number),
                size,
                count: count.map(number),
                launch: launch.map(number),
            },
        },
        creation_site: value.creation_site().map(|at| ResourceCreationSiteV2 {
            function_ordinal: number(at.function_ordinal as u64),
            block: at.block.0,
            operation: at.operation,
        }),
    }
}
pub(super) fn transition(
    value: SimulationAllocationTransitionV1,
) -> ResourceAllocationTransitionV2 {
    ResourceAllocationTransitionV2 {
        sequence: number(value.sequence()),
        descriptor: descriptor(value.descriptor()),
        kind: match value.kind() {
            SimulationAllocationTransitionKindV1::Preexisting => {
                ResourceAllocationTransitionKindV2::Preexisting
            }
            SimulationAllocationTransitionKindV1::Create {
                previous_allocation,
            } => ResourceAllocationTransitionKindV2::Create {
                previous_allocation: previous_allocation.map(number),
            },
            SimulationAllocationTransitionKindV1::Release => {
                ResourceAllocationTransitionKindV2::Release
            }
        },
    }
}
pub(super) fn access(value: SimulationDebugMemoryAccessV1) -> ResourceMemoryAccessKindV1 {
    match value {
        SimulationDebugMemoryAccessV1::Read => ResourceMemoryAccessKindV1::Read,
        SimulationDebugMemoryAccessV1::WriteCommitted => ResourceMemoryAccessKindV1::WriteCommitted,
        SimulationDebugMemoryAccessV1::AtomicRead => ResourceMemoryAccessKindV1::AtomicRead,
        SimulationDebugMemoryAccessV1::AtomicWriteCommitted => {
            ResourceMemoryAccessKindV1::AtomicWriteCommitted
        }
        SimulationDebugMemoryAccessV1::AtomicReadWriteCommitted => {
            ResourceMemoryAccessKindV1::AtomicReadWriteCommitted
        }
    }
}
pub(super) fn schedule(value: SimulationDebugScheduleV1) -> ResourceAccessScheduleV1 {
    ResourceAccessScheduleV1 {
        decision_ordinal: value.decision_ordinal,
        identity: match value.identity {
            SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxSerialV1 => {
                ResourceScheduleIdentityV1::WorkgroupMajorLocalZyxSerialV1
            }
            SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1 => {
                ResourceScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1
            }
            SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1 => {
                ResourceScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1
            }
        },
    }
}
