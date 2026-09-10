//! Native retained-custody qualification, not a device-overlap measurement.
#[cfg(not(feature = "hardware-qualification"))]
fn main() {
    eprintln!("hardware-qualification feature required");
    std::process::exit(2);
}

#[cfg(feature = "hardware-qualification")]
mod enabled {
    use fe2o3_runtime::qualification_gfx942_inplace_transform_v1::*;
    use fe2o3_runtime::*;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::error::Error;
    use std::fmt::Debug;
    use std::time::{Duration, Instant};

    type ResultV1<T> = Result<T, Box<dyn Error>>;
    const PAD: usize = 128;
    const SMALL: usize = 1024 * 1024 + 257;
    const LARGE: usize = 0x003f_ffe0 + 257;

    fn error(value: impl Debug) -> Box<dyn Error> {
        format!("{value:?}").into()
    }
    fn hex(value: [u8; 32]) -> String {
        value.iter().map(|byte| format!("{byte:02x}")).collect()
    }
    #[derive(Clone, Copy, Debug)]
    enum ObservationPhase {
        Setup,
        First,
        Both,
        AfterCopy,
        AfterCompute,
    }
    fn observation(
        context: &RuntimeContextV1<KfdRuntimeBackendV1>,
        phase: ObservationPhase,
    ) -> ResultV1<KfdR66RetainedCustodyObservationV1> {
        context
            .backend()
            .diagnose_r66_retained_custody_v1()
            .map_err(|failure| format!("R66 observation phase={phase:?}: {failure}").into())
    }
    fn json_observation(value: KfdR66RetainedCustodyObservationV1) -> Value {
        json!({"compute": value.compute.map(hex), "compute_membership": value.compute_membership.map(hex),
            "copy": value.copy.map(hex), "copy_membership": value.copy_membership.map(hex), "copy_packets": value.copy_packets})
    }
    fn region(
        allocation: RuntimeAllocationIdV1,
        access: RuntimeAccessV1,
        offset: usize,
        len: usize,
    ) -> RuntimeMemoryRegionV1 {
        RuntimeMemoryRegionV1 {
            allocation,
            access,
            byte_offset: offset as u64,
            byte_len: len as u64,
        }
    }
    fn finish<T>(
        context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
        stream: RuntimeStreamIdV1,
        submission: &mut RuntimeSubmissionV1<T>,
    ) -> ResultV1<()> {
        context.flush_stream(stream).map_err(error)?;
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("R66 completion deadline expired".into());
            }
            match context
                .wait(submission, remaining.min(Duration::from_micros(50)))
                .map_err(error)?
            {
                RuntimePollV1::Succeeded => return Ok(()),
                RuntimePollV1::Pending => context.flush_stream(stream).map_err(error)?,
                RuntimePollV1::Failed { code } => {
                    return Err(format!("R66 submission failed: {code}").into());
                }
            }
        }
    }
    fn copy(
        context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
        stream: RuntimeStreamIdV1,
        source: RuntimeAllocationIdV1,
        destination: RuntimeAllocationIdV1,
        offset: usize,
        bytes: usize,
    ) -> ResultV1<RuntimeSubmissionV1<RuntimeCopyV1>> {
        context
            .copy_async(
                stream,
                region(source, RuntimeAccessV1::Read, offset, bytes),
                region(destination, RuntimeAccessV1::Write, offset, bytes),
                &[],
            )
            .map_err(error)
    }
    fn copy_complete(
        context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
        stream: RuntimeStreamIdV1,
        source: RuntimeAllocationIdV1,
        destination: RuntimeAllocationIdV1,
        bytes: usize,
    ) -> ResultV1<()> {
        let mut submission = copy(context, stream, source, destination, 0, bytes)?;
        finish(context, stream, &mut submission)?;
        context.release_submission(submission).map_err(error)
    }
    fn check(
        context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
        allocation: RuntimeAllocationIdV1,
        expected: &[u8],
    ) -> ResultV1<String> {
        let mut actual = vec![0; expected.len()];
        context
            .read_allocation(allocation, 0, &mut actual)
            .map_err(error)?;
        if actual != expected {
            return Err("R66 independent full-buffer/padding comparison failed".into());
        }
        Ok(hex(Sha256::digest(&actual).into()))
    }
    fn shutdown(mut context: RuntimeContextV1<KfdRuntimeBackendV1>) -> ResultV1<()> {
        let report = context.cleanup();
        if !report.is_complete() {
            let detail = format!("R66 cleanup incomplete: {report:?}");
            std::mem::forget(context);
            return Err(detail.into());
        }
        let credits_clean = context.devices().iter().all(|device| {
            match context.allocation_admission_usage_v1(device.id()) {
                Ok(None) => true,
                Ok(Some(usage)) => {
                    usage.used == RuntimeResourceVectorV1::ZERO
                        && usage.reserved_records == 0
                        && usage.retained_records == 0
                        && usage.quarantined_records == 0
                        && !usage.poisoned
                }
                Err(_) => false,
            }
        });
        if !credits_clean {
            std::mem::forget(context);
            return Err("R66 cleanup retained requested-allocation credits".into());
        }
        let mut backend = match context.shutdown() {
            Ok(backend) => backend,
            Err(failure) => {
                let detail = format!("{failure:?}");
                std::mem::forget(failure.into_context());
                return Err(detail.into());
            }
        };
        if let Err(failure) = backend.shutdown_native_v1() {
            let detail = format!("{failure:?}");
            std::mem::forget(backend);
            return Err(detail.into());
        }
        Ok(())
    }
    fn case(
        unique_id: u64,
        ordinal: usize,
        bytes: usize,
        packets: usize,
        h2d: bool,
        compute_first: bool,
    ) -> ResultV1<Value> {
        let admitted = admit_gfx942_inplace_transform_qualification_v1()?;
        let (inputs, _) = admitted.host_buffers()?.into_parts();
        let input =
            Gfx942InplaceTransformQualificationInputV1::for_global_iteration(ordinal as u64);
        let input_bytes = &inputs[ordinal % 2];
        let backend =
            KfdRuntimeBackendV1::open_gfx942_inplace_transform_qualification_v1(unique_id)?;
        let mut context = RuntimeContextV1::open(backend).map_err(error)?;
        let result = (|| -> ResultV1<Value> {
            if context.devices().len() != 1 || context.devices()[0].target() != "gfx942:xnack-" {
                return Err("R66 target mismatch".into());
            }
            let device = context.devices()[0].id();
            let compute_stream = context.create_stream(device).map_err(error)?;
            let copy_stream = context.create_stream(device).map_err(error)?;
            let module = context
                .load_module(device, admitted.hsaco())
                .map_err(error)?;
            let kernel = context
                .resolve_kernel::<Gfx942InplaceTransformQualificationArgumentsV1>(
                    module,
                    GFX942_INPLACE_TRANSFORM_QUALIFICATION_KERNEL_V1,
                )
                .map_err(error)?;
            let kernel_bytes = GFX942_INPLACE_TRANSFORM_QUALIFICATION_BUFFER_BYTES_V1;
            let total = bytes + 2 * PAD;
            let requested_capacity = (3 * kernel_bytes + 4 * total) as u64;
            context
                .configure_allocation_admission_v1(device, requested_capacity, 7)
                .map_err(error)?;
            let mut allocate = |kind, len| {
                context
                    .allocate(device, kind, len as u64, 4096)
                    .map_err(error)
            };
            let compute_upload = allocate(RuntimeMemoryKindV1::HostVisible, kernel_bytes)?;
            let compute_device = allocate(RuntimeMemoryKindV1::DeviceLocal, kernel_bytes)?;
            let compute_download = allocate(RuntimeMemoryKindV1::HostVisible, kernel_bytes)?;
            let upload = allocate(RuntimeMemoryKindV1::HostVisible, total)?;
            let copy_device = allocate(RuntimeMemoryKindV1::DeviceLocal, total)?;
            let download = allocate(RuntimeMemoryKindV1::HostVisible, total)?;
            let verify = allocate(RuntimeMemoryKindV1::HostVisible, total)?;
            let full_usage = context
                .allocation_admission_usage_v1(device)
                .map_err(error)?
                .ok_or("R66 requested-allocation credit account missing")?;
            let expected_credits = RuntimeResourceVectorV1::ZERO
                .with(
                    RuntimeResourceKindV1::RequestedAllocationBytes,
                    requested_capacity,
                )
                .with(RuntimeResourceKindV1::AllocationRecords, 7);
            if full_usage.device != device
                || full_usage.used != expected_credits
                || full_usage.capacity != expected_credits
                || full_usage.record_capacity != 7
                || full_usage.reserved_records != 0
                || full_usage.retained_records != 7
                || full_usage.quarantined_records != 0
                || full_usage.poisoned
            {
                return Err("R66 full requested-allocation credit usage mismatch".into());
            }
            if !matches!(
                context.allocate(device, RuntimeMemoryKindV1::HostVisible, 1, 4096),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::Capacity
                ))
            ) || context
                .allocation_admission_usage_v1(device)
                .map_err(error)?
                != Some(full_usage)
            {
                return Err("R66 excess allocation was not rejected with unchanged credits".into());
            }
            let mut pattern = vec![0xc7; total];
            for index in 0..bytes {
                pattern[PAD + index] = ((index * 29 + index / 257 + ordinal * 17) % 251) as u8;
            }
            let mut device_expected = vec![0xa5; total];
            if !h2d {
                device_expected[PAD..PAD + bytes].copy_from_slice(&pattern[PAD..PAD + bytes]);
            }
            context
                .write_allocation(upload, 0, &device_expected)
                .map_err(error)?;
            copy_complete(&mut context, copy_stream, upload, copy_device, total)?;
            context
                .write_allocation(upload, 0, &pattern)
                .map_err(error)?;
            context
                .write_allocation(download, 0, &vec![0xd3; total])
                .map_err(error)?;
            context
                .write_allocation(compute_upload, 0, input_bytes)
                .map_err(error)?;
            copy_complete(
                &mut context,
                compute_stream,
                compute_upload,
                compute_device,
                kernel_bytes,
            )?;
            let setup = observation(&context, ObservationPhase::Setup)?;
            if setup.compute.is_some() || setup.copy.is_some() {
                return Err("R66 observation phase=Setup retained unexpected native work".into());
            }
            let arguments = Gfx942InplaceTransformQualificationArgumentsV1::new(compute_device);
            let (source, destination) = if h2d {
                (upload, copy_device)
            } else {
                (copy_device, download)
            };
            let mut compute_submission;
            let mut copy_submission;
            let first;
            if compute_first {
                compute_submission = context
                    .launch(
                        compute_stream,
                        &kernel,
                        &arguments,
                        GFX942_INPLACE_TRANSFORM_QUALIFICATION_GEOMETRY_V1,
                        &[],
                    )
                    .map_err(error)?;
                context.flush_stream(compute_stream).map_err(error)?;
                first = observation(&context, ObservationPhase::First)?;
                copy_submission = copy(&mut context, copy_stream, source, destination, PAD, bytes)?;
                context.flush_stream(copy_stream).map_err(error)?;
            } else {
                copy_submission = copy(&mut context, copy_stream, source, destination, PAD, bytes)?;
                context.flush_stream(copy_stream).map_err(error)?;
                first = observation(&context, ObservationPhase::First)?;
                compute_submission = context
                    .launch(
                        compute_stream,
                        &kernel,
                        &arguments,
                        GFX942_INPLACE_TRANSFORM_QUALIFICATION_GEOMETRY_V1,
                        &[],
                    )
                    .map_err(error)?;
                context.flush_stream(compute_stream).map_err(error)?;
            }
            let both = observation(&context, ObservationPhase::Both)?;
            if both.compute.is_none()
                || both.copy.is_none()
                || both.copy_packets != packets
                || (compute_first
                    && (first.compute != both.compute
                        || first.compute_membership != both.compute_membership
                        || first.copy.is_some()))
                || (!compute_first
                    && (first.copy != both.copy
                        || first.copy_membership != both.copy_membership
                        || first.compute.is_some()))
            {
                return Err(
                    "R66 observation phase=Both publication order or retained identity mismatch"
                        .into(),
                );
            }
            finish(&mut context, copy_stream, &mut copy_submission)?;
            context.release_submission(copy_submission).map_err(error)?;
            let after_copy = observation(&context, ObservationPhase::AfterCopy)?;
            if after_copy.compute != both.compute
                || after_copy.compute_membership != both.compute_membership
                || after_copy.copy.is_some()
                || after_copy.copy_packets != 0
            {
                return Err(
                    "R66 observation phase=AfterCopy copy retirement lost compute custody".into(),
                );
            }
            finish(&mut context, compute_stream, &mut compute_submission)?;
            context
                .release_submission(compute_submission)
                .map_err(error)?;
            let after_compute = observation(&context, ObservationPhase::AfterCompute)?;
            if after_compute.compute.is_some() || after_compute.copy.is_some() {
                return Err(
                    "R66 observation phase=AfterCompute native work retained after completion"
                        .into(),
                );
            }
            if context
                .allocation_admission_usage_v1(device)
                .map_err(error)?
                != Some(full_usage)
            {
                return Err("R66 work retirement prematurely refunded allocation credits".into());
            }
            let mut download_expected = vec![0xd3; total];
            if h2d {
                device_expected[PAD..PAD + bytes].copy_from_slice(&pattern[PAD..PAD + bytes]);
            } else {
                download_expected[PAD..PAD + bytes].copy_from_slice(&pattern[PAD..PAD + bytes]);
            }
            let download_sha256 = check(&mut context, download, &download_expected)?;
            copy_complete(&mut context, copy_stream, copy_device, verify, total)?;
            let device_sha256 = check(&mut context, verify, &device_expected)?;
            let upload_sha256 = check(&mut context, upload, &pattern)?;
            let compute_input_sha256 = check(&mut context, compute_upload, input_bytes)?;
            copy_complete(
                &mut context,
                compute_stream,
                compute_device,
                compute_download,
                kernel_bytes,
            )?;
            let mut computed = vec![0; kernel_bytes];
            context
                .read_allocation(compute_download, 0, &mut computed)
                .map_err(error)?;
            validate_gfx942_inplace_transform_output_v1(input, &computed)?;
            Ok(
                json!({"ordinal": ordinal, "bytes": bytes, "packets": packets,
                    "logical_credits": {"scope": "requested-allocation-bytes-and-records", "capacity_bytes": requested_capacity,
                        "capacity_records": 7, "full_bytes": requested_capacity, "full_records": 7,
                        "eighth_request": "capacity-rejected-unchanged", "retirement": "retained", "after_cleanup": "zero"},
                "order": if compute_first {"compute-first"} else {"copy-first"}, "direction": if h2d {"h2d"} else {"d2h"},
                "first": json_observation(first), "both": json_observation(both),
                "after_copy": json_observation(after_copy), "after_compute": json_observation(after_compute), "canaries": "complete",
                "download_sha256": download_sha256, "device_sha256": device_sha256, "upload_sha256": upload_sha256,
                "compute_input_sha256": compute_input_sha256, "compute_output_sha256": hex(Sha256::digest(&computed).into())}),
            )
        })();
        let cleanup = shutdown(context);
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => {
                Err(format!("R66 operation: {error}; cleanup: {cleanup}").into())
            }
        }
    }
    pub fn run() -> ResultV1<()> {
        let args = std::env::args().collect::<Vec<_>>();
        if args.len() != 2 {
            return Err("expected one GPU unique ID".into());
        }
        let unique_id = if let Some(hex) = args[1].strip_prefix("0x") {
            u64::from_str_radix(hex, 16)?
        } else {
            args[1].parse()?
        };
        if unique_id == 0 {
            return Err("zero GPU unique ID".into());
        }
        let cases = std::thread::Builder::new()
            .name("r66-owner".into())
            .spawn(move || -> Result<Vec<Value>, String> {
                let mut cases = Vec::new();
                for (bytes, packets) in [(SMALL, 1), (LARGE, 2)] {
                    for h2d in [true, false] {
                        for compute_first in [true, false] {
                            let ordinal = cases.len();
                            cases.push(
                                case(unique_id, ordinal, bytes, packets, h2d, compute_first)
                                    .map_err(|error| format!(
                                        "R66 cell={ordinal} bytes={bytes} packets={packets} direction={} order={}: {error}",
                                        if h2d { "h2d" } else { "d2h" },
                                        if compute_first { "compute-first" } else { "copy-first" },
                                    ))?,
                            );
                        }
                    }
                }
                Ok(cases)
            })?
            .join()
            .map_err(|_| "R66 owner panicked")?
            .map_err(|error| -> Box<dyn Error> { error.into() })?;
        println!(
            "{}",
            json!({"schema": "fe2o3.runtime.r66-retained-coexistence.v1", "cases": cases, "owner_threads": 1, "cleanup": "complete", "physical_overlap": "unmeasured"})
        );
        Ok(())
    }
}

#[cfg(feature = "hardware-qualification")]
fn main() {
    if let Err(error) = enabled::run() {
        eprintln!("R66 qualification failed: {error}");
        std::process::exit(1);
    }
}
