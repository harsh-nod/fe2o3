//! Native endpoint-budget correctness witness, not a performance benchmark.
//! The campaign must freshly admit both exact GPUs before invoking this process.

#![forbid(unsafe_code)]

use fe2o3_kfd::{Gfx942DeviceBackingBudgetV1, Gfx942HostVisibleBackingBudgetV1};
use fe2o3_runtime::*;
use std::{
    error::Error,
    fmt::Debug,
    time::{Duration, Instant},
};

type ResultV1<T> = Result<T, Box<dyn Error>>;
type Backend = KfdNativeXgmiRuntimeBackendV1;
type Context = RuntimeContextV1<Backend>;
const BYTES: usize = 4097;
const PAYLOAD: usize = 1024;
const PADDED: u64 = 8192;
const DEVICE_LIMITS: [(u64, usize); 2] = [(8192, 3), (24576, 2)];
const HOST_LIMITS: [(u64, usize); 2] = [(4096, 1), (8192, 2)];
const REQUEST_BYTES: u64 = 1 << 20;
const REQUEST_RECORDS: usize = 16;

fn error(value: impl Debug) -> Box<dyn Error> {
    format!("{value:?}").into()
}

fn require(value: bool, message: &'static str) -> ResultV1<()> {
    if value { Ok(()) } else { Err(message.into()) }
}

fn ids(arguments: &[String]) -> ResultV1<[u64; 2]> {
    require(arguments.len() == 2, "expected two unique IDs")?;
    let parse = |text: &str| -> ResultV1<u64> {
        let value = match text.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16)?,
            None => text.parse()?,
        };
        require(value != 0, "zero unique ID")?;
        Ok(value)
    };
    let pair = [parse(&arguments[0])?, parse(&arguments[1])?];
    require(pair[0] != pair[1], "distinct GPUs required")?;
    Ok(pair)
}

fn budgets() -> [KfdNativeXgmiBackingBudgetV1; 2] {
    std::array::from_fn(|index| KfdNativeXgmiBackingBudgetV1 {
        device: Some(
            Gfx942DeviceBackingBudgetV1::new(DEVICE_LIMITS[index].0, DEVICE_LIMITS[index].1)
                .expect("fixed device limits"),
        ),
        host_visible: Some(
            Gfx942HostVisibleBackingBudgetV1::new(HOST_LIMITS[index].0, HOST_LIMITS[index].1)
                .expect("fixed coherent limits"),
        ),
    })
}

fn check_usage(
    actual: [KfdNativeXgmiBackingUsageV1; 2],
    unique_ids: [u64; 2],
    device: [(u64, u64); 2],
    coherent: [u64; 2],
) -> ResultV1<()> {
    let limits = budgets();
    for index in 0..2 {
        let usage = actual[index];
        let native = usage.device.ok_or("missing device account")?;
        let host = usage.host_visible.ok_or("missing coherent account")?;
        require(
            usage.backend_device == unique_ids[index],
            "endpoint identity/order",
        )?;
        require(
            Some(native.budget) == limits[index].device
                && Some(host.budget) == limits[index].host_visible,
            "endpoint budget/order",
        )?;
        require(
            (native.used_backing_bytes, native.used_allocation_records) == device[index]
                && native.retained_records as u64 == device[index].1
                && native.reserved_records == 0
                && native.quarantined_records == 0
                && !native.poisoned,
            "device debit or custody",
        )?;
        require(
            host.used_backing_bytes == coherent[index]
                && host.used_allocation_records == coherent[index] / 4096
                && host.retained_records as u64 == coherent[index] / 4096
                && host.reserved_records == 0
                && host.quarantined_records == 0
                && !host.poisoned,
            "coherent debit or custody",
        )?;
    }
    Ok(())
}

fn request_usage(
    context: &Context,
    device: RuntimeDeviceIdV1,
) -> ResultV1<RuntimeResourceCreditUsageV1> {
    context
        .allocation_admission_usage_v1(device)?
        .ok_or_else(|| "missing request account".into())
}

