//! Opt-in, bounded observation state for the direct-KFD backend.

use fe2o3_profiler_protocol::{
    KfdProfileDeviceV1, KfdProfileHostContentModeV1, KfdProfileHostContentV1,
    KfdProfileResourceKindV1, KfdProfileSemanticContractV1, KfdRuntimeProfileEventKindV1,
    KfdRuntimeProfileEventV1, KfdRuntimeProfileV1, KfdRuntimeSemanticObservationV1,
    KfdRuntimeSemanticProfileV1, MAX_KFD_RUNTIME_PROFILE_BYTES_V1,
    MAX_KFD_RUNTIME_PROFILE_EVENTS_V1, MAX_KFD_RUNTIME_PROFILE_FIXED_JSON_BYTES_V1,
    NativeRuntimeDispatchTimestampRecorderV1, ProfileContentIdentityV1, ProfileIdentityV1,
    encode_kfd_runtime_profile_v1, push_observed_event_with_encoded_len_v1, resource_identity_v1,
};

use crate::{
    AuthenticatedKfdRuntimeDispatchTimestampsV1, AuthenticatedKfdRuntimeDispatchTimestampsV2,
};

/// Explicit direct-KFD runtime profiling configuration.
///
/// `capture_scope` is a caller-generated, nonzero, per-execution identity. It
/// namespaces opaque resource identities without exposing runtime handles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeProfilerConfigV1 {
    capture_scope: ProfileIdentityV1,
    max_events: u32,
    host_content_mode: KfdProfileHostContentModeV1,
}

impl KfdRuntimeProfilerConfigV1 {
    pub fn new(
        capture_scope: [u8; 32],
        max_events: u32,
    ) -> Result<Self, KfdRuntimeProfilerConfigErrorV1> {
        if max_events == 0 || max_events > MAX_KFD_RUNTIME_PROFILE_EVENTS_V1 {
            return Err(KfdRuntimeProfilerConfigErrorV1::EventLimitOutOfRange);
        }
        let capture_scope = ProfileIdentityV1::new(capture_scope)
            .map_err(|_| KfdRuntimeProfilerConfigErrorV1::ZeroCaptureScope)?;
        Ok(Self {
            capture_scope,
            max_events,
            host_content_mode: KfdProfileHostContentModeV1::RangeOnly,
        })
    }

    /// Requests content identities for host staging records. This can be
    /// expensive for large reads or partial writes; range-only observation is
    /// the default and always remains explicit in the capture.
    pub fn with_host_content_identities(mut self) -> Self {
        self.host_content_mode = KfdProfileHostContentModeV1::ContentIdentity;
        self
    }

    pub const fn capture_scope(self) -> ProfileIdentityV1 {
        self.capture_scope
    }

    pub const fn max_events(self) -> u32 {
        self.max_events
    }

    pub const fn host_content_mode(self) -> KfdProfileHostContentModeV1 {
        self.host_content_mode
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdRuntimeProfilerConfigErrorV1 {
    ZeroCaptureScope,
    EventLimitOutOfRange,
}

impl std::fmt::Display for KfdRuntimeProfilerConfigErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid direct-KFD profiler configuration: {self:?}"
        )
    }
}

impl std::error::Error for KfdRuntimeProfilerConfigErrorV1 {}

#[derive(Debug)]
pub(crate) struct KfdRuntimeProfileRecorderV1 {
    scope: ProfileIdentityV1,
    device: KfdProfileDeviceV1,
    host_content_mode: KfdProfileHostContentModeV1,
    max_events: usize,
    events: Vec<KfdRuntimeProfileEventV1>,
    event_semantics: Option<Vec<Option<KfdProfileSemanticContractV1>>>,
    dispatch_timestamps: NativeRuntimeDispatchTimestampRecorderV1,
    encoded_event_bytes: u64,
    dropped_events: u64,
}

type FinishedKfdRuntimeProfileRecorderV1 = (
    KfdRuntimeProfileV1,
    NativeRuntimeDispatchTimestampRecorderV1,
    Option<Vec<Option<KfdProfileSemanticContractV1>>>,
);

impl KfdRuntimeProfileRecorderV1 {
    pub(crate) fn new(
        config: KfdRuntimeProfilerConfigV1,
        device_unique_id: u64,
        target_profile: &str,
        wave_width: u16,
    ) -> Result<Self, String> {
        Self::new_inner(config, device_unique_id, target_profile, wave_width, false)
    }

