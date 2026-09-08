use fe2o3_compiler_lineage::{
    ExactLlvmPassOccurrenceContentsV1, ExactProductionLlvmPhaseContentsV1,
    FIXED_PRODUCTION_LLVM_PHASES_V1, FixedProductionLlvmPhaseV1, LlvmPassInvocationV1,
    LlvmPassIrUnitV1, PostLlvmPipelineOccurrenceTranscriptV1, PostLlvmStageCustodyErrorV1,
    PostLlvmStageCustodyV1, check_exact_fixed_production_pipeline_contents_v1,
    check_exact_post_llvm_pipeline_occurrence_v1, check_exact_post_llvm_stage_contents_v1,
};

const LLVM: &[u8] = b"target triple = \"amdgcn-amd-amdhsa\"";
const PRE: &[u8] = b"BC\xc0\xde-pre";
const POST: &[u8] = b"BC\xc0\xde-post";
const OBJECT: &[u8] = b"object";
const HSACO: &[u8] = b"hsaco";
const WORKER: &[u8] = b"exact production worker executable";
const ASSEMBLER: &[u8] = b"exact llvm text assembler executable";
const ASSEMBLED: &[u8] = b"BC\xc0\xde-assembled-text-only";

fn production_phases() -> Vec<ExactProductionLlvmPhaseContentsV1> {
    let after_strip = b"bitcode after strip-debug";
    let after_exports = b"bitcode after llvm.used export preservation";
    let after_o2 = b"bitcode after default O2 pipeline";
    FIXED_PRODUCTION_LLVM_PHASES_V1
        .into_iter()
        .zip([
            (LLVM, PRE),
            (PRE, after_strip),
            (after_strip, after_exports),
            (after_exports, after_o2),
            (after_o2, POST),
            (POST, POST),
            (POST, OBJECT),
            (OBJECT, HSACO),
        ])
        .map(|(phase, (input, output))| {
            ExactProductionLlvmPhaseContentsV1::new(phase, input, output).unwrap()
        })
        .collect()
}

fn passes() -> Vec<LlvmPassInvocationV1> {
    vec![
        LlvmPassInvocationV1::new("forceattrs", Box::<[u8]>::from([])).unwrap(),
        LlvmPassInvocationV1::new("instcombine", b"max-iterations=1".to_vec()).unwrap(),
        LlvmPassInvocationV1::new("instcombine", b"max-iterations=2".to_vec()).unwrap(),
    ]
}

fn pass_occurrences() -> Vec<ExactLlvmPassOccurrenceContentsV1> {
    let after_exports = b"bitcode after llvm.used export preservation";
    let after_forceattrs = b"bitcode after forceattrs";
    let after_instcombine_one = b"bitcode after first instcombine";
    let after_o2 = b"bitcode after default O2 pipeline";
    [
        (
            0,
            LlvmPassIrUnitV1::Module,
            "forceattrs",
            b"".as_slice(),
            after_exports.as_slice(),
            after_forceattrs.as_slice(),
        ),
        (
            1,
            LlvmPassIrUnitV1::Function,
            "instcombine",
            b"max-iterations=1".as_slice(),
            after_forceattrs.as_slice(),
            after_instcombine_one.as_slice(),
        ),
        (
            2,
            LlvmPassIrUnitV1::Function,
            "instcombine",
            b"max-iterations=2".as_slice(),
            after_instcombine_one.as_slice(),
            after_o2.as_slice(),
        ),
    ]
    .into_iter()
    .map(|(ordinal, unit, name, options, input, output)| {
        ExactLlvmPassOccurrenceContentsV1::new(ordinal, 1, unit, name, options, input, output)
            .unwrap()
    })
    .collect()
}

fn record() -> PostLlvmStageCustodyV1 {
    PostLlvmStageCustodyV1::from_exact_stage_bytes(
        LLVM,
        PRE,
        POST,
        OBJECT,
        HSACO,
        "llvmorg-22.0.0-fe2o3",
        passes(),
    )
    .unwrap()
}

fn checked_fixed() -> fe2o3_compiler_lineage::CheckedPostLlvmStageContentsV1 {
    let phases = production_phases();
    let record = record()
        .with_fixed_production_pipeline(WORKER, "worker-build-v1", &phases)
        .unwrap();
    let checked =
        check_exact_post_llvm_stage_contents_v1(record, LLVM, PRE, POST, OBJECT, HSACO).unwrap();
    check_exact_fixed_production_pipeline_contents_v1(
        checked,
        WORKER,
        "worker-build-v1",
        "llvmorg-22.0.0-fe2o3",
        phases,
    )
    .unwrap()
}

