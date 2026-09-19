//! Compact bounded acceptance observations. Not a serialized debugger protocol.
use super::topology::Topology;
use fe2o3_kernel_ir::AddressSpace;
use fe2o3_kir_sim::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const MAX_ROWS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Row {
    pub ordinal: u64,
    pub decision: u64,
    pub lane: u32,
    pub site: [u32; 3],
    pub kind: u32,
    pub activation: u64,
    pub attempt: u64,
    pub allocation: u64,
    pub offset: u64,
    pub bits: u32,
}
const _: () = assert!(std::mem::size_of::<Row>() <= 128);

impl Row {
    fn legacy(mut self) -> Self {
        self.activation = 0;
        self.attempt = 0;
        self
    }
    pub fn json(self) -> Value {
        json!([
            self.ordinal,
            self.decision,
            self.lane,
            self.site,
            self.kind,
            self.activation,
            self.attempt,
            self.allocation,
            self.offset,
            self.bits
        ])
    }
}

pub(super) struct Capture {
    pub contextual: bool,
    pub rows: Vec<Row>,
    failure: Option<&'static str>,
}

pub(super) fn invocation(lane: u32) -> SimulationInvocationV1 {
    SimulationInvocationV1 {
        global: [u64::from(lane), 0, 0],
        workgroup: [0, 0, 0],
        local: [lane, 0, 0],
        workgroup_size: [64, 1, 1],
        workgroup_count: [1, 1, 1],
        launch_extent: [4, 1, 1],
    }
}

impl Capture {
    pub fn new(contextual: bool) -> Result<Self, String> {
        let mut rows = Vec::new();
        rows.try_reserve_exact(MAX_ROWS)
            .map_err(|_| "observer row allocation")?;
        if rows.capacity() * std::mem::size_of::<Row>() > 512 * 1024 {
            return Err("observer actual row capacity cap".into());
        }
        Ok(Self {
            contextual,
            rows,
            failure: None,
        })
    }