    pub(crate) fn new_with_semantic_profile(
        config: KfdRuntimeProfilerConfigV1,
        device_unique_id: u64,
        target_profile: &str,
        wave_width: u16,
    ) -> Result<Self, String> {
        Self::new_inner(config, device_unique_id, target_profile, wave_width, true)
    }

    fn new_inner(
        config: KfdRuntimeProfilerConfigV1,
        device_unique_id: u64,
        target_profile: &str,
        wave_width: u16,
        semantic_profile: bool,
    ) -> Result<Self, String> {
        let device = KfdProfileDeviceV1::observed(device_unique_id, target_profile, wave_width)
            .map_err(|error| error.to_string())?;
        let max_events = config.max_events as usize;
        let mut events = Vec::new();
        events
            .try_reserve_exact(max_events)
            .map_err(|_| "direct-KFD profiler event reservation failed".to_owned())?;
        let event_semantics = if semantic_profile {
            let mut semantics = Vec::new();
            semantics
                .try_reserve_exact(max_events)
                .map_err(|_| "direct-KFD profiler semantic reservation failed".to_owned())?;
            Some(semantics)
        } else {
            None
        };
        let dispatch_timestamps =
            NativeRuntimeDispatchTimestampRecorderV1::new(config.capture_scope, max_events)
                .map_err(|error| error.to_string())?;
        Ok(Self {
            scope: config.capture_scope,
            device,
            host_content_mode: config.host_content_mode,
            max_events,
            events,
            event_semantics,
            dispatch_timestamps,
            encoded_event_bytes: 0,
            dropped_events: 0,
        })
    }

    pub(crate) fn resource(
        &self,
        kind: KfdProfileResourceKindV1,
        private_runtime_handle: u64,
    ) -> Option<ProfileIdentityV1> {
        resource_identity_v1(self.scope, kind, private_runtime_handle).ok()
    }

    pub(crate) fn host_content(
        &self,
        bytes: &[u8],
        known_sha256: Option<[u8; 32]>,
    ) -> Option<KfdProfileHostContentV1> {
        let byte_len = u64::try_from(bytes.len()).ok()?;
        match self.host_content_mode {
            KfdProfileHostContentModeV1::RangeOnly => {
                Some(KfdProfileHostContentV1::RangeOnly { byte_len })
            }
            KfdProfileHostContentModeV1::ContentIdentity => {
                let content = match known_sha256 {
                    Some(sha256) => ProfileContentIdentityV1::observed_sha256(byte_len, sha256),
                    None => ProfileContentIdentityV1::observed(bytes),
                }
                .ok()?;
                Some(KfdProfileHostContentV1::ContentIdentity { content })
            }
        }
    }

    pub(crate) const fn captures_semantic_profile(&self) -> bool {
        self.event_semantics.is_some()
    }

    /// Records a fact without changing runtime success or failure. Once any
    /// fact is lost, later facts are counted but omitted so the retained data
    /// remains a valid prefix rather than a misleading trace with holes.
    pub(crate) fn observe(&mut self, event: Option<KfdRuntimeProfileEventKindV1>) {
        self.observe_with_semantic_contract(event, None);
    }

    pub(crate) fn observe_dispatch(
        &mut self,
        event: Option<KfdRuntimeProfileEventKindV1>,
        semantic_contract: Option<KfdProfileSemanticContractV1>,
    ) {
        self.observe_with_semantic_contract(event, semantic_contract);
    }

