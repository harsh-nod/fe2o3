//! Real generated invocations followed by complete, serial directed peer qualification.
use super::*;
use crate::roster_case::{CampaignLedger, MAX_FRAME_BYTES};

pub(super) fn execute(
    engine: &mut Engine,
    handle: &Handle,
    artifact: &Arc<Artifact>,
    budget: &GeneratedRuntimeResultBudgetV1,
    case: RosterCase,
    deadline: Instant,
) -> Result<(Arc<Vec<ChargedTypedResultV1<u32>>>, CampaignReport)> {
    let (devices, streams) = with_context(engine, handle, deadline, move |context| {
        if context.devices().len() != case.count()
            || context
                .devices()
                .iter()
                .any(|device| device.target() != "gfx942:xnack-")
        {
            return Err(
                "roster requires the exact number of admitted gfx942:xnack- devices".into(),
            );
        }
        let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
        let mut observed = Vec::with_capacity(case.count());
        for device in &devices {
            let observation = checked(
                "roster hardware identity",
                context.with_gfx942_preparation_device_v1(*device, |owner| {
                    Ok::<_, String>(owner.observation().unique_id())
                }),
            )?;
            observed.push(*observation.value());
        }
        case.check_observed_roster(&observed)?;
        let streams = devices
            .iter()
            .map(|device| checked("roster stream", context.create_stream(*device)))
            .collect::<Result<Vec<_>>>()?;
        Ok((devices, streams))
    })?;
    let mut pending = Vec::with_capacity(case.count());
    for (shard, (&device, &stream)) in devices.iter().zip(&streams).enumerate() {
        let (output, observer) = GeneratedRuntimeWriteSlice::new_charged(
            vec![u32::MAX; case.elements(shard)].into_boxed_slice(),
        );
        let prepare = checked(
            "prepare roster fill",
            artifact.prepare_generated_multi_context_invocation_async(
                fill_write_only_gpu::RuntimeArguments::new(output),
                handle,
                device,
                checked(
                    "roster geometry",
                    AqlDispatchGeometryV1::new([128, 1, 1], [64, 1, 1]),
                )?,
                0,
                30_000,
                GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1),
                budget,
                deadline,
            ),
        )?;
        let prepared = checked("prepare roster reply", drive(engine, prepare, deadline)?)?;
        let prepared = checked("prepare roster fill", prepared)?;
        let reserve = handle
            .try_reserve_prepared_v1(prepared)
            .map_err(|error| format!("reserve roster fill: {:?}", error.error))?;
        let reserved = checked("reserve roster reply", drive(engine, reserve, deadline)?)?
            .map_err(|error| format!("reserve roster fill: {:?}", error.error))?;
        let activate = handle
            .try_activate_generated_v1(reserved, stream)
            .map_err(|error| format!("activate roster fill: {:?}", error.error))?;
        let completion = checked("activate roster reply", drive(engine, activate, deadline)?)?
            .map_err(|error| format!("activate roster fill: {:?}", error.error))?;
        pending.push((completion, observer));
    }
    let mut ledger = CampaignLedger::new(case);
    let mut results = Vec::with_capacity(case.count());
    for (shard, (completion, mut observer)) in pending.into_iter().enumerate() {
        let receipt = checked(
            "roster fill completion",
            checked(
                "roster completion reply",
                drive(engine, completion, deadline)?,
            )?,
        )?;
        let result = take_result(&mut observer, &receipt, deadline)?;
        case.check_fill(shard, result.as_slice())?;
        ledger.record_fill(shard)?;
        results.push(result);
    }
    let results = Arc::new(results);
    transport(
        engine,
        handle,
        &devices,
        &streams,
        &results,
        case,
        &mut ledger,
        deadline,
    )?;
    checked(
        "retained roster artifact after transport",
        artifact.revalidate(deadline),
    )?;
    with_context(engine, handle, deadline, move |context| {
        for stream in streams.into_iter().rev() {
            checked("destroy roster stream", context.destroy_stream(stream))?;
        }
        Ok(())
    })?;
    let drain = checked("begin roster drain", handle.begin_drain(1024))?;
    let report = checked("roster drain reply", drive(engine, drain, deadline)?)?;
    if report.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent {
        return Err(format!("roster drain incomplete: {report:?}"));
    }
    Ok((results, ledger.finish()?))
}

