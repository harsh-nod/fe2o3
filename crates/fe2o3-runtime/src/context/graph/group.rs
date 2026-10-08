//! Closed descriptive roster created and revalidated by the original Context.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeGraphDeviceCoverageV1 {
    Selected,
    AllAdmitted,
}

/// A bounded group of original streams, not native or data-version authority.
/// Graph admission revalidates every retained coordinate before reservation.
/// AllAdmitted means this Context's admitted roster, never all host GPUs.
///
/// ```compile_fail
/// let group = fe2o3_runtime::RuntimeGraphGroupV1 { generation: 1 };
/// ```
#[derive(Debug)]
pub struct RuntimeGraphGroupV1 {
    generation: u64,
    identity: ContextIdentityV1,
    coverage: RuntimeGraphDeviceCoverageV1,
    devices: Vec<RuntimeDeviceIdV1>,
    streams: Vec<(RuntimeStreamIdV1, RuntimeDeviceIdV1, u64)>,
}

impl RuntimeGraphGroupV1 {
    pub fn context_identity(&self) -> ContextIdentityV1 {
        self.identity
    }

    pub fn devices(&self) -> &[RuntimeDeviceIdV1] {
        &self.devices
    }

    pub fn coverage(&self) -> RuntimeGraphDeviceCoverageV1 {
        self.coverage
    }

    pub fn stream_identity(
        &self,
        stream: RuntimeStreamIdV1,
    ) -> Result<StreamIdentityV1, RuntimeValidationErrorV1> {
        if self
            .streams
            .binary_search_by_key(&stream, |entry| entry.0)
            .is_err()
        {
            return Err(RuntimeValidationErrorV1::UnknownStream);
        }
        Ok(StreamIdentityV1::new(
            self.identity,
            identity_bytes(b"fe2o3.group.str1", self.generation, stream.local),
        ))
    }

    pub(crate) fn bindings(&self) -> Vec<(StreamIdentityV1, RuntimeStreamIdV1)> {
        self.streams
            .iter()
            .map(|entry| {
                (
                    StreamIdentityV1::new(
                        self.identity,
                        identity_bytes(b"fe2o3.group.str1", self.generation, entry.0.local),
                    ),
                    entry.0,
                )
            })
            .collect()
    }

    pub(crate) fn contains_device(&self, device: RuntimeDeviceIdV1) -> bool {
        self.devices.binary_search(&device).is_ok()
    }

    pub(crate) fn revalidate<B: RuntimeBackendV1>(
        &self,
        context: &RuntimeContextV1<B>,
    ) -> Result<(), RuntimeValidationErrorV1> {
        context.require_live()?;
        if self.generation != context.context_generation {
            return Err(RuntimeValidationErrorV1::WrongDevice);
        }
        for &(stream, device, backend_stream) in &self.streams {
            let original = context.unheld_stream_v1(stream)?;
            if original.device != device || original.backend_stream != backend_stream {
                return Err(RuntimeValidationErrorV1::UnknownStream);
            }
            context.device(device)?;
        }
        if self.coverage == RuntimeGraphDeviceCoverageV1::AllAdmitted
            && (context.devices.len() != self.devices.len()
                || context
                    .devices
                    .iter()
                    .any(|device| !self.contains_device(device.id())))
        {
            return Err(RuntimeValidationErrorV1::WrongDevice);
        }
        Ok(())
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Creates an exact 2..=256-device roster from 2..=256 original streams.
    /// It allocates only bounded host metadata and burns a descriptive Context
    /// identity. It creates no queues, reservations, copy facts or GPU owners.
    pub fn create_graph_group_v1(
        &mut self,
        streams: &[RuntimeStreamIdV1],
        coverage: RuntimeGraphDeviceCoverageV1,
    ) -> Result<RuntimeGraphGroupV1, RuntimeValidationErrorV1> {
        self.require_live()?;
        if !(2..=crate::MAX_RUNTIME_GRAPH_NODES_V1).contains(&streams.len()) {
            return Err(RuntimeValidationErrorV1::Capacity);
        }
        let mut originals = Vec::new();
        originals
            .try_reserve_exact(streams.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        for &stream in streams {
            let record = self.unheld_stream_v1(stream)?;
            originals.push((stream, record.device, record.backend_stream));
        }
        originals.sort_unstable_by_key(|entry| entry.0);
        if originals.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(RuntimeValidationErrorV1::UnknownStream);
        }
        let mut devices = Vec::new();
        devices
            .try_reserve_exact(originals.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        devices.extend(originals.iter().map(|entry| entry.1));
        devices.sort_unstable();
        devices.dedup();
        if !(2..=MAX_RUNTIME_DEVICES_V1).contains(&devices.len())
            || (coverage == RuntimeGraphDeviceCoverageV1::AllAdmitted
                && (devices.len() != self.devices.len()
                    || self
                        .devices
                        .iter()
                        .any(|device| devices.binary_search(&device.id()).is_err())))
        {
            return Err(RuntimeValidationErrorV1::WrongDevice);
        }
        let local = self.next_id()?;
        Ok(RuntimeGraphGroupV1 {
            generation: self.context_generation,
            identity: ContextIdentityV1::new(
                DeviceIdentityV1::from_bytes(identity_bytes(
                    b"fe2o3.group.dev1",
                    self.context_generation,
                    local,
                )),
                identity_bytes(b"fe2o3.group.ctx1", self.context_generation, local),
            ),
            coverage,
            devices,
            streams: originals,
        })
    }
}
