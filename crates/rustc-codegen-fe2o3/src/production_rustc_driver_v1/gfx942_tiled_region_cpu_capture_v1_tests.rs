//! Fixed observations of the existing Engine, not a second numerical interpreter.
use super::oracle::{self, Control, Words};
use super::*;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, BlockId, ScalarType, ValueId};
use fe2o3_kir_sim::*;

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct RunRow {
    pub pattern: usize,
    pub output_length: usize,
    pub values_row_major_le_hex: Words,
    pub output_with_canaries_le_hex: oracle::Output,
    pub actual_allocations: [u64; 3],
    pub matrix_lane_mask: u64,
    pub committed_store_lane_mask: u64,
    pub records: u64,
    pub steps: u64,
    pub storage_floor: usize,
    pub storage_after: usize,
    pub work_before: usize,
    pub work_after: usize,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct NegativeRow {
    pub control: Control,
    pub observed: &'static str,
    pub matrix_lane_mask: u64,
    pub global_writes: u64,
    pub floor_restored: bool,
}
#[derive(Clone, Copy)]
pub(super) struct Sites {
    pub matrix: (BlockId, u32),
    pub store: (BlockId, u32),
    pub results: [ValueId; 4],
    pub parameters: [ValueId; 4],
}
pub(super) struct Sink {
    sites: Sites,
    expected: Words,
    output_length: usize,
    validate: bool,
    stop: bool,
    pub failure: Option<&'static str>,
    pub records: u64,
    pub matrix_mask: u64,
    pub store_mask: u64,
    pub global_writes: u64,
    pub allocations: Option<[u64; 3]>,
    pub values: Words,
}
impl Sink {
    pub fn new(sites: Sites, pattern: usize, length: usize, control: Control) -> Self {
        Self {
            sites,
            expected: oracle::expected(pattern),
            output_length: length,
            validate: matches!(control, Control::Positive),
            stop: matches!(control, Control::DebugStop),
            failure: None,
            records: 0,
            matrix_mask: 0,
            store_mask: 0,
            global_writes: 0,
            allocations: None,
            values: Words([0; 256]),
        }
    }
    fn inspect(&mut self, record: &SimulationDebugRecordV1) -> Result<(), &'static str> {
        if record.ordinal != self.records {
            return Err("non-dense actual debug ordinal");
        }
        self.records += 1;
        let invocation = record.invocation;
        let lane = invocation.local[0] as usize;
        if lane >= 64
            || invocation.local != [lane as u32, 0, 0]
            || invocation.global != [lane as u64, 0, 0]
            || invocation.workgroup != [0, 0, 0]
            || invocation.workgroup_size != [64, 1, 1]
            || invocation.launch_extent != [64, 1, 1]
        {
            return Err("actual invocation shape");
        }
        let site = (record.site.block, record.site.operation);
        match &record.kind {
            SimulationDebugRecordKindV1::Checkpoint {
                phase: SimulationDebugCheckpointPhaseV1::AfterOperation,
                stack,
                ..
            } if record.site.function_ordinal == 0 && site == self.sites.matrix => {
                if self.matrix_mask & (1 << lane) != 0 {
                    return Err("duplicate actual MFMA completion");
                }
                let SimulationDebugCollectionV1::Captured(frames) = stack else {
                    return Err("matrix frame absent");
                };
                let [frame] = frames.as_slice() else {
                    return Err("matrix frame roster");
                };
                if frame.depth != 0 || frame.function_ordinal != 0 {
                    return Err("foreign matrix frame");
                }
                let SimulationDebugCollectionV1::Captured(values) = &frame.values else {
                    return Err("matrix values absent");
                };
                let mut allocations = [0; 3];
                for (slot, target) in allocations.iter_mut().enumerate() {
                    let mut matches = values
                        .iter()
                        .filter(|v| v.value == self.sites.parameters[slot]);
                    let value = matches.next().ok_or("actual parameter binding absent")?;
                    if matches.next().is_some() {
                        return Err("duplicate parameter binding");
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
                        return Err("actual source argument is not a slice");
                    };
                    let (wanted_ty, wanted_access, wanted_offset, wanted_len) = if slot < 2 {
                        (ScalarType::U16, AccessMode::ReadOnly, 0, 256)
                    } else {
                        (
                            ScalarType::F32,
                            AccessMode::ReadWrite,
                            8,
                            self.output_length,
                        )
                    };
                    if *element != wanted_ty
                        || *access != wanted_access
                        || *address_space != AddressSpace::Global
                        || *byte_offset != wanted_offset
                        || *elements != wanted_len
                        || *byte_len != wanted_len * if slot < 2 { 2 } else { 4 }
                    {
                        return Err("actual source slice relation differs");
                    }
                    *target = *allocation;
                }
                if allocations[0] == allocations[1]
                    || allocations[0] == allocations[2]
                    || allocations[1] == allocations[2]
                {
                    return Err("actual independent allocation identities alias");
                }
                if self.allocations.is_some_and(|old| old != allocations) {
                    return Err("allocation identity changed across lanes");
                }
                self.allocations = Some(allocations);
                for component in 0..4 {
                    let mut matches = values
                        .iter()
                        .filter(|v| v.value == self.sites.results[component]);
                    let value = matches.next().ok_or("actual MFMA result binding absent")?;
                    if matches.next().is_some() {
                        return Err("duplicate MFMA binding");
                    }
                    let SimulationDebugValueV1::Scalar(scalar) = &value.observed else {
                        return Err("MFMA result not scalar");
                    };
                    if scalar.ty() != ScalarType::F32 {
                        return Err("MFMA result not F32");
                    }
                    let bits = u32::try_from(scalar.bits()).map_err(|_| "F32 value width")?;
                    if self.validate && bits != oracle::lane_word(&self.expected, lane, component) {
                        return Err(
                            "actual four-component MFMA differs from independent integer oracle",
                        );
                    }
                    self.values.0[(4 * (lane / 16) + component) * 16 + lane % 16] = bits;
                }
                self.matrix_mask |= 1 << lane;
            }
            SimulationDebugRecordKindV1::Memory {
                access: SimulationDebugMemoryAccessV1::WriteCommitted,
                allocation,
                byte_offset,
                byte_len,
                address_space: AddressSpace::Global,
                value,
            } => {
                self.global_writes += 1;
                if !self.validate {
                    return Ok(());
                }
                let allocations = self
                    .allocations
                    .ok_or("global store precedes matrix completion")?;
                if site != self.sites.store
                    || record.site.function_ordinal != 0
                    || *allocation != allocations[2]
                    || *byte_offset != 8 + 4 * lane
                    || *byte_len != 4
                    || lane >= self.output_length
                    || self.store_mask & (1 << lane) != 0
                    || self.matrix_mask & (1 << lane) == 0
                {
                    return Err("source-authored store identity or range differs");
                }
                let SimulationDebugValueV1::Scalar(scalar) = value else {
                    return Err("store is not scalar");
                };
                if scalar.ty() != ScalarType::F32
                    || scalar.bits() != u128::from(oracle::lane_word(&self.expected, lane, 0))
                {
                    return Err("source-authored store data differs");
                }
                self.store_mask |= 1 << lane;
            }
            _ => {}
        }
        Ok(())
    }
    pub fn complete(&self) {
        assert_eq!(self.failure, None);
        assert_eq!(self.matrix_mask, u64::MAX);
        let expected = if self.output_length == 64 {
            u64::MAX
        } else {
            (1u64 << self.output_length) - 1
        };
        assert_eq!(self.store_mask, expected);
        assert_eq!(self.global_writes, self.output_length as u64);
        assert_eq!(self.values, self.expected);
        assert!(self.allocations.is_some());
    }
}
impl SimulationDebugSinkV1 for Sink {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        if self.stop {
            return SimulationDebugSinkControlV1::Stop;
        }
        if let Err(error) = self.inspect(&record) {
            self.failure = Some(error);
            return SimulationDebugSinkControlV1::Stop;
        }
        SimulationDebugSinkControlV1::Continue
    }
}
pub(super) struct Events {
    pub fail: bool,
}
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, _: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        if self.fail {
            Err(SimulationEventSinkErrorV1 {
                detail: "source CPU event control".into(),
            })
        } else {
            Ok(())
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct PreflightSummary {
    pub kind: &'static str,
    pub detail: Option<&'static str>,
    pub numbers: [u64; 3],
    pub first_site: Option<[u32; 2]>,
}
pub(super) fn preflight_summary(
    result: Result<&SimulationExecutionV1, &SimulationErrorV1>,
) -> Option<PreflightSummary> {
    use SimulationPreflightErrorV1 as P;
    let Err(SimulationErrorV1::Preflight(error)) = result else {
        return None;
    };
    let mut row = PreflightSummary {
        kind: "other-preflight",
        detail: None,
        numbers: [0; 3],
        first_site: None,
    };
    row.kind = match error {
        P::ResourceLimit {
            resource,
            actual,
            limit,
        } => {
            row.detail = Some(resource);
            row.numbers = [*actual, *limit, 0];
            "resource-limit"
        }
        P::Unsupported(report) => {
            row.numbers = [report.total_findings(), report.findings().len() as u64, 0];
            if let Some(first) = report.findings().first() {
                row.first_site = first
                    .block
                    .zip(first.operation)
                    .map(|(block, op)| [block.0, op]);
                row.detail = Some(unsupported_feature_name(&first.feature));
            }
            "unsupported"
        }
        P::StaticLaunchMismatch {
            axis,
            expected,
            actual,
        } => {
            row.numbers = [*axis as u64, *expected, *actual];
            "static-launch"
        }
        P::WorkgroupMismatch { expected, actual } => {
            row.numbers = [u64::from(expected[0]), u64::from(actual[0]), 0];
            "workgroup"
        }
        P::ArgumentCount { expected, actual } => {
            row.numbers = [*expected as u64, *actual as u64, 0];
            "argument-count"
        }
        P::ArgumentType { argument, .. } => {
            row.numbers[0] = *argument as u64;
            "argument-type"
        }
        P::BufferAccess { argument, .. } => {
            row.numbers[0] = *argument as u64;
            "buffer-access"
        }
        P::TargetLayout { argument } => {
            row.numbers[0] = *argument as u64;
            "target-layout"
        }
        P::TargetValueOutOfRange { argument } => {
            row.numbers[0] = *argument as u64;
            "target-value"
        }
        P::MissingBacking { argument, backing } => {
            row.numbers = [*argument as u64, u64::from(*backing), 0];
            "missing-backing"
        }
        P::BufferViewBounds { argument } => {
            row.numbers[0] = *argument as u64;
            "view-bounds"
        }
        P::SharedTargetLayout(backing) => {
            row.numbers[0] = u64::from(*backing);
            "shared-target-layout"
        }
        P::DuplicateBacking(backing) => {
            row.numbers[0] = u64::from(*backing);
            "duplicate-backing"
        }
        P::InvalidLaunch(detail) => {
            row.detail = Some(detail);
            "invalid-launch"
        }
        P::InvalidLimits(_) => "invalid-limits",
        P::UnknownKernel(_) => "unknown-kernel",
        P::MissingEntry(_) => "missing-entry",
        P::DynamicWorkgroupMemory(_) => "dynamic-workgroup-memory",
        P::AllocationFailure => "allocation-failure",
        P::PhysicalEntrySymbolicDebugUnavailableV20 => "physical-entry-debug",
        P::PhysicalGlobalCopyPendingDebugUnavailableV21 => "physical-copy-debug",
        P::PhysicalLdsExchangeDebugUnavailableV22 => "physical-lds-debug",
        P::PhysicalLdsExchangeAliasedArgumentsV22 => "physical-lds-alias",
        P::PhysicalGlobalCopyAliasedArgumentsV21 => "physical-copy-alias",
    };
    Some(row)
}
fn unsupported_feature_name(feature: &UnsupportedFeatureV1) -> &'static str {
    use UnsupportedFeatureV1 as F;
    match feature {
        F::InertV12Carrier => "inert-v12-carrier",
        F::InertExecutionV15 => "inert-execution-v15",
        F::FloatType(_) => "float-type",
        F::UnsupportedType => "unsupported-type",
        F::MemoryIntrinsic => "memory-intrinsic",
        F::ExternalVolatileMemory => "external-volatile",
        F::MemoryIntrinsicTargetLayout => "memory-intrinsic-layout",
        F::FloatConstant => "float-constant",
        F::FloatOperation => "float-operation",
        F::FloatFunction(_) => "float-function",
        F::InvalidIntegerCast { .. } => "integer-cast",
        F::ExternalCall(_) => "external-call",
        F::NonInternalCall { .. } => "non-internal-call",
        F::WorkgroupAllocation => "workgroup-allocation",
        F::NonScalarMemory => "non-scalar-memory",
        F::UnsupportedAddressSpace(_) => "address-space",
        F::Barrier => "barrier",
        F::Atomic => "atomic",
        F::Fence => "fence",
        F::WorkgroupBarrier => "workgroup-barrier",
        F::WorkgroupMemory => "workgroup-memory",
        F::DynamicWorkgroupMemory => "dynamic-workgroup-memory",
        F::Matrix => "matrix",
        F::UnsupportedNumericalContract => "numerical-contract",
        F::Wave => "wave",
        F::Gfx950LdsTranspose => "gfx950-lds-transpose",
        F::InlineAssembly => "inline-assembly",
        F::OrderedRegion => "ordered-region",
        F::OrderedRegionProfile => "ordered-region-profile",
        F::UnsupportedScalarOperation => "scalar-operation",
        F::TargetConstantOutOfRange => "target-constant",
        F::OrderedProgram => "ordered-program",
        F::OrderedProgramProfile => "ordered-program-profile",
        F::CompleteBody => "complete-body",
        F::CompleteBodyProfile => "complete-body-profile",
        F::PhysicalEntry => "physical-entry",
        F::PhysicalEntryProfile => "physical-entry-profile",
        F::PhysicalGlobalCopyProfile => "physical-copy-profile",
        F::PhysicalLdsExchangeProfile => "physical-lds-profile",
    }
}

