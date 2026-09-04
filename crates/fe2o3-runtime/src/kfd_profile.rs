//! Opt-in, bounded observation state for the direct-KFD backend.

use fe2o3_profiler_protocol::{
    KfdProfileDeviceV1, KfdProfileHostContentModeV1, KfdProfileHostContentV1,
    KfdProfileResourceKindV1, KfdRuntimeProfileEventKindV1, KfdRuntimeProfileEventV1,
    KfdRuntimeProfileV1, MAX_KFD_RUNTIME_PROFILE_BYTES_V1, MAX_KFD_RUNTIME_PROFILE_EVENTS_V1,
    MAX_KFD_RUNTIME_PROFILE_FIXED_JSON_BYTES_V1, NativeRuntimeDispatchTimestampRecorderV1,
    ProfileContentIdentityV1, ProfileIdentityV1, encode_kfd_runtime_profile_v1,
    push_observed_event_with_encoded_len_v1, resource_identity_v1,
};

use crate::AuthenticatedKfdRuntimeDispatchTimestampsV1;

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
    dispatch_timestamps: NativeRuntimeDispatchTimestampRecorderV1,
    encoded_event_bytes: u64,
    dropped_events: u64,
}

impl KfdRuntimeProfileRecorderV1 {
    pub(crate) fn new(
        config: KfdRuntimeProfilerConfigV1,
        device_unique_id: u64,
        target_profile: &str,
        wave_width: u16,
    ) -> Result<Self, String> {
        let device = KfdProfileDeviceV1::observed(device_unique_id, target_profile, wave_width)
            .map_err(|error| error.to_string())?;
        let max_events = config.max_events as usize;
        let mut events = Vec::new();
        events
            .try_reserve_exact(max_events)
            .map_err(|_| "direct-KFD profiler event reservation failed".to_owned())?;
        let dispatch_timestamps =
            NativeRuntimeDispatchTimestampRecorderV1::new(config.capture_scope, max_events)
                .map_err(|error| error.to_string())?;
        Ok(Self {
            scope: config.capture_scope,
            device,
            host_content_mode: config.host_content_mode,
            max_events,
            events,
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

    /// Records a fact without changing runtime success or failure. Once any
    /// fact is lost, later facts are counted but omitted so the retained data
    /// remains a valid prefix rather than a misleading trace with holes.
    pub(crate) fn observe(&mut self, event: Option<KfdRuntimeProfileEventKindV1>) {
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
            .map(|(runtime, _)| runtime)
    }

    pub(crate) fn finish_with_dispatch_timestamps(
        self,
    ) -> Result<AuthenticatedKfdRuntimeDispatchTimestampsV1, String> {
        let (runtime, timestamp_output) = self.finish_runtime_and_timestamps()?;
        let timestamp_output = timestamp_output
            .finish(&runtime)
            .map_err(|error| error.to_string())?;
        AuthenticatedKfdRuntimeDispatchTimestampsV1::new(runtime, timestamp_output)
    }

    fn finish_runtime_and_timestamps(
        self,
    ) -> Result<
        (
            KfdRuntimeProfileV1,
            NativeRuntimeDispatchTimestampRecorderV1,
        ),
        String,
    > {
        let capture = KfdRuntimeProfileV1::new(
            self.scope,
            self.device,
            self.host_content_mode,
            self.events,
            self.dropped_events,
        )
        .map_err(|error| error.to_string())?;
        encode_kfd_runtime_profile_v1(&capture).map_err(|error| error.to_string())?;
        Ok((capture, self.dispatch_timestamps))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_profiler_protocol::{
        KfdProfileAccessV1, KfdProfileBindingV1, KfdProfileHostTimingV1, KfdProfileLaunchV1,
        KfdProfileMemoryKindV1, MAX_KFD_RUNTIME_PROFILE_BINDINGS_V1,
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
}