fn check_requests(
    context: &Context,
    devices: [RuntimeDeviceIdV1; 2],
    counts: [u64; 2],
) -> ResultV1<()> {
    for index in 0..2 {
        let usage = request_usage(context, devices[index])?;
        let capacity = RuntimeResourceVectorV1::ZERO
            .with(
                RuntimeResourceKindV1::RequestedAllocationBytes,
                REQUEST_BYTES,
            )
            .with(
                RuntimeResourceKindV1::AllocationRecords,
                REQUEST_RECORDS as u64,
            );
        let expected = RuntimeResourceVectorV1::ZERO
            .with(
                RuntimeResourceKindV1::RequestedAllocationBytes,
                BYTES as u64 * counts[index],
            )
            .with(RuntimeResourceKindV1::AllocationRecords, counts[index]);
        require(
            usage.device == devices[index]
                && usage.capacity == capacity
                && usage.record_capacity == REQUEST_RECORDS
                && usage.used == expected
                && usage.reserved_records == 0
                && usage.retained_records as u64 == counts[index]
                && usage.quarantined_records == 0
                && !usage.poisoned,
            "Context request debit or custody",
        )?;
    }
    Ok(())
}

fn initial(index: usize) -> Vec<u8> {
    if index == 1 {
        return vec![0xb7; BYTES];
    }
    (0..BYTES)
        .map(|position| ((position * 29 + position / 257 + index * 43) % 127) as u8)
        .collect()
}

fn expected(index: usize, phase: usize) -> Vec<u8> {
    let mut output = initial(index);
    // Derive from the original seeds, never from observed GPU bytes.
    if index == 1 && phase >= 1 {
        output[17..17 + PAYLOAD].copy_from_slice(&initial(0)[32..32 + PAYLOAD]);
    }
    if index == 0 && phase >= 2 {
        output[2048..2048 + PAYLOAD].copy_from_slice(&initial(2)[64..64 + PAYLOAD]);
    }
    output
}

fn check_bytes(index: usize, phase: usize, actual: &[u8]) -> ResultV1<()> {
    require(
        actual == expected(index, phase),
        "complete payload/source/guard check",
    )
}

fn check_buffers(
    context: &mut Context,
    allocations: [RuntimeAllocationIdV1; 3],
    phase: usize,
) -> ResultV1<()> {
    for (index, allocation) in allocations.into_iter().enumerate() {
        let mut actual = vec![0; BYTES];
        context.read_allocation(allocation, 0, &mut actual)?;
        check_bytes(index, phase, &actual)?;
    }
    Ok(())
}

fn allocate(
    context: &mut Context,
    device: RuntimeDeviceIdV1,
    bytes: u64,
) -> ResultV1<RuntimeAllocationIdV1> {
    Ok(context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, bytes, 4096)?)
}

fn pressure(context: &mut Context, devices: [RuntimeDeviceIdV1; 2]) -> ResultV1<()> {
    for device in devices {
        let before = context.backend().backing_usage_v1();
        let requests = [
            request_usage(context, devices[0])?,
            request_usage(context, devices[1])?,
        ];
        let journal = context.version_journal_usage_v1();
        match context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, 1, 4096) {
            Err(RuntimeErrorV1::BackendRejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity => {}
            result => {
                return Err(format!("expected native capacity rejection, got {result:?}").into());
            }
        }
        require(!context.is_terminal(), "pressure terminalized Context")?;
        require(
            context.backend().backing_usage_v1() == before,
            "pressure changed native accounts",
        )?;
        require(
            [
                request_usage(context, devices[0])?,
                request_usage(context, devices[1])?,
            ] == requests,
            "pressure retained requested credits",
        )?;
        require(
            context.version_journal_usage_v1() == journal,
            "pressure retained provisional journal entry",
        )?;
    }
    Ok(())
}

fn release_retry(
    context: &mut Context,
    device: RuntimeDeviceIdV1,
    prior: RuntimeAllocationIdV1,
) -> ResultV1<RuntimeAllocationIdV1> {
    context.release_allocation(prior)?;
    let retry = allocate(context, device, 1)?;
    require(
        retry.get() > prior.get(),
        "one-byte retry reused released identity",
    )?;
    context.write_allocation(retry, 0, &[0x5a])?;
    let mut byte = [0];
    context.read_allocation(retry, 0, &mut byte)?;
    require(byte == [0x5a], "retry bytes")?;
    context.release_allocation(retry)?;
    let restored = allocate(context, device, BYTES as u64)?;
    require(
        restored.get() > prior.get() && restored.get() > retry.get(),
        "restored allocation reused identity",
    )?;
    Ok(restored)
}

