//! Private actual-source headless qualification, not a transcript import API.
//! Every capture retains the same frontend owner and immutable request scope.
//! Logical SSA observations never represent physical VGPR/EXEC contents.

use super::*;
use fe2o3_kernel_ir::{
    AssemblySourceIdentity, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrWorkBudgetV1, ValueId, WaveWidth,
};
use fe2o3_kir_debugger::{
    DebugInspectionUnavailableV1, DebugInspectionV1, DebugKirIdentityV1, DebugNavigationV1,
    DebugSessionV1, DebugTranscriptCompletenessV1, DebugTranscriptTruncationV1, DebugWaveWidthV1,
    DebuggerLimitsV1, capture_debugger_run_v1,
};
use fe2o3_kir_sim::{
    IndexWidthV1, SimulationDebugCaptureLimitsV1, SimulationDebugCheckpointPhaseV1,
    SimulationDebugRecordKindV1, SimulationDebugSiteV1, SimulationDebugUnavailableReasonV1,
    SimulationExecutionV1, SimulationScheduleIdentityV1,
};
use fe2o3_lower_mir_kernel::{
    ProductionOrderedRegionInspectionAvailabilityV1, ProductionOrderedRegionInspectionLimitsV1,
    ProductionOrderedRegionInspectionV1,
};

type SourceOwner =
    crate::production_pipeline::ordered_region_qualification_v31::OrderedRegionObservationOwnerV31;
const LANES: usize = 64;
const MAX_RECORDS: usize = 16_384;
const MAX_REPORT: usize = 64 * 1024;
const BACKING: BufferBackingIdV1 = BufferBackingIdV1(1);
const CASES: [[u32; 3]; 6] = [
    [0, 0, 0],
    [u32::MAX, 0, 1],
    [u32::MAX, 1, 2],
    [0x8000_0000, 0, 0x8000_0000],
    [0xaaaa_5555, 0x5555_aaaa, 19],
    [19, 23, 42],
];

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_canonical_bytes: 1024 * 1024,
        max_reachable_functions: 1,
        max_reachable_operations: 256,
        max_invocations: 64,
        max_workgroups: 1,
        max_scheduled_slots: 64,
        max_steps: 8192,
        max_call_depth: 1,
        max_ssa_values: 128,
        max_allocations: 4,
        max_allocation_bytes: 1024,
        max_total_bytes: 4096,
        max_resident_bytes: 64 * 1024 * 1024,
        max_events: 8192,
        max_memory_access_records: 1024,
    }
}

fn capture_limits(values: usize) -> SimulationDebugCaptureLimitsV1 {
    SimulationDebugCaptureLimitsV1::new(1, values, 4, 1024).unwrap()
}

fn debugger_limits(records: usize) -> DebuggerLimitsV1 {
    DebuggerLimitsV1::new(records, 1024 * 1024, 16 * 1024 * 1024).unwrap()
}

fn request(owner: &SourceOwner, values: [u32; 3]) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0xa5; (LANES + 2) * 4],
        vec![false; (LANES + 2) * 4],
        target,
    )
    .unwrap();
    let view = BufferViewArgumentV1::new(
        BACKING,
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        4,
        LANES,
        target,
    )
    .unwrap();
    let mut arguments = Vec::with_capacity(4);
    arguments.push(SimulationArgumentV1::BufferView(view));
    arguments.extend(values.map(|value| {
        SimulationArgumentV1::Scalar(
            ScalarBitsV1::new(ScalarType::U32, u128::from(value), target).unwrap(),
        )
    }));
    SimulationRequestV1::new(
        owner.materialized().executable().module().kernels[0]
            .id
            .clone(),
        [64, 1, 1],
        [64, 1, 1],
        arguments,
    )
    .with_shared_buffers(vec![SharedBufferV1 {
        id: BACKING,
        buffer,
    }])
}

fn site(view: &ProductionOrderedRegionInspectionV1<'_>) -> SimulationDebugSiteV1 {
    SimulationDebugSiteV1 {
        function_ordinal: usize::try_from(view.coordinate().block.function.0).unwrap(),
        block: view.kernel_ir_block(),
        operation: view.coordinate().operation,
    }
}

fn identity(view: &ProductionOrderedRegionInspectionV1<'_>) -> DebugKirIdentityV1 {
    DebugKirIdentityV1 {
        digest: *view.canonical_identity().digest(),
        canonical_len: view.canonical_identity().canonical_length(),
    }
}