    fn observe_with_semantic_contract(
        &mut self,
        event: Option<KfdRuntimeProfileEventKindV1>,
        semantic_contract: Option<KfdProfileSemanticContractV1>,
    ) {
        if semantic_contract.is_some()
            && !matches!(
                &event,
                Some(KfdRuntimeProfileEventKindV1::DispatchPublished { .. })
            )
        {
            if let Some(sample) = event
                .as_ref()
                .and_then(|event| self.dispatch_timestamps.sample(event))
            {
                self.dispatch_timestamps.discard(sample);
            }
            self.dropped_events = self.dropped_events.saturating_add(1);
            return;
        }
        let timestamp_sample = event
            .as_ref()
            .and_then(|event| self.dispatch_timestamps.sample(event));
        if self.dropped_events != 0 || self.events.len() >= self.max_events {
            if let Some(sample) = timestamp_sample {
                self.dispatch_timestamps.discard(sample);
            }
            self.dropped_events = self.dropped_events.saturating_add(1);
            return;
        }
        let Some(event) = event else {
            self.dropped_events = self.dropped_events.saturating_add(1);
            return;
        };
        let event_len =
            match push_observed_event_with_encoded_len_v1(self.scope, &mut self.events, event) {
                Ok(len) => len,
                Err(_) => {
                    if let Some(sample) = timestamp_sample {
                        self.dispatch_timestamps.discard(sample);
                    }
                    self.dropped_events = self.dropped_events.saturating_add(1);
                    return;
                }
            };
        let encoded_len = Some(event_len)
            .and_then(|len| len.checked_add(1))
            .and_then(|len| self.encoded_event_bytes.checked_add(len));
        let encoded_budget = MAX_KFD_RUNTIME_PROFILE_BYTES_V1
            .saturating_sub(MAX_KFD_RUNTIME_PROFILE_FIXED_JSON_BYTES_V1);
        match encoded_len.filter(|len| *len <= encoded_budget) {
            Some(len) => {
                self.encoded_event_bytes = len;
                if let Some(event_semantics) = self.event_semantics.as_mut() {
                    event_semantics.push(semantic_contract);
                }
                if let Some(sample) = timestamp_sample {
                    self.dispatch_timestamps
                        .commit(sample, self.events.last().expect("event was just retained"));
                }
            }
            None => {
                self.events.pop();
                if let Some(sample) = timestamp_sample {
                    self.dispatch_timestamps.discard(sample);
                }
                self.dropped_events = self.dropped_events.saturating_add(1);
            }
        }
    }

    pub(crate) fn finish(self) -> Result<KfdRuntimeProfileV1, String> {
        self.finish_runtime_and_timestamps()
            .map(|(runtime, _, _)| runtime)
    }

    pub(crate) fn finish_with_semantic_profile(
        self,
    ) -> Result<KfdRuntimeProfileWithSemanticSidecarV1, String> {
        let (runtime_profile, _, event_semantics) = self.finish_runtime_and_timestamps()?;
        let semantic_profile = build_semantic_profile_v1(&runtime_profile, event_semantics)?;
        Ok(KfdRuntimeProfileWithSemanticSidecarV1 {
            runtime_profile,
            semantic_profile,
        })
    }

    pub(crate) fn finish_with_dispatch_timestamps(
        self,
    ) -> Result<AuthenticatedKfdRuntimeDispatchTimestampsV1, String> {
        let (runtime, timestamp_output, _) = self.finish_runtime_and_timestamps()?;
        let timestamp_output = timestamp_output
            .finish(&runtime)
            .map_err(|error| error.to_string())?;
        AuthenticatedKfdRuntimeDispatchTimestampsV1::new(runtime, timestamp_output)
    }

    pub(crate) fn finish_with_dispatch_timestamps_v2(
        self,
    ) -> Result<AuthenticatedKfdRuntimeDispatchTimestampsV2, String> {
        let (runtime, timestamp_output, event_semantics) = self.finish_runtime_and_timestamps()?;
        let semantic_profile = build_semantic_profile_v1(&runtime, event_semantics)?;
        let timestamp_output = timestamp_output
            .finish(&runtime)
            .map_err(|error| error.to_string())?;
        AuthenticatedKfdRuntimeDispatchTimestampsV2::new(
            runtime,
            timestamp_output,
            semantic_profile,
        )
    }

    fn finish_runtime_and_timestamps(self) -> Result<FinishedKfdRuntimeProfileRecorderV1, String> {
        if self
            .event_semantics
            .as_ref()
            .is_some_and(|semantics| self.events.len() != semantics.len())
        {
            return Err("direct-KFD profiler semantic/event alignment failed".to_owned());
        }
        let capture = KfdRuntimeProfileV1::new(
            self.scope,
            self.device,
            self.host_content_mode,
            self.events,
            self.dropped_events,
        )
        .map_err(|error| error.to_string())?;
        encode_kfd_runtime_profile_v1(&capture).map_err(|error| error.to_string())?;
        Ok((capture, self.dispatch_timestamps, self.event_semantics))
    }
}

