//! Test-only observation tap. It copies actual delivered values, never executes KIR.
use super::gfx942_bf16_publication_sidecar_v1_tests as wire;
use super::gfx942_tiled_region_qualification_v1_tests::observation::cpu::oracle;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, BlockId, ScalarType, ValueId};
use fe2o3_kir_sim::*;
use std::path::PathBuf;

#[derive(Clone, Copy)]
pub(super) struct Sites {
    pub root: usize,
    pub helper: Option<usize>,
    pub matrix: (BlockId, u32),
    pub call: Option<(BlockId, u32)>,
    pub store: (BlockId, u32),
    pub matrix_results: [ValueId; 4],
    pub call_results: [ValueId; 4],
    pub parameters: [ValueId; 4],
}
pub(super) struct Stream {
    session: u32,
    path: PathBuf,
    writer: Option<wire::Writer>,
    last_prefix: Option<(usize, u32, bool)>,
    pub summary: Option<wire::Summary>,
    pub failure: Option<&'static str>,
    pub io_raw_error: Option<i32>,
    pub attempted_creation: bool,
}
impl Stream {
    pub fn new(session: u32, path: PathBuf) -> Result<Self, &'static str> {
        if session >= 4 {
            return Err("CPU stream session");
        }
        Ok(Self {
            session,
            path,
            writer: None,
            last_prefix: None,
            summary: None,
            failure: None,
            io_raw_error: None,
            attempted_creation: false,
        })
    }
    pub fn start(
        &mut self,
        source: [u8; 32],
        canonical: [u8; 32],
        sites: Sites,
    ) -> Result<(), &'static str> {
        if self.attempted_creation {
            return Err("CPU sidecar creation repeated");
        }
        let number = |n: usize| u32::try_from(n).map_err(|_| "CPU site ordinal");
        let header = wire::Header {
            role: self.session % 2,
            order: self.session / 2,
            session: self.session,
            source_sha256: source,
            canonical_sha256: canonical,
            root: number(sites.root)?,
            helper: sites.helper.map(number).transpose()?.unwrap_or(u32::MAX),
            call: sites.call.map(|(b, o)| [b.0, o]).unwrap_or([u32::MAX; 2]),
            matrix: [sites.matrix.0.0, sites.matrix.1],
            store: [sites.store.0.0, sites.store.1],
        };
        self.attempted_creation = true;
        self.last_prefix = Some((0, 0, true));
        match wire::Writer::create_new(&self.path, header) {
            Ok(writer) => {
                self.last_prefix = Some(writer.committed_prefix());
                self.writer = Some(writer);
                Ok(())
            }
            Err(error) => {
                self.note_error(error, "CPU sidecar create failed");
                Err("CPU sidecar create failed")
            }
        }
    }
    pub fn requested_permutation(&self) -> [u8; 4] {
        if self.session == 3 {
            [1, 0, 2, 3]
        } else {
            [0, 1, 2, 3]
        }
    }
    pub fn positive(
        &mut self,
        trace: Trace,
        memory: Memory,
        steps: u64,
    ) -> Result<(), &'static str> {
        let allocations = trace
            .allocations
            .ok_or("actual root allocation mapping absent")?;
        let a = memory.a.with_allocation(allocations[0]);
        let b = memory.b.with_allocation(allocations[1]);
        let output = memory.output.with_allocation(allocations[2]);
        let row = wire::Positive {
            pattern: trace.pattern as u32,
            output_length: trace.length as u32,
            records: trace.records,
            steps,
            matrix_mask: trace.matrix_mask,
            caller_mask: trace.call_mask,
            store_mask: trace.store_mask,
            allocations,
            matrix: trace.matrix,
            caller: trace.caller,
            a,
            b,
            output,
            write_count: trace.store_mask.count_ones(),
            writes: trace.writes,
        };
        // Independently compare ACTUAL copied arrays/backings to dense CPU oracle.
        let dense = oracle::expected(trace.pattern);
        if row.matrix != dense.0
            || row.a.bytes.as_slice() != oracle::bytes(trace.pattern, 0).as_slice()
            || row.b.bytes.as_slice() != oracle::bytes(trace.pattern, 1).as_slice()
        {
            return Err("actual CPU words/input bytes differ from dense oracle");
        }
        let permutation = self.requested_permutation();
        for lane in 0..64 {
            for c in 0..4 {
                let at = (4 * (lane / 16) + c) * 16 + lane % 16;
                if row.caller[at] != oracle::lane_word(&dense, lane, usize::from(permutation[c])) {
                    return Err("actual returned words differ from requested independent order");
                }
            }
        }
        // Zero/uniform cases are not falsely required to distinguish a swap.
        if matches!(trace.pattern, 1 | 2) && trace.length != 0 {
            let opposite = if permutation == [0, 1, 2, 3] {
                [1, 0, 2, 3]
            } else {
                [0, 1, 2, 3]
            };
            let mut caller_diff = false;
            let mut output_diff = false;
            for lane in 0..64 {
                for c in 0..4 {
                    let at = (4 * (lane / 16) + c) * 16 + lane % 16;
                    caller_diff |= row.caller[at] != oracle::lane_word(&dense, lane, opposite[c]);
                }
                if lane < trace.length {
                    output_diff |= row.output.bytes[8 + 4 * lane..12 + 4 * lane]
                        != oracle::lane_word(&dense, lane, opposite[0]).to_le_bytes();
                }
            }
            if !caller_diff || !output_diff {
                return Err("nonsymmetric wrong permutation not rejected");
            }
        }
        let writer = self.writer.as_mut().ok_or("CPU writer absent")?;
        let result = writer.positive(&row);
        self.last_prefix = Some(writer.committed_prefix());
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                self.note_error(error, "CPU sidecar positive failed");
                Err("CPU sidecar positive failed")
            }
        }
    }
    pub fn negative(
        &mut self,
        ordinal: usize,
        observed: &str,
        matrix_mask: u64,
        call_mask: u64,
        writes: u64,
        floor_restored: bool,
    ) -> Result<(), &'static str> {
        let classification = match observed {
            "uninitialized-read" => 1,
            "matrix-domain-a" => 2,
            "matrix-domain-b" => 3,
            "step-limit" => 4,
            "incomplete-observation" => 5,
            "event-sink" => 6,
            "profile-launch" => 7,
            _ => return Err("unexpected CPU negative classification"),
        };
        let control = u32::try_from(ordinal.checked_sub(18).ok_or("negative ordinal")?)
            .map_err(|_| "negative ordinal width")?;
        let row = wire::Negative {
            control,
            classification,
            matrix_mask,
            caller_mask: call_mask,
            global_writes: writes,
            floor_restored: u32::from(floor_restored),
        };
        let writer = self.writer.as_mut().ok_or("CPU writer absent")?;
        let result = writer.negative(row);
        self.last_prefix = Some(writer.committed_prefix());
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                self.note_error(error, "CPU sidecar negative failed");
                Err("CPU sidecar negative failed")
            }
        }
    }
    pub fn finish(&mut self) -> Result<(), &'static str> {
        let writer = self.writer.take().ok_or("CPU writer absent")?;
        self.last_prefix = Some(writer.committed_prefix());
        match writer.finish() {
            Ok(summary) => {
                self.summary = Some(summary);
                Ok(())
            }
            Err(error) => {
                self.note_error(error, "CPU footer failed");
                Err("CPU footer failed")
            }
        }
    }
    fn note_error(&mut self, error: wire::Error, stage: &'static str) {
        self.failure = Some(stage);
        self.io_raw_error = match error {
            wire::Error::Io(error) => error.raw_os_error(),
            _ => None,
        };
    }
    pub fn diagnostic(&self) -> serde_json::Value {
        serde_json::json!({"path":self.path,"creation_attempted":self.attempted_creation,
            "committed_prefix":self.writer.as_ref().map(wire::Writer::committed_prefix).or(self.last_prefix),
            "completed":self.summary.map(|s|serde_json::json!({"bytes":s.bytes,"rows":s.rows,
                "sha256":super::lower_hex_v1(&s.sha256)})),
            "failure":self.failure,"io_raw_error":self.io_raw_error,"footer_is_outer_success":false})
    }
}

