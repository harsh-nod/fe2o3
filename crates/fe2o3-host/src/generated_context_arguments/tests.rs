use super::*;
use fe2o3_runtime::{
    BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1, BackendPollV1,
    RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeCapabilitiesV1, RuntimeContextV1,
    RuntimeMemoryKindV1,
};
use std::{collections::HashSet, io, time::Instant};

#[derive(Default)]
struct AllocationBackend {
    next: u64,
    live: HashSet<u64>,
}

type BackendResult<T> = Result<T, RuntimeBackendFailureV1<io::Error>>;

macro_rules! unused_backend_method {
    ($name:ident($($argument:ident: $ty:ty),*) -> $output:ty) => {
        fn $name(&mut self, $($argument: $ty),*) -> BackendResult<$output> {
            $(let _ = $argument;)*
            panic!("descriptor fixture does not invoke {}", stringify!($name));
        }
    };
}

impl RuntimeBackendV1 for AllocationBackend {
    type Error = io::Error;

    fn enumerate_devices_v1(&mut self) -> BackendResult<Vec<BackendDeviceDescriptionV1>> {
        Ok(vec![BackendDeviceDescriptionV1 {
            backend_device: 1,
            name: "inert Context descriptor fixture".to_owned(),
            target: "gfx942".to_owned(),
            global_memory_bytes: 1_024,
            capabilities: RuntimeCapabilitiesV1 {
                typed_async_launch: false,
                device_memory: true,
                host_visible_memory: false,
                streams: false,
                events: false,
                peer_copy: false,
                multi_device: false,
                atomics: false,
                collectives: false,
            },
        }])
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> BackendResult<u64> {
        assert_eq!(device, 1);
        assert_eq!(kind, RuntimeMemoryKindV1::DeviceLocal);
        assert_eq!((byte_len, alignment), (64, 8));
        self.next += 1;
        assert!(self.live.insert(self.next));
        Ok(self.next)
    }

    fn release_allocation_v1(&mut self, allocation: u64) -> BackendResult<()> {
        assert!(self.live.remove(&allocation));
        Ok(())
    }

    unused_backend_method!(create_stream_v1(device: u64) -> u64);
    unused_backend_method!(destroy_stream_v1(stream: u64) -> ());
    unused_backend_method!(write_allocation_v1(allocation: u64, byte_offset: u64, bytes: &[u8]) -> ());
    unused_backend_method!(read_allocation_v1(allocation: u64, byte_offset: u64, destination: &mut [u8]) -> ());
    unused_backend_method!(load_module_v1(device: u64, image: &[u8]) -> u64);
    unused_backend_method!(unload_module_v1(module: u64) -> ());
    unused_backend_method!(resolve_kernel_v1(module: u64, name: &str, signature: [u8; 32]) -> u64);
    unused_backend_method!(submit_v1(launch: BackendLaunchV1<'_>) -> u64);
    unused_backend_method!(poll_v1(submission: u64) -> BackendPollV1);
    unused_backend_method!(wait_v1(submission: u64, deadline: Instant) -> BackendPollV1);
    unused_backend_method!(release_submission_v1(submission: u64) -> ());
    unused_backend_method!(record_event_v1(stream: u64, submission: u64) -> u64);
    unused_backend_method!(release_event_v1(event: u64) -> ());
    unused_backend_method!(peer_copy_v1(stream: u64, source: BackendMemoryRegionV1, destination: BackendMemoryRegionV1, dependencies: &[u64]) -> u64);
}

fn with_allocation(test: impl FnOnce(RuntimeAllocationIdV1)) {
    let mut context = RuntimeContextV1::open(AllocationBackend::default()).unwrap();
    let allocation = context
        .allocate(
            context.devices()[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            64,
            8,
        )
        .unwrap();
    test(allocation);
    context.release_allocation(allocation).unwrap();
    context.shutdown().unwrap();
}

fn scalar_metadata<T: GeneratedDeviceScalarV1>(allocation: RuntimeAllocationIdV1) {
    let width = T::RUST_SCALAR_TYPE.size_bytes();
    let read = GeneratedContextReadSlice::<T>::new(allocation, width * 2, 3).unwrap();
    let write = GeneratedContextWriteSlice::<T>::new(allocation, width * 2, 3).unwrap();
    let read_write = GeneratedContextReadWriteSlice::<T>::new(allocation, width * 2, 3).unwrap();
    for (region, elements, access) in [
        (read.region_v1(), read.elements_v1(), RuntimeAccessV1::Read),
        (
            write.region_v1(),
            write.elements_v1(),
            RuntimeAccessV1::Write,
        ),
        (
            read_write.region_v1(),
            read_write.elements_v1(),
            RuntimeAccessV1::ReadWrite,
        ),
    ] {
        assert_eq!(
            region,
            RuntimeMemoryRegionV1 {
                allocation,
                access,
                byte_offset: width * 2,
                byte_len: width * 3,
            }
        );
        assert_eq!(elements, 3);
    }
}

#[test]
fn all_scalar_descriptors_preserve_exact_identity_count_and_fixed_access() {
    with_allocation(|allocation| {
        scalar_metadata::<i8>(allocation);
        scalar_metadata::<u8>(allocation);
        scalar_metadata::<i16>(allocation);
        scalar_metadata::<u16>(allocation);
        scalar_metadata::<i32>(allocation);
        scalar_metadata::<u32>(allocation);
        scalar_metadata::<i64>(allocation);
        scalar_metadata::<u64>(allocation);
        scalar_metadata::<f32>(allocation);
        scalar_metadata::<f64>(allocation);
    });
}

#[test]
fn all_access_modes_reject_empty_unaligned_and_overflowing_metadata() {
    with_allocation(|allocation| {
        for (offset, elements) in [
            (0, 0),
            (1, 1),
            (2, 1),
            (4, 1),
            (0, u64::MAX),
            (0, u64::MAX / 8 + 1),
            (8, u64::MAX / 8),
            (u64::MAX - 7, 1),
        ] {
            assert!(matches!(
                GeneratedContextReadSlice::<u64>::new(allocation, offset, elements),
                Err(RuntimeValidationErrorV1::InvalidRange)
            ));
            assert!(matches!(
                GeneratedContextWriteSlice::<u64>::new(allocation, offset, elements),
                Err(RuntimeValidationErrorV1::InvalidRange)
            ));
            assert!(matches!(
                GeneratedContextReadWriteSlice::<u64>::new(allocation, offset, elements),
                Err(RuntimeValidationErrorV1::InvalidRange)
            ));
        }
    });
}

#[test]
fn descriptor_arithmetic_accepts_exact_u64_boundaries_without_allocation_claims() {
    with_allocation(|allocation| {
        let byte = GeneratedContextReadSlice::<u8>::new(allocation, u64::MAX - 1, 1).unwrap();
        assert_eq!(byte.region_v1().byte_offset, u64::MAX - 1);
        assert_eq!(byte.region_v1().byte_len, 1);
        assert!(matches!(
            GeneratedContextReadSlice::<u8>::new(allocation, u64::MAX, 1),
            Err(RuntimeValidationErrorV1::InvalidRange)
        ));
        let wide = GeneratedContextWriteSlice::<f64>::new(allocation, 0, u64::MAX / 8).unwrap();
        assert_eq!(wide.region_v1().byte_len, u64::MAX - 7);
        assert_eq!(wide.elements_v1(), u64::MAX / 8);
        let tail = GeneratedContextReadWriteSlice::<u32>::new(allocation, u64::MAX - 7, 1).unwrap();
        assert_eq!(tail.region_v1().byte_len, 4);
    });
}

#[test]
fn descriptor_does_not_retain_allocation_or_borrow_context() {
    let mut context = RuntimeContextV1::open(AllocationBackend::default()).unwrap();
    let allocation = context
        .allocate(
            context.devices()[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            64,
            8,
        )
        .unwrap();
    let descriptor = GeneratedContextReadWriteSlice::<u32>::new(allocation, 4, 3).unwrap();
    context.release_allocation(allocation).unwrap();
    context.shutdown().unwrap();
    assert_eq!(descriptor.region_v1().allocation, allocation);
    assert_eq!(descriptor.elements_v1(), 3);
}