fn build_semantic_profile_v1(
    capture: &KfdRuntimeProfileV1,
    event_semantics: Option<Vec<Option<KfdProfileSemanticContractV1>>>,
) -> Result<KfdRuntimeSemanticProfileV1, String> {
    let event_semantics = event_semantics
        .ok_or_else(|| "semantic profiling was not enabled for this capture".to_owned())?;
    if capture.events.len() != event_semantics.len() {
        return Err("direct-KFD profiler semantic/event alignment failed".to_owned());
    }
    let mut observations = Vec::new();
    observations
        .try_reserve_exact(
            capture
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event.event,
                        KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
                    )
                })
                .count(),
        )
        .map_err(|_| "direct-KFD semantic sidecar allocation failed".to_owned())?;
    for (event, semantic_contract) in capture.events.iter().zip(event_semantics) {
        match &event.event {
            KfdRuntimeProfileEventKindV1::DispatchPublished { dispatch, .. } => {
                observations.push(KfdRuntimeSemanticObservationV1 {
                    dispatch: *dispatch,
                    semantic_contract,
                });
            }
            _ if semantic_contract.is_some() => {
                return Err(
                    "direct-KFD semantic contract was not aligned to a publication".to_owned(),
                );
            }
            _ => {}
        }
    }
    KfdRuntimeSemanticProfileV1::new(capture, observations).map_err(|error| error.to_string())
}

/// Frozen Runtime Profile V1 plus its separately versioned semantic sidecar.
///
/// Both captures are authority-free observations. The sidecar is content-bound
/// to the exact V1 profile and cannot authorize a launch or prove machine
/// behavior.
#[derive(Debug)]
pub struct KfdRuntimeProfileWithSemanticSidecarV1 {
    runtime_profile: KfdRuntimeProfileV1,
    semantic_profile: KfdRuntimeSemanticProfileV1,
}

impl KfdRuntimeProfileWithSemanticSidecarV1 {
    pub const fn runtime_profile(&self) -> &KfdRuntimeProfileV1 {
        &self.runtime_profile
    }

    pub const fn semantic_profile(&self) -> &KfdRuntimeSemanticProfileV1 {
        &self.semantic_profile
    }