fn transcript(
    phases: &[ExactProductionLlvmPhaseContentsV1],
    passes: &[ExactLlvmPassOccurrenceContentsV1],
) -> PostLlvmPipelineOccurrenceTranscriptV1 {
    PostLlvmPipelineOccurrenceTranscriptV1::capture_unavailable_production_v4(
        [1; 32],
        [2; 32],
        [3; 32],
        WORKER,
        ASSEMBLER,
        "worker-build-v1",
        "llvmorg-22.0.0-fe2o3",
        LLVM,
        ASSEMBLED,
        phases,
        passes,
    )
    .unwrap()
}

#[test]
fn exact_stage_bodies_and_recorded_ordered_passes_are_retained_without_authority() {
    let record = record();
    assert_eq!(record.passes()[1].name(), "instcombine");
    assert_eq!(record.passes()[2].canonical_options(), b"max-iterations=2");
    assert!(!record.proves_optimization_semantics());
    assert!(!record.proves_instruction_selection());
    assert!(!record.proves_object_to_code_object_preservation());
    assert!(!record.grants_authority());

    let checked =
        check_exact_post_llvm_stage_contents_v1(record, LLVM, PRE, POST, OBJECT, HSACO).unwrap();
    assert!(checked.retains_exact_stage_contents_and_recorded_passes());
    assert!(!checked.proves_semantic_refinement());
    assert!(!checked.grants_authority());
}