// This metadata equality is only one necessary condition. The actual scope gate
// below additionally requires identical live source-owner and request borrows.
fn metadata_matches(
    actual: (
        DebugKirIdentityV1,
        SimulationDebugSiteV1,
        AssemblySourceIdentity,
    ),
    expected: (
        DebugKirIdentityV1,
        SimulationDebugSiteV1,
        AssemblySourceIdentity,
    ),
) -> bool {
    actual == expected
}

struct ScopedCapture<'a> {
    owner: &'a SourceOwner,
    request: &'a SimulationRequestV1,
    target: SimulationTargetV1,
    wave: DebugWaveWidthV1,
    site: SimulationDebugSiteV1,
    source: AssemblySourceIdentity,
    session: DebugSessionV1,
    execution: SimulationExecutionV1,
}

impl<'a> ScopedCapture<'a> {
    fn capture(
        owner: &'a SourceOwner,
        view: &ProductionOrderedRegionInspectionV1<'_>,
        admitted: &AdmittedSimulationModuleV1,
        request: &'a SimulationRequestV1,
        values: usize,
        records: usize,
    ) -> Self {
        let expected = identity(view);
        assert_eq!(
            view.canonical_identity(),
            owner.materialized().executable().identity()
        );
        assert_eq!(
            view.semantic_sha256(),
            owner
                .materialized()
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes()
        );
        assert_eq!(admitted.identity().wire_version(), 16);
        assert_eq!(admitted.identity().digest(), &expected.digest);
        assert_eq!(
            admitted.identity().canonical_length(),
            expected.canonical_len
        );
        let target = SimulationTargetV1::amdgpu_64();
        let wave = DebugWaveWidthV1::Wave64;
        let original = request.clone();
        let run = capture_debugger_run_v1(
            admitted,
            request,
            target,
            limits(),
            capture_limits(values),
            debugger_limits(records),
            wave,
        );
        assert_eq!(
            request, &original,
            "capture cannot mutate the retained request"
        );
        assert_eq!(run.transcript.identity(), expected);
        assert_eq!(run.transcript.wave_width(), wave);
        assert!(run.transcript.terminal_fault().is_none());
        let execution = run.execution.unwrap();
        assert_eq!(execution.identity(), admitted.identity());
        assert!(!execution.grants_execution_authority());
        assert_eq!(execution.invocations_executed(), 64);
        assert_eq!(execution.schedule_coverage().workgroups(), 1);
        assert_eq!(execution.schedule_coverage().decisions(), 64);
        assert_eq!(execution.schedule_coverage().barrier_releases(), 0);
        assert_eq!(
            execution.schedule(),
            SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1
        );
        Self {
            owner,
            request,
            target,
            wave,
            site: site(view),
            source: view.region().source(),
            session: DebugSessionV1::new(run.transcript),
            execution,
        }
    }

    fn matches_scope(
        &self,
        owner: &SourceOwner,
        request: &SimulationRequestV1,
        target: SimulationTargetV1,
        wave: DebugWaveWidthV1,
        view: &ProductionOrderedRegionInspectionV1<'_>,
    ) -> bool {
        std::ptr::eq(self.owner, owner)
            && std::ptr::eq(self.request, request)
            && self.target == target
            && self.wave == wave
            && metadata_matches(
                (self.session.transcript().identity(), self.site, self.source),
                (identity(view), site(view), view.region().source()),
            )
    }

    fn selected_records(&self) -> [[usize; 2]; LANES] {
        assert_eq!(
            self.session.transcript().completeness(),
            DebugTranscriptCompletenessV1::Complete
        );
        assert!(self.session.transcript().records().len() <= MAX_RECORDS);
        let mut selected = [[usize::MAX; 2]; LANES];
        for (index, record) in self.session.transcript().records().iter().enumerate() {
            if record.site != self.site {
                continue;
            }
            assert_eq!(record.schedule.identity, self.execution.schedule());
            let invocation = record.invocation;
            let lane = usize::try_from(invocation.local[0]).unwrap();
            assert!(lane < LANES);
            assert_eq!(invocation.local, [lane as u32, 0, 0]);
            assert_eq!(invocation.global, [lane as u64, 0, 0]);
            assert_eq!(invocation.workgroup, [0, 0, 0]);
            assert_eq!(invocation.workgroup_size, self.request.workgroup.0);
            assert_eq!(invocation.launch_extent, self.request.grid.0);
            assert_eq!(invocation.workgroup_count, [1, 1, 1]);
            assert_eq!(record.schedule.decision_ordinal, lane as u64);
            let SimulationDebugRecordKindV1::Checkpoint { phase, .. } = record.kind else {
                panic!("NoMemory ordered operation cannot have a memory/fence/barrier record");
            };
            let slot = match phase {
                SimulationDebugCheckpointPhaseV1::BeforeOperation => 0,
                SimulationDebugCheckpointPhaseV1::AfterOperation => 1,
            };
            assert_eq!(selected[lane][slot], usize::MAX, "duplicate atomic phase");
            selected[lane][slot] = index;
        }
        for [before, after] in selected {
            assert_ne!(before, usize::MAX);
            assert_eq!(
                before.checked_add(1),
                Some(after),
                "one atomic CPU operation, no microsteps"
            );
        }
        selected
    }