fn copy(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    allocations: [RuntimeAllocationIdV1; 2],
    offsets: [u64; 2],
    deadline: Instant,
    coherent: [u64; 2],
    unique_ids: [u64; 2],
) -> ResultV1<()> {
    let region = |allocation, access, byte_offset| RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset,
        byte_len: PAYLOAD as u64,
    };
    let mut submission = context.directed_peer_copy_v1(
        stream,
        region(allocations[0], RuntimeAccessV1::Read, offsets[0]),
        region(allocations[1], RuntimeAccessV1::Write, offsets[1]),
        &[],
    )?;
    require(Instant::now() < deadline, "copy publication deadline")?;
    require(
        context.progress_directed_peer_copy_v1(&mut submission)? == RuntimePollV1::Pending,
        "fresh directed publication did not retain a pending observation",
    )?;
    // This observes retained in-flight mappings, not physical engine timing.
    check_usage(
        context.backend().backing_usage_v1(),
        unique_ids,
        [(PADDED, 1), (2 * PADDED, 2)],
        coherent,
    )?;
    loop {
        require(Instant::now() < deadline, "copy observation deadline")?;
        match context.progress_directed_peer_copy_v1(&mut submission)? {
            RuntimePollV1::Succeeded => break,
            RuntimePollV1::Pending => std::thread::yield_now(),
            other => return Err(format!("directed copy did not succeed: {other:?}").into()),
        }
    }
    context.release_submission(submission).map_err(error)?;
    Ok(())
}

fn workload(context: &mut Context, unique_ids: [u64; 2]) -> ResultV1<()> {
    let deadline = Instant::now() + Duration::from_secs(60);
    require(context.devices().len() == 2, "exact endpoint roster")?;
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    for device in devices {
        context.configure_allocation_admission_v1(device, REQUEST_BYTES, REQUEST_RECORDS)?;
    }
    check_usage(
        context.backend().backing_usage_v1(),
        unique_ids,
        [(0, 0); 2],
        [0; 2],
    )?;
    let mut allocations = [
        allocate(context, devices[0], BYTES as u64)?,
        allocate(context, devices[1], BYTES as u64)?,
        allocate(context, devices[1], BYTES as u64)?,
    ];
    for (index, allocation) in allocations.into_iter().enumerate() {
        context.write_allocation(allocation, 0, &initial(index))?;
    }
    check_usage(
        context.backend().backing_usage_v1(),
        unique_ids,
        [(PADDED, 1), (2 * PADDED, 2)],
        [0; 2],
    )?;
    check_requests(context, devices, [1, 2])?;
    pressure(context, devices)?;
    check_buffers(context, allocations, 0)?;
    allocations[0] = release_retry(context, devices[0], allocations[0])?;
    allocations[1] = release_retry(context, devices[1], allocations[1])?;
    for (index, allocation) in allocations.into_iter().enumerate().take(2) {
        context.write_allocation(allocation, 0, &initial(index))?;
    }
    check_usage(
        context.backend().backing_usage_v1(),
        unique_ids,
        [(PADDED, 1), (2 * PADDED, 2)],
        [0; 2],
    )?;
    check_requests(context, devices, [1, 2])?;
    let streams = [
        context.create_stream(devices[1])?,
        context.create_stream(devices[0])?,
    ];
    copy(
        context,
        streams[0],
        [allocations[0], allocations[1]],
        [32, 17],
        deadline,
        [4096, 0],
        unique_ids,
    )?;
    check_usage(
        context.backend().backing_usage_v1(),
        unique_ids,
        [(PADDED, 1), (2 * PADDED, 2)],
        [4096, 0],
    )?;
    check_buffers(context, allocations, 1)?;
    copy(
        context,
        streams[1],
        [allocations[2], allocations[0]],
        [64, 2048],
        deadline,
        [4096, 4096],
        unique_ids,
    )?;
    check_usage(
        context.backend().backing_usage_v1(),
        unique_ids,
        [(PADDED, 1), (2 * PADDED, 2)],
        [4096, 4096],
    )?;
    check_buffers(context, allocations, 2)?;
    for stream in streams {
        context.destroy_stream(stream)?;
    }
    for allocation in allocations {
        context.release_allocation(allocation)?;
    }
    check_requests(context, devices, [0, 0])?;
    check_usage(
        context.backend().backing_usage_v1(),
        unique_ids,
        [(0, 0); 2],
        [4096, 4096],
    )?;
    Ok(())
}

