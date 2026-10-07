//! Actual native V5 ownership on every selected gfx942 child, then serial copies.
mod native_bootstrap;
mod native_roster_case;
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_conditional_custodian_application::fill_write_only_gpu;
use fe2o3_host::{
    ChargedTypedResultV1, GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeResultBudgetV1,
    GeneratedRuntimeWriteSlice, NativeConditionalFillIntakeErrorV1 as IntakeError,
    ProvedNativeConditionalFillApplicationV1 as Proved,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_runtime::*;
use native_bootstrap::{Result, Summary, checked};
use native_roster_case::{Case, FRAME_BYTES, GUARD_BYTES, Ledger, Mode};
use std::time::Instant;
type Backend = KfdMultiDeviceRuntimeBackendV1;
type Context = RuntimeContextV1<Backend>;

fn main() -> Result<()> {
    let case = native_roster_case::parse(std::env::args_os().skip(1))?;
    native_bootstrap::run(&case.producer_source, |application, budget, deadline| {
        let fields = run(application, budget, &case, deadline)?;
        Ok(Summary {
            schema: "fe2o3.native-conditional-fill-roster.v1",
            fields,
        })
    })
}

fn run<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    case: &Case,
    deadline: Instant,
) -> Result<String> {
    let backend = checked(match case.mode {
        Mode::NativeXgmi => {
            Backend::open_worker_v3_generated_only_with_native_peer_copy_v1(case.devices.clone())
        }
        Mode::HostStaged => Backend::open_worker_v3_generated_only_v1(case.devices.clone()),
    })?;
    let mut context = match Context::open_with_version_journal_v1(backend, 256, 256) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("native roster context initialization refused: {error:?}");
            std::process::abort();
        }
    };
    let operation = execute(application, budget, &mut context, case, deadline);
    let mut backend = match context.shutdown() {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("native roster context did not settle: {error:?}");
            std::process::abort();
        }
    };
    if let Err(error) = backend.shutdown_native_v1() {
        eprintln!("native roster backend did not settle: {error:?}");
        std::process::abort();
    }
    drop(backend);
    operation
}

fn execute<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    context: &mut Context,
    case: &Case,
    deadline: Instant,
) -> Result<String> {
    if context.devices().len() != case.devices.len()
        || context
            .devices()
            .iter()
            .any(|d| d.target() != "gfx942:xnack-")
    {
        return Err("native roster requires the exact selected gfx942 device set".into());
    }
    let devices: Vec<_> = context.devices().iter().map(|d| d.id()).collect();
    let mut observed = Vec::with_capacity(devices.len());
    let mut streams = Vec::with_capacity(devices.len());
    for device in &devices {
        let fact = checked(context.with_gfx942_preparation_device_v1(*device, |owner| {
            Ok::<_, String>((
                owner.observation().unique_id(),
                owner.observation().render_minor(),
            ))
        }))?;
        observed.push(*fact.value());
        streams.push(checked(context.create_stream(*device))?);
    }
    case.check_roster(&observed)?;
    let result_budget = checked(GeneratedRuntimeResultBudgetV1::new(8192, devices.len()))?;
    let results = checked(
        application.with_native_invocation_scope(budget, deadline, |native| {
            context
                .with_generated_gfx942_scope_v1(devices.len(), deadline, |scope| {
                    let mut pending = Vec::with_capacity(devices.len());
                    for (shard, (&device, &stream)) in devices.iter().zip(&streams).enumerate() {
                        let (output, observer) = GeneratedRuntimeWriteSlice::new_charged(
                            vec![u32::MAX; case.elements(shard)].into_boxed_slice(),
                        );
                        let ticket = scope
                            .try_submit_v1(device, stream, |checked_device| {
                                native
                                    .prepare_generated_invocation::<fill_write_only_gpu::Marker, _>(
                                        fill_write_only_gpu::RuntimeArguments::new(output),
                                        checked_device,
                                        AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1])
                                            .expect("fixed closed geometry"),
                                        30_000,
                                        GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1),
                                        &result_budget,
                                    )
                            })
                            .map_err(|e| {
                                IntakeError::Rejected(format!("native roster submission: {e:?}"))
                            })?;
                        pending.push((ticket, observer));
                    }
                    scope.drain_v1().map_err(|e| {
                        IntakeError::Rejected(format!("native roster settlement: {e:?}"))
                    })?;
                    pending
                        .into_iter()
                        .map(|(ticket, mut observer)| {
                            observer
                                .take_scoped_completed_v1(scope, &ticket)
                                .map_err(|e| {
                                    IntakeError::Rejected(format!("native roster decoder: {e:?}"))
                                })?
                                .ok_or_else(|| {
                                    IntakeError::Rejected("native roster result contention".into())
                                })
                        })
                        .collect::<std::result::Result<Vec<_>, _>>()
                })
                .map_err(|e| IntakeError::Rejected(format!("native roster lexical scope: {e:?}")))?
        }),
    )?;
    let mut ledger = Ledger::new();
    for (shard, result) in results.iter().enumerate() {
        case.check_fill(shard, result.as_slice())?;
        ledger.fill(case, shard)?;
    }
    checked(application.revalidate(deadline, budget))?;
    transport(
        context,
        &devices,
        &streams,
        case,
        &results,
        &mut ledger,
        deadline,
    )?;
    checked(application.revalidate(deadline, budget))?;
    drop(results);
    for stream in streams.into_iter().rev() {
        checked(context.destroy_stream(stream))?;
    }
    let usage = result_budget.usage();
    if usage.reserved_peak_bytes != 0
        || usage.retained_members != 0
        || usage.quarantined_members != 0
        || usage.unissued_members != 0
        || usage.poisoned
    {
        return Err(format!(
            "native roster result credits not fully refunded: {usage:?}"
        ));
    }
    ledger.finish(case, &observed)
}

