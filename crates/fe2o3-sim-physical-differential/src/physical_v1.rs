use std::{error::Error, fmt, io::Write};

use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use fe2o3_host::{
    GeneratedKfdPackingObservationV1, GeneratedPackingComponentKindV1,
    GeneratedWorkerV3KfdDifferentialBindingV1, GeneratedWorkerV3KfdDifferentialObservationV1,
};
use fe2o3_hsaco_finalize::{
    ProductionKirV7BridgeKirVersionV1, ProductionKirV7BridgeTargetV1,
    ProductionKirV7StructuralBridgeV1,
};
use fe2o3_kernel_ir::{AccessMode, ScalarType, VerifiedSimulationBundleV4};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, IndexWidthV1, SimulationArgumentV1, SimulationExecutionV1,
    SimulationFailureReductionReportV1, SimulationLimitsV1, SimulationRequestV1,
    SimulationScheduleIdentityV1, SimulationScheduleRequestV1, SimulationTargetV1,
};
use fe2o3_runtime::Gfx942RuntimeBufferAccessV1;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const PHYSICAL_DIFFERENTIAL_SCHEMA_V1: &str = "fe2o3-simulator-direct-kfd-differential-v1";
pub const PHYSICAL_DIFFERENTIAL_SIMULATOR_CONTRACT_V1: &str =
    "fe2o3-kir-sim-admitted-scheduled-execution-v1";
pub const PHYSICAL_DIFFERENTIAL_CAPABILITIES_SCHEMA_V1: &str =
    "fe2o3-simulator-direct-kfd-differential-capabilities-v1";
pub const MAX_PHYSICAL_DIFFERENTIAL_BYTES_V1: usize = 16 * 1024 * 1024;
const REPORT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SIM-DIFFERENTIAL/PHYSICAL-REPORT/V1\0";
const REQUEST_SNAPSHOT_DOMAIN_V1: &[u8] = b"FE2O3/SIM-DIFFERENTIAL/REQUEST-SNAPSHOT/V1\0";
const MAX_MISMATCHES_V1: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalDifferentialLimitsV1 {
    pub max_snapshot_bytes: usize,
    pub max_output_bytes: usize,
    pub max_report_bytes: usize,
}

impl Default for PhysicalDifferentialLimitsV1 {
    fn default() -> Self {
        Self {
            max_snapshot_bytes: 4 * 1024 * 1024,
            max_output_bytes: 4 * 1024 * 1024,
            max_report_bytes: 16 * 1024 * 1024,
        }
    }
}