struct Bytes<const N: usize, const I: usize> {
    id: u64,
    byte_length: u32,
    init_bit_length: u32,
    offset: u64,
    length: u64,
    bytes: [u8; N],
    initialized: [u8; I],
}
impl<const N: usize, const I: usize> Bytes<N, I> {
    fn capture(
        run: &SimulationExecutionV1,
        slot: usize,
        length: usize,
    ) -> Result<Self, &'static str> {
        let id = oracle::INPUT_IDS[slot];
        let SimulationArgumentV1::BufferView(view) =
            run.arguments().get(slot).ok_or("actual ABI slot")?
        else {
            return Err("actual ABI input is not shared view");
        };
        let (element, access, offset, elements, alignment) = if slot < 2 {
            (ScalarType::U16, AccessMode::ReadOnly, 0, 256, 2)
        } else {
            (ScalarType::F32, AccessMode::ReadWrite, 8, length, 4)
        };
        if view.backing() != id
            || view.element() != element
            || view.access() != access
            || view.byte_offset() != offset
            || view.elements() != elements
            || view.alignment() != alignment
        {
            return Err("actual copied ABI view does not name expected backing");
        }
        let mut selected = run.shared_buffers().iter().filter(|row| row.id == id);
        let shared = selected.next().ok_or("actual named backing absent")?;
        if selected.next().is_some() {
            return Err("duplicate actual named backing");
        }
        let buffer = &shared.buffer;
        if buffer.element() != element
            || buffer.access() != access
            || buffer.alignment() != alignment
            || buffer.bytes().len() != N
            || buffer.initialized().len() != N
            || I != (N + 7) / 8
        {
            return Err("actual backing type/length/init/alignment");
        }
        let mut initialized = [0u8; I];
        for (bit, value) in buffer.initialized().iter().enumerate() {
            if *value {
                initialized[bit / 8] |= 1 << (bit % 8);
            }
        }
        Ok(Self {
            id: u64::from(id.0),
            byte_length: u32::try_from(buffer.bytes().len()).map_err(|_| "backing length")?,
            init_bit_length: u32::try_from(buffer.initialized().len())
                .map_err(|_| "init length")?,
            offset: u64::try_from(view.byte_offset()).map_err(|_| "view offset")?,
            length: u64::try_from(view.elements()).map_err(|_| "view length")?,
            bytes: buffer
                .bytes()
                .try_into()
                .map_err(|_| "actual fixed backing")?,
            initialized,
        })
    }
    fn with_allocation(self, allocation: u64) -> wire::Backing<N, I> {
        // The ID join is not guessed: execute.rs's selected BufferView lowering
        // and copy_back_shared_buffers use the SAME shared_allocations map.
        // Actual root SSA Slice IDs are separately captured in Trace below.
        wire::Backing {
            input_id: self.id,
            allocation,
            byte_length: self.byte_length,
            init_bit_length: self.init_bit_length,
            view_offset: self.offset,
            view_length: self.length,
            bytes: self.bytes,
            initialized: self.initialized,
        }
    }
}
pub(super) struct Memory {
    a: Bytes<512, 64>,
    b: Bytes<512, 64>,
    output: Bytes<272, 34>,
}
impl Memory {
    pub fn capture(run: &SimulationExecutionV1, length: usize) -> Result<Self, &'static str> {
        if run.arguments().len() != 4
            || run.shared_buffers().len() != 3
            || run.invocations_executed() != 64
            || run.workgroups_visited() != 1
            || run.grants_execution_authority()
        {
            return Err("actual CPU output roster/profile");
        }
        let SimulationArgumentV1::Scalar(selector) = &run.arguments()[3] else {
            return Err("actual selector scalar absent");
        };
        if selector.ty() != ScalarType::U32 || selector.bits() != 0 {
            return Err("actual selector differs");
        }
        Ok(Self {
            a: Bytes::capture(run, 0, length)?,
            b: Bytes::capture(run, 1, length)?,
            output: Bytes::capture(run, 2, length)?,
        })
    }
}
pub(super) struct Trace {
    sites: Sites,
    pattern: usize,
    length: usize,
    records: u64,
    matrix: [u32; 256],
    caller: [u32; 256],
    matrix_mask: u64,
    call_mask: u64,
    store_mask: u64,
    allocations: Option<[u64; 3]>,
    writes: [[u32; 8]; 64],
}
impl Trace {
    pub fn new(sites: Sites, pattern: usize, length: usize) -> Result<Self, &'static str> {
        if pattern >= 6
            || ![64, 13, 0].contains(&length)
            || sites.helper.is_some() != sites.call.is_some()
        {
            return Err("CPU trace profile");
        }
        Ok(Self {
            sites,
            pattern,
            length,
            records: 0,
            matrix: [0; 256],
            caller: [0; 256],
            matrix_mask: 0,
            call_mask: 0,
            store_mask: 0,
            allocations: None,
            writes: [[0; 8]; 64],
        })
    }
    fn record(&mut self, record: &SimulationDebugRecordV1) -> Result<(), &'static str> {
        if record.ordinal != self.records {
            return Err("sidecar debug ordinal");
        }
        self.records = self
            .records
            .checked_add(1)
            .ok_or("sidecar record overflow")?;
        let lane = usize::try_from(record.invocation.local[0]).map_err(|_| "CPU lane")?;
        if lane >= 64
            || record.invocation.local != [lane as u32, 0, 0]
            || record.invocation.global != [lane as u64, 0, 0]
            || record.invocation.workgroup != [0, 0, 0]
            || record.invocation.workgroup_size != [64, 1, 1]
            || record.invocation.launch_extent != [64, 1, 1]
        {
            return Err("actual sidecar invocation");
        }
        let site = (record.site.block, record.site.operation);
        let matrix = site == self.sites.matrix
            && record.site.function_ordinal == self.sites.helper.unwrap_or(self.sites.root);
        let call = self.sites.call == Some(site) && record.site.function_ordinal == self.sites.root;
        match &record.kind {
            SimulationDebugRecordKindV1::Checkpoint {
                phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
                stack,
                ..
            } if matrix || call => {
                let SimulationDebugCollectionV1::Captured(frames) = stack else {
                    return Err("actual frames absent");
                };
                let depth = usize::from(matrix && self.sites.helper.is_some());
                if frames.len() != depth + 1
                    || frames[0].depth != 0
                    || frames[0].function_ordinal != self.sites.root
                {
                    return Err("actual source/caller frame roster");
                }
                let active = &frames[depth];
                if active.depth != depth as u32
                    || active.function_ordinal != record.site.function_ordinal
                {
                    return Err("actual active frame");
                }
                let SimulationDebugCollectionV1::Captured(root_values) = &frames[0].values else {
                    return Err("actual root values absent");
                };
                let mut allocations = [0; 3];
                for slot in 0..3 {
                    let mut values = root_values
                        .iter()
                        .filter(|v| v.value == self.sites.parameters[slot]);
                    let value = values.next().ok_or("actual root parameter")?;
                    if values.next().is_some() {
                        return Err("duplicate actual parameter");
                    }
                    let SimulationDebugValueV1::Slice {
                        allocation,
                        elements,
                        element,
                        address_space,
                        access,
                        byte_offset,
                        byte_len,
                    } = &value.observed
                    else {
                        return Err("actual root slice absent");
                    };
                    let (ty, mode, offset, count, width) = if slot < 2 {
                        (ScalarType::U16, AccessMode::ReadOnly, 0, 256, 2)
                    } else {
                        (ScalarType::F32, AccessMode::ReadWrite, 8, self.length, 4)
                    };
                    if *element != ty
                        || *access != mode
                        || *address_space != AddressSpace::Global
                        || *byte_offset != offset
                        || *elements != count
                        || *byte_len != count * width
                    {
                        return Err("actual root slice identity/extent");
                    }
                    allocations[slot] = *allocation;
                }
                if allocations[0] == allocations[1]
                    || allocations[0] == allocations[2]
                    || allocations[1] == allocations[2]
                    || self.allocations.is_some_and(|old| old != allocations)
                {
                    return Err("actual backing identity alias/drift");
                }
                self.allocations = Some(allocations);
                let SimulationDebugCollectionV1::Captured(values) = &active.values else {
                    return Err("actual active values absent");
                };
                let mask = if matrix {
                    self.matrix_mask
                } else {
                    self.call_mask
                };
                if mask & (1 << lane) != 0 {
                    return Err("duplicate actual checkpoint");
                }
                let ids = if matrix {
                    self.sites.matrix_results
                } else {
                    self.sites.call_results
                };
                for (component, id) in ids.into_iter().enumerate() {
                    let mut matching = values.iter().filter(|v| v.value == id);
                    let value = matching.next().ok_or("actual scalar result absent")?;
                    if matching.next().is_some() {
                        return Err("duplicate actual scalar result");
                    }
                    let SimulationDebugValueV1::Scalar(scalar) = &value.observed else {
                        return Err("actual result not scalar");
                    };
                    if scalar.ty() != ScalarType::F32 {
                        return Err("actual result not f32");
                    }
                    let bits = u32::try_from(scalar.bits()).map_err(|_| "actual f32 bit width")?;
                    let index = (4 * (lane / 16) + component) * 16 + lane % 16;
                    if matrix {
                        self.matrix[index] = bits;
                    }
                    if call || self.sites.helper.is_none() {
                        self.caller[index] = bits;
                    }
                }
                if matrix {
                    self.matrix_mask |= 1 << lane;
                }
                if call || self.sites.helper.is_none() {
                    self.call_mask |= 1 << lane;
                }
            }
            SimulationDebugRecordKindV1::Memory {
                access: SimulationDebugMemoryAccessV1::WriteCommitted,
                allocation,
                byte_offset,
                byte_len,
                address_space: AddressSpace::Global,
                value,
            } => {
                let allocations = self.allocations.ok_or("Store before actual root mapping")?;
                if record.site.function_ordinal != self.sites.root
                    || site != self.sites.store
                    || *allocation != allocations[2]
                    || *byte_offset != 8 + 4 * lane
                    || *byte_len != 4
                    || lane >= self.length
                    || self.store_mask & (1 << lane) != 0
                    || self.call_mask & (1 << lane) == 0
                {
                    return Err("actual Store source/order/identity");
                }
                let SimulationDebugValueV1::Scalar(scalar) = value else {
                    return Err("actual Store not scalar");
                };
                if scalar.ty() != ScalarType::F32 {
                    return Err("actual Store not f32");
                }
                let bits = u32::try_from(scalar.bits()).map_err(|_| "actual Store bit width")?;
                let offset = u64::try_from(*byte_offset).map_err(|_| "actual Store offset")?;
                self.writes[lane] = [
                    lane as u32,
                    4,
                    offset as u32,
                    (offset >> 32) as u32,
                    bits,
                    *allocation as u32,
                    (*allocation >> 32) as u32,
                    0,
                ];
                self.store_mask |= 1 << lane;
            }
            _ => {}
        }
        Ok(())
    }
}
pub(super) struct Tee<S> {
    pub inner: S,
    pub trace: Option<Trace>,
    pub tap_failure: Option<&'static str>,
}
impl<S> std::ops::Deref for Tee<S> {
    type Target = S;
    fn deref(&self) -> &S {
        &self.inner
    }
}
impl<S> std::ops::DerefMut for Tee<S> {
    fn deref_mut(&mut self) -> &mut S {
        &mut self.inner
    }
}
impl<S: SimulationDebugSinkV1> SimulationDebugSinkV1 for Tee<S> {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        if let Some(trace) = &mut self.trace {
            if let Err(error) = trace.record(&record) {
                self.tap_failure = Some(error);
                return SimulationDebugSinkControlV1::Stop;
            }
        }
        self.inner.record(record)
    }
}
pub(super) fn scratch_fits<S>() -> bool {
    // Both stack copies, positive row and encoder fixed buffer are covered by
    // the unchanged 65536-byte per-attempt prepayment before construction.
    2 * std::mem::size_of::<Tee<S>>()
        + 2 * std::mem::size_of::<Memory>()
        + 2 * std::mem::size_of::<wire::Positive>()
        + wire::POSITIVE_BYTES
        + 8192
        <= 65536
}