#[derive(Clone, Copy)]
struct Buffers {
    host: RuntimeAllocationIdV1,
    source: RuntimeAllocationIdV1,
    guard: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
}
fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    offset: usize,
    bytes: usize,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: offset as u64,
        byte_len: bytes as u64,
    }
}
fn settle<A>(
    context: &mut Context,
    mut submission: RuntimeSubmissionV1<A>,
    deadline: Instant,
) -> Result<()> {
    loop {
        if Instant::now() >= deadline {
            return Err("native roster transfer deadline; not settlement".into());
        }
        checked(context.drain(&mut submission, deadline))?;
        match checked(context.query_submission(&submission))? {
            RuntimeCompletionStatusV1::Succeeded => break,
            RuntimeCompletionStatusV1::Pending => std::thread::yield_now(),
            other => return Err(format!("native roster transfer did not succeed: {other:?}")),
        }
    }
    checked(context.release_submission(submission))
}

#[allow(clippy::too_many_arguments)]
fn transport(
    context: &mut Context,
    devices: &[RuntimeDeviceIdV1],
    streams: &[RuntimeStreamIdV1],
    case: &Case,
    results: &[ChargedTypedResultV1<u32>],
    ledger: &mut Ledger,
    deadline: Instant,
) -> Result<()> {
    if context.backend().completed_compute_xgmi_copies_v1() != 0 {
        return Err("native peer counter was not initially zero".into());
    }
    let mut buffers = Vec::with_capacity(devices.len());
    let mut payloads = Vec::with_capacity(devices.len());
    for (shard, &device) in devices.iter().enumerate() {
        // This UID/shard tag is host-authored routing data, not a kernel theorem.
        let payload = case.payload(shard, results[shard].as_slice())?;
        let mut allocate =
            |kind, bytes| checked(context.allocate(device, kind, bytes as u64, 4096));
        let buffer = Buffers {
            host: allocate(RuntimeMemoryKindV1::HostVisible, payload.len())?,
            source: allocate(RuntimeMemoryKindV1::DeviceLocal, payload.len())?,
            guard: allocate(RuntimeMemoryKindV1::HostVisible, FRAME_BYTES)?,
            destination: allocate(RuntimeMemoryKindV1::DeviceLocal, FRAME_BYTES)?,
        };
        checked(context.write_host_visible_allocation_v1(buffer.host, &payload))?;
        let upload = checked(context.copy_async(
            streams[shard],
            region(buffer.host, RuntimeAccessV1::Read, 0, payload.len()),
            region(buffer.source, RuntimeAccessV1::Write, 0, payload.len()),
            &[],
        ))?;
        settle(context, upload, deadline)?;
        payloads.push(payload);
        buffers.push(buffer);
    }
    // All generated submissions are settled. Each complete guarded route is
    // settled/read back before the next: this deliberately makes no overlap claim.
    for (source, destination) in case.routes() {
        let source_buffer = buffers[source];
        let destination_buffer = buffers[destination];
        let payload = &payloads[source];
        checked(context.write_host_visible_allocation_v1(
            destination_buffer.guard,
            &case.frame(source, destination),
        ))?;
        let reset = checked(context.copy_async(
            streams[destination],
            region(
                destination_buffer.guard,
                RuntimeAccessV1::Read,
                0,
                FRAME_BYTES,
            ),
            region(
                destination_buffer.destination,
                RuntimeAccessV1::Write,
                0,
                FRAME_BYTES,
            ),
            &[],
        ))?;
        settle(context, reset, deadline)?;
        let copy = checked(context.peer_copy(
            streams[destination],
            region(
                source_buffer.source,
                RuntimeAccessV1::Read,
                0,
                payload.len(),
            ),
            region(
                destination_buffer.destination,
                RuntimeAccessV1::Write,
                GUARD_BYTES,
                payload.len(),
            ),
            &[],
        ))?;
        settle(context, copy, deadline)?;
        let mut source_readback = vec![0; payload.len()];
        let mut destination_readback = vec![0; FRAME_BYTES];
        checked(context.read_allocation(source_buffer.source, 0, &mut source_readback))?;
        checked(context.read_allocation(
            destination_buffer.destination,
            0,
            &mut destination_readback,
        ))?;
        case.check_route(
            source,
            destination,
            payload,
            &source_readback,
            &destination_readback,
        )?;
        ledger.route(
            case,
            (source, destination),
            context.backend().completed_compute_xgmi_copies_v1(),
        )?;
    }
    for (buffer, payload) in buffers.iter().zip(&payloads) {
        let mut actual = vec![0; payload.len()];
        checked(context.read_allocation(buffer.source, 0, &mut actual))?;
        if actual != *payload {
            return Err("native roster source changed after its final directed copy".into());
        }
    }
    for buffer in buffers.into_iter().rev() {
        for allocation in [buffer.destination, buffer.guard, buffer.source, buffer.host] {
            checked(context.release_allocation(allocation))?;
        }
    }
    Ok(())
}