    pub fn into_parts(self) -> (KfdRuntimeProfileV1, KfdRuntimeSemanticProfileV1) {
        (self.runtime_profile, self.semantic_profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_profiler_protocol::{
        KfdProfileAccessV1, KfdProfileAtomicContractV1, KfdProfileAtomicOperationV1,
        KfdProfileBindingV1, KfdProfileHostTimingV1, KfdProfileLaunchV1, KfdProfileMemoryKindV1,
        KfdProfileMemoryOrderV1, KfdProfileMemoryScopeV1, KfdProfileSemanticContractV1,
        MAX_KFD_RUNTIME_PROFILE_BINDINGS_V1,
    };

    #[test]
    fn maximal_binding_events_freeze_before_the_wire_byte_limit() {
        let config =
            KfdRuntimeProfilerConfigV1::new([3; 32], MAX_KFD_RUNTIME_PROFILE_EVENTS_V1).unwrap();
        let mut recorder =
            KfdRuntimeProfileRecorderV1::new(config, 7, "future_target:xnack+", 32).unwrap();
        let queue = recorder
            .resource(KfdProfileResourceKindV1::NativeQueue, 1)
            .unwrap();
        let stream = recorder
            .resource(KfdProfileResourceKindV1::Stream, 2)
            .unwrap();
        let module = recorder
            .resource(KfdProfileResourceKindV1::Module, 3)
            .unwrap();
        let kernel = recorder
            .resource(KfdProfileResourceKindV1::Kernel, 4)
            .unwrap();
        recorder.observe(Some(KfdRuntimeProfileEventKindV1::NativeQueueCreated {
            queue,
        }));
        recorder.observe(Some(KfdRuntimeProfileEventKindV1::StreamCreated { stream }));
        recorder.observe(Some(KfdRuntimeProfileEventKindV1::ModuleLoaded {
            module,
            artifact: fe2o3_profiler_protocol::ProfileContentIdentityV1::observed(&[1]).unwrap(),
        }));
        recorder.observe(Some(KfdRuntimeProfileEventKindV1::KernelResolved {
            kernel,
            module,
            name: fe2o3_profiler_protocol::ProfileContentIdentityV1::observed(b"kernel").unwrap(),
            signature: fe2o3_profiler_protocol::ProfileContentIdentityV1::observed(&[2; 32])
                .unwrap(),
        }));
        let mut bindings = Vec::new();
        for handle in 0..MAX_KFD_RUNTIME_PROFILE_BINDINGS_V1 as u64 {
            let allocation = recorder
                .resource(KfdProfileResourceKindV1::Allocation, 100 + handle)
                .unwrap();
            recorder.observe(Some(KfdRuntimeProfileEventKindV1::AllocationCreated {
                allocation,
                memory_kind: KfdProfileMemoryKindV1::HostVisible,
                byte_len: 1,
                alignment: 1,
            }));
            bindings.push(KfdProfileBindingV1 {
                allocation,
                access: KfdProfileAccessV1::Read,
                byte_offset: 0,
                byte_len: 1,
                kernarg_byte_offset: handle as u32 * 8,
            });
        }
        for handle in 1_000..10_000 {
            let dispatch = recorder
                .resource(KfdProfileResourceKindV1::Dispatch, handle)
                .unwrap();
            recorder.observe(Some(KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                queue,
                stream,
                kernel,
                dispatch_shape: fe2o3_profiler_protocol::ProfileContentIdentityV1::observed(
                    &[5; 32],
                )
                .unwrap(),
                launch: KfdProfileLaunchV1 {
                    grid: [1, 1, 1],
                    workgroup: [1, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                bindings: bindings.clone(),
            }));
            recorder.observe(Some(KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: KfdProfileHostTimingV1::default(),
            }));
            recorder.observe(Some(KfdRuntimeProfileEventKindV1::SubmissionReleased {
                dispatch,
            }));
            if recorder.dropped_events != 0 {
                break;
            }
        }
        assert_ne!(recorder.dropped_events, 0);
        let retained = recorder.events.len();
        recorder.observe(Some(KfdRuntimeProfileEventKindV1::StreamDestroyed {
            stream,
        }));
        assert_eq!(recorder.events.len(), retained);
        let capture = recorder.finish().unwrap();
        let bytes = encode_kfd_runtime_profile_v1(&capture).unwrap();
        assert!(bytes.len() as u64 <= MAX_KFD_RUNTIME_PROFILE_BYTES_V1);
        assert!(!capture.coverage.complete_runtime_operation_history);
    }

    #[test]
    fn native_timestamp_bundle_keeps_exact_retained_event_identity() {
        let config = KfdRuntimeProfilerConfigV1::new([23; 32], 32).unwrap();
        let mut recorder =
            KfdRuntimeProfileRecorderV1::new(config, 7, "gfx942:xnack-", 64).unwrap();
        let queue = recorder
            .resource(KfdProfileResourceKindV1::NativeQueue, 1)
            .unwrap();
        let stream = recorder
            .resource(KfdProfileResourceKindV1::Stream, 2)
            .unwrap();
        let module = recorder
            .resource(KfdProfileResourceKindV1::Module, 3)
            .unwrap();
        let kernel = recorder
            .resource(KfdProfileResourceKindV1::Kernel, 4)
            .unwrap();
        let dispatch = recorder
            .resource(KfdProfileResourceKindV1::Dispatch, 5)
            .unwrap();
        for event in [
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
            KfdRuntimeProfileEventKindV1::StreamCreated { stream },
            KfdRuntimeProfileEventKindV1::ModuleLoaded {
                module,
                artifact: ProfileContentIdentityV1::observed(b"object").unwrap(),
            },
            KfdRuntimeProfileEventKindV1::KernelResolved {
                kernel,
                module,
                name: ProfileContentIdentityV1::observed(b"kernel").unwrap(),
                signature: ProfileContentIdentityV1::observed(b"signature").unwrap(),
            },
            KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                queue,
                stream,
                kernel,
                dispatch_shape: ProfileContentIdentityV1::observed(b"shape").unwrap(),
                launch: KfdProfileLaunchV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                bindings: Vec::new(),
            },
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: KfdProfileHostTimingV1::default(),
            },
            KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch },
            KfdRuntimeProfileEventKindV1::ModuleUnloaded { module },
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        ] {
            recorder.observe(Some(event));
        }
        let bundle = recorder.finish_with_dispatch_timestamps().unwrap();
        let timestamps = bundle.dispatch_timestamps();
        assert!(timestamps.coverage().complete_runtime_operation_history);
        assert_eq!(timestamps.records().len(), 1);
        assert_eq!(
            timestamps.records()[0].publication().runtime_event,
            bundle.runtime_profile().events[4].identity
        );
        assert_eq!(
            timestamps.records()[0].completion().unwrap().runtime_event,
            bundle.runtime_profile().events[5].identity
        );
    }

    #[test]
    fn frozen_v1_recorder_does_not_allocate_or_require_semantic_sidecar_state() {
        let config = KfdRuntimeProfilerConfigV1::new([24; 32], 8).unwrap();
        let recorder = KfdRuntimeProfileRecorderV1::new(config, 7, "gfx942:xnack-", 64).unwrap();
        assert!(recorder.event_semantics.is_none());
        recorder.finish().unwrap();
    }

    #[test]
    fn semantic_recorder_rejects_contract_beside_non_publication_event() {
        let config = KfdRuntimeProfilerConfigV1::new([25; 32], 8).unwrap();
        let mut recorder =
            KfdRuntimeProfileRecorderV1::new_with_semantic_profile(config, 7, "gfx942:xnack-", 64)
                .unwrap();
        let stream = recorder
            .resource(KfdProfileResourceKindV1::Stream, 1)
            .unwrap();
        let contract = KfdProfileSemanticContractV1::Atomic(KfdProfileAtomicContractV1 {
            operation: KfdProfileAtomicOperationV1::Add,
            scope: KfdProfileMemoryScopeV1::Workgroup,
            order: KfdProfileMemoryOrderV1::Relaxed,
            failure_order: None,
            weak: false,
            geometry: KfdProfileLaunchV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
        });
        recorder.observe_dispatch(
            Some(KfdRuntimeProfileEventKindV1::StreamCreated { stream }),
            Some(contract),
        );
        assert!(recorder.events.is_empty());
        assert_eq!(recorder.dropped_events, 1);
        let captures = recorder.finish_with_semantic_profile().unwrap();
        assert!(captures.semantic_profile().records().is_empty());
        assert!(
            !captures
                .semantic_profile()
                .coverage()
                .complete_runtime_operation_history
        );
    }

    #[test]
    fn semantic_recorder_binds_contract_to_retained_dispatch_publication() {
        let config = KfdRuntimeProfilerConfigV1::new([26; 32], 16).unwrap();
        let mut recorder =
            KfdRuntimeProfileRecorderV1::new_with_semantic_profile(config, 7, "gfx942:xnack-", 64)
                .unwrap();
        let queue = recorder
            .resource(KfdProfileResourceKindV1::NativeQueue, 1)
            .unwrap();
        let stream = recorder
            .resource(KfdProfileResourceKindV1::Stream, 2)
            .unwrap();
        let module = recorder
            .resource(KfdProfileResourceKindV1::Module, 3)
            .unwrap();
        let kernel = recorder
            .resource(KfdProfileResourceKindV1::Kernel, 4)
            .unwrap();
        let dispatch = recorder
            .resource(KfdProfileResourceKindV1::Dispatch, 5)
            .unwrap();
        for event in [
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
            KfdRuntimeProfileEventKindV1::StreamCreated { stream },
            KfdRuntimeProfileEventKindV1::ModuleLoaded {
                module,
                artifact: ProfileContentIdentityV1::observed(b"object").unwrap(),
            },
            KfdRuntimeProfileEventKindV1::KernelResolved {
                kernel,
                module,
                name: ProfileContentIdentityV1::observed(b"kernel").unwrap(),
                signature: ProfileContentIdentityV1::observed(b"signature").unwrap(),
            },
        ] {
            recorder.observe(Some(event));
        }
        let launch = KfdProfileLaunchV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        };
        let contract = KfdProfileSemanticContractV1::Atomic(KfdProfileAtomicContractV1 {
            operation: KfdProfileAtomicOperationV1::CompareExchange,
            scope: KfdProfileMemoryScopeV1::Device,
            order: KfdProfileMemoryOrderV1::AcquireRelease,
            failure_order: Some(KfdProfileMemoryOrderV1::Acquire),
            weak: true,
            geometry: launch,
        });
        recorder.observe_dispatch(
            Some(KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                queue,
                stream,
                kernel,
                dispatch_shape: ProfileContentIdentityV1::observed(b"shape").unwrap(),
                launch,
                bindings: Vec::new(),
            }),
            Some(contract),
        );
        for event in [
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: KfdProfileHostTimingV1::default(),
            },
            KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch },
            KfdRuntimeProfileEventKindV1::ModuleUnloaded { module },
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        ] {
            recorder.observe(Some(event));
        }
        let captures = recorder.finish_with_semantic_profile().unwrap();
        assert_eq!(captures.semantic_profile().records().len(), 1);
        assert_eq!(
            captures.semantic_profile().records()[0].semantic_contract(),
            Some(contract)
        );
        assert_eq!(
            captures.semantic_profile().records()[0].runtime_event(),
            captures.runtime_profile().events[4].identity
        );
    }
}
