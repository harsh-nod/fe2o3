//! Explicit historical output from the checked native register producer.
//!
//! Output-only: no decoder or live-owner import. Existing V4/V5 records are
//! unchanged. Serialization occurs only after the existing owner teardown
//! returned; this does not assert independently confirmed process reaping.

use crate::{
    MAX_LIVE_GPU_RESPONSE_LINE_BYTES_V3, RocgdbHardwareStopResourcesV1,
    RocgdbMiNativeInspectionProbeV5, RocgdbMiNativeInspectionUnavailableReasonV5,
    RocgdbMiNativeProbeV4, RocgdbMiNativeUnavailableReasonV4,
};
use serde::Serialize;
use std::io::{self, Write};

pub const ROCGDB_HARDWARE_CAPTURE_SCHEMA_V1: &str = "fe2o3-rocgdb-kfd-resource-capture-v1";
pub const MAX_ROCGDB_HARDWARE_CAPTURE_BYTES_V1: usize = MAX_LIVE_GPU_RESPONSE_LINE_BYTES_V3;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum RocgdbHardwareCaptureSchemaV1 {
    #[serde(rename = "fe2o3-rocgdb-kfd-resource-capture-v1")]
    V1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum RocgdbHardwareCaptureLifetimeV1 {
    #[serde(rename = "historical_same_stop_capture")]
    HistoricalSameStopCapture,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RocgdbHardwareLocalsCompletionV1 {
    Captured,
    CommandUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum RocgdbHardwareCaptureUnavailableV1 {
    NativeCapture {
        reason: RocgdbMiNativeUnavailableReasonV4,
    },
    RegisterInspection {
        reason: RocgdbMiNativeInspectionUnavailableReasonV5,
    },
    LocalsInspection {
        reason: RocgdbMiNativeInspectionUnavailableReasonV5,
    },
    ProjectionNotRetained {},
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
// Keep the bounded inert projection inline: moving it out must not introduce
// an infallible box allocation after the live owner has relinquished custody.
#[allow(clippy::large_enum_variant)]
pub enum RocgdbHardwareCaptureResultV1 {
    Captured {
        probe: RocgdbMiNativeProbeV4,
        inspection_probe: RocgdbMiNativeInspectionProbeV5,
        locals_completion: RocgdbHardwareLocalsCompletionV1,
        projection: RocgdbHardwareStopResourcesV1,
    },
    Unavailable {
        probe: RocgdbMiNativeProbeV4,
        inspection_probe: RocgdbMiNativeInspectionProbeV5,
        reason: RocgdbHardwareCaptureUnavailableV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RocgdbHardwareCaptureResponseV1 {
    pub schema: RocgdbHardwareCaptureSchemaV1,
    pub observation_lifetime: RocgdbHardwareCaptureLifetimeV1,
    pub result: RocgdbHardwareCaptureResultV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RocgdbHardwareCaptureErrorV1 {
    InvalidProbe,
    InvalidProjection,
    InvalidUnavailable,
    Serialization,
    OutputLimit,
    OutputIo,
}

impl RocgdbHardwareCaptureResponseV1 {
    /// Structural output validation, never authenticates imported historical bytes.
    pub fn validate(&self) -> Result<(), RocgdbHardwareCaptureErrorV1> {
        use RocgdbHardwareCaptureErrorV1 as E;
        match &self.result {
            RocgdbHardwareCaptureResultV1::Captured {
                probe,
                inspection_probe,
                locals_completion,
                projection,
            } => {
                if !(native_admitted(probe)
                    && inspection_probe.register_names
                    && inspection_probe.register_values)
                    || (matches!(
                        locals_completion,
                        RocgdbHardwareLocalsCompletionV1::Captured
                    ) != inspection_probe.simple_locals)
                {
                    return Err(E::InvalidProbe);
                }
                projection.validate().map_err(|_| E::InvalidProjection)
            }
            RocgdbHardwareCaptureResultV1::Unavailable {
                probe,
                inspection_probe,
                reason,
            } => {
                let valid = match reason {
                    // Preserve the existing one-shot native failure and probes.
                    RocgdbHardwareCaptureUnavailableV1::NativeCapture { .. } => true,
                    RocgdbHardwareCaptureUnavailableV1::RegisterInspection { reason } => {
                        native_admitted(probe)
                            && inspection_reason(
                                *reason,
                                inspection_probe.register_names && inspection_probe.register_values,
                            )
                    }
                    RocgdbHardwareCaptureUnavailableV1::LocalsInspection { reason } => {
                        native_admitted(probe)
                            && inspection_probe.register_names
                            && inspection_probe.register_values
                            && inspection_reason(*reason, inspection_probe.simple_locals)
                    }
                    RocgdbHardwareCaptureUnavailableV1::ProjectionNotRetained {} => {
                        native_admitted(probe)
                            && inspection_probe.register_names
                            && inspection_probe.register_values
                    }
                };
                valid.then_some(()).ok_or(E::InvalidUnavailable)
            }
        }
    }
}

fn native_admitted(probe: &RocgdbMiNativeProbeV4) -> bool {
    probe.structured_mi_commands
        && probe.direct_kfd_device_admitted
        && probe.cooperative_v2_declaration
        && probe.cooperative_v2_publication
}

fn inspection_reason(reason: RocgdbMiNativeInspectionUnavailableReasonV5, supported: bool) -> bool {
    use RocgdbMiNativeInspectionUnavailableReasonV5 as R;
    match reason {
        R::MachineCommandUnavailable => !supported,
        R::BackendRejected => supported,
        R::NotCaptured => true,
        R::RequiresAuthenticatedSourceMap
        | R::RequiresArtifactRelativeInstructionBinding
        | R::RequiresAllocationRelativeAuthority => false,
    }
}

/// Validates before allocating output; emits at most the inherited 2 MiB cap,
/// including exactly one final LF, and flushes. No partial output is emitted
/// for validation/serialization/cap failures; writer I/O itself may be partial.
pub fn write_rocgdb_hardware_capture_v1(
    writer: &mut impl Write,
    response: &RocgdbHardwareCaptureResponseV1,
) -> Result<(), RocgdbHardwareCaptureErrorV1> {
    use RocgdbHardwareCaptureErrorV1 as E;
    response.validate()?;
    let mut buffer = CaptureBufferV1 {
        bytes: Vec::new(),
        limit_hit: false,
    };
    if serde_json::to_writer(&mut buffer, response).is_err() {
        return Err(if buffer.limit_hit {
            E::OutputLimit
        } else {
            E::Serialization
        });
    }
    if buffer.write_all(b"\n").is_err() {
        return Err(if buffer.limit_hit {
            E::OutputLimit
        } else {
            E::Serialization
        });
    }
    writer.write_all(&buffer.bytes).map_err(|_| E::OutputIo)?;
    writer.flush().map_err(|_| E::OutputIo)
}

struct CaptureBufferV1 {
    bytes: Vec<u8>,
    limit_hit: bool,
}

impl Write for CaptureBufferV1 {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let needed = self.bytes.len().checked_add(bytes.len());
        let Some(needed) = needed.filter(|len| *len <= MAX_ROCGDB_HARDWARE_CAPTURE_BYTES_V1) else {
            self.limit_hit = true;
            return Err(io::Error::other("historical capture byte cap"));
        };
        if needed > self.bytes.capacity() {
            // Geometric requested capacity avoids per-token reallocations. The
            // allocator may round capacity up; the serialized byte cap remains
            // exact and every reservation is fallible before stdout is touched.
            let target = self
                .bytes
                .capacity()
                .checked_mul(2)
                .unwrap_or(MAX_ROCGDB_HARDWARE_CAPTURE_BYTES_V1)
                .max(256)
                .max(needed)
                .min(MAX_ROCGDB_HARDWARE_CAPTURE_BYTES_V1);
            self.bytes
                .try_reserve_exact(target - self.bytes.len())
                .map_err(|_| io::Error::other("historical capture allocation"))?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "rocgdb_hardware_capture_v1_tests.rs"]
mod tests;