    fn retain(
        &mut self,
        record: SimulationDebugRecordV1,
        context: Option<SimulationDebugOriginContextV1>,
    ) -> Result<(), &'static str> {
        if self.rows.len() == MAX_ROWS {
            return Err("observer record cap");
        }
        if record.ordinal != self.rows.len() as u64 {
            return Err("observer record ordinal");
        }
        let lane = u32::try_from(record.invocation.global[0]).map_err(|_| "observer lane")?;
        if lane >= 4 || record.invocation != invocation(lane) {
            return Err("observer full invocation");
        }
        let mut row = Row {
            ordinal: record.ordinal,
            decision: record.schedule.decision_ordinal,
            lane,
            site: [
                u32::try_from(record.site.function_ordinal).map_err(|_| "observer function")?,
                record.site.block.0,
                record.site.operation,
            ],
            kind: 0,
            activation: 0,
            attempt: 0,
            allocation: 0,
            offset: 0,
            bits: 0,
        };
        if self.contextual {
            let Some(SimulationDebugOriginContextV1::Available(origin)) = context else {
                return Err("observer origin unavailable");
            };
            if origin.invocation() != record.invocation
                || origin.site() != record.site
                || origin.activation() == 0
                || origin.attempt() == 0
            {
                return Err("observer origin invocation/site/token mismatch");
            }
            row.activation = origin.activation();
            row.attempt = origin.attempt();
        } else if context.is_some() {
            return Err("observer unexpected opt-out context");
        }
        match record.kind {
            SimulationDebugRecordKindV1::Checkpoint { phase, .. } => {
                row.kind = match phase {
                    SimulationDebugCheckpointPhaseV1::BeforeOperation => 0,
                    SimulationDebugCheckpointPhaseV1::AfterOperation => 1,
                }
            }
            SimulationDebugRecordKindV1::Memory {
                access: SimulationDebugMemoryAccessV1::WriteCommitted,
                allocation,
                byte_offset,
                byte_len: 4,
                address_space: AddressSpace::Global,
                value: SimulationDebugValueV1::Scalar(value),
            } if value.ty() == fe2o3_kernel_ir::ScalarType::U32 => {
                row.kind = 2;
                row.allocation = allocation;
                row.offset = byte_offset as u64;
                row.bits = u32::try_from(value.bits()).map_err(|_| "observer memory value")?;
            }
            _ => return Err("observer unexpected record in pure-loop source profile"),
        }
        self.rows.push(row);
        Ok(())
    }

    fn delivered(
        &mut self,
        record: SimulationDebugRecordV1,
        context: Option<SimulationDebugOriginContextV1>,
    ) -> SimulationDebugSinkControlV1 {
        if self.failure.is_some() {
            return SimulationDebugSinkControlV1::DropAndStop;
        }
        match self.retain(record, context) {
            Ok(()) => SimulationDebugSinkControlV1::Continue,
            Err(error) => {
                self.failure = Some(error);
                SimulationDebugSinkControlV1::DropAndStop
            }
        }
    }

    pub fn ensure(&self) -> Result<(), String> {
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        Ok(())
    }

    pub fn same_legacy(&self, other: &Self) -> Result<(), String> {
        self.ensure()?;
        other.ensure()?;
        if self.rows.iter().copied().map(Row::legacy).ne(other
            .rows
            .iter()
            .copied()
            .map(Row::legacy))
        {
            return Err("observer compact legacy projection changed".into());
        }
        Ok(())
    }

    pub fn validate(
        &self,
        topology: &Topology,
        trips: u32,
        expected: u32,
    ) -> Result<Value, String> {
        self.ensure()?;
        if !self.contextual || self.rows.is_empty() || self.rows.len() > MAX_ROWS || trips > 3 {
            return Err("observer validation profile/cap".into());
        }
        let mut pending = BTreeMap::new();
        let mut last = BTreeMap::new();
        let mut calls = Vec::new();
        let mut helper = BTreeMap::<(u32, u64), (u64, u64)>::new();
        let mut writes = BTreeSet::new();
        let mut allocation = None;
        for (index, row) in self.rows.iter().enumerate() {
            if row.ordinal != index as u64
                || row.lane >= 4
                || row.activation == 0
                || row.attempt == 0
            {
                return Err("observer retained row identity".into());
            }
            if row.site[0] == topology.entry as u32 && row.activation != 1 {
                return Err("observer root activation".into());
            }
            let key = (row.lane, row.activation, row.attempt);
            match row.kind {
                0 => {
                    let prior = last.entry((row.lane, row.activation)).or_insert(0);
                    if row.attempt != *prior + 1
                        || pending.insert(key, (row.site, row.ordinal)).is_some()
                    {
                        return Err("observer repeated/missing attempt".into());
                    }
                    *prior = row.attempt;
                }
                1 => {
                    let (site, before) = pending
                        .remove(&key)
                        .ok_or("observer after without before")?;
                    if site != row.site || before >= row.ordinal {
                        return Err("observer after site/order".into());
                    }
                    if site == topology.call {
                        calls.push((row.lane, before, row.ordinal));
                    }
                }
                2 => {
                    let (site, before) =
                        pending.get(&key).ok_or("observer memory without before")?;
                    if *site != row.site
                        || *before >= row.ordinal
                        || row.offset != 4 + 4 * u64::from(row.lane)
                        || row.bits != expected
                        || !writes.insert(row.lane)
                    {
                        return Err("observer unexpected output memory".into());
                    }
                    if let Some(prior) = allocation {
                        if row.allocation != prior {
                            return Err("observer changed output allocation".into());
                        }
                    } else {
                        allocation = Some(row.allocation);
                    }
                }
                _ => return Err("observer retained row kind".into()),
            }
            if row.site[0] == topology.helper as u32 {
                if row.activation == 1 || !topology.helper_sites.contains(&row.site) {
                    return Err("observer helper activation/site".into());
                }
                let interval = helper
                    .entry((row.lane, row.activation))
                    .or_insert((row.ordinal, row.ordinal));
                interval.1 = row.ordinal;
            }
        }
        if !pending.is_empty() || writes != BTreeSet::from([0, 1, 2, 3]) {
            return Err("observer unclosed attempt or missing output".into());
        }
        for lane in 0..4 {
            let local_calls = calls
                .iter()
                .copied()
                .filter(|row| row.0 == lane)
                .collect::<Vec<_>>();
            let local_helpers = helper
                .iter()
                .filter(|((l, _), _)| *l == lane)
                .map(|(&(_, activation), &(first, last))| (activation, first, last))
                .collect::<Vec<_>>();
            if local_calls.len() != trips as usize || local_helpers.len() != trips as usize {
                return Err("observer actual repeated-call/helper count".into());
            }
            for &(_, first, last) in &local_helpers {
                if local_calls
                    .iter()
                    .filter(|(_, before, after)| *before < first && last < *after)
                    .count()
                    != 1
                {
                    return Err("observer helper not within exact caller interval".into());
                }
            }
            for &(_, before, after) in &local_calls {
                if local_helpers
                    .iter()
                    .filter(|(_, first, last)| before < *first && *last < after)
                    .count()
                    != 1
                {
                    return Err("observer caller lacks exactly one helper activation".into());
                }
            }
        }
        Ok(
            json!({"records":self.rows.len(),"call_attempts":calls.len(),
            "helper_activations":helper.len(),"global_writes":writes.len(),
            "rows":self.rows.iter().copied().map(Row::json).collect::<Vec<_>>()}),
        )
    }
}

impl SimulationDebugSinkV1 for Capture {
    fn wants_operation_origin_v1(&self) -> bool {
        self.contextual
    }
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        self.delivered(record, None)
    }
    fn record_with_operation_origin_v1(
        &mut self,
        record: SimulationDebugRecordV1,
        origin: SimulationDebugOriginContextV1,
    ) -> SimulationDebugSinkControlV1 {
        self.delivered(record, Some(origin))
    }
}
