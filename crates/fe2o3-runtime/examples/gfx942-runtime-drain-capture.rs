//! Copy-only outstanding-prefix drain qualification; no timing or compute claims.
#[cfg(feature = "hardware-qualification")]
mod enabled {
    use fe2o3_runtime::completion::*;
    use fe2o3_runtime::*;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::error::Error;
    use std::future::Future;
    use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Wake, Waker};
    use std::thread;
    use std::time::{Duration, Instant};

    const BODY: usize = 1024 * 1024 + 257;
    const DATA: usize = BODY + 514;
    const VERIFY_OFFSET: usize = DATA + 193;
    const CAPTURE: usize = VERIFY_OFFSET + DATA + 211;
    const REQUESTED: usize = 2 * DATA + CAPTURE;
    const DRAIN_TICKS: usize = 128;
    const DEADLINE: Duration = Duration::from_secs(20);
    type ResultV1<T> = Result<T, Box<dyn Error + Send + Sync>>;
    type Snapshot = KfdDrainCaptureCustodyObservationV1;
    type Receipt = KfdDrainCaptureCopyObservationV1;

    struct ThreadWake(thread::Thread);
    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    fn wait<F: Future>(future: F) -> ResultV1<F::Output> {
        let mut future = Box::pin(future);
        let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
                return Ok(result);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("copy drain qualification deadline expired".into());
            }
            thread::park_timeout(remaining);
        }
    }

    struct Pause {
        entered: SyncSender<()>,
        release: Receiver<()>,
    }

    impl Pause {
        fn hold(self) {
            self.entered
                .send(())
                .expect("qualification receiver exists");
            self.release
                .recv_timeout(DEADLINE)
                .expect("qualification owner pause was not released");
        }
    }

    fn pause() -> (Pause, Receiver<()>, SyncSender<()>) {
        let (entered, received) = sync_channel(1);
        let (release, released) = sync_channel(1);
        (
            Pause {
                entered,
                release: released,
            },
            received,
            release,
        )
    }

    #[derive(Clone, Copy)]
    struct CopyCall {
        id: u64,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
    }

    struct Trace {
        streams: Vec<u64>,
        allocations: Vec<u64>,
        calls: Vec<CopyCall>,
        receipts: Vec<Receipt>,
        enabled: bool,
        failure: Option<KfdDrainCaptureObservationFailureV1>,
        fault: Option<&'static str>,
        last: Option<Snapshot>,
        capture_before: Option<Snapshot>,
        capture_after: Option<Snapshot>,
        capture_calls: usize,
        shutdown_calls: usize,
        first_publication_pause: Option<Pause>,
    }

    impl Trace {
        fn new() -> Self {
            Self {
                streams: Vec::with_capacity(2),
                allocations: Vec::with_capacity(3),
                calls: Vec::with_capacity(3),
                receipts: Vec::with_capacity(3),
                enabled: false,
                failure: None,
                fault: None,
                last: None,
                capture_before: None,
                capture_after: None,
                capture_calls: 0,
                shutdown_calls: 0,
                first_publication_pause: None,
            }
        }

        fn retain(&mut self, snapshot: Snapshot) {
            for row in snapshot
                .copies()
                .iter()
                .filter(|row| row.phase == KfdDrainCaptureCopyPhaseV1::DirectionalPublished)
            {
                if let Some(previous) = self
                    .receipts
                    .iter()
                    .find(|old| old.submission == row.submission)
                {
                    if previous != row {
                        self.fault = Some("retained copy receipt changed");
                    }
                } else if self.receipts.len() < 3 {
                    self.receipts.push(*row);
                } else {
                    self.fault = Some("work receipt capacity exceeded");
                }
            }
            self.last = Some(snapshot);
        }
    }

    // All recording storage is allocated before owner startup. Recording only
    // observes real delegated outcomes; it never creates Pending or native IDs.
    struct ObservedBackend {
        inner: KfdRuntimeBackendV1,
        trace: Arc<Mutex<Trace>>,
    }

    impl ObservedBackend {
        fn observe(&self) -> Option<Snapshot> {
            let mut trace = self.trace.lock().unwrap();
            if !trace.enabled {
                return None;
            }
            match self.inner.diagnose_drain_capture_custody_v1() {
                Ok(snapshot) => {
                    trace.retain(snapshot);
                    Some(snapshot)
                }
                Err(error) => {
                    trace.failure = Some(error);
                    None
                }
            }
        }

        fn after_progress(&self) {
            let snapshot = self.observe();
            let pause = {
                let mut trace = self.trace.lock().unwrap();
                if snapshot.is_some_and(|snapshot| {
                    snapshot.native_retained_copies() == 1 && trace.receipts.len() == 1
                }) {
                    trace.first_publication_pause.take()
                } else {
                    None
                }
            };
            if let Some(pause) = pause {
                // The native record already exists. Only this owner thread is
                // paused; the GPU is neither stopped nor assumed still running.
                pause.hold();
            }
        }

        fn forbidden_after_setup(&self, operation: &'static str) {
            let mut trace = self.trace.lock().unwrap();
            if trace.enabled {
                trace.fault = Some(operation);
            }
        }
    }

    macro_rules! forward {
        ($name:ident($($arg:ident: $ty:ty),*) -> $output:ty) => {
            fn $name(&mut self, $($arg: $ty),*) -> Result<$output, RuntimeBackendFailureV1<Self::Error>> {
                self.inner.$name($($arg),*)
            }
        };
    }

    impl RuntimeBackendV1 for ObservedBackend {
        type Error = KfdRuntimeBackendErrorV1;
        forward!(enumerate_devices_v1() -> Vec<BackendDeviceDescriptionV1>);
        fn create_stream_v1(
            &mut self,
            device: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let id = self.inner.create_stream_v1(device)?;
            let mut trace = self.trace.lock().unwrap();
            assert!(trace.streams.len() < 2);
            trace.streams.push(id);
            Ok(id)
        }
        fn allocate_v1(
            &mut self,
            device: u64,
            kind: RuntimeMemoryKindV1,
            byte_len: u64,
            alignment: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let id = self.inner.allocate_v1(device, kind, byte_len, alignment)?;
            let mut trace = self.trace.lock().unwrap();
            assert!(trace.allocations.len() < 3);
            trace.allocations.push(id);
            Ok(id)
        }
        forward!(release_allocation_v1(allocation: u64) -> ());
        fn write_allocation_v1(
            &mut self,
            allocation: u64,
            byte_offset: u64,
            bytes: &[u8],
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.forbidden_after_setup("ordinary write after setup");
            self.inner
                .write_allocation_v1(allocation, byte_offset, bytes)
        }
        fn read_allocation_v1(
            &mut self,
            allocation: u64,
            byte_offset: u64,
            destination: &mut [u8],
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.forbidden_after_setup("ordinary read after setup");
            self.inner
                .read_allocation_v1(allocation, byte_offset, destination)
        }
        fn capture_coherent_host_range_v1(
            &mut self,
            request: BackendHostCaptureV1<'_>,
        ) -> Result<(), RuntimeBackendFailureV1<RuntimeHostCaptureErrorV1>> {
            let before = self.observe();
            {
                let mut trace = self.trace.lock().unwrap();
                trace.capture_calls += 1;
                trace.capture_before = before;
            }
            let result = self.inner.capture_coherent_host_range_v1(request);
            let after = self.observe();
            self.trace.lock().unwrap().capture_after = after;
            result
        }
        forward!(load_module_v1(device: u64, image: &[u8]) -> u64);
        forward!(unload_module_v1(module: u64) -> ());
        forward!(resolve_kernel_v1(module: u64, name: &str, signature: [u8;32]) -> u64);
        forward!(submit_v1(launch: BackendLaunchV1<'_>) -> u64);
        fn poll_v1(
            &mut self,
            submission: u64,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            self.observe();
            let result = self.inner.poll_v1(submission);
            self.after_progress();
            result
        }
        forward!(wait_v1(submission: u64, deadline: Instant) -> BackendPollV1);
        forward!(release_submission_v1(submission: u64) -> ());
        forward!(record_event_v1(stream: u64, submission: u64) -> u64);
        forward!(release_event_v1(event: u64) -> ());
        forward!(peer_copy_v1(stream: u64, source: BackendMemoryRegionV1, destination: BackendMemoryRegionV1, dependencies: &[u64]) -> u64);
        forward!(destroy_stream_v1(stream: u64) -> ());
    }

    impl RuntimeAsyncCopyBackendV1 for ObservedBackend {
        fn copy_async_v1(
            &mut self,
            stream: u64,
            source: BackendMemoryRegionV1,
            destination: BackendMemoryRegionV1,
            dependencies: &[u64],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let id = self
                .inner
                .copy_async_v1(stream, source, destination, dependencies)?;
            {
                let mut trace = self.trace.lock().unwrap();
                if trace.enabled {
                    if trace.calls.len() < 3 && dependencies.is_empty() {
                        trace.calls.push(CopyCall {
                            id,
                            stream,
                            source,
                            destination,
                        });
                    } else {
                        trace.fault = Some("unexpected work submission or dependency");
                    }
                }
            }
            self.after_progress();
            Ok(id)
        }
    }

    impl RuntimeFlushBackendV1 for ObservedBackend {
        fn flush_stream_v1(
            &mut self,
            stream: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.observe();
            let result = self.inner.flush_stream_v1(stream);
            self.after_progress();
            result
        }
    }

    impl RuntimeOwnedShutdownBackendV1 for ObservedBackend {
        fn shutdown_owned_v1(&mut self) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.trace.lock().unwrap().shutdown_calls += 1;
            self.inner.shutdown_owned_v1()
        }
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

    fn coordinates(
        allocations: &[RuntimeAllocationIdV1],
        index: usize,
    ) -> (RuntimeMemoryRegionV1, RuntimeMemoryRegionV1) {
        let (from, to, source_offset, destination_offset, bytes) = match index {
            0 => (0, 1, 131, 137, BODY),
            1 => (1, 2, 137, 139, BODY),
            2 => (1, 2, 0, VERIFY_OFFSET, DATA),
            _ => unreachable!(),
        };
        (
            region(
                allocations[from],
                RuntimeAccessV1::Read,
                source_offset,
                bytes,
            ),
            region(
                allocations[to],
                RuntimeAccessV1::Write,
                destination_offset,
                bytes,
            ),
        )
    }

    fn node(id: u32) -> CompletionNodeIdV1 {
        CompletionNodeIdV1::new(id).unwrap()
    }

    fn graph(
        streams: &[RuntimeStreamIdV1],
        identities: &[StreamIdentityV1],
        allocations: &[RuntimeAllocationIdV1],
    ) -> ResultV1<(
        CompletionGraphIdentityV1,
        RuntimeGraphRequestV1<ObservedBackend>,
    )> {
        let event = EventIdentityV1::new(identities[0].context(), [2; 32]);
        let future = |id: u32, stream, predecessor: Option<u32>| {
            CompletionNodeV1::future(
                node(id),
                FutureIdentityV1::new(stream, [id as u8; 32]),
                predecessor.map(node),
            )
        };
        let graph = CompletionGraphV1::new(
            identities[0].context(),
            identities.to_vec(),
            vec![
                future(1, identities[0], None),
                CompletionNodeV1::record_event(node(2), identities[0], event, Some(node(1))),
                CompletionNodeV1::wait_event(node(3), identities[1], event, node(2), None),
                future(4, identities[1], Some(3)),
                future(5, identities[1], Some(4)),
            ],
        )?;
        let identity = graph.identity();
        let mut request = RuntimeGraphRequestV1::new(
            graph,
            identities
                .iter()
                .copied()
                .zip(streams.iter().copied())
                .collect(),
        )?;
        for (index, id) in [1, 4, 5].into_iter().enumerate() {
            let (source, destination) = coordinates(allocations, index);
            request.bind_copy(node(id), source, destination)?;
        }
        Ok((identity, request))
    }

    fn warm_copy(
        context: &mut RuntimeContextV1<ObservedBackend>,
        stream: RuntimeStreamIdV1,
        source: RuntimeAllocationIdV1,
        destination: RuntimeAllocationIdV1,
    ) -> ResultV1<()> {
        let mut submission = context.copy_async(
            stream,
            region(source, RuntimeAccessV1::Read, 0, DATA),
            region(destination, RuntimeAccessV1::Write, 0, DATA),
            &[],
        )?;
        let deadline = Instant::now() + DEADLINE;
        loop {
            context.flush_stream(stream)?;
            match context.poll(&mut submission)? {
                RuntimePollV1::Succeeded => break,
                RuntimePollV1::Pending if Instant::now() < deadline => thread::yield_now(),
                status => return Err(format!("warmup copy failed or expired: {status:?}").into()),
            }
        }
        context
            .release_submission(submission)
            .map_err(|_| "warmup submission disposal failed")?;
        Ok(())
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
    fn hash(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    fn receipt_json(row: &Receipt) -> Value {
        json!({"submission": row.submission, "stream": row.stream,
            "source": row.source, "destination": row.destination,
            "source_offset": row.source_offset, "destination_offset": row.destination_offset,
            "byte_len": row.byte_len, "dependencies": row.dependencies(),
            "phase": match row.phase { KfdDrainCaptureCopyPhaseV1::Ready => "ready",
                KfdDrainCaptureCopyPhaseV1::DirectionalPublished => "directional-published" },
            "native_receipt": row.native_receipt.map(|bytes| hex(&bytes)),
            "runtime_membership": row.runtime_membership.map(|bytes| hex(&bytes)),
            "native_packets": row.native_packets})
    }

    fn snapshot_json(snapshot: Snapshot) -> Value {
        json!({"publication_ids": snapshot.publication_ids(),
            "native_retained_copies": snapshot.native_retained_copies(),
            "copies": snapshot.copies().iter().map(receipt_json).collect::<Vec<_>>()})
    }

    fn run_cell(
        unique_id: u64,
        ordinal: usize,
        native: bool,
        stream_count: usize,
        retain: bool,
    ) -> ResultV1<Value> {
        let trace = Arc::new(Mutex::new(Trace::new()));
        let worker_trace = Arc::clone(&trace);
        let config = RuntimeAsyncEngineConfigV1::new(16, 16, 1, 1, Duration::from_millis(1))?
            .with_reply_capacity(16)?
            .with_drain_capture_byte_capacity(CAPTURE)?;
        let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
            move || -> ResultV1<_> {
                Ok(RuntimeContextV1::open(ObservedBackend {
                    inner: KfdRuntimeBackendV1::open_copy_only_qualification_v1(unique_id)?,
                    trace: worker_trace,
                })?)
            },
            config,
            RuntimeAsyncProgressConfigV1::new(2, 1)?,
        )
        .map_err(|error| format!("owner startup: {error}"))?;
        let (streams, identities, allocations, source, usage) = wait(
            handle
                .observer()
                .enqueue_with_context(move |context| -> ResultV1<_> {
                    let device = context.devices()[0].id();
                    context.configure_allocation_admission_v1(device, REQUESTED as u64, 3)?;
                    let streams = (0..stream_count)
                        .map(|_| context.create_stream(device))
                        .collect::<Result<Vec<_>, _>>()?;
                    let identities = streams
                        .iter()
                        .map(|&stream| context.completion_stream_identity_v1(stream))
                        .collect::<Result<Vec<_>, _>>()?;
                    let mut allocations = Vec::with_capacity(3);
                    for (kind, bytes) in [
                        (RuntimeMemoryKindV1::HostVisible, DATA),
                        (RuntimeMemoryKindV1::DeviceLocal, DATA),
                        (RuntimeMemoryKindV1::HostVisible, CAPTURE),
                    ] {
                        allocations.push(context.allocate(device, kind, bytes as u64, 4096)?);
                    }
                    context.write_allocation(allocations[0], 0, &vec![0xa5; DATA])?;
                    context.write_allocation(allocations[2], 0, &vec![0xd3; CAPTURE])?;
                    warm_copy(context, streams[0], allocations[0], allocations[1])?;
                    warm_copy(context, streams[0], allocations[1], allocations[2])?;
                    let mut input = vec![0xc7; DATA];
                    for (index, byte) in input[131..131 + BODY].iter_mut().enumerate() {
                        *byte = ((index * 29 + index / 257 + ordinal * 17) % 251) as u8;
                    }
                    context.write_allocation(allocations[0], 0, &input)?;
                    context.write_allocation(allocations[2], 0, &vec![0xd3; CAPTURE])?;
                    let source =
                        context.prepare_host_drain_capture_v1(allocations[2], 0, CAPTURE)?;
                    let usage = context
                        .allocation_admission_usage_v1(device)?
                        .ok_or("missing request credits")?;
                    Ok((streams, identities, allocations, source, usage))
                })?,
        )???;
        let expected_usage = RuntimeResourceVectorV1::ZERO
            .with(
                RuntimeResourceKindV1::RequestedAllocationBytes,
                REQUESTED as u64,
            )
            .with(RuntimeResourceKindV1::AllocationRecords, 3);
        if usage.used != expected_usage
            || usage.retained_records != 3
            || usage.poisoned
            || usage.reserved_records != 0
            || usage.quarantined_records != 0
        {
            return Err("request credit roster mismatch".into());
        }

        let (initial_pause, initial_entered, initial_release) = pause();
        let (publication_pause, publication_entered, publication_release) = pause();
        if native {
            trace.lock().unwrap().first_publication_pause = Some(publication_pause);
        }
        let start_trace = Arc::clone(&trace);
        let started =
            handle
                .observer()
                .enqueue_with_context(move |context| -> ResultV1<Snapshot> {
                    let baseline = context
                        .backend()
                        .inner
                        .diagnose_drain_capture_custody_v1()?;
                    if baseline.publication_ids().len() != 2
                        || !baseline.copies().is_empty()
                        || baseline.native_retained_copies() != 0
                    {
                        return Err("warmup has no exact empty native frontier".into());
                    }
                    {
                        let mut trace = start_trace.lock().unwrap();
                        trace.last = Some(baseline);
                        trace.enabled = true;
                    }
                    initial_pause.hold();
                    Ok(baseline)
                })?;
        initial_entered.recv_timeout(DEADLINE)?;
        let baseline = trace.lock().unwrap().last.ok_or("missing baseline")?;
        let mut operation_futures = Vec::with_capacity(3);
        let mut graph_future = None;
        let mut graph_identity = None;
        if stream_count == 1 {
            let (source, destination) = coordinates(&allocations, 0);
            operation_futures.push(handle.copy_async(streams[0], source, destination, vec![])?);
        } else {
            let (identity, request) = graph(&streams, &identities, &allocations)?;
            graph_identity = Some(identity);
            graph_future = Some(handle.submit_graph(request)?);
        }
        if native {
            initial_release.send(())?;
            publication_entered.recv_timeout(DEADLINE)?;
        }
        if stream_count == 1 {
            for index in 1..3 {
                let (source, destination) = coordinates(&allocations, index);
                operation_futures.push(handle.copy_async(
                    streams[0],
                    source,
                    destination,
                    vec![],
                )?);
            }
        }
        if !retain {
            operation_futures.clear();
            drop(graph_future.take());
        }
        let captured = handle.begin_drain_with_capture(
            DRAIN_TICKS,
            source,
            vec![0x6e; CAPTURE].into_boxed_slice(),
        )?;
        let reserved = handle.observer().drain_capture_bytes_in_use();
        let at_cutoff = trace
            .lock()
            .unwrap()
            .last
            .ok_or("missing cutoff snapshot")?;
        if reserved != CAPTURE
            || (!native && at_cutoff != baseline)
            || (native
                && (at_cutoff.native_retained_copies() != 1
                    || at_cutoff.publication_ids().len() != 3
                    || at_cutoff.copies().len() != 1))
        {
            return Err("incorrect cutoff frontier or capture credit charge".into());
        }
        if !matches!(
            handle.observer().enqueue_with_context(|_| ()),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ) {
            return Err("capture cutoff left admission open".into());
        }
        if native {
            publication_release.send(())?;
        } else {
            initial_release.send(())?;
        }
        if wait(started)??? != baseline {
            return Err("baseline changed".into());
        }
        let report = wait(captured)??;
        let bytes = report.capture?;
        let after_extraction = handle.observer().drain_capture_bytes_in_use();
        let mut completed_observers = 0;
        let mut submission_ids = Vec::with_capacity(3);
        for future in operation_futures {
            let result = wait(future)??;
            let submission = result.submission.ok_or("copy observer has no submission")?;
            if result.observation? != RuntimeCompletionStatusV1::Succeeded
                || result.rejected_observations != 0
                || result.last_rejected_observation.is_some()
                || submission.stream() != streams[0]
                || submission_ids.contains(&submission.id())
            {
                return Err("copy observer identity or completion mismatch".into());
            }
            submission_ids.push(submission.id());
            completed_observers += 1;
        }
        if let Some(future) = graph_future {
            let graph_report =
                wait(future)??.map_err(|error| format!("graph result: {error:?}"))?;
            let identity = graph_identity.ok_or("missing graph identity")?;
            if graph_report.completion.graph_identity() != identity
                || graph_report.execution.graph_identity() != identity
                || graph_report.execution.context() != identities[0].context()
                || graph_report.execution.generation() == 0
                || !graph_report.errors.is_empty()
                || graph_report.rejected_observations != 0
                || graph_report.rejected_releases != 0
                || graph_report.completion.entries().len() != 5
                || graph_report
                    .completion
                    .entries()
                    .iter()
                    .enumerate()
                    .any(|(index, entry)| {
                        entry.node() != node(index as u32 + 1)
                            || entry.state() != CompletionNodeStateV1::Succeeded
                    })
            {
                return Err("graph observer identity or completion mismatch".into());
            }
            let mut observations: Vec<_> = graph_report
                .observations
                .iter()
                .map(|(node, status)| (node.get(), *status))
                .collect();
            observations.sort_unstable_by_key(|entry| entry.0);
            if observations != [1, 4, 5].map(|node| (node, RuntimeCompletionStatusV1::Succeeded)) {
                return Err("graph native observation roster mismatch".into());
            }
            completed_observers += 1;
        }
        let drain = report.drain;
        let retained = if stream_count == 1 { 3 } else { 0 };
        if drain.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent
            || drain.ticks == 0
            || drain.ticks > DRAIN_TICKS
            || !drain.queued_commands_exhausted
            || drain.operations_remaining != 0
            || drain.graph_active
            || drain.retained_submissions
                != (RuntimeStreamObservationV1 {
                    total_submissions: retained,
                    succeeded: retained,
                    ..Default::default()
                })
        {
            return Err(format!("accepted prefix drain mismatch: {drain:?}").into());
        }
        let shutdown = engine.shutdown()?;
        let cleanup = shutdown.cleanup.as_ref().ok_or("missing cleanup report")?;
        if shutdown.disposition != RuntimeAsyncOwnedDispositionV1::Released
            || shutdown.worker_panicked
            || shutdown.native_failure.is_some()
            || !cleanup.is_complete()
            || !cleanup.failures().is_empty()
            || cleanup.allocation_credit_records_v1() != 0
        {
            return Err(format!("owner shutdown retained custody: {shutdown:?}").into());
        }
        let after_shutdown = handle.observer().drain_capture_bytes_in_use();
        if bytes.len() != CAPTURE || after_extraction != CAPTURE || after_shutdown != CAPTURE {
            return Err("owned capture bytes refunded before disposal".into());
        }
        // Hash real capture storage only after owner shutdown. The independent
        // Python checker derives all body and guard bytes from the frozen recipe.
        let output_hash = hash(bytes.as_bytes());
        let device_hash = hash(&bytes.as_bytes()[VERIFY_OFFSET..VERIFY_OFFSET + DATA]);
        let body_hash = hash(&bytes.as_bytes()[139..139 + BODY]);
        drop(bytes);
        let after_disposal = handle.observer().drain_capture_bytes_in_use();
        let replies = handle.observer().reply_cells_in_use();
        let snapshots = handle.observer().snapshot_bytes_in_use();
        if after_disposal != 0 || replies != 0 || snapshots != 0 {
            return Err("capture or observer credits remain after final disposal".into());
        }
        let trace = trace.lock().unwrap();
        if let Some(error) = trace.failure {
            return Err(format!("native custody observation: {error}").into());
        }
        if let Some(error) = trace.fault {
            return Err(error.into());
        }
        let before = trace
            .capture_before
            .ok_or("missing capture-entry observation")?;
        let after = trace
            .capture_after
            .ok_or("missing capture-return observation")?;
        if trace.capture_calls != 1
            || trace.shutdown_calls != 1
            || trace.calls.len() != 3
            || trace.receipts.len() != 3
            || before != after
            || !before.copies().is_empty()
            || before.native_retained_copies() != 0
            || before.publication_ids().len() != 5
            || before.publication_ids()[..2] != *baseline.publication_ids()
            || trace.streams.len() != stream_count
            || trace.allocations.len() != 3
        {
            return Err("capture, publication, or cleanup occurrence mismatch".into());
        }
        for (index, (call, receipt)) in trace.calls.iter().zip(&trace.receipts).enumerate() {
            if call.id != receipt.submission
                || call.id != before.publication_ids()[index + 2]
                || call.stream != receipt.stream
                || call.source.allocation != receipt.source
                || call.destination.allocation != receipt.destination
                || call.source.byte_offset != receipt.source_offset
                || call.destination.byte_offset != receipt.destination_offset
                || call.source.byte_len != receipt.byte_len
                || call.destination.byte_len != receipt.byte_len
            {
                return Err("receipt does not name its exact accepted backend copy".into());
            }
        }
        Ok(
            json!({"ordinal": ordinal, "cutoff": if native { "native-retained" } else { "queued" },
            "streams": stream_count, "observers": if retain { "retained" } else { "dropped" },
            "profile": {"body_bytes": BODY, "data_bytes": DATA, "capture_bytes": CAPTURE,
                "input_offset": 131, "device_offset": 137, "output_offset": 139, "verify_offset": VERIFY_OFFSET},
            "context": hex(&identities[0].context().local_bytes()),
            "resources": {"streams": streams.iter().zip(&trace.streams)
                .map(|(runtime, backend)| json!({"runtime": runtime.get(), "backend": backend})).collect::<Vec<_>>(),
                "allocations": allocations.iter().zip(&trace.allocations).zip([("input", DATA), ("device", DATA), ("output", CAPTURE)])
                    .map(|((runtime, backend), (role, bytes))| json!({"runtime": runtime.get(), "backend": backend, "role": role, "bytes": bytes})).collect::<Vec<_>>()},
            "baseline": snapshot_json(baseline), "at_cutoff": snapshot_json(at_cutoff),
            "capture_before": snapshot_json(before), "capture_after": snapshot_json(after),
            "copies": trace.receipts.iter().map(receipt_json).collect::<Vec<_>>(),
            "drain": {"outcome": "quiescent", "ticks": drain.ticks,
                "total_submissions": drain.retained_submissions.total_submissions,
                "succeeded": drain.retained_submissions.succeeded, "pending": drain.retained_submissions.pending,
                "queued_commands_exhausted": drain.queued_commands_exhausted,
                "operations_remaining": drain.operations_remaining, "graph_active": drain.graph_active},
            "completed_observers": completed_observers,
            "graph_sha256": graph_identity.map(|identity| hex(&identity.sha256())),
            "credits": {"requested_bytes": REQUESTED, "allocation_records": usage.retained_records,
                "capture_reserved": reserved, "after_extraction": after_extraction,
                "after_shutdown": after_shutdown, "after_disposal": after_disposal,
                "reply_after_disposal": replies, "snapshot_after_shutdown": snapshots,
                "allocation_records_after_cleanup": cleanup.allocation_credit_records_v1()},
            "output_sha256": output_hash, "device_sha256": device_hash, "body_sha256": body_hash,
            "cleanup": "complete"}),
        )
    }

    pub fn run() -> ResultV1<()> {
        let mut args = std::env::args().skip(1);
        let unique_id: u64 = args
            .next()
            .ok_or("usage: gfx942-runtime-drain-capture UNIQUE_ID")?
            .parse()?;
        if unique_id == 0 || args.next().is_some() {
            return Err("exactly one nonzero unique ID required".into());
        }
        let mut cases = Vec::with_capacity(8);
        for native in [false, true] {
            for streams in [1, 2] {
                for retain in [true, false] {
                    let ordinal = cases.len();
                    cases.push(run_cell(unique_id, ordinal, native, streams, retain)
                        .map_err(|error| format!("cell={ordinal} native={native} streams={streams} retained={retain}: {error}"))?);
                }
            }
        }
        println!(
            "{}",
            json!({"schema": "fe2o3.runtime.drain-capture-copy.v1", "owner_threads": 1,
            "physical_overlap": "unmeasured", "performance": "unmeasured", "cases": cases})
        );
        Ok(())
    }
}

#[cfg(feature = "hardware-qualification")]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    enabled::run()
}

#[cfg(not(feature = "hardware-qualification"))]
fn main() {
    eprintln!("requires --features hardware-qualification");
    std::process::exit(2);
}
