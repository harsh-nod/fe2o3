//! Fixed-size observations of actual Call/Return frames, never a second executor.
use super::*;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, BlockId, OperationKind, ScalarType, ValueId};
use fe2o3_kir_sim::*;

#[derive(Clone, Copy, Debug)]
pub(super) struct Sites {
    pub root: usize,
    pub helper: usize,
    pub call: (BlockId, u32),
    pub matrix: (BlockId, u32),
    pub store: (BlockId, u32),
    pub matrix_results: [ValueId; 4],
    pub call_results: [ValueId; 4],
    pub parameters: [ValueId; 4],
    pub permutation: [u8; 4],
}
impl Serialize for Sites {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut row = serializer.serialize_struct("HelperSourceSites", 9)?;
        row.serialize_field("root", &self.root)?;
        row.serialize_field("helper", &self.helper)?;
        row.serialize_field("call", &[self.call.0.0, self.call.1])?;
        row.serialize_field("matrix", &[self.matrix.0.0, self.matrix.1])?;
        row.serialize_field("store", &[self.store.0.0, self.store.1])?;
        row.serialize_field("matrix_results", &self.matrix_results.map(|id| id.0))?;
        row.serialize_field("call_results", &self.call_results.map(|id| id.0))?;
        row.serialize_field("parameters", &self.parameters.map(|id| id.0))?;
        row.serialize_field("permutation", &self.permutation)?;
        row.end()
    }
}
pub(super) fn sites(
    emission: &fe2o3_lower_mir_kernel::Bf16CallInstanceEmissionViewV1<'_>,
) -> Result<Sites, Error> {
    let graph = emission.owner().executable().module();
    let root = graph
        .functions
        .iter()
        .position(|f| &f.id == emission.root_function())
        .ok_or(Error::Unavailable("actual emitted root absent"))?;
    let helper = graph
        .functions
        .iter()
        .position(|f| &f.id == emission.helper_function())
        .ok_or(Error::Unavailable("actual emitted helper absent"))?;
    if root == helper
        || graph.kernels.len() != 1
        || graph.kernels[0].entry != graph.functions[root].id
    {
        return Err(Error::Unavailable("actual emitted root/helper/kernel join"));
    }
    let body = graph.functions[root]
        .body
        .as_ref()
        .ok_or(Error::Unavailable("actual root body"))?;
    let parameters = body
        .parameters
        .as_slice()
        .try_into()
        .map_err(|_| Error::Unavailable("actual four root parameters"))?;
    let matrix_results = emission
        .producer_components(fe2o3_lower_mir_kernel::Bf16CallInstanceRoleV1::Result)
        .try_into()
        .map_err(|_| Error::Unavailable("actual four helper MFMA results"))?;
    let call_results = *emission.call_results();
    let mut store = None;
    for block in &body.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            if matches!(operation.kind, OperationKind::Store { value, .. } if value == call_results[0])
            {
                if store.replace((block.id, ordinal as u32)).is_some() {
                    return Err(Error::Unavailable("actual source output Store repeated"));
                }
            }
        }
    }
    let store = store.ok_or(Error::Unavailable("actual source output Store absent"))?;
    Ok(Sites {
        root,
        helper,
        call: emission.call_site(),
        matrix: emission.matrix_site(),
        store,
        matrix_results,
        call_results,
        parameters,
        permutation: emission.return_permutation(),
    })
}
pub(super) fn expected_call(expected: &oracle::Words, permutation: [u8; 4]) -> oracle::Words {
    let mut words = oracle::Words([0; 256]);
    for lane in 0..64 {
        for component in 0..4 {
            words.0[(4 * (lane / 16) + component) * 16 + lane % 16] =
                oracle::lane_word(expected, lane, usize::from(permutation[component]));
        }
    }
    words
}
pub(super) fn expected_output(
    pattern: usize,
    length: usize,
    permutation: [u8; 4],
) -> oracle::Output {
    let words = oracle::expected(pattern);
    let mut output = oracle::Output([0; 272]);
    for (slot, raw) in output.0.chunks_exact_mut(4).enumerate() {
        let word = if (2..2 + length).contains(&slot) {
            oracle::lane_word(&words, slot - 2, usize::from(permutation[0]))
        } else {
            oracle::CANARY
        };
        raw.copy_from_slice(&word.to_le_bytes());
    }
    output
}
pub(super) fn check_output(
    run: &SimulationExecutionV1,
    pattern: usize,
    length: usize,
    permutation: [u8; 4],
) -> oracle::Output {
    assert_eq!(run.invocations_executed(), 64);
    assert_eq!(run.workgroups_visited(), 1);
    assert_eq!(run.shared_buffers().len(), 3);
    for input in 0..2 {
        let buffer = run
            .shared_buffer(oracle::INPUT_IDS[input])
            .expect("actual input backing");
        assert_eq!(buffer.bytes(), oracle::bytes(pattern, input));
        assert!(buffer.initialized().iter().all(|value| *value));
    }
    let output = run
        .shared_buffer(oracle::INPUT_IDS[2])
        .expect("actual output backing");
    assert!(output.initialized().iter().all(|value| *value));
    let wanted = expected_output(pattern, length, permutation);
    assert_eq!(output.bytes(), wanted.0);
    assert!(!run.grants_execution_authority());
    oracle::Output(
        output
            .bytes()
            .try_into()
            .expect("272-byte output with canaries"),
    )
}

