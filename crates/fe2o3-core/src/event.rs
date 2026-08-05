use crate::{Error, GpuContext, HipError, Result, Stream, check};
use std::sync::Arc;

/// Creation options for a HIP event.
///
/// [`EventOptions::default`] matches `hipEventDefault`: synchronization
/// actively polls for low latency and timing is enabled. Blocking
/// synchronization yields the CPU while waiting. Disabling timing avoids
/// profiling overhead for synchronization-only events, but prevents elapsed
/// time measurements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventOptions {
    blocking_sync: bool,
    timing_enabled: bool,
}

impl EventOptions {
    pub const fn new() -> Self {
        Self {
            blocking_sync: false,
            timing_enabled: true,
        }
    }

    #[must_use]
    pub const fn blocking_sync(mut self) -> Self {
        self.blocking_sync = true;
        self
    }

    #[must_use]
    pub const fn without_timing(mut self) -> Self {
        self.timing_enabled = false;
        self
    }

    pub const fn uses_blocking_sync(self) -> bool {
        self.blocking_sync
    }

    pub const fn timing_enabled(self) -> bool {
        self.timing_enabled
    }

    const fn bits(self) -> u32 {
        let mut bits = fe2o3_hip_sys::HIP_EVENT_DEFAULT;
        if self.blocking_sync {
            bits |= fe2o3_hip_sys::HIP_EVENT_BLOCKING_SYNC;
        }
        if !self.timing_enabled {
            bits |= fe2o3_hip_sys::HIP_EVENT_DISABLE_TIMING;
        }
        bits
    }
}

impl Default for EventOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// An owned HIP event associated with one device.
///
/// The event retains its [`GpuContext`]. Every HIP operation first makes that
/// context's device current on the calling thread.
#[derive(Debug)]
pub struct Event {
    raw: fe2o3_hip_sys::hipEvent_t,
    context: Arc<GpuContext>,
    options: EventOptions,
    recorded: bool,
}

// Moving an event to another thread is sound because every operation binds
// its owning device. Event is intentionally not Sync: recording requires
// exclusive access, and HIP forbids re-recording a pending event.
unsafe impl Send for Event {}

impl Event {
    /// Creates a timing-enabled event with active synchronization.
    pub fn new(context: &Arc<GpuContext>) -> Result<Self> {
        Self::with_options(context, EventOptions::default())
    }

    pub fn with_options(context: &Arc<GpuContext>, options: EventOptions) -> Result<Self> {
        context.bind_to_thread()?;
        let mut raw = core::ptr::null_mut();
        check(unsafe { fe2o3_hip_sys::hipEventCreateWithFlags(&mut raw, options.bits()) })?;
        Ok(Self {
            raw,
            context: context.clone(),
            options,
            recorded: false,
        })
    }

    /// Records the event after all work already queued in `stream`.
    ///
    /// A pending event cannot be recorded again because HIP defines that case
    /// as undefined. Synchronize or wait for [`Event::query`] to report
    /// completion before reusing an event.
    pub fn record(&mut self, stream: &Stream) -> Result<()> {
        ensure_stream_device(self.context.device_id(), stream.context().device_id())?;
        if self.recorded && !self.query()? {
            return Err(Error::EventPending);
        }

        self.context.bind_to_thread()?;
        check(unsafe { fe2o3_hip_sys::hipEventRecord(self.raw, stream.raw()) })?;
        self.recorded = true;
        Ok(())
    }

    /// Blocks until all work preceding the most recent recording is complete.
    ///
    /// HIP reports success for an event that has never been recorded.
    pub fn synchronize(&self) -> Result<()> {
        self.context.bind_to_thread()?;
        check(unsafe { fe2o3_hip_sys::hipEventSynchronize(self.raw) })
    }

    /// Returns whether the event has completed without blocking.
    ///
    /// HIP reports an unrecorded event as complete.
    pub fn query(&self) -> Result<bool> {
        self.context.bind_to_thread()?;
        decode_query(unsafe { fe2o3_hip_sys::hipEventQuery(self.raw) })
    }

    /// Returns HIP's elapsed time from `start` to this event, in milliseconds.
    ///
    /// Both events must have timing enabled, belong to the same device, and
    /// have completed recording. HIP reports pending or unrecorded events as
    /// errors. The underlying timestamp resolution is approximately 1 us.
    pub fn elapsed_time_ms_since(&self, start: &Self) -> Result<f32> {
        ensure_event_device(start.context.device_id(), self.context.device_id())?;
        if !start.options.timing_enabled() || !self.options.timing_enabled() {
            return Err(Error::EventTimingDisabled);
        }

        self.context.bind_to_thread()?;
        let mut milliseconds = 0.0;
        check(unsafe {
            fe2o3_hip_sys::hipEventElapsedTime(&mut milliseconds, start.raw, self.raw)
        })?;
        Ok(milliseconds)
    }