    fn seek(&mut self, index: usize) {
        assert!(
            matches!(self.session.seek_record_index(index), DebugNavigationV1::Stopped(stop) if stop.record_index == index)
        );
        assert_eq!(self.session.current().unwrap().site, self.site);
    }
}

fn logical_u32(session: &DebugSessionV1, id: ValueId) -> u32 {
    let DebugInspectionV1::Available(frames) = session.stack() else {
        panic!("required logical stack is unavailable");
    };
    let [frame] = frames else {
        panic!("the source profile must retain one direct-root frame");
    };
    let site = session.current().unwrap().site;
    assert_eq!(frame.depth, 0);
    assert_eq!(frame.function_ordinal, site.function_ordinal);
    assert_eq!(frame.block, site.block);
    let DebugInspectionV1::Available(value) = session.scalar(0, id) else {
        panic!("required logical SSA value is unavailable");
    };
    assert_eq!(value.ty(), ScalarType::U32);
    u32::try_from(value.bits()).unwrap()
}

fn assert_output(execution: &SimulationExecutionV1, expected: u32) {
    let output = execution.shared_buffer(BACKING).unwrap();
    for lane in 0..LANES {
        assert_eq!(
            &output.bytes()[4 + lane * 4..8 + lane * 4],
            &expected.to_le_bytes()
        );
    }
    assert!(output.initialized()[4..65 * 4].iter().all(|value| *value));
    for range in [0..4, 65 * 4..66 * 4] {
        assert_eq!(&output.bytes()[range.clone()], &[0xa5; 4]);
        assert_eq!(&output.initialized()[range], &[false; 4]);
    }
}

