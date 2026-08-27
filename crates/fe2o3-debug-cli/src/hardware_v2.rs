use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_debug_protocol::*;
use fe2o3_kfd::{
    KfdDebugDeviceObservationV1, KfdDebugEventObservationV1, KfdDebugExceptionInfoV1,
    KfdDebugQueueObservationV1, KfdDebugQueueOperationObservationV1, KfdDebugQueueOperationStateV1,
    KfdLiveDebugSessionErrorV1, KfdLiveDebugSessionV1,
};
use fe2o3_kfd_uapi::{
    KfdDebugExceptionMaskV1, KfdDebugRuntimeStateV1, KfdDebugTrapExceptionCodeV1,
};

const NOTIFICATION_DRAIN_V2: usize = 1_024;

const EXCEPTION_CODES_V2: [KfdDebugTrapExceptionCodeV1; 23] = [
    KfdDebugTrapExceptionCodeV1::QueueWaveAbort,
    KfdDebugTrapExceptionCodeV1::QueueWaveTrap,
    KfdDebugTrapExceptionCodeV1::QueueWaveMathError,
    KfdDebugTrapExceptionCodeV1::QueueWaveIllegalInstruction,
    KfdDebugTrapExceptionCodeV1::QueueWaveMemoryViolation,
    KfdDebugTrapExceptionCodeV1::QueueWaveApertureViolation,
    KfdDebugTrapExceptionCodeV1::QueuePacketDispatchDimensionsInvalid,
    KfdDebugTrapExceptionCodeV1::QueuePacketDispatchGroupSegmentSizeInvalid,
    KfdDebugTrapExceptionCodeV1::QueuePacketDispatchCodeInvalid,
    KfdDebugTrapExceptionCodeV1::QueuePacketReserved,
    KfdDebugTrapExceptionCodeV1::QueuePacketUnsupported,
    KfdDebugTrapExceptionCodeV1::QueuePacketDispatchWorkgroupSizeInvalid,
    KfdDebugTrapExceptionCodeV1::QueuePacketDispatchRegisterInvalid,
    KfdDebugTrapExceptionCodeV1::QueuePacketVendorUnsupported,
    KfdDebugTrapExceptionCodeV1::QueuePreemptionError,
    KfdDebugTrapExceptionCodeV1::QueueNew,
    KfdDebugTrapExceptionCodeV1::DeviceQueueDelete,
    KfdDebugTrapExceptionCodeV1::DeviceMemoryViolation,
    KfdDebugTrapExceptionCodeV1::DeviceRasError,
    KfdDebugTrapExceptionCodeV1::DeviceFatalHalt,
    KfdDebugTrapExceptionCodeV1::DeviceNew,
    KfdDebugTrapExceptionCodeV1::ProcessRuntime,
    KfdDebugTrapExceptionCodeV1::ProcessDeviceRemove,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeDeviceV2 {
    gpu_id: u32,
    gfx_target_version: u32,
    xcc_count: u32,
    trap_debug_supported: bool,
    debug_firmware_supported: bool,
    launch_mode_supported: bool,
    launch_override_supported: bool,
    precise_memory_supported: bool,
    precise_alu_supported: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeQueueV2 {
    queue_id: u32,
    gpu_id: u32,
    ring_bytes: u32,
    queue_type: u32,
    context_save_area_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeEventV2 {
    exceptions: KfdDebugExceptionMaskV1,
    gpu_id: u32,
    queue_id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NativeExceptionInfoV2 {
    Runtime(KfdDebugRuntimeStateV1),
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeQueueOutcomeV2 {
    queue_id: u32,
    state: KfdDebugQueueOperationStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HardwareTransportErrorV2 {
    pub(crate) effect: HardwareEffectV2,
    pub(crate) operation: &'static str,
}

pub(crate) trait HardwareDebugTransportV2 {
    fn device_snapshot(&mut self) -> Result<Vec<NativeDeviceV2>, HardwareTransportErrorV2>;
    fn queue_snapshot(&mut self) -> Result<Vec<NativeQueueV2>, HardwareTransportErrorV2>;
    fn drain_notifications(&mut self) -> Result<(), HardwareTransportErrorV2>;
    fn query_event(&mut self) -> Result<Option<NativeEventV2>, HardwareTransportErrorV2>;
    fn query_exception_info(
        &mut self,
        source_id: u32,
        code: KfdDebugTrapExceptionCodeV1,
        clear: bool,
    ) -> Result<NativeExceptionInfoV2, HardwareTransportErrorV2>;
    fn acknowledge_runtime_transition(
        &mut self,
        event: NativeEventV2,
    ) -> Result<(), HardwareTransportErrorV2>;
    fn suspend_queues(
        &mut self,
        queues: &[u32],
        grace_period: u32,
    ) -> Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2>;
    fn resume_queues(
        &mut self,
        queues: &[u32],
    ) -> Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2>;
}

pub(crate) struct LiveKfdTransportV2 {
    session: KfdLiveDebugSessionV1,
    pending_runtime_event: Option<KfdDebugEventObservationV1>,
}

impl LiveKfdTransportV2 {
    pub(crate) fn new(session: KfdLiveDebugSessionV1) -> Self {
        Self {
            session,
            pending_runtime_event: None,
        }
    }

    pub(crate) fn finish(self) -> Result<(), KfdLiveDebugSessionErrorV1> {
        self.session.finish()
    }

    fn error(operation: &'static str, effect: HardwareEffectV2) -> HardwareTransportErrorV2 {
        HardwareTransportErrorV2 { effect, operation }
    }
}

impl HardwareDebugTransportV2 for LiveKfdTransportV2 {
    fn device_snapshot(&mut self) -> Result<Vec<NativeDeviceV2>, HardwareTransportErrorV2> {
        self.session
            .device_snapshot(KfdDebugExceptionMaskV1::NONE)
            .map(|items| items.into_iter().map(native_device).collect())
            .map_err(|_| Self::error("KFD device snapshot", HardwareEffectV2::None))
    }

    fn queue_snapshot(&mut self) -> Result<Vec<NativeQueueV2>, HardwareTransportErrorV2> {
        self.session
            .queue_snapshot(KfdDebugExceptionMaskV1::NONE)
            .map(|items| items.into_iter().map(native_queue).collect())
            .map_err(|_| Self::error("KFD queue snapshot", HardwareEffectV2::None))
    }

    fn drain_notifications(&mut self) -> Result<(), HardwareTransportErrorV2> {
        self.session
            .drain_notifications(NOTIFICATION_DRAIN_V2)
            .map(|_| ())
            .map_err(|_| Self::error("KFD notification drain", HardwareEffectV2::None))
    }

    fn query_event(&mut self) -> Result<Option<NativeEventV2>, HardwareTransportErrorV2> {
        let event = self
            .session
            .query_event(KfdDebugExceptionMaskV1::NONE)
            .map_err(|_| Self::error("KFD exception event query", HardwareEffectV2::None))?;
        if event.is_some_and(|event| {
            event
                .exceptions()
                .contains(KfdDebugTrapExceptionCodeV1::ProcessRuntime)
        }) {
            self.pending_runtime_event = event;
        }
        Ok(event.map(native_event))
    }

    fn query_exception_info(
        &mut self,
        source_id: u32,
        code: KfdDebugTrapExceptionCodeV1,
        clear: bool,
    ) -> Result<NativeExceptionInfoV2, HardwareTransportErrorV2> {
        self.session
            .query_exception_info(source_id, code, clear)
            .map(|info| match info {
                KfdDebugExceptionInfoV1::Runtime(runtime) => {
                    NativeExceptionInfoV2::Runtime(runtime.state())
                }
                KfdDebugExceptionInfoV1::NoPayload
                | KfdDebugExceptionInfoV1::DeviceMemoryViolation { .. } => {
                    NativeExceptionInfoV2::Other
                }
            })
            .map_err(|_| {
                Self::error(
                    "KFD exception information query",
                    if clear {
                        HardwareEffectV2::Indeterminate
                    } else {
                        HardwareEffectV2::None
                    },
                )
            })
    }

    fn acknowledge_runtime_transition(
        &mut self,
        _event: NativeEventV2,
    ) -> Result<(), HardwareTransportErrorV2> {
        let event = self.pending_runtime_event.take().ok_or_else(|| {
            Self::error("missing retained KFD runtime event", HardwareEffectV2::None)
        })?;
        self.session
            .acknowledge_runtime_transition(event)
            .map_err(|_| {
                Self::error(
                    "KFD runtime transition acknowledgement",
                    HardwareEffectV2::Indeterminate,
                )
            })
    }

    fn suspend_queues(
        &mut self,
        queues: &[u32],
        grace_period: u32,
    ) -> Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2> {
        self.session
            .suspend_queues(queues, KfdDebugExceptionMaskV1::NONE, grace_period)
            .map(|items| items.into_iter().map(native_outcome).collect())
            .map_err(|_| Self::error("KFD queue suspend", HardwareEffectV2::Indeterminate))
    }

    fn resume_queues(
        &mut self,
        queues: &[u32],
    ) -> Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2> {
        self.session
            .resume_queues(queues)
            .map(|items| items.into_iter().map(native_outcome).collect())
            .map_err(|_| Self::error("KFD queue resume", HardwareEffectV2::Indeterminate))
    }
}

fn native_device(value: KfdDebugDeviceObservationV1) -> NativeDeviceV2 {
    NativeDeviceV2 {
        gpu_id: value.gpu_id(),
        gfx_target_version: value.gfx_target_version(),
        xcc_count: value.xcc_count(),
        trap_debug_supported: value.supports_trap_debug(),
        debug_firmware_supported: value.supports_debug_firmware(),
        launch_mode_supported: value.supports_launch_mode(),
        launch_override_supported: value.supports_launch_override(),
        precise_memory_supported: value.supports_precise_memory_operations(),
        precise_alu_supported: value.supports_precise_alu_operations(),
    }
}

fn native_queue(value: KfdDebugQueueObservationV1) -> NativeQueueV2 {
    NativeQueueV2 {
        queue_id: value.queue_id(),
        gpu_id: value.gpu_id(),
        ring_bytes: value.ring_size(),
        queue_type: value.queue_type(),
        context_save_area_bytes: value.context_save_area_size(),
    }
}

fn native_event(value: KfdDebugEventObservationV1) -> NativeEventV2 {
    NativeEventV2 {
        exceptions: value.exceptions(),
        gpu_id: value.gpu_id(),
        queue_id: value.queue_id(),
    }
}

fn native_outcome(value: KfdDebugQueueOperationObservationV1) -> NativeQueueOutcomeV2 {
    NativeQueueOutcomeV2 {
        queue_id: value.queue_id(),
        state: value.state(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeviceRecordV2 {
    native: NativeDeviceV2,
    logical: HardwareDeviceIdV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueueRecordV2 {
    native: NativeQueueV2,
    logical: HardwareQueueIdV2,
    device: HardwareDeviceIdV2,
}

pub(crate) struct HardwareBackendV2<T: HardwareDebugTransportV2> {
    transport: T,
    limits: HardwareProtocolLimitsV2,
    control_revision: u64,
    commands_processed: u64,
    observation_sequence: u64,
    generation: u64,
    initialized_identity: bool,
    runtime_enabled: bool,
    poisoned: bool,
    terminated: bool,
    devices: Vec<DeviceRecordV2>,
    queues: Vec<QueueRecordV2>,
    suspended_native: BTreeSet<u32>,
    events: VecDeque<HardwareEventViewV2>,
}

impl<T: HardwareDebugTransportV2> HardwareBackendV2<T> {
    pub(crate) fn new(transport: T) -> Self {
        Self {
            transport,
            limits: HardwareProtocolLimitsV2::default(),
            control_revision: 0,
            commands_processed: 0,
            observation_sequence: 0,
            generation: 1,
            initialized_identity: false,
            runtime_enabled: false,
            poisoned: false,
            terminated: false,
            devices: Vec::new(),
            queues: Vec::new(),
            suspended_native: BTreeSet::new(),
            events: VecDeque::new(),
        }
    }

    pub(crate) const fn limits(&self) -> HardwareProtocolLimitsV2 {
        self.limits
    }

    pub(crate) fn into_transport(self) -> T {
        self.transport
    }

    pub(crate) fn pump_async_observations(&mut self) -> Result<(), HardwareTransportErrorV2> {
        if self.terminated || self.poisoned {
            return Ok(());
        }
        match self.pump_events_once() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.poisoned |= error.effect != HardwareEffectV2::None;
                Err(error)
            }
        }
    }

    fn session_view(&self) -> HardwareSessionViewV2 {
        HardwareSessionViewV2 {
            state: if self.terminated {
                HardwareSessionStateV2::Terminated
            } else if self.poisoned {
                HardwareSessionStateV2::Poisoned
            } else {
                HardwareSessionStateV2::Running
            },
            commands_processed: self.commands_processed,
            control_revision: self.control_revision,
            observation_sequence: self.observation_sequence,
            identity_generation: self.generation,
            runtime_enabled: self.runtime_enabled,
            hardware_observed: true,
            simulated: false,
            performance_prediction: false,
        }
    }

    pub(crate) fn handle(&mut self, request: HardwareDebugRequestV2) -> HardwareDebugResponseV2 {
        let request_id = request.request_id();
        let operation = request.operation();
        if self.terminated {
            return self.error(
                request_id,
                operation,
                (
                    HardwareErrorStageV2::Session,
                    HardwareErrorCodeV2::SessionTerminated,
                ),
                HardwareEffectV2::None,
                false,
                "hardware debug session is terminated",
            );
        }
        if self.poisoned {
            return self.error(
                request_id,
                operation,
                (
                    HardwareErrorStageV2::Session,
                    HardwareErrorCodeV2::SessionPoisoned,
                ),
                HardwareEffectV2::None,
                true,
                "hardware debug session is terminally poisoned",
            );
        }
        if self.commands_processed == MAX_HARDWARE_SESSION_COMMANDS_V2 {
            self.poisoned = true;
            return self.error(
                request_id,
                operation,
                (
                    HardwareErrorStageV2::Session,
                    HardwareErrorCodeV2::ResourceLimit,
                ),
                HardwareEffectV2::None,
                true,
                "hardware debugger command limit is exhausted",
            );
        }
        self.commands_processed += 1;
        if request.expected_control_revision() == self.control_revision
            && let HardwareDebugRequestV2::QueryHardwareExceptionEvents { page, .. } = &request
            && page.after_sequence <= self.observation_sequence
            && self.events.front().is_none_or(|first| {
                page.after_sequence == 0 || page.after_sequence.saturating_add(1) >= first.sequence
            })
        {
            while self
                .events
                .front()
                .is_some_and(|event| event.sequence <= page.after_sequence)
            {
                self.events.pop_front();
            }
        }
        if let Err(error) = self.pump_events_once() {
            return self.transport_error(request_id, operation, HardwareErrorStageV2::Event, error);
        }
        if request.expected_control_revision() != self.control_revision {
            return self.error(
                request_id,
                operation,
                (
                    HardwareErrorStageV2::Session,
                    HardwareErrorCodeV2::StaleControlRevision,
                ),
                HardwareEffectV2::None,
                false,
                "expected_control_revision is stale",
            );
        }
        match request {
            HardwareDebugRequestV2::DiscoverCapabilities { .. } => self.ok(
                request_id,
                operation,
                HardwareDebugResultV2::Capabilities {
                    capabilities: hardware_capabilities_v2(),
                },
            ),
            HardwareDebugRequestV2::GetState { .. } => {
                self.ok(request_id, operation, HardwareDebugResultV2::State)
            }
            HardwareDebugRequestV2::InspectHardwareDevices { page, .. } => {
                self.inspect_devices(request_id, operation, page)
            }
            HardwareDebugRequestV2::InspectHardwareQueues { page, .. } => {
                self.inspect_queues(request_id, operation, page)
            }
            HardwareDebugRequestV2::QueryHardwareExceptionEvents { page, .. } => {
                self.query_events(request_id, operation, page)
            }
            HardwareDebugRequestV2::SuspendQueues {
                queues,
                grace_period,
                ..
            } => self.control_queues(request_id, operation, &queues, Some(grace_period)),
            HardwareDebugRequestV2::ResumeQueues { queues, .. } => {
                self.control_queues(request_id, operation, &queues, None)
            }
            HardwareDebugRequestV2::Terminate { .. } => {
                if self.bump_control_revision().is_err() {
                    return self.resource_exhausted(request_id, operation);
                }
                self.terminated = true;
                self.ok(request_id, operation, HardwareDebugResultV2::Terminated)
            }
        }
    }

    fn inspect_devices(
        &mut self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        page: HardwarePageRequestV2,
    ) -> HardwareDebugResponseV2 {
        if let Err(error) = self.refresh_identity() {
            return self.transport_error(
                request_id,
                operation,
                HardwareErrorStageV2::Snapshot,
                error,
            );
        }
        if page.expected_generation != 0 && page.expected_generation != self.generation {
            return self.stale_generation(request_id, operation);
        }
        let (range, next_start) = page_range(self.devices.len(), page.start, page.limit);
        let items = self.devices[range]
            .iter()
            .map(|record| device_view(*record))
            .collect();
        self.ok(
            request_id,
            operation,
            HardwareDebugResultV2::Devices {
                generation: self.generation,
                items,
                next_start,
            },
        )
    }

    fn inspect_queues(
        &mut self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        page: HardwarePageRequestV2,
    ) -> HardwareDebugResponseV2 {
        if let Err(error) = self.refresh_identity() {
            return self.transport_error(
                request_id,
                operation,
                HardwareErrorStageV2::Snapshot,
                error,
            );
        }
        if page.expected_generation != 0 && page.expected_generation != self.generation {
            return self.stale_generation(request_id, operation);
        }
        let (range, next_start) = page_range(self.queues.len(), page.start, page.limit);
        let items = self.queues[range]
            .iter()
            .map(|record| HardwareQueueViewV2 {
                id: record.logical,
                device: record.device,
                ring_bytes: record.native.ring_bytes,
                queue_type: record.native.queue_type,
                context_save_area_bytes: record.native.context_save_area_bytes,
                suspended_by_session: self.suspended_native.contains(&record.native.queue_id),
            })
            .collect();
        self.ok(
            request_id,
            operation,
            HardwareDebugResultV2::Queues {
                generation: self.generation,
                items,
                next_start,
            },
        )
    }

    fn query_events(
        &mut self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        page: HardwareEventPageRequestV2,
    ) -> HardwareDebugResponseV2 {
        if page.after_sequence > self.observation_sequence {
            return self.error(
                request_id,
                operation,
                (
                    HardwareErrorStageV2::Event,
                    HardwareErrorCodeV2::StaleEventCursor,
                ),
                HardwareEffectV2::None,
                false,
                "event cursor exceeds the latest observation",
            );
        }
        let deadline = Instant::now() + Duration::from_millis(u64::from(page.wait_milliseconds));
        loop {
            if self
                .events
                .iter()
                .any(|event| event.sequence > page.after_sequence)
                || Instant::now() >= deadline
            {
                break;
            }
            if let Err(error) = self.pump_events_once() {
                return self.transport_error(
                    request_id,
                    operation,
                    HardwareErrorStageV2::Event,
                    error,
                );
            }
            if page.wait_milliseconds == 0 {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        if let Some(first) = self.events.front()
            && page.after_sequence != 0
            && page.after_sequence.saturating_add(1) < first.sequence
        {
            return self.error(
                request_id,
                operation,
                (
                    HardwareErrorStageV2::Event,
                    HardwareErrorCodeV2::StaleEventCursor,
                ),
                HardwareEffectV2::None,
                false,
                "event cursor predates retained observations",
            );
        }
        while self
            .events
            .front()
            .is_some_and(|event| event.sequence <= page.after_sequence)
        {
            self.events.pop_front();
        }
        let items: Vec<_> = self
            .events
            .iter()
            .filter(|event| event.sequence > page.after_sequence)
            .take(usize::from(page.limit))
            .copied()
            .collect();
        let next_after_sequence = items
            .last()
            .map_or(page.after_sequence, |event| event.sequence);
        self.ok(
            request_id,
            operation,
            HardwareDebugResultV2::Events {
                items,
                next_after_sequence,
            },
        )
    }

    fn control_queues(
        &mut self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        queues: &[HardwareQueueIdV2],
        suspend_grace: Option<u32>,
    ) -> HardwareDebugResponseV2 {
        if !self.runtime_enabled {
            return self.unavailable(
                request_id,
                operation,
                if suspend_grace.is_some() {
                    HardwareCapabilityNameV2::QueueSuspend
                } else {
                    HardwareCapabilityNameV2::QueueResume
                },
                HardwareUnavailableReasonV2::RuntimeNotEnabled,
                "target KFD runtime is not enabled",
            );
        }
        if let Err(error) = self.refresh_identity() {
            return self.transport_error(
                request_id,
                operation,
                HardwareErrorStageV2::Snapshot,
                error,
            );
        }
        if queues
            .iter()
            .any(|queue| queue.generation != self.generation)
        {
            return self.stale_generation(request_id, operation);
        }
        let by_logical: BTreeMap<_, _> = self
            .queues
            .iter()
            .map(|record| (record.logical, record.native.queue_id))
            .collect();
        let Some(native): Option<Vec<u32>> = queues
            .iter()
            .map(|queue| by_logical.get(queue).copied())
            .collect()
        else {
            return self.error(
                request_id,
                operation,
                (
                    HardwareErrorStageV2::Control,
                    HardwareErrorCodeV2::UnknownLogicalId,
                ),
                HardwareEffectV2::None,
                false,
                "queue identity is not live in this generation",
            );
        };
        let device_capability_absent = queues.iter().any(|logical| {
            self.queues
                .iter()
                .find(|record| record.logical == *logical)
                .and_then(|queue| {
                    self.devices
                        .iter()
                        .find(|device| device.logical == queue.device)
                })
                .is_none_or(|device| !device.native.trap_debug_supported)
        });
        if device_capability_absent {
            return self.unavailable(
                request_id,
                operation,
                if suspend_grace.is_some() {
                    HardwareCapabilityNameV2::QueueSuspend
                } else {
                    HardwareCapabilityNameV2::QueueResume
                },
                HardwareUnavailableReasonV2::DeviceCapabilityAbsent,
                "target device does not report KFD trap-debug support",
            );
        }
        if (suspend_grace.is_none()
            && native
                .iter()
                .any(|queue| !self.suspended_native.contains(queue)))
            || (suspend_grace.is_some()
                && native
                    .iter()
                    .any(|queue| self.suspended_native.contains(queue)))
        {
            return self.error(
                request_id,
                operation,
                (
                    HardwareErrorStageV2::Control,
                    HardwareErrorCodeV2::InvalidRequest,
                ),
                HardwareEffectV2::None,
                false,
                "queue control conflicts with session-owned suspend state",
            );
        }
        let operation_result = if let Some(grace) = suspend_grace {
            self.transport.suspend_queues(&native, grace)
        } else {
            self.transport.resume_queues(&native)
        };
        let observations = match operation_result {
            Ok(observations) => observations,
            Err(error) => {
                let _ = self.bump_control_revision();
                return self.transport_error(
                    request_id,
                    operation,
                    HardwareErrorStageV2::Control,
                    error,
                );
            }
        };
        if self.bump_control_revision().is_err() {
            self.poisoned = true;
            return self.resource_exhausted(request_id, operation);
        }
        let logical_by_native: BTreeMap<_, _> = self
            .queues
            .iter()
            .map(|record| (record.native.queue_id, record.logical))
            .collect();
        let mut observations_by_native = BTreeMap::new();
        let exact_identity = observations.len() == native.len()
            && observations.iter().all(|observation| {
                native.contains(&observation.queue_id)
                    && observations_by_native
                        .insert(observation.queue_id, observation.state)
                        .is_none()
            })
            && observations_by_native.len() == native.len();
        if !exact_identity {
            self.poisoned = true;
            return self.error(
                request_id,
                operation,
                (
                    HardwareErrorStageV2::Control,
                    HardwareErrorCodeV2::BackendFailure,
                ),
                HardwareEffectV2::Indeterminate,
                true,
                "queue control result identity was inconsistent",
            );
        }
        let mut complete = 0_usize;
        let outcomes = native
            .iter()
            .map(|queue_id| {
                let state = match observations_by_native[queue_id] {
                    KfdDebugQueueOperationStateV1::Complete => {
                        complete += 1;
                        HardwareQueueControlStateV2::Complete
                    }
                    KfdDebugQueueOperationStateV1::HardwareError => {
                        HardwareQueueControlStateV2::HardwareError
                    }
                    KfdDebugQueueOperationStateV1::Invalid => HardwareQueueControlStateV2::Invalid,
                };
                HardwareQueueControlResultV2 {
                    queue: logical_by_native[queue_id],
                    state,
                }
            })
            .collect::<Vec<_>>();
        for queue_id in native.iter().copied().filter(|queue_id| {
            observations_by_native[queue_id] == KfdDebugQueueOperationStateV1::Complete
        }) {
            if suspend_grace.is_some() {
                self.suspended_native.insert(queue_id);
            } else {
                self.suspended_native.remove(&queue_id);
            }
        }
        let effect = if complete == outcomes.len() {
            HardwareEffectV2::Committed
        } else if complete == 0 {
            HardwareEffectV2::None
        } else {
            HardwareEffectV2::Partial
        };
        self.ok(
            request_id,
            operation,
            HardwareDebugResultV2::QueueControl { outcomes, effect },
        )
    }

    fn refresh_identity(&mut self) -> Result<(), HardwareTransportErrorV2> {
        let mut devices = self.transport.device_snapshot()?;
        let mut queues = self.transport.queue_snapshot()?;
        devices.sort_by_key(|device| device.gpu_id);
        queues.sort_by_key(|queue| (queue.gpu_id, queue.queue_id));
        let mut native_queue_ids = BTreeSet::new();
        if devices
            .windows(2)
            .any(|pair| pair[0].gpu_id == pair[1].gpu_id)
            || queues
                .iter()
                .any(|queue| !native_queue_ids.insert(queue.queue_id))
        {
            return Err(HardwareTransportErrorV2 {
                effect: HardwareEffectV2::None,
                operation: "KFD snapshot contained duplicate native identity",
            });
        }
        let changed = self.initialized_identity
            && (devices
                != self
                    .devices
                    .iter()
                    .map(|record| record.native)
                    .collect::<Vec<_>>()
                || queues
                    != self
                        .queues
                        .iter()
                        .map(|record| record.native)
                        .collect::<Vec<_>>());
        let generation = if changed {
            self.generation
                .checked_add(1)
                .ok_or(HardwareTransportErrorV2 {
                    effect: HardwareEffectV2::None,
                    operation: "logical identity generation exhausted",
                })?
        } else {
            self.generation
        };
        let device_ids: BTreeMap<_, _> = devices
            .iter()
            .enumerate()
            .map(|(index, device)| {
                (
                    device.gpu_id,
                    HardwareDeviceIdV2 {
                        generation,
                        ordinal: u32::try_from(index + 1).expect("bounded KFD device snapshot"),
                    },
                )
            })
            .collect();
        let admitted_devices = devices
            .into_iter()
            .map(|native| DeviceRecordV2 {
                logical: device_ids[&native.gpu_id],
                native,
            })
            .collect();
        let admitted_queues =
            queues
                .iter()
                .copied()
                .enumerate()
                .map(|(index, native)| {
                    let device = device_ids.get(&native.gpu_id).copied().ok_or(
                        HardwareTransportErrorV2 {
                            effect: HardwareEffectV2::None,
                            operation: "queue snapshot referenced an unknown device",
                        },
                    )?;
                    Ok(QueueRecordV2 {
                        native,
                        logical: HardwareQueueIdV2 {
                            generation,
                            ordinal: u32::try_from(index + 1).expect("bounded KFD queue snapshot"),
                        },
                        device,
                    })
                })
                .collect::<Result<_, _>>()?;
        let mut suspended_native = self.suspended_native.clone();
        if changed {
            suspended_native.retain(|queue| queues.iter().any(|entry| entry.queue_id == *queue));
        }
        self.generation = generation;
        self.initialized_identity = true;
        self.devices = admitted_devices;
        self.queues = admitted_queues;
        self.suspended_native = suspended_native;
        Ok(())
    }

    fn pump_events_once(&mut self) -> Result<(), HardwareTransportErrorV2> {
        self.transport.drain_notifications()?;
        while let Some(event) = self.transport.query_event()? {
            for code in EXCEPTION_CODES_V2 {
                if !event.exceptions.contains(code) {
                    continue;
                }
                if code == KfdDebugTrapExceptionCodeV1::ProcessRuntime {
                    let info = self.transport.query_exception_info(0, code, true)?;
                    let NativeExceptionInfoV2::Runtime(runtime) = info else {
                        return Err(HardwareTransportErrorV2 {
                            effect: HardwareEffectV2::Partial,
                            operation: "runtime event payload kind",
                        });
                    };
                    let state = match runtime {
                        KfdDebugRuntimeStateV1::Enabled => HardwareRuntimeStateV2::Enabled,
                        KfdDebugRuntimeStateV1::Disabled => HardwareRuntimeStateV2::Disabled,
                        _ => {
                            return Err(HardwareTransportErrorV2 {
                                effect: HardwareEffectV2::Partial,
                                operation: "runtime event state",
                            });
                        }
                    };
                    self.runtime_enabled = state == HardwareRuntimeStateV2::Enabled;
                    if !self.runtime_enabled {
                        self.suspended_native.clear();
                        self.invalidate_queue_identity()?;
                    }
                    self.push_event(HardwareEventPayloadV2::RuntimeTransition { state })?;
                    self.transport.acknowledge_runtime_transition(event)?;
                } else {
                    let source_id = event_source_id(code, event);
                    let _cleared = self.transport.query_exception_info(source_id, code, true)?;
                    self.push_event(HardwareEventPayloadV2::Exception {
                        exception: map_exception_kind(code).expect("runtime handled separately"),
                        scope: self.event_scope(event),
                    })?;
                }
            }
        }
        Ok(())
    }

    fn invalidate_queue_identity(&mut self) -> Result<(), HardwareTransportErrorV2> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(HardwareTransportErrorV2 {
                effect: HardwareEffectV2::Partial,
                operation: "logical identity generation exhausted",
            })?;
        self.initialized_identity = false;
        self.queues.clear();
        self.devices.clear();
        Ok(())
    }

    fn event_scope(&self, event: NativeEventV2) -> HardwareEventScopeV2 {
        let device = self
            .devices
            .iter()
            .find(|record| record.native.gpu_id == event.gpu_id)
            .map(|record| record.logical);
        let queue = self
            .queues
            .iter()
            .find(|record| {
                record.native.queue_id == event.queue_id && record.native.gpu_id == event.gpu_id
            })
            .map(|record| record.logical);
        match (device, queue) {
            (Some(device), Some(queue)) => HardwareEventScopeV2::Queue { device, queue },
            (Some(device), None) => HardwareEventScopeV2::Device { device },
            (None, None) if event.gpu_id == 0 && event.queue_id == 0 => {
                HardwareEventScopeV2::Process
            }
            _ => HardwareEventScopeV2::UnresolvedNativeSource,
        }
    }

    fn push_event(
        &mut self,
        payload: HardwareEventPayloadV2,
    ) -> Result<(), HardwareTransportErrorV2> {
        if self.events.len() == MAX_HARDWARE_RETAINED_EVENTS_V2 {
            return Err(HardwareTransportErrorV2 {
                effect: HardwareEffectV2::Partial,
                operation: "hardware event retention exhausted",
            });
        }
        self.observation_sequence =
            self.observation_sequence
                .checked_add(1)
                .ok_or(HardwareTransportErrorV2 {
                    effect: HardwareEffectV2::Partial,
                    operation: "hardware event sequence exhausted",
                })?;
        self.events.push_back(HardwareEventViewV2 {
            sequence: self.observation_sequence,
            identity_generation: self.generation,
            payload,
        });
        Ok(())
    }

    fn bump_control_revision(&mut self) -> Result<(), ()> {
        self.control_revision = self.control_revision.checked_add(1).ok_or(())?;
        Ok(())
    }

    fn ok(
        &self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        result: HardwareDebugResultV2,
    ) -> HardwareDebugResponseV2 {
        HardwareDebugResponseV2::Ok {
            schema: HardwareResponseSchemaV2::V2,
            request_id,
            operation,
            session: self.session_view(),
            result,
        }
    }

    fn unavailable(
        &self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        capability: HardwareCapabilityNameV2,
        reason: HardwareUnavailableReasonV2,
        detail: &str,
    ) -> HardwareDebugResponseV2 {
        HardwareDebugResponseV2::Unavailable {
            schema: HardwareResponseSchemaV2::V2,
            request_id,
            operation,
            session: self.session_view(),
            capability,
            reason,
            detail: bounded(detail),
        }
    }

    fn stale_generation(
        &self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
    ) -> HardwareDebugResponseV2 {
        self.error(
            request_id,
            operation,
            (
                HardwareErrorStageV2::Session,
                HardwareErrorCodeV2::StaleIdentityGeneration,
            ),
            HardwareEffectV2::None,
            false,
            "logical identity generation is stale",
        )
    }

    fn resource_exhausted(
        &mut self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
    ) -> HardwareDebugResponseV2 {
        self.poisoned = true;
        self.error(
            request_id,
            operation,
            (
                HardwareErrorStageV2::Session,
                HardwareErrorCodeV2::ResourceLimit,
            ),
            HardwareEffectV2::Indeterminate,
            true,
            "hardware debugger bounded state is exhausted",
        )
    }

    fn transport_error(
        &mut self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        stage: HardwareErrorStageV2,
        error: HardwareTransportErrorV2,
    ) -> HardwareDebugResponseV2 {
        let terminal = error.effect != HardwareEffectV2::None;
        self.poisoned |= terminal;
        self.error(
            request_id,
            operation,
            (stage, HardwareErrorCodeV2::BackendFailure),
            error.effect,
            terminal,
            error.operation,
        )
    }

    fn error(
        &self,
        request_id: u64,
        operation: HardwareDebugOperationV2,
        classification: (HardwareErrorStageV2, HardwareErrorCodeV2),
        effect: HardwareEffectV2,
        terminal: bool,
        message: &str,
    ) -> HardwareDebugResponseV2 {
        let (stage, code) = classification;
        HardwareDebugResponseV2::Error {
            schema: HardwareResponseSchemaV2::V2,
            request_id,
            operation,
            session: self.session_view(),
            error: HardwareDebugErrorV2 {
                stage,
                code,
                effect,
                terminal,
                message: bounded(message),
            },
        }
    }
}

fn page_range(total: usize, start: u32, limit: u16) -> (std::ops::Range<usize>, u32) {
    let start = usize::try_from(start).unwrap_or(usize::MAX).min(total);
    let end = start.saturating_add(usize::from(limit)).min(total);
    let next = if end < total {
        u32::try_from(end).expect("bounded snapshot index")
    } else {
        0
    };
    (start..end, next)
}

fn device_view(record: DeviceRecordV2) -> HardwareDeviceViewV2 {
    HardwareDeviceViewV2 {
        id: record.logical,
        gfx_target_version: record.native.gfx_target_version,
        xcc_count: record.native.xcc_count,
        trap_debug_supported: record.native.trap_debug_supported,
        debug_firmware_supported: record.native.debug_firmware_supported,
        launch_mode_supported: record.native.launch_mode_supported,
        launch_override_supported: record.native.launch_override_supported,
        precise_memory_supported: record.native.precise_memory_supported,
        precise_alu_supported: record.native.precise_alu_supported,
    }
}

fn event_source_id(code: KfdDebugTrapExceptionCodeV1, event: NativeEventV2) -> u32 {
    if (code as u32) < KfdDebugTrapExceptionCodeV1::DeviceQueueDelete as u32 {
        event.queue_id
    } else {
        event.gpu_id
    }
}

fn map_exception_kind(code: KfdDebugTrapExceptionCodeV1) -> Option<HardwareExceptionKindV2> {
    Some(match code {
        KfdDebugTrapExceptionCodeV1::QueueWaveAbort => HardwareExceptionKindV2::QueueWaveAbort,
        KfdDebugTrapExceptionCodeV1::QueueWaveTrap => HardwareExceptionKindV2::QueueWaveTrap,
        KfdDebugTrapExceptionCodeV1::QueueWaveMathError => {
            HardwareExceptionKindV2::QueueWaveMathError
        }
        KfdDebugTrapExceptionCodeV1::QueueWaveIllegalInstruction => {
            HardwareExceptionKindV2::QueueWaveIllegalInstruction
        }
        KfdDebugTrapExceptionCodeV1::QueueWaveMemoryViolation => {
            HardwareExceptionKindV2::QueueWaveMemoryViolation
        }
        KfdDebugTrapExceptionCodeV1::QueueWaveApertureViolation => {
            HardwareExceptionKindV2::QueueWaveApertureViolation
        }
        KfdDebugTrapExceptionCodeV1::QueuePacketDispatchDimensionsInvalid => {
            HardwareExceptionKindV2::QueuePacketDispatchDimensionsInvalid
        }
        KfdDebugTrapExceptionCodeV1::QueuePacketDispatchGroupSegmentSizeInvalid => {
            HardwareExceptionKindV2::QueuePacketDispatchGroupSegmentSizeInvalid
        }
        KfdDebugTrapExceptionCodeV1::QueuePacketDispatchCodeInvalid => {
            HardwareExceptionKindV2::QueuePacketDispatchCodeInvalid
        }
        KfdDebugTrapExceptionCodeV1::QueuePacketReserved => {
            HardwareExceptionKindV2::QueuePacketReserved
        }
        KfdDebugTrapExceptionCodeV1::QueuePacketUnsupported => {
            HardwareExceptionKindV2::QueuePacketUnsupported
        }
        KfdDebugTrapExceptionCodeV1::QueuePacketDispatchWorkgroupSizeInvalid => {
            HardwareExceptionKindV2::QueuePacketDispatchWorkgroupSizeInvalid
        }
        KfdDebugTrapExceptionCodeV1::QueuePacketDispatchRegisterInvalid => {
            HardwareExceptionKindV2::QueuePacketDispatchRegisterInvalid
        }
        KfdDebugTrapExceptionCodeV1::QueuePacketVendorUnsupported => {
            HardwareExceptionKindV2::QueuePacketVendorUnsupported
        }
        KfdDebugTrapExceptionCodeV1::QueuePreemptionError => {
            HardwareExceptionKindV2::QueuePreemptionError
        }
        KfdDebugTrapExceptionCodeV1::QueueNew => HardwareExceptionKindV2::QueueNew,
        KfdDebugTrapExceptionCodeV1::DeviceQueueDelete => {
            HardwareExceptionKindV2::DeviceQueueDelete
        }
        KfdDebugTrapExceptionCodeV1::DeviceMemoryViolation => {
            HardwareExceptionKindV2::DeviceMemoryViolation
        }
        KfdDebugTrapExceptionCodeV1::DeviceRasError => HardwareExceptionKindV2::DeviceRasError,
        KfdDebugTrapExceptionCodeV1::DeviceFatalHalt => HardwareExceptionKindV2::DeviceFatalHalt,
        KfdDebugTrapExceptionCodeV1::DeviceNew => HardwareExceptionKindV2::DeviceNew,
        KfdDebugTrapExceptionCodeV1::ProcessDeviceRemove => {
            HardwareExceptionKindV2::ProcessDeviceRemove
        }
        KfdDebugTrapExceptionCodeV1::ProcessRuntime => return None,
    })
}

fn hardware_capabilities_v2() -> Vec<HardwareCapabilityV2> {
    use HardwareCapabilityAvailabilityV2::{Available, Unavailable};
    use HardwareCapabilityNameV2::*;
    [
        (HardwareDeviceSnapshot, Available),
        (HardwareQueueSnapshot, Available),
        (HardwareExceptionEvents, Available),
        (QueueSuspend, Available),
        (QueueResume, Available),
        (Terminate, Available),
        (WaveState, Unavailable),
        (LaneState, Unavailable),
        (RegisterValues, Unavailable),
        (CwsrDecode, Unavailable),
        (CallStack, Unavailable),
        (SourceSites, Unavailable),
        (KirSites, Unavailable),
        (Step, Unavailable),
        (Replay, Unavailable),
        (Breakpoints, Unavailable),
        (Values, Unavailable),
        (TargetMemory, Unavailable),
        (SemanticTrace, Unavailable),
        (AddressWatch, Unavailable),
        (DispatchSubmission, Unavailable),
    ]
    .into_iter()
    .map(|(name, availability)| HardwareCapabilityV2 { name, availability })
    .collect()
}

fn bounded(message: &str) -> String {
    if message.len() <= MAX_HARDWARE_ERROR_MESSAGE_BYTES_V2 {
        message.to_owned()
    } else {
        let boundary = message
            .char_indices()
            .map(|(index, _)| index)
            .take_while(|index| *index <= MAX_HARDWARE_ERROR_MESSAGE_BYTES_V2)
            .last()
            .unwrap_or(0);
        message[..boundary].to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct ScriptedTransportV2 {
        devices: Vec<NativeDeviceV2>,
        queues: Vec<NativeQueueV2>,
        events: VecDeque<NativeEventV2>,
        exception_info: VecDeque<NativeExceptionInfoV2>,
        suspend_result: Option<Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2>>,
        resume_result: Option<Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2>>,
        runtime_acknowledgements: usize,
        suspend_calls: usize,
        resume_calls: usize,
    }

    impl HardwareDebugTransportV2 for ScriptedTransportV2 {
        fn device_snapshot(&mut self) -> Result<Vec<NativeDeviceV2>, HardwareTransportErrorV2> {
            Ok(self.devices.clone())
        }

        fn queue_snapshot(&mut self) -> Result<Vec<NativeQueueV2>, HardwareTransportErrorV2> {
            Ok(self.queues.clone())
        }

        fn drain_notifications(&mut self) -> Result<(), HardwareTransportErrorV2> {
            Ok(())
        }

        fn query_event(&mut self) -> Result<Option<NativeEventV2>, HardwareTransportErrorV2> {
            Ok(self.events.pop_front())
        }

        fn query_exception_info(
            &mut self,
            _source_id: u32,
            _code: KfdDebugTrapExceptionCodeV1,
            _clear: bool,
        ) -> Result<NativeExceptionInfoV2, HardwareTransportErrorV2> {
            Ok(self
                .exception_info
                .pop_front()
                .unwrap_or(NativeExceptionInfoV2::Other))
        }

        fn acknowledge_runtime_transition(
            &mut self,
            _event: NativeEventV2,
        ) -> Result<(), HardwareTransportErrorV2> {
            self.runtime_acknowledgements += 1;
            Ok(())
        }

        fn suspend_queues(
            &mut self,
            queues: &[u32],
            _grace_period: u32,
        ) -> Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2> {
            self.suspend_calls += 1;
            self.suspend_result.take().unwrap_or_else(|| {
                Ok(queues
                    .iter()
                    .map(|queue_id| NativeQueueOutcomeV2 {
                        queue_id: *queue_id,
                        state: KfdDebugQueueOperationStateV1::Complete,
                    })
                    .collect())
            })
        }

        fn resume_queues(
            &mut self,
            queues: &[u32],
        ) -> Result<Vec<NativeQueueOutcomeV2>, HardwareTransportErrorV2> {
            self.resume_calls += 1;
            self.resume_result.take().unwrap_or_else(|| {
                Ok(queues
                    .iter()
                    .map(|queue_id| NativeQueueOutcomeV2 {
                        queue_id: *queue_id,
                        state: KfdDebugQueueOperationStateV1::Complete,
                    })
                    .collect())
            })
        }
    }

    fn device(gpu_id: u32) -> NativeDeviceV2 {
        NativeDeviceV2 {
            gpu_id,
            gfx_target_version: 94_200,
            xcc_count: 8,
            trap_debug_supported: true,
            debug_firmware_supported: true,
            launch_mode_supported: true,
            launch_override_supported: true,
            precise_memory_supported: true,
            precise_alu_supported: true,
        }
    }

    fn queue(gpu_id: u32, queue_id: u32) -> NativeQueueV2 {
        NativeQueueV2 {
            queue_id,
            gpu_id,
            ring_bytes: 4096,
            queue_type: 0,
            context_save_area_bytes: 0,
        }
    }

    fn runtime_enabled_transport(queues: Vec<NativeQueueV2>) -> ScriptedTransportV2 {
        ScriptedTransportV2 {
            devices: vec![device(17)],
            queues,
            events: VecDeque::from([NativeEventV2 {
                exceptions: KfdDebugExceptionMaskV1::from_code(
                    KfdDebugTrapExceptionCodeV1::ProcessRuntime,
                ),
                gpu_id: 0,
                queue_id: 0,
            }]),
            exception_info: VecDeque::from([NativeExceptionInfoV2::Runtime(
                KfdDebugRuntimeStateV1::Enabled,
            )]),
            ..ScriptedTransportV2::default()
        }
    }

    fn inspect_queues(
        backend: &mut HardwareBackendV2<ScriptedTransportV2>,
        expected_generation: u64,
    ) -> HardwareDebugResponseV2 {
        backend.handle(HardwareDebugRequestV2::InspectHardwareQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 1,
            expected_control_revision: 0,
            page: HardwarePageRequestV2 {
                expected_generation,
                start: 0,
                limit: 16,
            },
        })
    }

    fn queue_ids(response: &HardwareDebugResponseV2) -> Vec<HardwareQueueIdV2> {
        let HardwareDebugResponseV2::Ok {
            result: HardwareDebugResultV2::Queues { items, .. },
            ..
        } = response
        else {
            panic!("expected queue page: {response:?}");
        };
        items.iter().map(|item| item.id).collect()
    }

    #[test]
    fn snapshots_are_paged_and_stale_generations_fail_closed() {
        let mut backend = HardwareBackendV2::new(ScriptedTransportV2 {
            devices: vec![device(17)],
            queues: vec![queue(17, 90), queue(17, 91)],
            ..ScriptedTransportV2::default()
        });
        let first = backend.handle(HardwareDebugRequestV2::InspectHardwareQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 1,
            expected_control_revision: 0,
            page: HardwarePageRequestV2 {
                expected_generation: 0,
                start: 0,
                limit: 1,
            },
        });
        let ids = queue_ids(&first);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].generation, 1);
        assert!(matches!(
            first,
            HardwareDebugResponseV2::Ok {
                result: HardwareDebugResultV2::Queues { next_start: 1, .. },
                ..
            }
        ));
        let second = backend.handle(HardwareDebugRequestV2::InspectHardwareQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 2,
            expected_control_revision: 0,
            page: HardwarePageRequestV2 {
                expected_generation: 1,
                start: 1,
                limit: 1,
            },
        });
        assert_eq!(queue_ids(&second).len(), 1);

        backend.transport.queues[0].ring_bytes = 8192;
        let stale = inspect_queues(&mut backend, 1);
        assert!(matches!(
            stale,
            HardwareDebugResponseV2::Error {
                error: HardwareDebugErrorV2 {
                    code: HardwareErrorCodeV2::StaleIdentityGeneration,
                    effect: HardwareEffectV2::None,
                    terminal: false,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn runtime_event_is_observed_then_auto_acknowledged_without_revision_change() {
        let mut backend = HardwareBackendV2::new(runtime_enabled_transport(vec![queue(17, 90)]));
        backend.pump_async_observations().unwrap();
        assert_eq!(backend.transport.runtime_acknowledgements, 1);
        assert_eq!(backend.control_revision, 0);
        assert_eq!(backend.observation_sequence, 1);
        let page = inspect_queues(&mut backend, 0);
        let id = queue_ids(&page)[0];
        assert_eq!(backend.transport.runtime_acknowledgements, 1);
        assert_eq!(backend.control_revision, 0);
        assert_eq!(backend.observation_sequence, 1);

        let suspended = backend.handle(HardwareDebugRequestV2::SuspendQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 2,
            expected_control_revision: 0,
            queues: vec![id],
            grace_period: 0,
        });
        assert!(matches!(
            suspended,
            HardwareDebugResponseV2::Ok {
                session: HardwareSessionViewV2 {
                    control_revision: 1,
                    observation_sequence: 1,
                    ..
                },
                result: HardwareDebugResultV2::QueueControl {
                    effect: HardwareEffectV2::Committed,
                    ..
                },
                ..
            }
        ));
        assert!(backend.suspended_native.contains(&90));
    }

    #[test]
    fn malformed_queue_result_identity_poisoning_does_not_mutate_suspend_ownership() {
        let cases = [
            vec![
                NativeQueueOutcomeV2 {
                    queue_id: 90,
                    state: KfdDebugQueueOperationStateV1::Complete,
                },
                NativeQueueOutcomeV2 {
                    queue_id: 90,
                    state: KfdDebugQueueOperationStateV1::Complete,
                },
            ],
            vec![NativeQueueOutcomeV2 {
                queue_id: 90,
                state: KfdDebugQueueOperationStateV1::Complete,
            }],
            vec![
                NativeQueueOutcomeV2 {
                    queue_id: 90,
                    state: KfdDebugQueueOperationStateV1::Complete,
                },
                NativeQueueOutcomeV2 {
                    queue_id: 999,
                    state: KfdDebugQueueOperationStateV1::Complete,
                },
            ],
        ];
        for observations in cases {
            let mut backend = HardwareBackendV2::new(runtime_enabled_transport(vec![
                queue(17, 90),
                queue(17, 91),
            ]));
            let ids = queue_ids(&inspect_queues(&mut backend, 0));
            backend.transport.suspend_result = Some(Ok(observations));
            let response = backend.handle(HardwareDebugRequestV2::SuspendQueues {
                schema: HardwareRequestSchemaV2::V2,
                request_id: 2,
                expected_control_revision: 0,
                queues: ids,
                grace_period: 0,
            });
            assert!(matches!(
                response,
                HardwareDebugResponseV2::Error {
                    error: HardwareDebugErrorV2 {
                        effect: HardwareEffectV2::Indeterminate,
                        terminal: true,
                        ..
                    },
                    ..
                }
            ));
            assert!(backend.suspended_native.is_empty());
        }
    }

    #[test]
    fn known_partial_control_effect_is_nonterminal_and_tracks_only_completed_queues() {
        let mut backend = HardwareBackendV2::new(runtime_enabled_transport(vec![
            queue(17, 90),
            queue(17, 91),
        ]));
        let ids = queue_ids(&inspect_queues(&mut backend, 0));
        backend.transport.suspend_result = Some(Ok(vec![
            NativeQueueOutcomeV2 {
                queue_id: 90,
                state: KfdDebugQueueOperationStateV1::Complete,
            },
            NativeQueueOutcomeV2 {
                queue_id: 91,
                state: KfdDebugQueueOperationStateV1::HardwareError,
            },
        ]));
        let response = backend.handle(HardwareDebugRequestV2::SuspendQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 2,
            expected_control_revision: 0,
            queues: ids,
            grace_period: 0,
        });
        assert!(matches!(
            response,
            HardwareDebugResponseV2::Ok {
                result: HardwareDebugResultV2::QueueControl {
                    effect: HardwareEffectV2::Partial,
                    ..
                },
                ..
            }
        ));
        assert_eq!(backend.suspended_native, BTreeSet::from([90]));
        assert!(!backend.poisoned);
    }

    #[test]
    fn indeterminate_ioctl_error_advances_revision_and_poison_session() {
        let mut backend = HardwareBackendV2::new(runtime_enabled_transport(vec![queue(17, 90)]));
        let id = queue_ids(&inspect_queues(&mut backend, 0))[0];
        backend.transport.suspend_result = Some(Err(HardwareTransportErrorV2 {
            effect: HardwareEffectV2::Indeterminate,
            operation: "injected suspend failure",
        }));
        let response = backend.handle(HardwareDebugRequestV2::SuspendQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 2,
            expected_control_revision: 0,
            queues: vec![id],
            grace_period: 0,
        });
        assert!(matches!(
            response,
            HardwareDebugResponseV2::Error {
                session: HardwareSessionViewV2 {
                    state: HardwareSessionStateV2::Poisoned,
                    control_revision: 1,
                    ..
                },
                error: HardwareDebugErrorV2 {
                    effect: HardwareEffectV2::Indeterminate,
                    terminal: true,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn resume_without_owned_suspend_is_rejected_before_ioctl() {
        let mut backend = HardwareBackendV2::new(runtime_enabled_transport(vec![queue(17, 90)]));
        let id = queue_ids(&inspect_queues(&mut backend, 0))[0];
        let response = backend.handle(HardwareDebugRequestV2::ResumeQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 2,
            expected_control_revision: 0,
            queues: vec![id],
        });
        assert!(matches!(
            response,
            HardwareDebugResponseV2::Error {
                error: HardwareDebugErrorV2 {
                    code: HardwareErrorCodeV2::InvalidRequest,
                    effect: HardwareEffectV2::None,
                    ..
                },
                ..
            }
        ));
        assert_eq!(backend.transport.resume_calls, 0);
        assert_eq!(backend.control_revision, 0);
    }

    #[test]
    fn future_event_cursor_is_rejected_without_waiting_or_mutation() {
        let mut backend = HardwareBackendV2::new(ScriptedTransportV2::default());
        let response = backend.handle(HardwareDebugRequestV2::QueryHardwareExceptionEvents {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 1,
            expected_control_revision: 0,
            page: HardwareEventPageRequestV2 {
                after_sequence: 1,
                limit: 1,
                wait_milliseconds: MAX_HARDWARE_EVENT_WAIT_MILLISECONDS_V2,
            },
        });
        assert!(matches!(
            response,
            HardwareDebugResponseV2::Error {
                error: HardwareDebugErrorV2 {
                    code: HardwareErrorCodeV2::StaleEventCursor,
                    effect: HardwareEffectV2::None,
                    terminal: false,
                    ..
                },
                ..
            }
        ));
        assert_eq!(backend.control_revision, 0);
        assert_eq!(backend.observation_sequence, 0);
    }

    #[test]
    fn rejected_snapshot_never_partially_commits_identity_generation() {
        let mut backend = HardwareBackendV2::new(ScriptedTransportV2 {
            devices: vec![device(17)],
            queues: vec![queue(17, 90)],
            ..ScriptedTransportV2::default()
        });
        assert_eq!(queue_ids(&inspect_queues(&mut backend, 0)).len(), 1);
        let original_devices = backend.devices.clone();
        let original_queues = backend.queues.clone();
        backend.transport.queues = vec![queue(999, 91)];

        let rejected = inspect_queues(&mut backend, 0);
        assert!(matches!(
            rejected,
            HardwareDebugResponseV2::Error {
                error: HardwareDebugErrorV2 {
                    stage: HardwareErrorStageV2::Snapshot,
                    effect: HardwareEffectV2::None,
                    terminal: false,
                    ..
                },
                ..
            }
        ));
        assert_eq!(backend.generation, 1);
        assert_eq!(backend.devices, original_devices);
        assert_eq!(backend.queues, original_queues);

        backend.transport.devices = vec![device(17), device(17)];
        backend.transport.queues = vec![queue(17, 90)];
        let duplicate = inspect_queues(&mut backend, 0);
        assert!(matches!(
            duplicate,
            HardwareDebugResponseV2::Error {
                error: HardwareDebugErrorV2 {
                    effect: HardwareEffectV2::None,
                    terminal: false,
                    ..
                },
                ..
            }
        ));
        assert_eq!(backend.generation, 1);
        assert_eq!(backend.devices, original_devices);
        assert_eq!(backend.queues, original_queues);
    }

    #[test]
    fn session_command_bound_poisoning_has_no_hardware_effect() {
        let mut backend = HardwareBackendV2::new(ScriptedTransportV2::default());
        backend.commands_processed = MAX_HARDWARE_SESSION_COMMANDS_V2;
        let response = backend.handle(HardwareDebugRequestV2::GetState {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 1,
            expected_control_revision: 0,
        });
        assert!(matches!(
            response,
            HardwareDebugResponseV2::Error {
                session: HardwareSessionViewV2 {
                    state: HardwareSessionStateV2::Poisoned,
                    commands_processed: MAX_HARDWARE_SESSION_COMMANDS_V2,
                    ..
                },
                error: HardwareDebugErrorV2 {
                    code: HardwareErrorCodeV2::ResourceLimit,
                    effect: HardwareEffectV2::None,
                    terminal: true,
                    ..
                },
                ..
            }
        ));
        assert_eq!(backend.control_revision, 0);
        assert_eq!(backend.observation_sequence, 0);
    }

    #[test]
    fn runtime_disable_event_invalidates_all_logical_and_suspend_state() {
        let mut backend = HardwareBackendV2::new(runtime_enabled_transport(vec![queue(17, 90)]));
        let id = queue_ids(&inspect_queues(&mut backend, 0))[0];
        let suspended = backend.handle(HardwareDebugRequestV2::SuspendQueues {
            schema: HardwareRequestSchemaV2::V2,
            request_id: 2,
            expected_control_revision: 0,
            queues: vec![id],
            grace_period: 0,
        });
        assert!(matches!(suspended, HardwareDebugResponseV2::Ok { .. }));
        backend.transport.events.push_back(NativeEventV2 {
            exceptions: KfdDebugExceptionMaskV1::from_code(
                KfdDebugTrapExceptionCodeV1::ProcessRuntime,
            ),
            gpu_id: 0,
            queue_id: 0,
        });
        backend
            .transport
            .exception_info
            .push_back(NativeExceptionInfoV2::Runtime(
                KfdDebugRuntimeStateV1::Disabled,
            ));
        backend.pump_async_observations().unwrap();

        assert!(!backend.runtime_enabled);
        assert!(backend.suspended_native.is_empty());
        assert!(backend.devices.is_empty());
        assert!(backend.queues.is_empty());
        assert_eq!(backend.generation, 2);
        assert_eq!(backend.observation_sequence, 2);
        assert_eq!(backend.transport.runtime_acknowledgements, 2);
    }
}