    pub fn context(&self) -> &Arc<GpuContext> {
        &self.context
    }

    pub fn options(&self) -> EventOptions {
        self.options
    }

    /// Returns the borrowed HIP event handle.
    ///
    /// # Safety
    ///
    /// The caller must not destroy the handle or use it after this event is
    /// dropped. Direct HIP operations must target this event's device and must
    /// not record the event while an earlier recording is still pending.
    pub unsafe fn raw(&self) -> fe2o3_hip_sys::hipEvent_t {
        self.raw
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        if self.raw.is_null() {
            return;
        }

        // If binding fails, leaking is preferable to destroying a device-owned
        // handle while another device is current.
        if self.context.bind_to_thread().is_ok() {
            let _ = check(unsafe { fe2o3_hip_sys::hipEventDestroy(self.raw) });
        }
    }
}

fn decode_query(code: fe2o3_hip_sys::hipError_t) -> Result<bool> {
    match code {
        fe2o3_hip_sys::HIP_SUCCESS => Ok(true),
        fe2o3_hip_sys::HIP_ERROR_NOT_READY => Ok(false),
        code => Err(HipError::new(code).into()),
    }
}

fn ensure_stream_device(event_device: i32, stream_device: i32) -> Result<()> {
    if event_device == stream_device {
        Ok(())
    } else {
        Err(Error::EventDeviceMismatch {
            event_device,
            stream_device,
        })
    }
}

fn ensure_event_device(start_device: i32, stop_device: i32) -> Result<()> {
    if start_device == stop_device {
        Ok(())
    } else {
        Err(Error::EventPairDeviceMismatch {
            start_device,
            stop_device,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, EventOptions, decode_query, ensure_event_device, ensure_stream_device};
    use crate::{Error, GpuContext, HipError};

    #[test]
    fn options_map_to_header_flags() {
        assert_eq!(EventOptions::default().bits(), 0x0);
        assert_eq!(EventOptions::new().blocking_sync().bits(), 0x1);
        assert_eq!(EventOptions::new().without_timing().bits(), 0x2);
        assert_eq!(
            EventOptions::new().blocking_sync().without_timing().bits(),
            0x3
        );
    }

    #[test]
    fn query_decodes_only_not_ready_as_incomplete() {
        assert!(decode_query(fe2o3_hip_sys::HIP_SUCCESS).unwrap());
        assert!(!decode_query(fe2o3_hip_sys::HIP_ERROR_NOT_READY).unwrap());
        assert!(matches!(
            decode_query(400),
            Err(Error::Hip(error)) if error == HipError::new(400)
        ));
    }

    #[test]
    fn device_checks_report_resource_roles() {
        assert!(ensure_stream_device(2, 2).is_ok());
        assert!(matches!(
            ensure_stream_device(2, 4),
            Err(Error::EventDeviceMismatch {
                event_device: 2,
                stream_device: 4
            })
        ));
        assert!(ensure_event_device(3, 3).is_ok());
        assert!(matches!(
            ensure_event_device(3, 5),
            Err(Error::EventPairDeviceMismatch {
                start_device: 3,
                stop_device: 5
            })
        ));
    }

    #[test]
    #[ignore = "requires a working HIP device"]
    fn event_records_queries_and_measures_time() -> crate::Result<()> {
        let context = GpuContext::new(0)?;
        let stream = context.create_stream()?;
        let mut start = Event::new(&context)?;
        let mut stop = Event::new(&context)?;

        assert!(start.query()?);
        start.record(&stream)?;
        stop.record(&stream)?;
        stop.synchronize()?;

        assert!(start.query()?);
        assert!(stop.query()?);
        assert!(stop.elapsed_time_ms_since(&start)? >= 0.0);
        Ok(())
    }

    #[test]
    #[ignore = "requires two working HIP devices"]
    fn recording_on_another_device_is_rejected_when_available() -> crate::Result<()> {
        let event_context = GpuContext::new(0)?;
        let mut event = Event::new(&event_context)?;
        let stream_context = match GpuContext::new(1) {
            Ok(context) => context,
            Err(Error::NoDevice { .. }) => return Ok(()),
            Err(error) => return Err(error),
        };
        let stream = stream_context.default_stream();

        assert!(matches!(
            event.record(&stream),
            Err(Error::EventDeviceMismatch {
                event_device: 0,
                stream_device: 1
            })
        ));
        Ok(())
    }
}