pub(super) fn observe(owner: &SourceOwner, feature: &str, output: &Path) {
    let materialized = owner.materialized();
    let executable = materialized.executable();
    let canonical_sha256 = Sha256::digest(executable.canonical_bytes());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2 * 1024 * 1024);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 8 * 1024 * 1024);
    budget
        .reserve_storage(materialized.executable_storage().retained_storage())
        .unwrap();
    budget
        .reserve_storage(materialized.call_correspondence_storage())
        .unwrap();
    let (view, storage) = materialized
        .inspect_ordered_region_v1(
            executable.identity(),
            None,
            ProductionOrderedRegionInspectionLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(view.declared_target(), "gfx942:xnack-");
    assert_eq!(view.declared_wave_width(), WaveWidth::Wave64);
    assert_eq!(
        view.source_association(),
        ProductionOrderedRegionInspectionAvailabilityV1::RetainedSemanticCorrespondence
    );
    for unavailable in [
        view.physical_values(),
        view.final_artifact(),
        view.source_insertion(),
    ] {
        assert_eq!(
            unavailable,
            ProductionOrderedRegionInspectionAvailabilityV1::Unavailable
        );
    }
    assert!(!view.authenticates_source());
    assert!(!view.grants_artifact_or_launch_authority());
    assert!(!view.grants_proof_or_resume_authority());
    assert!(!view.can_materialize_helper());
    let admitted = AdmittedSimulationModuleV1::admit_v16(executable, limits()).unwrap();
    let mut cases = Vec::with_capacity(CASES.len());
    for arguments in CASES {
        let request = request(owner, arguments);
        let mut capture =
            ScopedCapture::capture(owner, &view, &admitted, &request, 128, MAX_RECORDS);
        assert!(capture.matches_scope(
            owner,
            &request,
            SimulationTargetV1::amdgpu_64(),
            DebugWaveWidthV1::Wave64,
            &view
        ));
        let second_request = request.clone();
        assert!(!capture.matches_scope(
            owner,
            &second_request,
            SimulationTargetV1::amdgpu_64(),
            DebugWaveWidthV1::Wave64,
            &view
        ));
        assert!(!capture.matches_scope(
            owner,
            &request,
            SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
            DebugWaveWidthV1::Wave64,
            &view
        ));
        assert!(!capture.matches_scope(
            owner,
            &request,
            SimulationTargetV1::amdgpu_64(),
            DebugWaveWidthV1::Wave32,
            &view
        ));
        let indices = capture.selected_records();
        let [a, b, c] = arguments;
        // Independent host arithmetic, never region.profile().evaluate_bits().
        let expected = (a ^ b).wrapping_add(c);
        let mut results = [0_u32; LANES];
        for (lane, [before, after]) in indices.iter().copied().enumerate() {
            capture.seek(before);
            for (id, expected) in view.region().inputs().iter().zip(arguments) {
                assert_eq!(logical_u32(&capture.session, *id), expected);
            }
            assert_eq!(
                capture.session.scalar(0, view.region().result()),
                DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::UnknownValue)
            );
            capture.seek(after);
            results[lane] = logical_u32(&capture.session, view.region().result());
            assert_eq!(results[lane], expected);
            for (id, expected) in view.region().inputs().iter().zip(arguments) {
                assert_eq!(logical_u32(&capture.session, *id), expected);
            }
            assert_eq!(
                capture.session.current_hierarchy().unwrap().lane,
                lane as u16
            );
        }
        assert_output(
            &capture.execution,
            if feature == FEATURES[1] { a } else { expected },
        );
        cases.push(json!({
            "arguments_u32": arguments, "region_inputs_before_u32_each_lane": arguments,
            "region_results_after_u32": results.as_slice(),
            "before_after_record_indices": indices.as_slice(),
            "records": capture.session.transcript().records().len(),
            "transcript_completeness": "complete", "logical_lanes": LANES,
            "schedule": "workgroup-major-local-zyx-cooperative-v1",
            "schedule_transcript_identity": super::super::lower_hex_v1(capture.execution.schedule_transcript_identity()),
            "independent_oracle_matches": true, "canaries_unchanged": true,
        }));
    }

    // An actual second request shares KIR identity but changes observed data.
    // No detached-transcript API exists in this private qualification helper.
    let first_request = request(owner, [0, 0, 0]);
    let second_request = request(owner, [0, 0, 1]);
    let mut first =
        ScopedCapture::capture(owner, &view, &admitted, &first_request, 128, MAX_RECORDS);
    let mut second =
        ScopedCapture::capture(owner, &view, &admitted, &second_request, 128, MAX_RECORDS);
    assert_eq!(
        first.session.transcript().identity(),
        second.session.transcript().identity()
    );
    assert!(!first.matches_scope(owner, &second_request, second.target, second.wave, &view));
    assert!(!second.matches_scope(owner, &first_request, first.target, first.wave, &view));
    let first_indices = first.selected_records();
    first.seek(first_indices[0][1]);
    assert_eq!(logical_u32(&first.session, view.region().result()), 0);
    let second_indices = second.selected_records();
    second.seek(second_indices[0][1]);
    assert_eq!(logical_u32(&second.session, view.region().result()), 1);
    drop(first);
    drop(second);

    let mut truncated = ScopedCapture::capture(owner, &view, &admitted, &first_request, 128, 1);
    assert_eq!(
        truncated.session.transcript().completeness(),
        DebugTranscriptCompletenessV1::Truncated(DebugTranscriptTruncationV1::RecordLimit)
    );
    assert_eq!(
        truncated.session.seek_record_index(1),
        DebugNavigationV1::TranscriptTruncated(DebugTranscriptTruncationV1::RecordLimit)
    );
    drop(truncated);
    let mut unavailable =
        ScopedCapture::capture(owner, &view, &admitted, &first_request, 1, MAX_RECORDS);
    let unavailable_indices = unavailable.selected_records();
    unavailable.seek(unavailable_indices[0][0]);
    assert_eq!(
        unavailable.session.scalar(0, view.region().inputs()[0]),
        DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::Stack(
            SimulationDebugUnavailableReasonV1::ValueLimit
        ))
    );
    drop(unavailable);

    let coordinate = view.coordinate();
    let source = view.region().source();
    let registers = view.region().registers();
    let (inventory, plan) = owner.authenticated_source_identities();
    let mut report = json!({
        "schema": "fe2o3-test-source-ordered-region-debugger-observation-v1",
        "feature": feature, "scope": "same-private-source-owner-and-immutable-request CPU observation",
        "source_authentication_from_sidecar": false, "detached_transcript_admission": false,
        "grants_artifact_or_launch_authority": false, "grants_proof_or_resume_authority": false,
        "hardware_observed": false, "physical_register_values": "unavailable",
        "scratch_values": "unavailable", "exec_values": "unavailable", "instruction_microsteps": "unavailable",
        "semantic_sha256": super::super::lower_hex_v1(view.semantic_sha256()),
        "canonical_v16_identity": super::super::lower_hex_v1(view.canonical_identity().digest()),
        "canonical_v16_length": view.canonical_identity().canonical_length(),
        "canonical_bytes_sha256": super::super::lower_hex_v1(&canonical_sha256),
        "rustc_identity_inventory_sha256": super::super::lower_hex_v1(&inventory),
        "rustc_preflight_plan_sha256": super::super::lower_hex_v1(&plan),
        "region_source_ids": ([source.frontend_unit, source.function, source.contract, source.statement].map(|id| super::super::lower_hex_v1(&id))),
    });
    // Keep each macro invocation below the crate's existing recursion limit.
    let serde_json::Value::Object(observations) = json!({
        "kir_coordinate": [coordinate.block.function.0, coordinate.block.block, coordinate.operation],
        "kir_raw_block_id": view.kernel_ir_block().0,
        "semantic_coordinate": [view.semantic_function().index(), view.semantic_block().index()],
        "source_expansion_available": view.source_provenance().expansion().is_some(),
        "source_call_site_available": view.source_provenance().call_site().is_some(),
        "declared_target": view.declared_target(), "declared_wave_width": 64,
        "logical_input_value_ids": view.region().inputs().map(|id| id.0),
        "logical_result_value_id": view.region().result().0,
        "planned_physical_roles": {"scratch": registers.scratch(), "output": registers.output(), "inputs": registers.inputs()},
        "cpu_cases": cases, "qualified_lanes_per_case": LANES,
        "same_kir_second_request_not_interchangeable": true,
        "wrong_index_width_and_wave_not_interchangeable": true,
        "truncated_control": "record-limit", "unavailable_control": "value-limit",
        "capture_count": CASES.len() + 4,
        "limits": {"records_per_capture": MAX_RECORDS, "values_per_checkpoint": 128,
            "retained_values": 1024 * 1024, "retained_memory_bytes": 16 * 1024 * 1024,
            "simulation_steps": 8192, "logical_view_storage": storage.retained_storage()},
    }) else {
        unreachable!()
    };
    report.as_object_mut().unwrap().extend(observations);
    assert_eq!(
        Sha256::digest(executable.canonical_bytes()),
        canonical_sha256
    );
    let bytes = serde_json::to_vec(&report).unwrap();
    assert!(bytes.len() <= MAX_REPORT);
    // Separate additive output: never change the established six-case native join.
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("debugger-observation.json"))
        .unwrap();
    file.write_all(&bytes).unwrap();
}