#[allow(clippy::too_many_arguments)]
fn transport(
    engine: &mut Engine,
    handle: &Handle,
    devices: &[RuntimeDeviceIdV1],
    streams: &[RuntimeStreamIdV1],
    results: &Arc<Vec<ChargedTypedResultV1<u32>>>,
    case: RosterCase,
    ledger: &mut CampaignLedger,
    deadline: Instant,
) -> Result<()> {
    let devices = devices.to_vec();
    let staged_results = Arc::clone(results);
    let (buffers, payloads) = with_context(engine, handle, deadline, move |context| {
        if context.backend().completed_compute_xgmi_copies_v1() != 0 {
            return Err("roster native peer counter was not initially zero".into());
        }
        let mut buffers = Vec::with_capacity(case.count());
        let mut payloads = Vec::with_capacity(case.count());
        for (shard, device) in devices.into_iter().enumerate() {
            let mut encoded = vec![0; case.elements(shard) * 4];
            checked(
                "encode actual completed shard",
                staged_results[shard].encode_into_v1(&mut encoded),
            )?;
            let payload = case.payload(shard, &encoded)?;
            let host = checked(
                "allocate shard staging",
                context.allocate(
                    device,
                    RuntimeMemoryKindV1::HostVisible,
                    payload.len() as u64,
                    4096,
                ),
            )?;
            let source = checked(
                "allocate PUBLIC shard",
                context.allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    payload.len() as u64,
                    4096,
                ),
            )?;
            let guard_host = checked(
                "allocate route sentinel staging",
                context.allocate(
                    device,
                    RuntimeMemoryKindV1::HostVisible,
                    MAX_FRAME_BYTES as u64,
                    4096,
                ),
            )?;
            let destination = checked(
                "allocate PUBLIC route destination",
                context.allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    MAX_FRAME_BYTES as u64,
                    4096,
                ),
            )?;
            // Encoding and the UID tag establish ordinary host-write provenance only.
            checked(
                "stage labelled completed shard",
                context.write_host_visible_allocation_v1(host, &payload),
            )?;
            buffers.push(Buffers {
                host,
                source,
                guard_host,
                destination,
            });
            payloads.push(payload);
        }
        Ok((buffers, payloads))
    })?;
    for (shard, (&stream, buffer)) in streams.iter().zip(&buffers).enumerate() {
        let bytes = case.payload_bytes(shard);
        let upload = checked(
            "upload roster shard",
            handle.copy_async(
                stream,
                region(buffer.host, RuntimeAccessV1::Read, 0, bytes),
                region(buffer.source, RuntimeAccessV1::Write, 0, bytes),
                Vec::new(),
            ),
        )?;
        finish_transfer(engine, handle, upload, deadline)?;
    }
    // All compute receipts were consumed before this loop. Every edge is settled and
    // read back before the next edge; neither queued work nor these facts prove overlap.
    for (source, destination) in case.routes() {
        let source_buffer = buffers[source];
        let destination_buffer = buffers[destination];
        let payload = payloads[source].clone();
        with_context(engine, handle, deadline, move |context| {
            checked(
                "reset directed route guards",
                context.write_host_visible_allocation_v1(
                    destination_buffer.guard_host,
                    &case.destination_frame(source, destination),
                ),
            )
        })?;
        let reset = checked(
            "upload directed route guards",
            handle.copy_async(
                streams[destination],
                region(
                    destination_buffer.guard_host,
                    RuntimeAccessV1::Read,
                    0,
                    MAX_FRAME_BYTES,
                ),
                region(
                    destination_buffer.destination,
                    RuntimeAccessV1::Write,
                    0,
                    MAX_FRAME_BYTES,
                ),
                Vec::new(),
            ),
        )?;
        finish_transfer(engine, handle, reset, deadline)?;
        let peer = checked(
            "enqueue directed roster peer",
            handle.peer_copy_tracked(
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
                Vec::new(),
            ),
        )?;
        finish_transfer(engine, handle, peer, deadline)?;
        let count = with_context(engine, handle, deadline, move |context| {
            let mut source_readback = vec![0; payload.len()];
            let mut destination_readback = vec![0; MAX_FRAME_BYTES];
            checked(
                "read complete roster source",
                context.read_allocation(source_buffer.source, 0, &mut source_readback),
            )?;
            checked(
                "read complete directed destination",
                context.read_allocation(
                    destination_buffer.destination,
                    0,
                    &mut destination_readback,
                ),
            )?;
            case.check_route(
                source,
                destination,
                &payload,
                &source_readback,
                &destination_readback,
            )?;
            Ok(context.backend().completed_compute_xgmi_copies_v1())
        })?;
        ledger.record_peer((source, destination), count)?;
    }
    with_context(engine, handle, deadline, move |context| {
        for (buffer, payload) in buffers.iter().zip(&payloads) {
            let mut source = vec![0; payload.len()];
            checked(
                "final roster source readback",
                context.read_allocation(buffer.source, 0, &mut source),
            )?;
            if source != *payload {
                return Err("roster source changed after its final directed copy".into());
            }
        }
        for buffer in buffers.into_iter().rev() {
            for allocation in [
                buffer.destination,
                buffer.guard_host,
                buffer.source,
                buffer.host,
            ] {
                checked(
                    "release roster transport allocation",
                    context.release_allocation(allocation),
                )?;
            }
        }
        Ok(())
    })
}