#[derive(Debug)]
struct WorkloadAndCleanupError {
    primary: Box<dyn Error>,
    cleanup: Box<dyn Error>,
}

impl std::fmt::Display for WorkloadAndCleanupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "workload: {}; cleanup: {}",
            self.primary, self.cleanup
        )
    }
}

impl Error for WorkloadAndCleanupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.primary.as_ref())
    }
}

fn finish(work: ResultV1<()>, cleanup: ResultV1<()>) -> ResultV1<()> {
    match (work, cleanup) {
        (Ok(()), cleanup) => cleanup,
        (Err(primary), Ok(())) => Err(primary),
        (Err(primary), Err(cleanup)) => Err(Box::new(WorkloadAndCleanupError { primary, cleanup })),
    }
}

fn shutdown(context: Context, unique_ids: [u64; 2], completed: bool) -> ResultV1<()> {
    let mut backend = match context.shutdown() {
        Ok(backend) => backend,
        Err(failure) => {
            let detail = format!("logical cleanup={:?}", failure.report());
            std::mem::forget(failure.into_context());
            return Err(detail.into());
        }
    };
    let logical_usage = if completed {
        check_usage(
            backend.backing_usage_v1(),
            unique_ids,
            [(0, 0); 2],
            [4096, 4096],
        )
    } else {
        Ok(())
    };
    if let Err(cleanup) = backend.shutdown_native_v1() {
        std::mem::forget(backend);
        return finish(logical_usage, Err(error(cleanup)));
    }
    let final_usage = check_usage(backend.backing_usage_v1(), unique_ids, [(0, 0); 2], [0; 2]);
    finish(logical_usage, final_usage)
}