#[test]
fn debugger_join_metadata_requires_digest_length_source_and_raw_site() {
    // Synthetic equality controls only, never a captured or authenticated run.
    let identity = DebugKirIdentityV1 {
        digest: [1; 32],
        canonical_len: 1374,
    };
    let site = SimulationDebugSiteV1 {
        function_ordinal: 0,
        block: fe2o3_kernel_ir::BlockId(7),
        operation: 3,
    };
    let source = AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]);
    let expected = (identity, site, source);
    assert!(metadata_matches(expected, expected));
    assert!(!metadata_matches(
        (
            DebugKirIdentityV1 {
                digest: [9; 32],
                ..identity
            },
            site,
            source
        ),
        expected
    ));
    assert!(!metadata_matches(
        (
            DebugKirIdentityV1 {
                canonical_len: 1373,
                ..identity
            },
            site,
            source
        ),
        expected
    ));
    for site in [
        SimulationDebugSiteV1 {
            function_ordinal: 1,
            ..site
        },
        SimulationDebugSiteV1 {
            block: fe2o3_kernel_ir::BlockId(0),
            ..site
        },
        SimulationDebugSiteV1 {
            operation: 4,
            ..site
        },
    ] {
        assert!(!metadata_matches((identity, site, source), expected));
    }
    assert!(!metadata_matches(
        (
            identity,
            site,
            AssemblySourceIdentity::new([9; 32], [2; 32], [3; 32], [4; 32])
        ),
        expected
    ));
}

#[test]
fn debugger_qualification_limits_are_small_explicit_and_valid() {
    limits().validate().unwrap();
    assert_eq!(capture_limits(128).max_values_per_checkpoint(), 128);
    assert_eq!(debugger_limits(MAX_RECORDS).max_records(), MAX_RECORDS);
    assert!(DebuggerLimitsV1::new(0, 1, 1).is_err());
    assert!(SimulationDebugCaptureLimitsV1::new(1, 0, 1, 1).is_err());
}
