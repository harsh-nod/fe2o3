//! Genuine compiler/Worker artifact with test-signed inert V2 carriage, never launch authority.
use super::*;
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, AuthenticatedPhysicalMachineEffectWorkerV1,
    PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
    check_gfx942_fill_analysis_v1, inspect_physical_machine_effect_worker_candidate_v1,
};
use fe2o3_verifier::{
    check_conditional_fill_program_v1, validate_conditional_compiler_proof_inputs_v1,
    validate_conditional_compiler_target_lineage_v1,
};
use object::{Object, ObjectSection};

const FILL_BINDING: [u8; 32] = [
    201, 142, 90, 172, 23, 165, 117, 211, 118, 242, 35, 196, 101, 72, 69, 111, 43, 149, 220, 157,
    53, 77, 204, 69, 78, 200, 101, 203, 153, 151, 146, 240,
];

struct AuditOnlyFillMarker;

// SAFETY: this fixture marker matches the captured descriptor, as checked below.
// It is used only with the safe audit API, not with a verifier or preparation API.
unsafe impl KernelMarkerV1 for AuditOnlyFillMarker {
    type Function = fn();
    type Registration = ();
    const LOGICAL_NAME: &'static str = "fill_write_only";
    const EXPORT_NAME: &'static str = "fill_write_only";
    const FUNCTION: Self::Function = worker_v3_marker_function;
    const REGISTRATION: &'static Self::Registration = &();
}

// SAFETY: this marker supplies test-only request identity, not generated ABI authority.
unsafe impl CompilerGeneratedKernelExpectationV1 for AuditOnlyFillMarker {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::new(TEST_HOST_CONTRACT);
    const KERNEL_BINDING_ID_V1: [u8; 32] = FILL_BINDING;
}

#[test]
fn audit_marker_matches_genuine_fill_descriptor() {
    let outer = fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3::decode(include_bytes!(
        "../../../fe2o3-verifier/src/conditional_fill_program_v1/fill.handoff"
    ))
    .unwrap();
    let source = fe2o3_compiler_ffi::CompilerDescriptorSourceV1::decode(
        outer.capsule().receipts().abi().canonical_preimage(),
    )
    .unwrap();
    let [descriptor] = source.table().kernels() else {
        panic!("one fill entry")
    };
    assert_eq!(descriptor.kernel_id().as_bytes(), &FILL_BINDING);
    assert_eq!(
        descriptor.logical_name().as_str(),
        AuditOnlyFillMarker::LOGICAL_NAME
    );
    assert_eq!(
        descriptor.entry_name().as_str(),
        AuditOnlyFillMarker::EXPORT_NAME
    );
}

struct FillAuditor {
    worker: AuthenticatedPhysicalMachineEffectWorkerV1,
    limits: AuthenticatedPhysicalMachineEffectLimitsV1,
    calls: usize,
    replace: Option<(PathBuf, PathBuf)>,
}

struct ForeignRequestAuditor<'a> {
    program: &'a fe2o3_verifier::CheckedConditionalFillProgramV1<'a>,
    machine: &'a fe2o3_kernel_analysis::CheckedGfx942FillAnalysisV1<'a>,
}