pub(super) fn classify(result: Result<&SimulationExecutionV1, &SimulationErrorV1>) -> &'static str {
    match result {
        Ok(_) => "ok",
        Err(SimulationErrorV1::Preflight(_)) => "preflight",
        Err(SimulationErrorV1::Execution(error)) => match error.kind {
            SimulationExecutionErrorKindV1::UninitializedRead {
                offset: 510,
                bytes: 2,
                ..
            } => "uninitialized-read",
            SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain {
                role: MatrixInputRoleV1::A,
                lane: 63,
                component: 3,
            } => "matrix-domain-a",
            SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain {
                role: MatrixInputRoleV1::B,
                lane: 63,
                component: 3,
            } => "matrix-domain-b",
            SimulationExecutionErrorKindV1::StepLimit { limit: 1 } => "step-limit",
            SimulationExecutionErrorKindV1::EventSinkFailure(_) => "event-sink",
            _ => "other-execution-refusal",
        },
    }
}
pub(super) fn expected_refusal(control: Control) -> &'static str {
    match control {
        Control::UninitializedA | Control::UninitializedB => "uninitialized-read",
        Control::NegativeZeroA
        | Control::FractionalA
        | Control::SubnormalA
        | Control::OutsideDomainA => "matrix-domain-a",
        Control::NegativeZeroB | Control::FractionalB | Control::InfiniteB => "matrix-domain-b",
        Control::StepLimit => "step-limit",
        Control::RecordLimit | Control::DebugStop => "incomplete-observation",
        Control::EventFailure => "event-sink",
        Control::Grid63 | Control::Grid65 | Control::Wave32 => "profile-launch",
        Control::Positive => "ok",
    }
}
#[test]
fn negative_oracle_does_not_accept_other_refusal_or_success() {
    for control in oracle::NEGATIVES {
        assert_ne!(expected_refusal(control), "ok");
        assert_ne!(expected_refusal(control), "other-execution-refusal");
        assert_ne!(expected_refusal(control), "preflight");
    }
}