pub(super) struct Frames {
    sites: Sites,
    expected: oracle::Words,
    length: usize,
    pub matrix_values: oracle::Words,
    pub call_values: oracle::Words,
    pub matrix_mask: u64,
    pub call_mask: u64,
    pub store_mask: u64,
    pub writes: u64,
    pub records: u64,
    pub allocations: Option<[u64; 3]>,
    pub failure: Option<&'static str>,
}
impl Frames {
    pub fn new(sites: Sites, pattern: usize, length: usize) -> Self {
        Self {
            sites,
            expected: oracle::expected(pattern),
            length,
            matrix_values: oracle::Words([0; 256]),
            call_values: oracle::Words([0; 256]),
            matrix_mask: 0,
            call_mask: 0,
            store_mask: 0,
            writes: 0,
            records: 0,
            allocations: None,
            failure: None,
        }
    }
    fn inspect(&mut self, record: &SimulationDebugRecordV1) -> Result<(), &'static str> {
        if record.ordinal != self.records {
            return Err("non-dense actual record ordinal");
        }
        self.records += 1;
        let lane = record.invocation.local[0] as usize;
        if lane >= 64
            || record.invocation.local != [lane as u32, 0, 0]
            || record.invocation.global != [lane as u64, 0, 0]
            || record.invocation.workgroup != [0, 0, 0]
            || record.invocation.workgroup_size != [64, 1, 1]
            || record.invocation.launch_extent != [64, 1, 1]
        {
            return Err("actual helper invocation shape");
        }
        let site = (record.site.block, record.site.operation);
        let matrix = record.site.function_ordinal == self.sites.helper && site == self.sites.matrix;
        let call = record.site.function_ordinal == self.sites.root && site == self.sites.call;
        match &record.kind {
            SimulationDebugRecordKindV1::Checkpoint {
                phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
                stack,
                ..
            } if matrix || call => {
                let SimulationDebugCollectionV1::Captured(frames) = stack else {
                    return Err("actual complete frame stack absent");
                };
                if frames.len() != if matrix { 2 } else { 1 }
                    || frames[0].depth != 0
                    || frames[0].function_ordinal != self.sites.root
                {
                    return Err("actual root/callee frame stack differs");
                }
                let active = frames.last().ok_or("active frame absent")?;
                if active.depth != if matrix { 1 } else { 0 }
                    || active.function_ordinal
                        != if matrix {
                            self.sites.helper
                        } else {
                            self.sites.root
                        }
                {
                    return Err("actual active helper/caller frame differs");
                }
                let SimulationDebugCollectionV1::Captured(root_values) = &frames[0].values else {
                    return Err("complete suspended-root values absent");
                };
                let mut allocations = [0; 3];
                for (slot, target) in allocations.iter_mut().enumerate() {
                    let mut selected = root_values
                        .iter()
                        .filter(|v| v.value == self.sites.parameters[slot]);
                    let value = selected
                        .next()
                        .ok_or("actual root slice parameter absent")?;
                    if selected.next().is_some() {
                        return Err("duplicate root parameter");
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
                        return Err("actual root parameter not a slice");
                    };
                    let (ty, mode, offset, count) = if slot < 2 {
                        (ScalarType::U16, AccessMode::ReadOnly, 0, 256)
                    } else {
                        (ScalarType::F32, AccessMode::ReadWrite, 8, self.length)
                    };
                    if *element != ty
                        || *access != mode
                        || *address_space != AddressSpace::Global
                        || *byte_offset != offset
                        || *elements != count
                        || *byte_len != count * if slot < 2 { 2 } else { 4 }
                    {
                        return Err("actual root slice mapping differs");
                    }
                    *target = *allocation;
                }
                if allocations[0] == allocations[1]
                    || allocations[0] == allocations[2]
                    || allocations[1] == allocations[2]
                    || self.allocations.is_some_and(|old| old != allocations)
                {
                    return Err("actual backing identities alias or drift");
                }
                self.allocations = Some(allocations);
                let SimulationDebugCollectionV1::Captured(values) = &active.values else {
                    return Err("complete active SSA bindings absent");
                };
                let (defs, mask, words) = if matrix {
                    (
                        self.sites.matrix_results,
                        &mut self.matrix_mask,
                        &mut self.matrix_values,
                    )
                } else {
                    (
                        self.sites.call_results,
                        &mut self.call_mask,
                        &mut self.call_values,
                    )
                };
                if *mask & (1u64 << lane) != 0 {
                    return Err("duplicate helper/caller completion");
                }
                for (component, id) in defs.into_iter().enumerate() {
                    let mut selected = values.iter().filter(|v| v.value == id);
                    let value = selected
                        .next()
                        .ok_or("actual helper/caller result absent")?;
                    if selected.next().is_some() {
                        return Err("duplicate actual SSA result");
                    }
                    let SimulationDebugValueV1::Scalar(scalar) = &value.observed else {
                        return Err("actual helper/caller result not scalar");
                    };
                    let source_component = if matrix {
                        component
                    } else {
                        usize::from(self.sites.permutation[component])
                    };
                    let expected = oracle::lane_word(&self.expected, lane, source_component);
                    if scalar.ty() != ScalarType::F32 || scalar.bits() != u128::from(expected) {
                        return Err(
                            "actual helper/caller value differs from independent dense oracle",
                        );
                    }
                    words.0[(4 * (lane / 16) + component) * 16 + lane % 16] = expected;
                }
                *mask |= 1u64 << lane;
            }
            SimulationDebugRecordKindV1::Memory {
                access: SimulationDebugMemoryAccessV1::WriteCommitted,
                allocation,
                byte_offset,
                byte_len,
                address_space: AddressSpace::Global,
                value,
            } => {
                let allocations = self.allocations.ok_or("global Store preceded helper")?;
                if site != self.sites.store
                    || record.site.function_ordinal != self.sites.root
                    || *allocation != allocations[2]
                    || *byte_offset != 8 + 4 * lane
                    || *byte_len != 4
                    || lane >= self.length
                    || self.store_mask & (1u64 << lane) != 0
                    || self.call_mask & (1u64 << lane) == 0
                {
                    return Err("actual caller Store identity, bounds or ordering differs");
                }
                let SimulationDebugValueV1::Scalar(scalar) = value else {
                    return Err("actual Store not scalar");
                };
                if scalar.ty() != ScalarType::F32
                    || scalar.bits()
                        != u128::from(oracle::lane_word(
                            &self.expected,
                            lane,
                            usize::from(self.sites.permutation[0]),
                        ))
                {
                    return Err("actual caller Store value differs");
                }
                self.writes += 1;
                self.store_mask |= 1u64 << lane;
            }
            _ => {}
        }
        Ok(())
    }
    pub fn complete(&self) {
        assert_eq!(self.failure, None);
        assert_eq!(self.matrix_mask, u64::MAX);
        assert_eq!(self.call_mask, u64::MAX);
        assert_eq!(
            self.store_mask,
            if self.length == 64 {
                u64::MAX
            } else {
                (1u64 << self.length) - 1
            }
        );
        assert_eq!(self.writes, self.length as u64);
        assert_eq!(self.matrix_values, self.expected);
        assert_eq!(
            self.call_values,
            expected_call(&self.expected, self.sites.permutation)
        );
        assert!(self.allocations.is_some());
    }
}
impl SimulationDebugSinkV1 for Frames {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        if let Err(error) = self.inspect(&record) {
            self.failure = Some(error);
            return SimulationDebugSinkControlV1::Stop;
        }
        SimulationDebugSinkControlV1::Continue
    }
}

#[test]
fn identity_and_swap_oracle_permutation_preserve_all_four_components_and_canaries() {
    for pattern in 0..oracle::PATTERNS {
        let expected = oracle::expected(pattern);
        for permutation in [[0, 1, 2, 3], [1, 0, 2, 3]] {
            let returned = expected_call(&expected, permutation);
            for lane in 0..64 {
                for (component, selected) in permutation.into_iter().enumerate() {
                    assert_eq!(
                        oracle::lane_word(&returned, lane, component),
                        oracle::lane_word(&expected, lane, usize::from(selected))
                    );
                }
            }
            for length in oracle::LENGTHS {
                let output = expected_output(pattern, length, permutation);
                for slot in 0..68 {
                    let expected = if (2..2 + length).contains(&slot) {
                        oracle::lane_word(&expected, slot - 2, usize::from(permutation[0]))
                    } else {
                        oracle::CANARY
                    };
                    assert_eq!(&output.0[slot * 4..slot * 4 + 4], &expected.to_le_bytes());
                }
            }
        }
    }
}