impl WorkerV3AuditorV1<WorkerV3VecAddMarker> for ForeignRequestAuditor<'_> {
    type Error = Infallible;
    type Evidence = ();

    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, WorkerV3VecAddMarker>,
    ) -> Result<(), Infallible> {
        assert!(matches!(
            request.check_conditional_fill_analysis_v1(self.program, self.machine),
            Err(
                fe2o3_host::WorkerV3ConditionalFillAssociationErrorV1::CompilerInputs(
                    "proof binding"
                )
            )
        ));
        Ok(())
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3AuditorV1<K> for FillAuditor {
    type Error = Infallible;
    type Evidence = ([u8; 32], u64);

    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<Self::Evidence, Self::Error> {
        self.calls += 1;
        assert!(request.validate_compiler_proof_inputs_v4().is_err());
        let receipts = request.semantic_compiler_handoff().capsule().receipts();
        let inputs = validate_conditional_compiler_proof_inputs_v1(
            receipts.proof_binding(),
            receipts.semantic_mir(),
            receipts.middle_end(),
            receipts.kernel_ir(),
            receipts.mir_to_kir_correspondence(),
            receipts.formal_memory(),
        )
        .unwrap();
        let lineage = validate_conditional_compiler_target_lineage_v1(
            request.semantic_compiler_handoff().capsule(),
            &inputs,
        )
        .unwrap();
        let program = check_conditional_fill_program_v1(&inputs, &lineage).unwrap();
        let entries = || {
            vec![
                PhysicalMachineEffectEntryRequestV1::new(
                    program.function_symbol(),
                    PhysicalMachineEffectBudgetV1::new(2, 1, 1, 1, 0),
                )
                .unwrap(),
            ]
        };
        let execution = self
            .worker
            .analyze(
                request.finalized_hsaco_bytes().to_vec(),
                entries(),
                self.limits,
            )
            .unwrap();
        let machine = check_gfx942_fill_analysis_v1(&execution, program.function_symbol()).unwrap();
        let joined = request
            .check_conditional_fill_analysis_v1(&program, &machine)
            .unwrap();
        assert!(std::ptr::eq(joined.request(), request));
        assert!(std::ptr::eq(joined.program(), &program));
        assert!(std::ptr::eq(joined.machine(), &machine));
        assert!(!joined.grants_launch_authority());
        let independent = request
            .independently_revalidate_finalizer_derivation()
            .unwrap();
        assert_eq!(
            independent.identity(),
            request.finalizer_derivation().identity()
        );
        assert_eq!(machine.kernel().kernarg_storage_bytes(), 272);
        let (_foreign_directory, recovered) = recovered_host_fixture();
        let foreign_request = admit_recovered_worker_v3_descriptor_v1(
            recovered,
            KernelId::from_bytes(TEST_MARKER_BINDING),
        )
        .unwrap();
        audit_recovered_worker_v3_verification_v1::<WorkerV3VecAddMarker, _>(
            &foreign_request,
            &mut ForeignRequestAuditor {
                program: &program,
                machine: &machine,
            },
        )
        .unwrap();

        // A non-executable comment change preserves the fill instructions, but not
        // the exact publication. Analyze it genuinely; never forge an execution owner.
        let mut changed = request.finalized_hsaco_bytes().to_vec();
        let range = object::File::parse(changed.as_slice())
            .unwrap()
            .section_by_name(".comment")
            .unwrap()
            .file_range()
            .unwrap();
        let byte = changed[range.0 as usize..(range.0 + range.1) as usize]
            .iter_mut()
            .find(|byte| **byte != 0)
            .unwrap();
        *byte ^= 1;
        let foreign = self
            .worker
            .analyze(changed, entries(), self.limits)
            .unwrap();
        let foreign = check_gfx942_fill_analysis_v1(&foreign, program.function_symbol()).unwrap();
        assert!(matches!(
            request.check_conditional_fill_analysis_v1(&program, &foreign),
            Err(
                fe2o3_host::WorkerV3ConditionalFillAssociationErrorV1::Machine("finalized payload")
            )
        ));
        assert!(
            request
                .check_conditional_fill_analysis_v1(&program, &machine)
                .is_ok()
        );
        if let Some(root) = std::env::var_os("FE2O3_FILL_HOST_CAPTURE") {
            let path = Path::new(&root).join(format!("audit-{}", self.calls));
            fs::create_dir(&path).unwrap();
            for (name, bytes) in [
                (
                    "handoff",
                    request.semantic_compiler_handoff().canonical_bytes(),
                ),
                ("finalized.hsaco", request.finalized_hsaco_bytes()),
                ("analysis.request", execution.request().canonical_bytes()),
                ("analysis.bundle", execution.analysis().canonical_bytes()),
                ("analysis.receipt", execution.canonical_receipt_bytes()),
            ] {
                use std::io::Write;
                let mut file = fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(path.join(name))
                    .unwrap();
                file.write_all(bytes).unwrap();
                file.sync_all().unwrap();
            }
        }
        eprintln!(
            "host fill association: lineage={:?} analyzer={:?} bytes={}",
            request.lineage_identity(),
            execution.identity(),
            request.finalized_hsaco_length()
        );
        if let Some((root, moved)) = &self.replace {
            fs::rename(root, moved).unwrap();
            fs::create_dir(root).unwrap();
        }
        Ok((
            request.finalized_hsaco_sha256(),
            request.finalized_hsaco_length(),
        ))
    }
}

#[test]
#[ignore = "requires actual native Worker and exact build identities; no GPU or protected compiler issuer"]
fn genuine_fill_reaches_current_publication_audit_without_authority() {
    audit_marker_matches_genuine_fill_descriptor();
    let fixture = worker_v3_fixture::published_genuine_conditional_fill_fixture();
    let private_worker = fixture.directory.0.join("real-worker");
    // This helper supplies test-signed inert carriage only. The safe auditor
    // neither authenticates that producer nor creates protected verifier evidence.
    let (directory, recovered) = recover_published_worker_v3_fixture(fixture);
    let admitted =
        admit_recovered_worker_v3_descriptor_v1(recovered, KernelId::from_bytes(FILL_BINDING))
            .unwrap();
    let lineage = admitted.lineage_identity();
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        std::time::Duration::from_secs(60),
        1024 * 1024,
        16384,
    )
    .unwrap();
    let candidate =
        inspect_physical_machine_effect_worker_candidate_v1(&private_worker, limits).unwrap();
    let worker = AuthenticatedPhysicalMachineEffectWorkerV1::open(
        &private_worker,
        candidate.policy(),
        limits,
    )
    .unwrap();
    let mut auditor = FillAuditor {
        worker,
        limits,
        calls: 0,
        replace: None,
    };
    let (_, length) = audit_recovered_worker_v3_verification_v1::<AuditOnlyFillMarker, _>(
        &admitted,
        &mut auditor,
    )
    .unwrap();
    assert!(length > 0);
    assert_eq!(auditor.calls, 1);
    assert_eq!(admitted.lineage_identity(), lineage);
    admitted.revalidate_currentness().unwrap();
    assert!(!admitted.authenticates_verification_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
    assert!(matches!(
        audit_recovered_worker_v3_verification_v1::<WorkerV3VecAddMarker, _>(
            &admitted,
            &mut auditor
        ),
        Err(fe2o3_host::WorkerV3VerificationAuditErrorV1::Marker(_))
    ));
    assert_eq!(auditor.calls, 1);

    let root = directory.0.clone();
    let moved = root.with_extension("stale-fill-audit");
    auditor.replace = Some((root.clone(), moved.clone()));
    let stale = audit_recovered_worker_v3_verification_v1::<AuditOnlyFillMarker, _>(
        &admitted,
        &mut auditor,
    );
    fs::remove_dir(&root).unwrap();
    fs::rename(&moved, &root).unwrap();
    assert!(matches!(
        stale,
        Err(fe2o3_host::WorkerV3VerificationAuditErrorV1::CurrentPublication(_))
    ));
    assert_eq!(auditor.calls, 2);
    admitted.revalidate_currentness().unwrap();
}