impl PhysicalDifferentialLimitsV1 {
    pub fn validate(self) -> Result<Self, PhysicalDifferentialErrorV1> {
        for (field, value) in [
            ("max_snapshot_bytes", self.max_snapshot_bytes),
            ("max_output_bytes", self.max_output_bytes),
            ("max_report_bytes", self.max_report_bytes),
        ] {
            if value == 0 || value > MAX_PHYSICAL_DIFFERENTIAL_BYTES_V1 {
                return Err(PhysicalDifferentialErrorV1::InvalidLimit(field));
            }
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalDifferentialUnavailableV1 {
    NoKfdDevice,
    TargetProfileUnavailable,
    ProtectedVerifierUnavailable,
    GeneratedApplicationBridgeUnavailable,
}

/// Current production readiness is explicit and cannot be confused with a parity pass.
pub const fn physical_differential_production_readiness_v1() -> PhysicalDifferentialUnavailableV1 {
    PhysicalDifferentialUnavailableV1::ProtectedVerifierUnavailable
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PhysicalDifferentialCapabilitiesV1 {
    pub schema: &'static str,
    pub direct_kfd_only: bool,
    pub generated_worker_v3_only: bool,
    pub legacy_llvm_fixture_excluded: bool,
    pub hardware_unavailable_counts_as_pass: bool,
    pub hardware_passes: u32,
    pub parity_passes: u32,
    pub parity_requires_sealed_completed_observation: bool,
    pub executable_compare_surface: &'static str,
    pub current_production_blocker: PhysicalDifferentialUnavailableV1,
}

pub const fn physical_differential_capabilities_v1() -> PhysicalDifferentialCapabilitiesV1 {
    let qualification = crate::physical_differential_qualification_v2();
    PhysicalDifferentialCapabilitiesV1 {
        schema: PHYSICAL_DIFFERENTIAL_CAPABILITIES_SCHEMA_V1,
        direct_kfd_only: true,
        generated_worker_v3_only: true,
        legacy_llvm_fixture_excluded: true,
        hardware_unavailable_counts_as_pass: false,
        hardware_passes: if qualification.hardware_observed {
            1
        } else {
            0
        },
        parity_passes: if qualification.parity_observed { 1 } else { 0 },
        parity_requires_sealed_completed_observation: true,
        executable_compare_surface: "generated-host-library-api-only",
        current_production_blocker: physical_differential_production_readiness_v1(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhysicalDifferentialDispositionV1 {
    Agreement,
    Discrepancy,
    HardwareUnavailable(PhysicalDifferentialUnavailableV1),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PhysicalDifferentialByteMismatchV1 {
    pub argument_index: usize,
    pub buffer_index: usize,
    pub byte_offset: usize,
    pub simulator: u8,
    pub hardware: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PhysicalDifferentialBufferV1 {
    pub argument_index: usize,
    pub buffer_index: usize,
    pub access: &'static str,
    pub simulator_sha256: String,
    pub hardware_sha256: String,
    pub simulator_bytes_hex: String,
    pub hardware_bytes_hex: String,
    pub equal: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PhysicalDifferentialReportV1 {
    pub schema: &'static str,
    pub disposition: PhysicalDifferentialDispositionV1,
    pub authority: &'static str,
    pub differential_binding_sha256: String,
    pub kernel_id_sha256: String,
    pub kernel_logical_name: String,
    pub kernel_export_name: String,
    pub kernel_binding_sha256: String,
    pub generated_host_contract_sha256: String,
    pub direct_kfd_runtime_contract: String,
    pub worker_challenge_sha256: String,
    pub hardware_observed: bool,
    pub hardware_passes: u32,
    pub parity_passes: u32,
    pub request_snapshot_sha256: String,
    pub request_snapshot_hex: String,
    pub simulator_kir_version: u16,
    pub simulator_contract: &'static str,
    pub simulator_kir_sha256: String,
    pub simulator_kir_bytes: u64,
    pub production_kir_version: u16,
    pub production_kir_sha256: String,
    pub production_kir_bytes: u64,
    pub bundle_v1_sha256: String,
    pub bundle_v2_sha256: String,
    pub bundle_v3_sha256: String,
    pub bundle_v4_sha256: String,
    pub source_map_v2_sha256: String,
    pub structural_bridge_sha256: String,
    pub target: String,
    pub compiler_execution_subject_sha256: String,
    pub compiler_execution_receipt_sha256: String,
    pub worker_lineage_sha256: String,
    pub finalizer_derivation_sha256: String,
    pub finalized_hsaco_sha256: String,
    pub finalized_hsaco_bytes: u64,
    pub dispatch_contract_sha256: String,
    pub dispatch_grid: [u32; 3],
    pub dispatch_workgroup: [u16; 3],
    pub dynamic_group_segment_bytes: u32,
    pub generated_packing_sha256: String,
    pub device_unique_id: u64,
    pub device_topology_sha256: String,
    pub simulator_schedule: &'static str,
    pub simulator_schedule_transcript_sha256: String,
    pub simulator_schedule_record_sha256: Option<String>,
    pub simulator_limits_sha256: String,
    pub reduction_report_sha256: Option<String>,
    pub reduction_reproducer_sha256: Option<String>,
    pub compared_buffers: Vec<PhysicalDifferentialBufferV1>,
    pub total_byte_mismatches: u64,
    pub retained_mismatches: Vec<PhysicalDifferentialByteMismatchV1>,
    pub report_sha256: String,
}

impl PhysicalDifferentialReportV1 {
    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, PhysicalDifferentialErrorV1> {
        let mut writer = BoundedWriter::new(MAX_PHYSICAL_DIFFERENTIAL_BYTES_V1);
        serde_json::to_writer(&mut writer, self)
            .map_err(|_| PhysicalDifferentialErrorV1::ReportEncoding)?;
        Ok(writer.bytes)
    }
}

#[must_use = "a prepared physical differential must be completed or explicitly unavailable"]
pub struct PreparedPhysicalDifferentialV1 {
    binding_identity: [u8; 32],
    axes: ReportAxesV1,
    request_snapshot: Vec<u8>,
    simulator_outputs: Vec<ExpectedOutputV1>,
    physical_buffer_shape: Vec<PhysicalBufferShapeV1>,
    max_report_bytes: usize,
}

struct ExpectedOutputV1 {
    argument_index: usize,
    buffer_index: usize,
    access: Gfx942RuntimeBufferAccessV1,
    bytes: Vec<u8>,
}

struct PhysicalBufferShapeV1 {
    buffer_index: usize,
    access: Gfx942RuntimeBufferAccessV1,
    bytes: usize,
}

struct ReportAxesV1 {
    differential_binding: [u8; 32],
    kernel_id: [u8; 32],
    kernel_logical_name: String,
    kernel_export_name: String,
    kernel_binding: [u8; 32],
    generated_host_contract: [u8; 32],
    direct_kfd_runtime_contract: String,
    worker_challenge: [u8; 32],
    simulator_kir_version: u16,
    simulator_kir_sha256: [u8; 32],
    simulator_kir_bytes: u64,
    production_kir_sha256: [u8; 32],
    production_kir_bytes: u64,
    bundle_v1: [u8; 32],
    bundle_v2: [u8; 32],
    bundle_v3: [u8; 32],
    bundle_v4: [u8; 32],
    source_map_v2: [u8; 32],
    structural_bridge: [u8; 32],
    target: String,
    compiler_subject: [u8; 32],
    compiler_receipt: [u8; 32],
    worker_lineage: [u8; 32],
    finalizer: [u8; 32],
    hsaco: [u8; 32],
    hsaco_bytes: u64,
    dispatch: [u8; 32],
    dispatch_grid: [u32; 3],
    dispatch_workgroup: [u16; 3],
    dynamic_group_segment_bytes: u32,
    packing: [u8; 32],
    device_unique_id: u64,
    device_topology: [u8; 32],
    schedule: SimulationScheduleIdentityV1,
    schedule_transcript: [u8; 32],
    schedule_record: Option<[u8; 32]>,
    limits: [u8; 32],
    reduction: Option<([u8; 32], [u8; 32])>,
}

/// Simulates exact bundle V4 custody and prepares a single-use direct-KFD comparison.
#[allow(clippy::too_many_arguments)]
pub fn prepare_physical_differential_v1(
    bundle: &VerifiedSimulationBundleV4,
    bridge: &ProductionKirV7StructuralBridgeV1,
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    simulation_limits: SimulationLimitsV1,
    schedule: SimulationScheduleRequestV1<'_>,
    binding: &GeneratedWorkerV3KfdDifferentialBindingV1,
    reduction: Option<&SimulationFailureReductionReportV1>,
    limits: PhysicalDifferentialLimitsV1,
) -> Result<PreparedPhysicalDifferentialV1, PhysicalDifferentialErrorV1> {
    let limits = limits.validate()?;
    bundle
        .revalidate()
        .map_err(|error| PhysicalDifferentialErrorV1::Bundle(error.to_string()))?;
    validate_bundle_and_bridge(bundle, bridge, binding)?;
    validate_generated_inputs(request, target, binding.packing(), binding)?;
    let request_snapshot = encode_request_snapshot(request, target, simulation_limits, limits)?;
    let execution = module
        .simulate_scheduled(request, target, simulation_limits, schedule)
        .map_err(|error| PhysicalDifferentialErrorV1::Simulation(error.to_string()))?;
    let inner_v1 = bundle.inner_v3().inner_v2().inner_v1();
    let kir = inner_v1.canonical_kir_v7_identity();
    if execution.identity().wire_version() != 7
        || execution.identity().digest() != kir.digest()
        || execution.identity().canonical_length() != kir.canonical_length()
    {
        return Err(PhysicalDifferentialErrorV1::SimulatorKirSubstitution);
    }
    let simulator_outputs =
        collect_simulator_outputs(&execution, binding.packing(), limits.max_output_bytes)?;
    if simulator_outputs.is_empty() {
        return Err(PhysicalDifferentialErrorV1::NoWritableOutput);
    }
    let physical_buffer_shape = collect_physical_buffer_shape(binding.packing())?;
    let reduction =
        reduction.map(|report| (*report.report_identity(), *report.reproducer_identity()));
    let source_map_v2 = sha256(bundle.inner_v3().inner_v2().debug_map());
    let axes = ReportAxesV1 {
        differential_binding: *binding.identity(),
        kernel_id: *binding.kernel_id(),
        kernel_logical_name: binding.logical_name().to_owned(),
        kernel_export_name: binding.export_name().to_owned(),
        kernel_binding: *binding.kernel_binding_identity(),
        generated_host_contract: *binding.generated_host_contract_identity(),
        direct_kfd_runtime_contract: binding.direct_kfd_runtime_contract().to_owned(),
        worker_challenge: *binding.worker_challenge_identity(),
        simulator_kir_version: 7,
        simulator_kir_sha256: *kir.digest(),
        simulator_kir_bytes: kir.canonical_length(),
        production_kir_sha256: *binding.production_kir_v8_sha256(),
        production_kir_bytes: binding.production_kir_v8_bytes(),
        bundle_v1: *inner_v1.identity().as_bytes(),
        bundle_v2: *bundle.inner_v3().inner_v2().identity().as_bytes(),
        bundle_v3: *bundle.inner_v3().identity().as_bytes(),
        bundle_v4: *bundle.identity().as_bytes(),
        source_map_v2,
        structural_bridge: *bridge.identity(),
        target: binding.target().to_owned(),
        compiler_subject: *binding.compiler_execution_subject_identity(),
        compiler_receipt: *binding.compiler_execution_receipt_identity(),
        worker_lineage: *binding.worker_lineage_identity(),
        finalizer: *binding.finalizer_derivation_identity(),
        hsaco: *binding.finalized_hsaco_sha256(),
        hsaco_bytes: binding.finalized_hsaco_bytes(),
        dispatch: *binding.dispatch_contract_sha256(),
        dispatch_grid: binding.grid(),
        dispatch_workgroup: binding.workgroup(),
        dynamic_group_segment_bytes: binding.dynamic_group_segment_bytes(),
        packing: *binding.packing().identity(),
        device_unique_id: binding.device_unique_id(),
        device_topology: *binding.device_topology_identity(),
        schedule: execution.schedule(),
        schedule_transcript: *execution.schedule_transcript_identity(),
        schedule_record: execution
            .schedule_record()
            .map(|record| *record.record_integrity()),
        limits: simulation_limits_identity(simulation_limits),
        reduction,
    };
    Ok(PreparedPhysicalDifferentialV1 {
        binding_identity: *binding.identity(),
        axes,
        request_snapshot,
        simulator_outputs,
        physical_buffer_shape,
        max_report_bytes: limits.max_report_bytes,
    })
}

impl PreparedPhysicalDifferentialV1 {
    pub fn complete(
        self,
        observation: GeneratedWorkerV3KfdDifferentialObservationV1,
    ) -> Result<PhysicalDifferentialReportV1, PhysicalDifferentialErrorV1> {
        validate_observation_identity(&self.binding_identity, observation.binding().identity())?;
        let buffers = observation.result().buffers();
        if buffers.len() != self.physical_buffer_shape.len() {
            return Err(PhysicalDifferentialErrorV1::CompletedBufferSubstitution);
        }
        for (index, (expected, observed)) in
            self.physical_buffer_shape.iter().zip(buffers).enumerate()
        {
            validate_completed_buffer_shape(
                expected,
                index,
                observed.access(),
                observed.bytes().len(),
            )?;
        }
        let mut compared = Vec::new();
        compared
            .try_reserve_exact(self.simulator_outputs.len())
            .map_err(|_| PhysicalDifferentialErrorV1::AllocationFailure)?;
        let mut mismatches = Vec::new();
        mismatches
            .try_reserve_exact(MAX_MISMATCHES_V1)
            .map_err(|_| PhysicalDifferentialErrorV1::AllocationFailure)?;
        let mut total_mismatches = 0_u64;
        for expected in &self.simulator_outputs {
            let observed = buffers
                .get(expected.buffer_index)
                .ok_or(PhysicalDifferentialErrorV1::CompletedBufferSubstitution)?;
            if observed.access() != expected.access
                || observed.bytes().len() != expected.bytes.len()
            {
                return Err(PhysicalDifferentialErrorV1::CompletedBufferSubstitution);
            }
            for (byte_offset, (&simulator, &hardware)) in
                expected.bytes.iter().zip(observed.bytes()).enumerate()
            {
                if simulator != hardware {
                    total_mismatches = total_mismatches
                        .checked_add(1)
                        .ok_or(PhysicalDifferentialErrorV1::ResourceLimit)?;
                    if mismatches.len() < MAX_MISMATCHES_V1 {
                        mismatches.push(PhysicalDifferentialByteMismatchV1 {
                            argument_index: expected.argument_index,
                            buffer_index: expected.buffer_index,
                            byte_offset,
                            simulator,
                            hardware,
                        });
                    }
                }
            }
            compared.push(PhysicalDifferentialBufferV1 {
                argument_index: expected.argument_index,
                buffer_index: expected.buffer_index,
                access: runtime_access_name(expected.access),
                simulator_sha256: hex(&sha256(&expected.bytes)),
                hardware_sha256: hex(&sha256(observed.bytes())),
                simulator_bytes_hex: hex(&expected.bytes),
                hardware_bytes_hex: hex(observed.bytes()),
                equal: expected.bytes == observed.bytes(),
            });
        }
        self.report(
            if total_mismatches == 0 {
                PhysicalDifferentialDispositionV1::Agreement
            } else {
                PhysicalDifferentialDispositionV1::Discrepancy
            },
            true,
            1,
            u32::from(total_mismatches == 0),
            compared,
            total_mismatches,
            mismatches,
        )
    }

    pub fn hardware_unavailable(
        self,
        reason: PhysicalDifferentialUnavailableV1,
    ) -> Result<PhysicalDifferentialReportV1, PhysicalDifferentialErrorV1> {
        self.report(
            PhysicalDifferentialDispositionV1::HardwareUnavailable(reason),
            false,
            0,
            0,
            Vec::new(),
            0,
            Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn report(
        self,
        disposition: PhysicalDifferentialDispositionV1,
        hardware_observed: bool,
        hardware_passes: u32,
        parity_passes: u32,
        compared_buffers: Vec<PhysicalDifferentialBufferV1>,
        total_byte_mismatches: u64,
        retained_mismatches: Vec<PhysicalDifferentialByteMismatchV1>,
    ) -> Result<PhysicalDifferentialReportV1, PhysicalDifferentialErrorV1> {
        let snapshot_sha = sha256(&self.request_snapshot);
        let mut report = PhysicalDifferentialReportV1 {
            schema: PHYSICAL_DIFFERENTIAL_SCHEMA_V1,
            disposition,
            authority: "observation-only",
            differential_binding_sha256: hex(&self.axes.differential_binding),
            kernel_id_sha256: hex(&self.axes.kernel_id),
            kernel_logical_name: self.axes.kernel_logical_name,
            kernel_export_name: self.axes.kernel_export_name,
            kernel_binding_sha256: hex(&self.axes.kernel_binding),
            generated_host_contract_sha256: hex(&self.axes.generated_host_contract),
            direct_kfd_runtime_contract: self.axes.direct_kfd_runtime_contract,
            worker_challenge_sha256: hex(&self.axes.worker_challenge),
            hardware_observed,
            hardware_passes,
            parity_passes,
            request_snapshot_sha256: hex(&snapshot_sha),
            request_snapshot_hex: hex(&self.request_snapshot),
            simulator_kir_version: self.axes.simulator_kir_version,
            simulator_contract: PHYSICAL_DIFFERENTIAL_SIMULATOR_CONTRACT_V1,
            simulator_kir_sha256: hex(&self.axes.simulator_kir_sha256),
            simulator_kir_bytes: self.axes.simulator_kir_bytes,
            production_kir_version: 8,
            production_kir_sha256: hex(&self.axes.production_kir_sha256),
            production_kir_bytes: self.axes.production_kir_bytes,
            bundle_v1_sha256: hex(&self.axes.bundle_v1),
            bundle_v2_sha256: hex(&self.axes.bundle_v2),
            bundle_v3_sha256: hex(&self.axes.bundle_v3),
            bundle_v4_sha256: hex(&self.axes.bundle_v4),
            source_map_v2_sha256: hex(&self.axes.source_map_v2),
            structural_bridge_sha256: hex(&self.axes.structural_bridge),
            target: self.axes.target,
            compiler_execution_subject_sha256: hex(&self.axes.compiler_subject),
            compiler_execution_receipt_sha256: hex(&self.axes.compiler_receipt),
            worker_lineage_sha256: hex(&self.axes.worker_lineage),
            finalizer_derivation_sha256: hex(&self.axes.finalizer),
            finalized_hsaco_sha256: hex(&self.axes.hsaco),
            finalized_hsaco_bytes: self.axes.hsaco_bytes,
            dispatch_contract_sha256: hex(&self.axes.dispatch),
            dispatch_grid: self.axes.dispatch_grid,
            dispatch_workgroup: self.axes.dispatch_workgroup,
            dynamic_group_segment_bytes: self.axes.dynamic_group_segment_bytes,
            generated_packing_sha256: hex(&self.axes.packing),
            device_unique_id: self.axes.device_unique_id,
            device_topology_sha256: hex(&self.axes.device_topology),
            simulator_schedule: schedule_name(self.axes.schedule),
            simulator_schedule_transcript_sha256: hex(&self.axes.schedule_transcript),
            simulator_schedule_record_sha256: self.axes.schedule_record.map(|value| hex(&value)),
            simulator_limits_sha256: hex(&self.axes.limits),
            reduction_report_sha256: self.axes.reduction.map(|value| hex(&value.0)),
            reduction_reproducer_sha256: self.axes.reduction.map(|value| hex(&value.1)),
            compared_buffers,
            total_byte_mismatches,
            retained_mismatches,
            report_sha256: String::new(),
        };
        report.report_sha256 = report_identity(&report)?;
        let mut writer = BoundedWriter::new(self.max_report_bytes);
        serde_json::to_writer(&mut writer, &report)
            .map_err(|_| PhysicalDifferentialErrorV1::ReportTooLarge)?;
        Ok(report)
    }
}

fn validate_bundle_and_bridge(
    bundle: &VerifiedSimulationBundleV4,
    bridge: &ProductionKirV7StructuralBridgeV1,
    binding: &GeneratedWorkerV3KfdDifferentialBindingV1,
) -> Result<(), PhysicalDifferentialErrorV1> {
    let v2 = bundle.inner_v3().inner_v2();
    let v1 = v2.inner_v1();
    let production = v1.production_kir_identity();
    if production.version() != 8
        || production.digest() != *binding.production_kir_v8_sha256()
        || production.canonical_length() != binding.production_kir_v8_bytes()
    {
        return Err(PhysicalDifferentialErrorV1::ProductionKirSubstitution);
    }
    let association = v1
        .require_canonical_compiler_execution_association()
        .map_err(|_| PhysicalDifferentialErrorV1::CompilerExecutionAssociationUnavailable)?;
    let subject = InertCompilerExecutionSubjectV1::decode(association.canonical_bytes())
        .map_err(|_| PhysicalDifferentialErrorV1::CompilerExecutionAssociationInvalid)?;
    if association.claimed_identity() != binding.compiler_execution_subject_identity()
        || subject.identity().sha256() != binding.compiler_execution_subject_identity()
    {
        return Err(PhysicalDifferentialErrorV1::CompilerExecutionSubstitution);
    }
    if v1.target() != binding.target() || v1.target() != "gfx942:xnack-" {
        return Err(PhysicalDifferentialErrorV1::TargetSubstitution);
    }
    if bridge.target() != ProductionKirV7BridgeTargetV1::Gfx942
        || bridge.production_version() != ProductionKirV7BridgeKirVersionV1::V8
    {
        return Err(PhysicalDifferentialErrorV1::StructuralBridgeSubstitution);
    }
    let simulator = bridge.simulator_v7_identity();
    let neutral = bridge.neutral_production_identity();
    let source = bridge.source_map_v2_identity();
    let artifact = bridge.artifact_identity();
    if simulator.sha256() != *v1.canonical_kir_v7_identity().digest()
        || simulator.byte_len() != v1.canonical_kir_v7_identity().canonical_length()
        || neutral.sha256() != production.digest()
        || neutral.byte_len() != production.canonical_length()
        || source.sha256() != sha256(v2.debug_map())
        || source.byte_len()
            != u64::try_from(v2.debug_map().len())
                .map_err(|_| PhysicalDifferentialErrorV1::ResourceLimit)?
        || artifact.sha256() != *binding.finalized_hsaco_sha256()
        || artifact.byte_len() != binding.finalized_hsaco_bytes()
    {
        return Err(PhysicalDifferentialErrorV1::StructuralBridgeSubstitution);
    }
    Ok(())
}

fn validate_generated_inputs(
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    packing: &GeneratedKfdPackingObservationV1,
    binding: &GeneratedWorkerV3KfdDifferentialBindingV1,
) -> Result<(), PhysicalDifferentialErrorV1> {
    validate_simulation_target(target)?;
    if !request.shared_buffers.is_empty() {
        return Err(PhysicalDifferentialErrorV1::UnsupportedAliasedBufferView);
    }
    if request.kernel.as_str() != binding.logical_name() {
        return Err(PhysicalDifferentialErrorV1::KernelSubstitution);
    }
    validate_geometry(
        request.grid.0,
        request.workgroup.0,
        binding.grid(),
        binding.workgroup(),
    )?;
    if binding.dynamic_group_segment_bytes() != 0 {
        return Err(PhysicalDifferentialErrorV1::UnsupportedDynamicGroupSegment);
    }
    let mut explicit = Vec::new();
    explicit
        .try_reserve_exact(packing.explicit_kernarg_bytes())
        .map_err(|_| PhysicalDifferentialErrorV1::AllocationFailure)?;
    explicit.resize(packing.explicit_kernarg_bytes(), 0);
    let mut covered = Vec::new();
    covered
        .try_reserve_exact(request.arguments.len())
        .map_err(|_| PhysicalDifferentialErrorV1::AllocationFailure)?;
    covered.resize(request.arguments.len(), false);
    for (argument_index, argument) in request.arguments.iter().enumerate() {
        let mut components = packing
            .components()
            .iter()
            .copied()
            .filter(|component| component.argument_index() == argument_index);
        match argument {
            SimulationArgumentV1::Scalar(value) => {
                let Some(component) = components.next() else {
                    return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution);
                };
                if components.next().is_some() {
                    return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution);
                }
                if component.kind() != GeneratedPackingComponentKindV1::Scalar {
                    return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution);
                }
                let scalar_bytes = scalar_bytes(value.ty(), target)?;
                if component.size()
                    != u64::try_from(scalar_bytes)
                        .map_err(|_| PhysicalDifferentialErrorV1::ResourceLimit)?
                {
                    return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution);
                }
                write_component(
                    &mut explicit,
                    component,
                    &value.bits().to_le_bytes()[..scalar_bytes],
                )?;
            }
            SimulationArgumentV1::Buffer(buffer) => {
                let first = components
                    .next()
                    .ok_or(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?;
                let second = components
                    .next()
                    .ok_or(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?;
                if components.next().is_some() {
                    return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution);
                }
                let (pointer, length) = match (first.kind(), second.kind()) {
                    (
                        GeneratedPackingComponentKindV1::SlicePointer,
                        GeneratedPackingComponentKindV1::SliceLength,
                    ) => (first, second),
                    (
                        GeneratedPackingComponentKindV1::SliceLength,
                        GeneratedPackingComponentKindV1::SlicePointer,
                    ) => (second, first),
                    _ => return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution),
                };
                if pointer.size() != 8 || length.size() != 8 {
                    return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution);
                }
                let elements = buffer
                    .element_count(target)
                    .map_err(|_| PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?;
                write_component(
                    &mut explicit,
                    length,
                    &u64::try_from(elements)
                        .map_err(|_| PhysicalDifferentialErrorV1::ResourceLimit)?
                        .to_le_bytes(),
                )?;
                let generated = packing
                    .buffers()
                    .iter()
                    .find(|candidate| candidate.argument_index() == argument_index)
                    .ok_or(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?;
                let generated_matches = if buffer.bytes().is_empty() {
                    generated.buffer_index().is_none()
                        && generated.access().is_none()
                        && generated.initial_bytes() == 0
                        && generated.initial_sha256() == [0; 32]
                } else {
                    generated.buffer_index().is_some()
                        && generated.initial_bytes() == buffer.bytes().len()
                        && generated.initial_sha256() == sha256(buffer.bytes())
                        && generated
                            .access()
                            .is_some_and(|access| runtime_access_matches(access, buffer.access()))
                };
                if !buffer.initialized().iter().all(|initialized| *initialized)
                    || !generated_matches
                {
                    return Err(PhysicalDifferentialErrorV1::GeneratedInputSubstitution);
                }
            }
            SimulationArgumentV1::BufferView(_) => {
                return Err(PhysicalDifferentialErrorV1::UnsupportedAliasedBufferView);
            }
        }
        covered[argument_index] = true;
    }
    if !covered.iter().all(|covered| *covered)
        || packing
            .components()
            .iter()
            .any(|component| component.argument_index() >= request.arguments.len())
        || packing.buffers().len()
            != request
                .arguments
                .iter()
                .filter(|argument| matches!(argument, SimulationArgumentV1::Buffer(_)))
                .count()
        || !packing.matches_explicit_kernarg(&explicit)
    {
        return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution);
    }
    Ok(())
}

fn validate_simulation_target(
    target: SimulationTargetV1,
) -> Result<(), PhysicalDifferentialErrorV1> {
    if target != SimulationTargetV1::amdgpu_64() {
        return Err(PhysicalDifferentialErrorV1::TargetSubstitution);
    }
    Ok(())
}

fn validate_geometry(
    request_grid: [u64; 3],
    request_workgroup: [u32; 3],
    bound_grid: [u32; 3],
    bound_workgroup: [u16; 3],
) -> Result<(), PhysicalDifferentialErrorV1> {
    let converted_grid = request_grid
        .map(u32::try_from)
        .map(|value| value.map_err(|_| PhysicalDifferentialErrorV1::GeometrySubstitution));
    let [x, y, z] = converted_grid;
    if [x?, y?, z?] != bound_grid || request_workgroup != bound_workgroup.map(u32::from) {
        return Err(PhysicalDifferentialErrorV1::GeometrySubstitution);
    }
    Ok(())
}

fn validate_observation_identity(
    expected: &[u8; 32],
    observed: &[u8; 32],
) -> Result<(), PhysicalDifferentialErrorV1> {
    if expected != observed {
        return Err(PhysicalDifferentialErrorV1::PhysicalObservationSubstitution);
    }
    Ok(())
}

fn validate_completed_buffer_shape(
    expected: &PhysicalBufferShapeV1,
    observed_index: usize,
    observed_access: Gfx942RuntimeBufferAccessV1,
    observed_bytes: usize,
) -> Result<(), PhysicalDifferentialErrorV1> {
    if expected.buffer_index != observed_index
        || expected.access != observed_access
        || expected.bytes != observed_bytes
    {
        return Err(PhysicalDifferentialErrorV1::CompletedBufferSubstitution);
    }
    Ok(())
}

fn collect_simulator_outputs(
    execution: &SimulationExecutionV1,
    packing: &GeneratedKfdPackingObservationV1,
    max_bytes: usize,
) -> Result<Vec<ExpectedOutputV1>, PhysicalDifferentialErrorV1> {
    let mut total = 0_usize;
    let mut outputs = Vec::new();
    for generated in packing.buffers() {
        let Some(buffer_index) = generated.buffer_index() else {
            continue;
        };
        let Some(access) = generated.access() else {
            return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution);
        };
        if access == Gfx942RuntimeBufferAccessV1::ReadOnly {
            continue;
        }
        let buffer = execution
            .buffer(generated.argument_index())
            .ok_or(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?;
        total = total
            .checked_add(buffer.bytes().len())
            .ok_or(PhysicalDifferentialErrorV1::ResourceLimit)?;
        if total > max_bytes {
            return Err(PhysicalDifferentialErrorV1::OutputTooLarge);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(buffer.bytes().len())
            .map_err(|_| PhysicalDifferentialErrorV1::AllocationFailure)?;
        bytes.extend_from_slice(buffer.bytes());
        outputs.push(ExpectedOutputV1 {
            argument_index: generated.argument_index(),
            buffer_index,
            access,
            bytes,
        });
    }
    Ok(outputs)
}

fn collect_physical_buffer_shape(
    packing: &GeneratedKfdPackingObservationV1,
) -> Result<Vec<PhysicalBufferShapeV1>, PhysicalDifferentialErrorV1> {
    let count = packing
        .buffers()
        .iter()
        .filter(|binding| binding.buffer_index().is_some())
        .count();
    let mut shape = Vec::new();
    shape
        .try_reserve_exact(count)
        .map_err(|_| PhysicalDifferentialErrorV1::AllocationFailure)?;
    for expected_index in 0..count {
        let binding = packing
            .buffers()
            .iter()
            .find(|binding| binding.buffer_index() == Some(expected_index))
            .ok_or(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?;
        shape.push(PhysicalBufferShapeV1 {
            buffer_index: expected_index,
            access: binding
                .access()
                .ok_or(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?,
            bytes: binding.initial_bytes(),
        });
    }
    Ok(shape)
}

fn encode_request_snapshot(
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    differential_limits: PhysicalDifferentialLimitsV1,
) -> Result<Vec<u8>, PhysicalDifferentialErrorV1> {
    let mut writer = SnapshotWriter::new(differential_limits.max_snapshot_bytes);
    writer.bytes(REQUEST_SNAPSHOT_DOMAIN_V1)?;
    writer.string(request.kernel.as_str())?;
    for value in request.grid.0 {
        writer.u64(value)?;
    }
    for value in request.workgroup.0 {
        writer.u32(value)?;
    }
    writer.u8(match target.index_width() {
        IndexWidthV1::Bits32 => 32,
        IndexWidthV1::Bits64 => 64,
    })?;
    writer.usize(request.arguments.len())?;
    for argument in &request.arguments {
        match argument {
            SimulationArgumentV1::Scalar(value) => {
                writer.u8(1)?;
                writer.u8(scalar_tag(value.ty()))?;
                writer.bytes(&value.bits().to_le_bytes())?;
            }
            SimulationArgumentV1::Buffer(buffer) => {
                writer.u8(2)?;
                writer.u8(scalar_tag(buffer.element()))?;
                writer.u8(access_tag(buffer.access()))?;
                writer.u32(buffer.alignment())?;
                writer.bytes(buffer.bytes())?;
                writer.usize(buffer.initialized().len())?;
                for initialized in buffer.initialized() {
                    writer.u8(u8::from(*initialized))?;
                }
            }
            SimulationArgumentV1::BufferView(_) => {
                return Err(PhysicalDifferentialErrorV1::UnsupportedAliasedBufferView);
            }
        }
    }
    writer.usize(request.shared_buffers.len())?;
    writer.u8(match request.events {
        fe2o3_kir_sim::EventPolicyV1::Disabled => 0,
        fe2o3_kir_sim::EventPolicyV1::Enabled => 1,
    })?;
    encode_limits(&mut writer, limits)?;
    Ok(writer.bytes)
}

fn encode_limits(
    writer: &mut SnapshotWriter,
    limits: SimulationLimitsV1,
) -> Result<(), PhysicalDifferentialErrorV1> {
    for value in [
        limits.max_canonical_bytes,
        limits.max_reachable_functions,
        limits.max_reachable_operations,
        limits.max_call_depth,
        limits.max_ssa_values,
        limits.max_allocations,
        limits.max_allocation_bytes,
        limits.max_total_bytes,
        limits.max_resident_bytes,
        limits.max_memory_access_records,
    ] {
        writer.usize(value)?;
    }
    for value in [
        limits.max_invocations,
        limits.max_workgroups,
        limits.max_scheduled_slots,
        limits.max_steps,
        limits.max_events,
    ] {
        writer.u64(value)?;
    }
    Ok(())
}

fn write_component(
    bytes: &mut [u8],
    component: fe2o3_host::GeneratedPackingComponentV1,
    value: &[u8],
) -> Result<(), PhysicalDifferentialErrorV1> {
    let start = usize::try_from(component.offset())
        .map_err(|_| PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?;
    let end = start
        .checked_add(value.len())
        .ok_or(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?;
    if u64::try_from(value.len()).map_err(|_| PhysicalDifferentialErrorV1::ResourceLimit)?
        != component.size()
        || end > bytes.len()
    {
        return Err(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution);
    }
    bytes[start..end].copy_from_slice(value);
    Ok(())
}

fn scalar_bytes(
    ty: ScalarType,
    target: SimulationTargetV1,
) -> Result<usize, PhysicalDifferentialErrorV1> {
    let bits = match ty {
        ScalarType::Index => match target.index_width() {
            IndexWidthV1::Bits32 => 32,
            IndexWidthV1::Bits64 => 64,
        },
        _ => ty
            .bit_width()
            .ok_or(PhysicalDifferentialErrorV1::GeneratedPackingSubstitution)?,
    };
    Ok(if bits == 1 { 1 } else { usize::from(bits / 8) })
}

fn simulation_limits_identity(limits: SimulationLimitsV1) -> [u8; 32] {
    let mut writer = SnapshotWriter::new(1024);
    encode_limits(&mut writer, limits).expect("fixed limit encoding is bounded");
    sha256(&writer.bytes)
}

fn report_identity(
    report: &PhysicalDifferentialReportV1,
) -> Result<String, PhysicalDifferentialErrorV1> {
    debug_assert!(report.report_sha256.is_empty());
    let mut writer = BoundedWriter::new(MAX_PHYSICAL_DIFFERENTIAL_BYTES_V1);
    serde_json::to_writer(&mut writer, report)
        .map_err(|_| PhysicalDifferentialErrorV1::ReportEncoding)?;
    let mut hash = Sha256::new();
    hash.update(REPORT_IDENTITY_DOMAIN_V1);
    hash.update(&writer.bytes);
    Ok(hex(&hash.finalize()))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn scalar_tag(value: ScalarType) -> u8 {
    match value {
        ScalarType::Bool => 1,
        ScalarType::I8 => 2,
        ScalarType::I16 => 3,
        ScalarType::I32 => 4,
        ScalarType::I64 => 5,
        ScalarType::I128 => 6,
        ScalarType::U8 => 7,
        ScalarType::U16 => 8,
        ScalarType::U32 => 9,
        ScalarType::U64 => 10,
        ScalarType::U128 => 11,
        ScalarType::Index => 12,
        ScalarType::F16 => 13,
        ScalarType::Bf16 => 14,
        ScalarType::F32 => 15,
        ScalarType::F64 => 16,
    }
}
fn access_tag(value: AccessMode) -> u8 {
    match value {
        AccessMode::ReadOnly => 1,
        AccessMode::WriteOnly => 2,
        AccessMode::ReadWrite => 3,
    }
}
fn runtime_access_name(value: Gfx942RuntimeBufferAccessV1) -> &'static str {
    match value {
        Gfx942RuntimeBufferAccessV1::ReadOnly => "read_only",
        Gfx942RuntimeBufferAccessV1::WriteOnly => "write_only",
        Gfx942RuntimeBufferAccessV1::ReadWrite => "read_write",
        _ => "unsupported",
    }
}
fn runtime_access_matches(value: Gfx942RuntimeBufferAccessV1, expected: AccessMode) -> bool {
    matches!(
        (value, expected),
        (Gfx942RuntimeBufferAccessV1::ReadOnly, AccessMode::ReadOnly)
            | (
                Gfx942RuntimeBufferAccessV1::WriteOnly,
                AccessMode::WriteOnly
            )
            | (
                Gfx942RuntimeBufferAccessV1::ReadWrite,
                AccessMode::ReadWrite
            )
    )
}
fn schedule_name(value: SimulationScheduleIdentityV1) -> &'static str {
    match value {
        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxSerialV1 => {
            "workgroup_major_local_zyx_serial_v1"
        }
        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1 => {
            "workgroup_major_local_zyx_cooperative_v1"
        }
        SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1 => {
            "workgroup_major_seeded_runnable_cooperative_v1"
        }
    }
}

struct SnapshotWriter {
    bytes: Vec<u8>,
    maximum: usize,
}
impl SnapshotWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }
    fn bytes(&mut self, value: &[u8]) -> Result<(), PhysicalDifferentialErrorV1> {
        let end = self
            .bytes
            .len()
            .checked_add(8)
            .and_then(|v| v.checked_add(value.len()))
            .ok_or(PhysicalDifferentialErrorV1::ResourceLimit)?;
        if end > self.maximum {
            return Err(PhysicalDifferentialErrorV1::SnapshotTooLarge);
        }
        self.bytes
            .try_reserve_exact(end - self.bytes.len())
            .map_err(|_| PhysicalDifferentialErrorV1::AllocationFailure)?;
        self.bytes.extend_from_slice(
            &u64::try_from(value.len())
                .map_err(|_| PhysicalDifferentialErrorV1::ResourceLimit)?
                .to_le_bytes(),
        );
        self.bytes.extend_from_slice(value);
        Ok(())
    }
    fn string(&mut self, value: &str) -> Result<(), PhysicalDifferentialErrorV1> {
        self.bytes(value.as_bytes())
    }
    fn u8(&mut self, value: u8) -> Result<(), PhysicalDifferentialErrorV1> {
        self.bytes(&[value])
    }
    fn u32(&mut self, value: u32) -> Result<(), PhysicalDifferentialErrorV1> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), PhysicalDifferentialErrorV1> {
        self.bytes(&value.to_le_bytes())
    }
    fn usize(&mut self, value: usize) -> Result<(), PhysicalDifferentialErrorV1> {
        self.u64(u64::try_from(value).map_err(|_| PhysicalDifferentialErrorV1::ResourceLimit)?)
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    maximum: usize,
}
impl BoundedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
        }
    }
}
impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let end = self
            .bytes
            .len()
            .checked_add(buffer.len())
            .ok_or_else(|| std::io::Error::other("report size overflow"))?;
        if end > self.maximum {
            return Err(std::io::Error::other("report size limit"));
        }
        self.bytes
            .try_reserve_exact(buffer.len())
            .map_err(|_| std::io::Error::other("report allocation"))?;
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum PhysicalDifferentialErrorV1 {
    InvalidLimit(&'static str),
    Bundle(String),
    CompilerExecutionAssociationUnavailable,
    CompilerExecutionAssociationInvalid,
    CompilerExecutionSubstitution,
    ProductionKirSubstitution,
    StructuralBridgeSubstitution,
    TargetSubstitution,
    KernelSubstitution,
    GeometrySubstitution,
    GeneratedPackingSubstitution,
    GeneratedInputSubstitution,
    SimulatorKirSubstitution,
    PhysicalObservationSubstitution,
    CompletedBufferSubstitution,
    UnsupportedAliasedBufferView,
    UnsupportedDynamicGroupSegment,
    NoWritableOutput,
    SnapshotTooLarge,
    OutputTooLarge,
    ReportTooLarge,
    ResourceLimit,
    AllocationFailure,
    ReportEncoding,
    Simulation(String),
}
impl fmt::Display for PhysicalDifferentialErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "physical differential qualification failed: {self:?}")
    }
}
impl Error for PhysicalDifferentialErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_never_counts_as_hardware_or_parity() {
        let prepared = PreparedPhysicalDifferentialV1::test_fixture();
        let report = prepared
            .hardware_unavailable(PhysicalDifferentialUnavailableV1::NoKfdDevice)
            .unwrap();
        assert!(!report.hardware_observed);
        assert_eq!(report.hardware_passes, 0);
        assert_eq!(report.parity_passes, 0);
        assert!(report.compared_buffers.is_empty());
    }

    #[test]
    fn hostile_limits_fail_closed() {
        assert!(matches!(
            PhysicalDifferentialLimitsV1 {
                max_snapshot_bytes: 0,
                ..PhysicalDifferentialLimitsV1::default()
            }
            .validate(),
            Err(PhysicalDifferentialErrorV1::InvalidLimit(
                "max_snapshot_bytes"
            ))
        ));
        let mut writer = SnapshotWriter::new(8);
        assert!(matches!(
            writer.bytes(&[0]),
            Err(PhysicalDifferentialErrorV1::SnapshotTooLarge)
        ));
    }

    #[test]
    fn report_identity_and_unavailable_disposition_are_deterministic() {
        let first = PreparedPhysicalDifferentialV1::test_fixture()
            .hardware_unavailable(PhysicalDifferentialUnavailableV1::ProtectedVerifierUnavailable)
            .unwrap();
        let second = PreparedPhysicalDifferentialV1::test_fixture()
            .hardware_unavailable(PhysicalDifferentialUnavailableV1::ProtectedVerifierUnavailable)
            .unwrap();
        assert_eq!(first.report_sha256, second.report_sha256);
        assert_eq!(
            first.to_canonical_json_bytes().unwrap(),
            second.to_canonical_json_bytes().unwrap()
        );
    }

    #[test]
    fn discrepancy_counts_hardware_completion_but_not_parity() {
        let report = PreparedPhysicalDifferentialV1::test_fixture()
            .report(
                PhysicalDifferentialDispositionV1::Discrepancy,
                true,
                1,
                0,
                Vec::new(),
                1,
                vec![PhysicalDifferentialByteMismatchV1 {
                    argument_index: 0,
                    buffer_index: 0,
                    byte_offset: 0,
                    simulator: 1,
                    hardware: 2,
                }],
            )
            .unwrap();
        assert!(report.hardware_observed);
        assert_eq!(report.hardware_passes, 1);
        assert_eq!(report.parity_passes, 0);
    }

    #[test]
    fn report_schema_contains_no_native_address_or_kernarg_payload() {
        let report = PreparedPhysicalDifferentialV1::test_fixture()
            .hardware_unavailable(PhysicalDifferentialUnavailableV1::NoKfdDevice)
            .unwrap();
        let json = String::from_utf8(report.to_canonical_json_bytes().unwrap()).unwrap();
        for forbidden in [
            "native_address",
            "host_address",
            "device_address",
            "pointer_value",
            "kernarg_bytes",
            "descriptor_path",
            "/dev/kfd",
        ] {
            assert!(!json.contains(forbidden), "leaked field {forbidden}");
        }
    }

    #[test]
    fn configured_report_limit_fails_closed() {
        let mut prepared = PreparedPhysicalDifferentialV1::test_fixture();
        prepared.max_report_bytes = 64;
        assert!(matches!(
            prepared.hardware_unavailable(PhysicalDifferentialUnavailableV1::NoKfdDevice),
            Err(PhysicalDifferentialErrorV1::ReportTooLarge)
        ));
    }

    #[test]
    fn agent_capabilities_report_exact_zero_pass_blocker() {
        let capabilities = physical_differential_capabilities_v1();
        assert_eq!(capabilities.hardware_passes, 0);
        assert_eq!(capabilities.parity_passes, 0);
        assert_eq!(
            capabilities.current_production_blocker,
            PhysicalDifferentialUnavailableV1::ProtectedVerifierUnavailable
        );
        assert!(capabilities.parity_requires_sealed_completed_observation);
        assert!(capabilities.legacy_llvm_fixture_excluded);
    }

    #[test]
    fn hostile_geometry_observation_and_completed_shape_substitution_fail_closed() {
        assert!(validate_simulation_target(SimulationTargetV1::amdgpu_64()).is_ok());
        assert!(matches!(
            validate_simulation_target(SimulationTargetV1::little_endian(IndexWidthV1::Bits32)),
            Err(PhysicalDifferentialErrorV1::TargetSubstitution)
        ));
        assert!(validate_geometry([2, 3, 4], [64, 1, 1], [2, 3, 4], [64, 1, 1]).is_ok());
        for (grid, workgroup) in [
            ([3, 3, 4], [64, 1, 1]),
            ([2, 3, 4], [32, 1, 1]),
            ([u64::from(u32::MAX) + 1, 3, 4], [64, 1, 1]),
        ] {
            assert!(matches!(
                validate_geometry(grid, workgroup, [2, 3, 4], [64, 1, 1]),
                Err(PhysicalDifferentialErrorV1::GeometrySubstitution)
            ));
        }

        assert!(validate_observation_identity(&[1; 32], &[1; 32]).is_ok());
        assert!(matches!(
            validate_observation_identity(&[1; 32], &[2; 32]),
            Err(PhysicalDifferentialErrorV1::PhysicalObservationSubstitution)
        ));

        let expected = PhysicalBufferShapeV1 {
            buffer_index: 1,
            access: Gfx942RuntimeBufferAccessV1::ReadWrite,
            bytes: 128,
        };
        assert!(
            validate_completed_buffer_shape(
                &expected,
                1,
                Gfx942RuntimeBufferAccessV1::ReadWrite,
                128,
            )
            .is_ok()
        );
        for (index, access, bytes) in [
            (0, Gfx942RuntimeBufferAccessV1::ReadWrite, 128),
            (1, Gfx942RuntimeBufferAccessV1::ReadOnly, 128),
            (1, Gfx942RuntimeBufferAccessV1::ReadWrite, 127),
        ] {
            assert!(matches!(
                validate_completed_buffer_shape(&expected, index, access, bytes),
                Err(PhysicalDifferentialErrorV1::CompletedBufferSubstitution)
            ));
        }
    }

    impl PreparedPhysicalDifferentialV1 {
        fn test_fixture() -> Self {
            Self {
                binding_identity: [1; 32],
                request_snapshot: vec![2, 3],
                simulator_outputs: vec![],
                physical_buffer_shape: vec![],
                max_report_bytes: 64 * 1024,
                axes: ReportAxesV1 {
                    differential_binding: [24; 32],
                    kernel_id: [25; 32],
                    kernel_logical_name: "test_kernel".into(),
                    kernel_export_name: "test_kernel_v1".into(),
                    kernel_binding: [26; 32],
                    generated_host_contract: [27; 32],
                    direct_kfd_runtime_contract:
                        fe2o3_host::GENERATED_WORKER_V3_DIRECT_KFD_RUNTIME_CONTRACT_V1.into(),
                    worker_challenge: [28; 32],
                    simulator_kir_version: 7,
                    simulator_kir_sha256: [4; 32],
                    simulator_kir_bytes: 1,
                    production_kir_sha256: [5; 32],
                    production_kir_bytes: 1,
                    bundle_v1: [6; 32],
                    bundle_v2: [7; 32],
                    bundle_v3: [8; 32],
                    bundle_v4: [9; 32],
                    source_map_v2: [10; 32],
                    structural_bridge: [11; 32],
                    target: "gfx942:xnack-".into(),
                    compiler_subject: [12; 32],
                    compiler_receipt: [13; 32],
                    worker_lineage: [14; 32],
                    finalizer: [15; 32],
                    hsaco: [16; 32],
                    hsaco_bytes: 1,
                    dispatch: [17; 32],
                    dispatch_grid: [64, 1, 1],
                    dispatch_workgroup: [64, 1, 1],
                    dynamic_group_segment_bytes: 0,
                    packing: [18; 32],
                    device_unique_id: 19,
                    device_topology: [20; 32],
                    schedule: SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
                    schedule_transcript: [21; 32],
                    schedule_record: Some([22; 32]),
                    limits: [23; 32],
                    reduction: None,
                },
            }
        }
    }
}
