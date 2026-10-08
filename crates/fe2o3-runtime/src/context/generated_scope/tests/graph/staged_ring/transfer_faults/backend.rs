#![cfg(test)]
//! Faults are explicit CPU premises at the original backend boundary, not device failures.
use super::*;
use crate::{
    BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1, RuntimeBackendFailureV1,
};

type Reply<T> = Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fault {
    Rejected,
    Quiescent,
    Terminal,
}

pub(super) struct FaultBackend {
    pub inner: Backend,
    pub target: Option<u64>,
    pub peer_calls: usize,
    pub local_calls: usize,
    pub releases: usize,
    pub rejected: usize,
    pub ambiguous: Option<u64>,
    fault: Fault,
    error: KfdRuntimeBackendErrorV1,
}

impl FaultBackend {
    pub fn new(fault: Fault) -> Self {
        let mut inner = Backend::mock_preparation_v1();
        let error = match inner.read_allocation_v1(u64::MAX, 0, &mut []) {
            Err(RuntimeBackendFailureV1::Rejected(error)) => error,
            _ => panic!("unknown mock allocation must refuse"),
        };
        Self {
            inner,
            target: None,
            peer_calls: 0,
            local_calls: 0,
            releases: 0,
            rejected: 0,
            ambiguous: None,
            fault,
            error,
        }
    }
}

macro_rules! forward {
    ($name:ident($($arg:ident: $ty:ty),*) -> $out:ty) => {
        fn $name(&mut self, $($arg: $ty),*) -> Reply<$out> {
            self.inner.$name($($arg),*)
        }
    };
}

impl RuntimeBackendV1 for FaultBackend {
    type Error = KfdRuntimeBackendErrorV1;
    forward!(enumerate_devices_v1() -> Vec<BackendDeviceDescriptionV1>);
    forward!(create_stream_v1(device: u64) -> u64);
    forward!(destroy_stream_v1(stream: u64) -> ());
    forward!(allocate_v1(device: u64, kind: RuntimeMemoryKindV1, len: u64, alignment: u64) -> u64);
    forward!(release_allocation_v1(allocation: u64) -> ());
    forward!(read_allocation_v1(allocation: u64, offset: u64, bytes: &mut [u8]) -> ());
    forward!(write_allocation_v1(allocation: u64, offset: u64, bytes: &[u8]) -> ());
    forward!(load_module_v1(device: u64, bytes: &[u8]) -> u64);
    forward!(unload_module_v1(module: u64) -> ());
    forward!(resolve_kernel_v1(module: u64, name: &str, signature: [u8; 32]) -> u64);
    forward!(submit_v1(launch: BackendLaunchV1<'_>) -> u64);
    forward!(poll_v1(submission: u64) -> BackendPollV1);
    forward!(wait_v1(submission: u64, deadline: Instant) -> BackendPollV1);
    forward!(record_event_v1(stream: u64, submission: u64) -> u64);
    forward!(release_event_v1(event: u64) -> ());

    fn release_submission_v1(&mut self, submission: u64) -> Reply<()> {
        self.inner.release_submission_v1(submission)?;
        self.releases += 1;
        Ok(())
    }

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Reply<u64> {
        self.peer_calls += 1;
        if self.target != Some(destination.allocation) {
            return self
                .inner
                .peer_copy_v1(stream, source, destination, dependencies);
        }
        assert_eq!(self.rejected, 0, "a rejected graph copy cannot retry");
        assert!(
            self.ambiguous.is_none(),
            "an unknown graph copy cannot retry"
        );
        if self.fault == Fault::Rejected {
            self.rejected += 1;
            return Err(RuntimeBackendFailureV1::Rejected(self.error.clone()));
        }
        // The real inner router retains the original accepted copy before its
        // observation is lost. Neither a token nor successful settlement is
        // returned to Context; its original Pending replica must remain rooted.
        self.ambiguous =
            Some(
                self.inner
                    .peer_copy_v1(stream, source, destination, dependencies)?,
            );
        Err(match self.fault {
            Fault::Quiescent => RuntimeBackendFailureV1::Quiescent(self.error.clone()),
            Fault::Terminal => RuntimeBackendFailureV1::Terminal(self.error.clone()),
            Fault::Rejected => unreachable!(),
        })
    }
}

impl RuntimeFlushBackendV1 for FaultBackend {
    forward!(flush_stream_v1(stream: u64) -> ());
    forward!(progress_stream_v1(stream: u64) -> ());
}

impl RuntimeAsyncCopyBackendV1 for FaultBackend {
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Reply<u64> {
        self.local_calls += 1;
        self.inner
            .copy_async_v1(stream, source, destination, dependencies)
    }
}