#[test]
fn every_exact_stage_axis_rejects_substitution() {
    assert_eq!(
        check_exact_post_llvm_stage_contents_v1(
            record(),
            b"different lowered LLVM".as_slice(),
            PRE,
            POST,
            OBJECT,
            HSACO,
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::LoweredLlvmMismatch
    );
    for (pre, post, object, hsaco, expected) in [
        (
            b"other-pre".as_slice(),
            POST,
            OBJECT,
            HSACO,
            PostLlvmStageCustodyErrorV1::PreOptimizationMismatch,
        ),
        (
            PRE,
            b"other-post".as_slice(),
            OBJECT,
            HSACO,
            PostLlvmStageCustodyErrorV1::PostOptimizationMismatch,
        ),
        (
            PRE,
            POST,
            b"other-object".as_slice(),
            HSACO,
            PostLlvmStageCustodyErrorV1::GeneratedObjectMismatch,
        ),
        (
            PRE,
            POST,
            OBJECT,
            b"other-hsaco".as_slice(),
            PostLlvmStageCustodyErrorV1::FinalCodeObjectMismatch,
        ),
    ] {
        assert_eq!(
            check_exact_post_llvm_stage_contents_v1(record(), LLVM, pre, post, object, hsaco)
                .unwrap_err(),
            expected
        );
    }
}

#[test]
fn pass_order_and_options_are_identity_axes() {
    let baseline = record().identity();
    let mut reordered = passes();
    reordered.swap(0, 1);
    let reordered = PostLlvmStageCustodyV1::from_exact_stage_bytes(
        LLVM,
        PRE,
        POST,
        OBJECT,
        HSACO,
        "llvmorg-22.0.0-fe2o3",
        reordered,
    )
    .unwrap();
    assert_ne!(baseline, reordered.identity());

    let mut changed = passes();
    changed[2] = LlvmPassInvocationV1::new("instcombine", b"max-iterations=3".to_vec()).unwrap();
    let changed = PostLlvmStageCustodyV1::from_exact_stage_bytes(
        LLVM,
        PRE,
        POST,
        OBJECT,
        HSACO,
        "llvmorg-22.0.0-fe2o3",
        changed,
    )
    .unwrap();
    assert_ne!(baseline, changed.identity());
}

#[test]
fn empty_pass_declaration_is_data_not_a_completeness_claim() {
    let record = PostLlvmStageCustodyV1::from_exact_stage_bytes(
        LLVM,
        PRE,
        POST,
        OBJECT,
        HSACO,
        "llvmorg-22.0.0-fe2o3",
        Vec::<LlvmPassInvocationV1>::new(),
    )
    .unwrap();
    assert!(record.passes().is_empty());
    assert!(!record.proves_optimization_semantics());
}

#[test]
fn fixed_outer_pipeline_and_tool_build_contents_are_exact_but_not_execution_authentication() {
    let phases = production_phases();
    let record = record()
        .with_fixed_production_pipeline(WORKER, "worker-build-v1", &phases)
        .unwrap();
    assert_eq!(record.worker_build_identity(), Some("worker-build-v1"));
    assert!(record.worker_executable().unwrap().matches(WORKER));
    assert!(record.retains_complete_fixed_production_phase_order());
    assert!(!record.authenticates_expanded_llvm_pass_pipeline_execution());
    assert!(!record.authenticates_worker_execution());

    let checked =
        check_exact_post_llvm_stage_contents_v1(record, LLVM, PRE, POST, OBJECT, HSACO).unwrap();
    let checked = check_exact_fixed_production_pipeline_contents_v1(
        checked,
        WORKER,
        "worker-build-v1",
        "llvmorg-22.0.0-fe2o3",
        phases,
    )
    .unwrap();
    assert!(checked.retains_exact_fixed_production_pipeline_contents());
    assert_eq!(checked.worker_executable(), Some(WORKER));
    assert_eq!(checked.production_phases().unwrap().len(), 8);
    assert!(!checked.authenticates_production_pipeline_execution());
    assert!(!checked.proves_semantic_refinement());
}

#[test]
fn fixed_pipeline_order_boundary_worker_and_checkpoint_substitutions_fail_closed() {
    let mut reordered = production_phases();
    reordered.swap(1, 2);
    assert_eq!(
        record()
            .with_fixed_production_pipeline(WORKER, "worker-build-v1", &reordered)
            .unwrap_err(),
        PostLlvmStageCustodyErrorV1::ProductionPhaseOrderMismatch
    );

    let mut disconnected = production_phases();
    disconnected[1] = ExactProductionLlvmPhaseContentsV1::new(
        FixedProductionLlvmPhaseV1::StripDebugInfo,
        b"substituted pre-opt input".as_slice(),
        b"bitcode after strip-debug".as_slice(),
    )
    .unwrap();
    assert_eq!(
        record()
            .with_fixed_production_pipeline(WORKER, "worker-build-v1", &disconnected)
            .unwrap_err(),
        PostLlvmStageCustodyErrorV1::ProductionPhaseBoundaryMismatch
    );

    let phases = production_phases();
    let attached = record()
        .with_fixed_production_pipeline(WORKER, "worker-build-v1", &phases)
        .unwrap();
    let checked =
        check_exact_post_llvm_stage_contents_v1(attached, LLVM, PRE, POST, OBJECT, HSACO).unwrap();
    assert_eq!(
        check_exact_fixed_production_pipeline_contents_v1(
            checked,
            b"substituted worker executable".as_slice(),
            "worker-build-v1",
            "llvmorg-22.0.0-fe2o3",
            phases.clone(),
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::WorkerExecutableMismatch
    );

    let attached = record()
        .with_fixed_production_pipeline(WORKER, "worker-build-v1", &phases)
        .unwrap();
    let checked =
        check_exact_post_llvm_stage_contents_v1(attached, LLVM, PRE, POST, OBJECT, HSACO).unwrap();
    assert_eq!(
        check_exact_fixed_production_pipeline_contents_v1(
            checked,
            WORKER,
            "substituted-worker-build",
            "llvmorg-22.0.0-fe2o3",
            phases.clone(),
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::WorkerBuildIdentityMismatch
    );

    let attached = record()
        .with_fixed_production_pipeline(WORKER, "worker-build-v1", &phases)
        .unwrap();
    let checked =
        check_exact_post_llvm_stage_contents_v1(attached, LLVM, PRE, POST, OBJECT, HSACO).unwrap();
    assert_eq!(
        check_exact_fixed_production_pipeline_contents_v1(
            checked,
            WORKER,
            "worker-build-v1",
            "substituted-llvm-build",
            phases.clone(),
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::LlvmBuildIdentityMismatch
    );

    let attached = record()
        .with_fixed_production_pipeline(WORKER, "worker-build-v1", &phases)
        .unwrap();
    let checked =
        check_exact_post_llvm_stage_contents_v1(attached, LLVM, PRE, POST, OBJECT, HSACO).unwrap();
    let mut substituted = production_phases();
    let replacement = b"substituted connected checkpoint";
    substituted[2] = ExactProductionLlvmPhaseContentsV1::new(
        FixedProductionLlvmPhaseV1::PreserveExpectedExports,
        b"bitcode after strip-debug".as_slice(),
        replacement.as_slice(),
    )
    .unwrap();
    substituted[3] = ExactProductionLlvmPhaseContentsV1::new(
        FixedProductionLlvmPhaseV1::DefaultPerModulePipelineO2,
        replacement.as_slice(),
        b"bitcode after default O2 pipeline".as_slice(),
    )
    .unwrap();
    assert_eq!(
        check_exact_fixed_production_pipeline_contents_v1(
            checked,
            WORKER,
            "worker-build-v1",
            "llvmorg-22.0.0-fe2o3",
            substituted,
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::ProductionPhaseContentMismatch
    );
}

#[test]
fn complete_occurrence_and_exact_assembly_replay_are_required_custody_not_semantics() {
    let phases = production_phases();
    let passes = pass_occurrences();
    let checked = check_exact_post_llvm_pipeline_occurrence_v1(
        checked_fixed(),
        transcript(&phases, &passes),
        WORKER,
        ASSEMBLER,
        LLVM,
        ASSEMBLED,
        LLVM,
        ASSEMBLED,
        phases,
        passes,
    )
    .unwrap();
    assert!(checked.retains_complete_occurrence_and_assembly_replay_custody());
    assert_eq!(checked.assembly_input(), Some(LLVM));
    assert_eq!(checked.assembly_output(), Some(ASSEMBLED));
    assert_eq!(checked.pass_occurrences().unwrap().len(), 3);
    assert!(!checked.authenticates_production_pipeline_execution());
    assert!(!checked.proves_semantic_refinement());
    assert_eq!(
        checked
            .require_authenticated_production_pipeline_execution()
            .unwrap_err(),
        PostLlvmStageCustodyErrorV1::ProducerTranscriptUnavailable
    );
}

#[test]
fn omission_reorder_and_substitution_in_occurrence_custody_fail_closed() {
    let phases = production_phases();
    let passes = pass_occurrences();

    let mut omitted_phase = transcript(&phases, &passes).into_parts();
    omitted_phase.phases = omitted_phase.phases[1..].to_vec().into_boxed_slice();
    assert_eq!(
        PostLlvmPipelineOccurrenceTranscriptV1::from_parts(omitted_phase).unwrap_err(),
        PostLlvmStageCustodyErrorV1::OccurrencePhaseMismatch
    );

    let mut omitted = passes.clone();
    omitted.remove(1);
    assert_eq!(
        PostLlvmPipelineOccurrenceTranscriptV1::capture_unavailable_production_v4(
            [1; 32],
            [2; 32],
            [3; 32],
            WORKER,
            ASSEMBLER,
            "worker-build-v1",
            "llvmorg-22.0.0-fe2o3",
            LLVM,
            ASSEMBLED,
            &phases,
            &omitted,
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::OccurrencePassOrderMismatch
    );

    let mut reordered = passes.clone();
    reordered.swap(0, 1);
    assert_eq!(
        PostLlvmPipelineOccurrenceTranscriptV1::capture_unavailable_production_v4(
            [1; 32],
            [2; 32],
            [3; 32],
            WORKER,
            ASSEMBLER,
            "worker-build-v1",
            "llvmorg-22.0.0-fe2o3",
            LLVM,
            ASSEMBLED,
            &phases,
            &reordered,
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::OccurrencePassOrderMismatch
    );

    assert_eq!(
        check_exact_post_llvm_pipeline_occurrence_v1(
            checked_fixed(),
            transcript(&phases, &passes),
            WORKER,
            ASSEMBLER,
            LLVM,
            ASSEMBLED,
            LLVM,
            b"substituted replay output",
            phases,
            passes,
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::AssemblyReplayMismatch
    );

    assert_eq!(
        check_exact_post_llvm_pipeline_occurrence_v1(
            checked_fixed(),
            transcript(&production_phases(), &pass_occurrences()),
            WORKER,
            b"substituted assembler".as_slice(),
            LLVM,
            ASSEMBLED,
            LLVM,
            ASSEMBLED,
            production_phases(),
            pass_occurrences(),
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::AssemblerExecutableMismatch
    );

    let mut substituted_passes = pass_occurrences();
    substituted_passes[0] = ExactLlvmPassOccurrenceContentsV1::new(
        0,
        1,
        LlvmPassIrUnitV1::Module,
        "substituted-pass",
        b"".as_slice(),
        b"bitcode after llvm.used export preservation".as_slice(),
        b"bitcode after forceattrs".as_slice(),
    )
    .unwrap();
    assert_eq!(
        check_exact_post_llvm_pipeline_occurrence_v1(
            checked_fixed(),
            transcript(&production_phases(), &substituted_passes),
            WORKER,
            ASSEMBLER,
            LLVM,
            ASSEMBLED,
            LLVM,
            ASSEMBLED,
            production_phases(),
            substituted_passes,
        )
        .unwrap_err(),
        PostLlvmStageCustodyErrorV1::OccurrencePassRosterMismatch
    );
}