fn main() -> ResultV1<()> {
    let unique_ids = ids(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let backend =
        Backend::open_default_with_backing_budgets_v1(unique_ids[0], unique_ids[1], budgets())?;
    let mut context = match Context::open_with_version_journal_v1(backend, 16, 16) {
        Ok(context) => context,
        Err(failure) => {
            let (mut backend, original) = failure.into_parts();
            if let Err(cleanup) = backend.shutdown_native_v1() {
                std::mem::forget(backend);
                return finish(Err(original.into()), Err(error(cleanup)));
            }
            return Err(original.into());
        }
    };
    let result = workload(&mut context, unique_ids);
    let cleanup = shutdown(context, unique_ids, result.is_ok());
    finish(result, cleanup)?;
    println!(
        "PASS schema=fe2o3.runtime.xgmi-backing-budget.v1 uid0={:016x} uid1={:016x} device_limits=8192:3,24576:2 coherent_limits=4096:1,8192:2 pressure_dimensions=inferred-byte-record capacity_rejections=2 retries=2 fresh_identity=context directed_copies=2 statuses=2-succeeded published_snapshots=2 checked_bytes=36875 guards=complete n2_mapping_delta=0 n1=0/0-4096/0-4096/4096-0/0 request_credits=restored charges=zero cleanup=complete native_execution=true exclusive_reservation=false performance_claim=false formal_refinement=false aggregate_bound=false",
        unique_ids[0], unique_ids[1]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_cannot_replace_primary_failure() {
        let primary: Box<dyn Error> = Box::new(std::io::Error::other("primary"));
        let pointer = primary.as_ref() as *const dyn Error as *const ();
        let failure = finish(Err(primary), Ok(())).unwrap_err();
        assert_eq!(failure.as_ref() as *const dyn Error as *const (), pointer);
        let both = finish(Err(failure), Err("secondary".into())).unwrap_err();
        let combined = both.downcast_ref::<WorkloadAndCleanupError>().unwrap();
        assert_eq!(
            combined.primary.as_ref() as *const dyn Error as *const (),
            pointer
        );
        assert_eq!(combined.cleanup.to_string(), "secondary");
        assert_eq!(
            finish(Ok(()), Err("cleanup".into()))
                .unwrap_err()
                .to_string(),
            "cleanup"
        );
        assert!(finish(Ok(()), Ok(())).is_ok());
    }

    #[test]
    fn identities_reject_before_native_open() {
        assert_eq!(ids(&["0xab".into(), "19".into()]).unwrap(), [171, 19]);
        for values in [
            vec![],
            vec!["1"],
            vec!["0", "2"],
            vec!["1", "1"],
            vec!["0x", "2"],
            vec!["1", "2", "3"],
        ] {
            assert!(ids(&values.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }

    #[test]
    fn pressure_separates_padded_byte_and_record_limits() {
        assert_eq!((BYTES as u64).div_ceil(4096) * 4096, PADDED);
        assert!(PADDED + 4096 > DEVICE_LIMITS[0].0);
        assert!(2 <= DEVICE_LIMITS[0].1);
        assert!(2 * PADDED + 4096 <= DEVICE_LIMITS[1].0);
        assert!(3 > DEVICE_LIMITS[1].1);
        assert_ne!(budgets()[0], budgets()[1]);
    }

    #[test]
    fn oracle_checks_every_payload_and_guard_byte() {
        for phase in 0..=2 {
            for index in 0..3 {
                let original = expected(index, phase);
                assert_eq!(original.len(), BYTES);
                assert!(check_bytes(index, phase, &original).is_ok());
                for position in 0..BYTES {
                    let mut mutated = original.clone();
                    mutated[position] ^= 1;
                    assert!(check_bytes(index, phase, &mutated).is_err());
                }
                assert!(check_bytes(index, phase, &original[..BYTES - 1]).is_err());
            }
        }
        assert_eq!(expected(2, 2), initial(2));
        assert_eq!(expected(0, 1), initial(0));
        assert_eq!(
            &expected(1, 1)[17..17 + PAYLOAD],
            &initial(0)[32..32 + PAYLOAD]
        );
        assert_eq!(
            &expected(0, 2)[2048..2048 + PAYLOAD],
            &initial(2)[64..64 + PAYLOAD]
        );
    }

    #[test]
    fn usage_oracle_requires_exact_order_budgets_and_custody() {
        let limits = budgets();
        let zero = std::array::from_fn(|index| KfdNativeXgmiBackingUsageV1 {
            backend_device: index as u64 + 1,
            device: Some(fe2o3_kfd::Gfx942DeviceBackingUsageV1 {
                budget: limits[index].device.unwrap(),
                used_backing_bytes: 0,
                used_allocation_records: 0,
                reserved_records: 0,
                retained_records: 0,
                quarantined_records: 0,
                poisoned: false,
            }),
            host_visible: Some(fe2o3_kfd::Gfx942HostVisibleBackingUsageV1 {
                budget: limits[index].host_visible.unwrap(),
                used_backing_bytes: 0,
                used_allocation_records: 0,
                reserved_records: 0,
                retained_records: 0,
                quarantined_records: 0,
                poisoned: false,
            }),
        });
        let validate = |actual| check_usage(actual, [1, 2], [(0, 0); 2], [0; 2]);
        assert!(validate(zero).is_ok());
        let mut swapped = zero;
        swapped.swap(0, 1);
        assert!(validate(swapped).is_err());
        for index in 0..2 {
            for mutation in 0..17 {
                let mut actual = zero;
                let endpoint = &mut actual[index];
                match mutation {
                    0 => endpoint.backend_device += 10,
                    1 => endpoint.device = None,
                    2 => endpoint.host_visible = None,
                    3 => {
                        endpoint.device.as_mut().unwrap().budget = limits[1 - index].device.unwrap()
                    }
                    4 => {
                        endpoint.host_visible.as_mut().unwrap().budget =
                            limits[1 - index].host_visible.unwrap()
                    }
                    5 => endpoint.device.as_mut().unwrap().used_backing_bytes = 4096,
                    6 => endpoint.device.as_mut().unwrap().used_allocation_records = 1,
                    7 => endpoint.device.as_mut().unwrap().reserved_records = 1,
                    8 => endpoint.device.as_mut().unwrap().retained_records = 1,
                    9 => endpoint.device.as_mut().unwrap().quarantined_records = 1,
                    10 => endpoint.device.as_mut().unwrap().poisoned = true,
                    11 => endpoint.host_visible.as_mut().unwrap().used_backing_bytes = 4096,
                    12 => {
                        endpoint
                            .host_visible
                            .as_mut()
                            .unwrap()
                            .used_allocation_records = 1
                    }
                    13 => endpoint.host_visible.as_mut().unwrap().reserved_records = 1,
                    14 => endpoint.host_visible.as_mut().unwrap().poisoned = true,
                    15 => endpoint.host_visible.as_mut().unwrap().retained_records = 1,
                    16 => endpoint.host_visible.as_mut().unwrap().quarantined_records = 1,
                    _ => unreachable!(),
                }
                assert!(validate(actual).is_err());
            }
        }
    }
}
