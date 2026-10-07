//! Bounded CPU byte transport only; every kernel/module operation is denied.

use fe2o3_runtime::*;
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Mutex},
    time::Instant,
};

pub(super) type Result<T> = std::result::Result<T, RuntimeBackendFailureV1<io::Error>>;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Failure {
    #[default]
    None,
    Rejected,
    Quiescent,
    Terminal,
    Panic,
}

fn outcome(mode: Failure) -> Result<()> {
    let error = io::Error::other("scripted transport failure");
    match mode {
        Failure::None => Ok(()),
        Failure::Rejected => Err(RuntimeBackendFailureV1::Rejected(error)),
        Failure::Quiescent => Err(RuntimeBackendFailureV1::Quiescent(error)),
        Failure::Terminal => Err(RuntimeBackendFailureV1::Terminal(error)),
        Failure::Panic => std::panic::panic_any(83u32),
    }
}

struct Copy {
    stream: u64,
    source: BackendMemoryRegionV1,
    destination: BackendMemoryRegionV1,
    ready: bool,
    failure: Failure,
}

#[derive(Default)]
pub(super) struct State {
    next: u64,
    memory: BTreeMap<u64, Vec<u8>>,
    streams: BTreeMap<u64, u64>,
    copies: BTreeMap<u64, Copy>,
    pub writes: usize,
    pub write_failure: Failure,
    pub copy_failure: Failure,
    pub copies_submitted: usize,
    pub peer_submitted: usize,
}

impl State {
    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }
}

pub(super) struct Backend(pub Arc<Mutex<State>>);

impl Backend {
    fn enqueue(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        peer: bool,
    ) -> Result<u64> {
        let mut state = self.0.lock().unwrap();
        let failure = state.copy_failure;
        assert!(matches!(failure, Failure::None | Failure::Quiescent));
        let id = state.id();
        state.copies.insert(
            id,
            Copy {
                stream,
                source,
                destination,
                ready: false,
                failure,
            },
        );
        state.copies_submitted += 1;
        state.peer_submitted += usize::from(peer);
        Ok(id)
    }
}

macro_rules! denied {
    ($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty) => {
        fn $name(&mut self, $($arg: $ty),*) -> Result<$ret> {
            $(let _ = $arg;)*
            Err(RuntimeBackendFailureV1::Rejected(io::Error::other("no kernel/event authority")))
        }
    };
}

impl RuntimeBackendV1 for Backend {
    type Error = io::Error;
    fn enumerate_devices_v1(&mut self) -> Result<Vec<BackendDeviceDescriptionV1>> {
        Ok([7, 8]
            .into_iter()
            .map(|backend_device| BackendDeviceDescriptionV1 {
                backend_device,
                name: "CPU staging transport".into(),
                target: "gfx942".into(),
                global_memory_bytes: 1 << 20,
                capabilities: RuntimeCapabilitiesV1 {
                    typed_async_launch: false,
                    device_memory: true,
                    host_visible_memory: true,
                    streams: true,
                    events: false,
                    peer_copy: true,
                    multi_device: true,
                    atomics: false,
                    collectives: false,
                },
            })
            .collect())
    }
    fn create_stream_v1(&mut self, device: u64) -> Result<u64> {
        let mut state = self.0.lock().unwrap();
        let id = state.id();
        state.streams.insert(id, device);
        Ok(id)
    }
    fn destroy_stream_v1(&mut self, stream: u64) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        assert!(
            state
                .copies
                .values()
                .all(|copy| copy.stream != stream || copy.ready)
        );
        assert!(state.streams.remove(&stream).is_some());
        Ok(())
    }
    fn allocate_v1(
        &mut self,
        _: u64,
        _: RuntimeMemoryKindV1,
        byte_len: u64,
        _: u64,
    ) -> Result<u64> {
        let mut state = self.0.lock().unwrap();
        let id = state.id();
        state.memory.insert(id, vec![0xa5; byte_len as usize]);
        Ok(id)
    }
    fn release_allocation_v1(&mut self, allocation: u64) -> Result<()> {
        assert!(self.0.lock().unwrap().memory.remove(&allocation).is_some());
        Ok(())
    }
    fn write_allocation_v1(&mut self, allocation: u64, offset: u64, bytes: &[u8]) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        state.writes += 1;
        let mode = state.write_failure;
        if mode != Failure::Rejected {
            let count = if mode == Failure::None {
                bytes.len()
            } else {
                bytes.len().min(2)
            };
            state.memory.get_mut(&allocation).unwrap()[offset as usize..offset as usize + count]
                .copy_from_slice(&bytes[..count]);
        }
        drop(state);
        outcome(mode)
    }
    fn read_allocation_v1(&mut self, allocation: u64, offset: u64, bytes: &mut [u8]) -> Result<()> {
        bytes.copy_from_slice(
            &self.0.lock().unwrap().memory[&allocation]
                [offset as usize..offset as usize + bytes.len()],
        );
        Ok(())
    }
    fn poll_v1(&mut self, submission: u64) -> Result<BackendPollV1> {
        let state = self.0.lock().unwrap();
        let copy = &state.copies[&submission];
        if !copy.ready {
            return Ok(BackendPollV1::Pending);
        }
        outcome(copy.failure)?;
        Ok(BackendPollV1::Succeeded)
    }
    fn wait_v1(&mut self, submission: u64, _: Instant) -> Result<BackendPollV1> {
        self.poll_v1(submission)
    }
    fn release_submission_v1(&mut self, submission: u64) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        assert!(state.copies[&submission].ready);
        state.copies.remove(&submission);
        Ok(())
    }
    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64> {
        assert!(dependencies.is_empty());
        self.enqueue(stream, source, destination, true)
    }
    denied!(load_module_v1(device: u64, image: &[u8]) -> u64);
    denied!(unload_module_v1(module: u64) -> ());
    denied!(resolve_kernel_v1(module: u64, name: &str, signature: [u8; 32]) -> u64);
    denied!(submit_v1(launch: BackendLaunchV1<'_>) -> u64);
    denied!(record_event_v1(stream: u64, submission: u64) -> u64);
    denied!(release_event_v1(event: u64) -> ());
}

impl RuntimeAsyncCopyBackendV1 for Backend {
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64> {
        assert!(dependencies.is_empty());
        self.enqueue(stream, source, destination, false)
    }
}

impl RuntimeFlushBackendV1 for Backend {
    fn flush_stream_v1(&mut self, stream: u64) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        let ids: Vec<_> = state
            .copies
            .iter()
            .filter_map(|(id, copy)| (copy.stream == stream && !copy.ready).then_some(*id))
            .collect();
        for id in ids {
            let copy = &state.copies[&id];
            let (source, destination) = (copy.source, copy.destination);
            let count = if copy.failure == Failure::None {
                source.byte_len as usize
            } else {
                2
            };
            let bytes = state.memory[&source.allocation]
                [source.byte_offset as usize..source.byte_offset as usize + count]
                .to_vec();
            state.memory.get_mut(&destination.allocation).unwrap()
                [destination.byte_offset as usize..destination.byte_offset as usize + count]
                .copy_from_slice(&bytes);
            state.copies.get_mut(&id).unwrap().ready = true;
        }
        Ok(())
    }
}
