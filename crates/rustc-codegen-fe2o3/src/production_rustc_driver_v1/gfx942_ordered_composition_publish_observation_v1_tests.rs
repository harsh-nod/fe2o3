//! Live seed+inspection publisher, then independent fresh-frontend CPU observations.
use super::*;
use crate::production_ordered_composition_source_v1::{
    OrderedCompositionSourcePublishEffectV1 as Effect,
    OrderedCompositionSourcePublishRequestV1 as Request, OrderedCompositionTypedEditV1 as Edit,
    publish_ordered_composition_source_v1,
};
use fe2o3_kernel_ir::{AccessMode, ScalarType};
use fe2o3_kernel_ir::{
    Gfx942ProgramDestinationV1 as Dest, Gfx942ProgramInstructionV1 as Step,
    Gfx942ProgramRoleV1 as Role, Gfx942U32ProgramV1,
};
use fe2o3_kir_sim::*;
use std::os::unix::fs::MetadataExt;

const HELPER: &str = "__fe2o3_region_0123456789abcdef";
fn edited_program() -> Gfx942U32ProgramV1 {
    Gfx942U32ProgramV1::from_instructions(&[Step::Move {
        destination: Dest::Output,
        source: Role::Input2,
    }])
    .unwrap()
}
pub(super) fn cpu(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV17,
    edited: bool,
    started: std::time::Instant,
) -> Value {
    let limits = SimulationLimitsV1 {
        max_canonical_bytes: 256 * 1024,
        max_reachable_functions: 3,
        max_reachable_operations: 4096,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 128,
        max_steps: 500_000,
        max_call_depth: 2,
        max_ssa_values: 4096,
        max_allocations: 16,
        max_allocation_bytes: 65536,
        max_total_bytes: 65536,
        max_resident_bytes: 64 * 1024 * 1024,
        max_events: 1_000_000,
        max_memory_access_records: 8192,
    }
    .validate()
    .unwrap();
    let admitted = AdmittedSimulationModuleV1::admit_v17(owner, limits).unwrap();
    let machine = SimulationTargetV1::amdgpu_64();
    let mut cases = 0;
    let mut hash = Sha256::new();
    for [a, b, c] in [
        [0_u32, 0, 0],
        [0, u32::MAX, 1],
        [u32::MAX, 0, u32::MAX],
        [19, 23, 42],
    ] {
        for length in [0_usize, 13, 64, 129] {
            for grid in [64_u64, 128] {
                timely(started.elapsed(), 300).unwrap();
                let backing = BufferBackingIdV1(7);
                let buffer = BufferArgumentV1::new(
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    vec![0x5a; (length + 4) * 4],
                    vec![false; (length + 4) * 4],
                    machine,
                )
                .unwrap();
                let view = BufferViewArgumentV1::new(
                    backing,
                    ScalarType::U32,
                    AccessMode::ReadWrite,
                    4,
                    8,
                    length,
                    machine,
                )
                .unwrap();
                let request = SimulationRequestV1::new(
                    owner.module().kernels[0].id.clone(),
                    [grid, 1, 1],
                    [64, 1, 1],
                    vec![
                        SimulationArgumentV1::BufferView(view),
                        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(a)),
                        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(b)),
                        SimulationArgumentV1::Scalar(ScalarBitsV1::u32(c)),
                    ],
                )
                .with_shared_buffers(vec![SharedBufferV1 {
                    id: backing,
                    buffer,
                }]);
                let unchanged = request.clone();
                let execution = admitted.simulate(&request, machine, limits).unwrap();
                assert_eq!(request, unchanged);
                assert!(!execution.grants_execution_authority());
                assert_eq!(execution.identity().digest(), owner.identity().digest());
                assert_eq!(execution.identity().wire_version(), 17);
                let expected = if edited { c } else { (a ^ b) & c };
                let output = execution.shared_buffer(backing).unwrap();
                let written = length.min(grid as usize);
                for element in 0..length + 4 {
                    let range = element * 4..(element + 1) * 4;
                    if (2..2 + written).contains(&element) {
                        assert_eq!(&output.bytes()[range.clone()], &expected.to_le_bytes());
                        assert!(output.initialized()[range].iter().all(|v| *v));
                    } else {
                        assert_eq!(&output.bytes()[range.clone()], &[0x5a; 4]);
                        assert!(output.initialized()[range].iter().all(|v| !*v));
                    }
                }
                hash.update(output.bytes());
                for bit in output.initialized() {
                    hash.update([u8::from(*bit)]);
                }
                cases += 1;
                timely(started.elapsed(), 300).unwrap();
            }
        }
    }
    assert_eq!(cases, 32);
    json!({"cases":cases,"edited_intent":edited,"view_offset":8,"canaries_and_initialization":true,
        "output_sha256":super::super::super::lower_hex_v1(&hash.finalize()),
        "domain":"independent bounded CPU observer; original source ledger retained",
        "native_or_physical_execution":false})
}
fn refusal(case: &str) -> &'static str {
    match case {
        "collision" => "publisher helper name collides in actual HIR",
        "const" => "publisher refuses non-marker local const items",
        "local" => "publisher refuses local, constant or captured operands",
        "wrapper" => "publisher refuses wrapper macros or substituted expansions",
        _ => panic!("not a negative publisher case"),
    }
}
pub(super) fn observe(
    mut target:crate::production_pipeline::ordered_composition_v1::AuthenticatedOrderedCompositionDiagnosticV1<'_>,
    case: &str,
    root: &Path,
    started: std::time::Instant,
) -> Result<Value, String> {
    target
        .with_observation_budget(|owner, budget| owner.verify_equivalence(budget))
        .map_err(|e| e.to_string())?;
    let source = target.materialized();
    let executable = source.executable();
    let semantic = source.semantic_ssa().source_semantic();
    assert_eq!(
        target.source_seed().semantic_sha256(),
        semantic.semantic_sha256().as_bytes()
    );
    assert_eq!(source.composition().definitions().len(), 1);
    let canonical_identity = *executable.identity();
    let semantic_identity = *semantic.semantic_sha256().as_bytes();
    let definition = source.composition().definitions()[0].key();
    let canonical_before = digest(executable.canonical_bytes());
    let promoted = matches!(case, "recompile-copy" | "recompile-edit");
    if promoted {
        let edited = case == "recompile-edit";
        assert_eq!(source.composition().helpers().len(), 1);
        assert_eq!(source.composition().calls().len(), 1);
        assert_eq!(source.composition().occurrences().len(), 1);
        // Public borrowed inspection resolves the actual generated helper body,
        // not text search or an old source owner's canonical projection.
        target
            .with_observation_budget(|owner, budget| {
                fe2o3_lower_mir_kernel::with_ordered_composition_inspection_v1(
                    owner,
                    budget,
                    |view, budget| {
                        let region = view.definition(
                            definition,
                            &canonical_identity,
                            &semantic_identity,
                            budget,
                        )?;
                        assert_ne!(
                            region.canonical_site().function_ordinal(),
                            owner.composition().root_function_ordinal()
                        );
                        if edited {
                            assert_eq!(*region.program().program(), edited_program());
                        }
                        Ok(())
                    },
                )
            })
            .map_err(|e| e.to_string())?;
        let observed = cpu(target.materialized().executable(), edited, started);
        return Ok(
            json!({"stage":"fresh_promoted_source_frontend","cpu":observed,
            "semantic_sha256":super::super::super::lower_hex_v1(&semantic_identity),
            "canonical_identity":super::super::super::lower_hex_v1(canonical_identity.digest()),
            "canonical_bytes_sha256":canonical_before,"helpers":1,"calls":1,
            "program_edited":edited,"publication_attempted":false,"source_owner_reused":false,
            "normal_checked_handoff_qualified":false}),
        );
    }
    assert_eq!(source.composition().helpers().len(), 0);
    assert_eq!(source.composition().calls().len(), 0);
    let original = "package-original/src/ordered_composition_publish_v1.rs";
    let candidate = match case {
        "copy" => "package-promoted-copy/src/ordered_composition_publish_v1.rs",
        "edit" => "package-promoted-edit/src/ordered_composition_publish_v1.rs",
        "inspection-after-publish" => "inspection-after-publish.rs",
        "collision" => "refused-collision.rs",
        "const" => "refused-const.rs",
        "local" => "refused-local.rs",
        "wrapper" => "refused-wrapper.rs",
        _ => return Err("unknown publication case".into()),
    };
    let original_bytes = read_bounded(&root.join(original), 64 * 1024).unwrap();
    let original_hash: [u8; 32] = Sha256::digest(&original_bytes).into();
    let mut published = None;
    let mut publish_error = None;
    let mut duplicate_refusal = None;
    let mut wrong_hash_refused = false;
    // These effect facts outlive the inspection callback. An accounting failure
    // AFTER publication cannot erase them or become a NotAttempted claim.
    let inspected=target.with_source_observation_budget(|owner,seed,budget|{
        fe2o3_lower_mir_kernel::with_ordered_composition_inspection_v1(owner,budget,|view,budget|{
            let region=view.definition(definition,&canonical_identity,&semantic_identity,budget)?;
            let edit=(case=="edit").then_some(Edit{program:edited_program(),registers:region.program().registers()});
            let mut request=Request{definition,expected_canonical:&canonical_identity,
                expected_semantic:semantic_identity,original_path:original,original_sha256:original_hash,
                candidate_path:candidate,helper_name:HELPER,edit};
            if matches!(case,"copy"|"edit"){
                request.original_sha256[0]^=1;
                match publish_ordered_composition_source_v1(seed,view,&request,budget){
                    Ok(receipt)=>{published=Some(receipt);return Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch);}
                    Err(error)=>{
                        assert_eq!(error.effect,Effect::NotAttempted);
                        assert!(error.to_string().contains("publisher current source size or digest differs"));
                        assert!(!root.join(candidate).exists());
                        wrong_hash_refused=true;
                    }
                }
                request.original_sha256=original_hash;
            }
            match publish_ordered_composition_source_v1(seed,view,&request,budget){
                Ok(receipt)=>{
                    published=Some(receipt);
                    if case=="inspection-after-publish"{
                        return Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let before=read_bounded(&root.join(candidate),72*1024).unwrap();
                    match publish_ordered_composition_source_v1(seed,view,&request,budget){
                        Ok(_)=>return Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::CorrespondenceMismatch),
                        Err(error)=>{
                            assert_eq!(error.effect,Effect::MayHaveCreatedCandidate);
                            assert!(error.to_string().contains("publisher create-new publication refused"));
                            duplicate_refusal=Some(error.to_string());
                        }
                    }
                    assert_eq!(read_bounded(&root.join(candidate),72*1024).unwrap(),before);
                }
                Err(error)=>publish_error=Some(error),
            }
            Ok(())
        })
    });
    let facts = json!({
        "schema":"fe2o3-test-ordered-composition-publication-facts-v1","case":case,
        "candidate":candidate,
        "published":published.as_ref().map(|p|json!({"original_sha256":super::super::super::lower_hex_v1(&p.original_sha256),
            "candidate_sha256":super::super::super::lower_hex_v1(&p.candidate_sha256),
            "original_bytes":p.original_bytes,"candidate_bytes":p.candidate_bytes,
            "candidate_device":p.candidate_device,"candidate_inode":p.candidate_inode,
            "program_edited":p.program_edited})),
        "publisher_error":publish_error.as_ref().map(ToString::to_string),
        "publication_may_have_created":published.is_some()||publish_error.as_ref().is_some_and(|e|e.effect==Effect::MayHaveCreatedCandidate),
        "inspection_error":inspected.as_ref().err().map(ToString::to_string),
        "wrong_hash_refused":wrong_hash_refused,"duplicate_no_replace_refusal":duplicate_refusal,
        "acceptance":"historical effects only; requires successful completed child/parent",
    });
    publish_json(root, &format!("{case}.publication-facts.json"), &facts);
    if case == "inspection-after-publish" {
        assert!(inspected.is_err() && publish_error.is_none());
        let receipt = published.expect("actual publication preceded deliberate inspection refusal");
        let bytes = read_bounded(&root.join(candidate), 72 * 1024).unwrap();
        let meta = fs::symlink_metadata(root.join(candidate)).unwrap();
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&bytes)),
            receipt.candidate_sha256
        );
        assert_eq!(
            (meta.dev(), meta.ino()),
            (receipt.candidate_device, receipt.candidate_inode)
        );
        assert_eq!(
            read_bounded(&root.join(original), 64 * 1024).unwrap(),
            original_bytes
        );
        assert_eq!(facts["publication_may_have_created"], true);
        timely(started.elapsed(), 300).map_err(str::to_owned)?;
        return Ok(
            json!({"stage":"post_publication_inspection_refused","facts":facts,
            "publication_occurred":true,"inspection_accepted":false,"fresh_frontend_attempted":false}),
        );
    }
    inspected.map_err(|e| format!("inspection failed AFTER retained publication facts: {e}"))?;
    assert_eq!(
        read_bounded(&root.join(original), 64 * 1024).unwrap(),
        original_bytes
    );
    assert_eq!(
        digest(target.materialized().executable().canonical_bytes()),
        canonical_before
    );
    timely(started.elapsed(), 300).map_err(str::to_owned)?;
    if matches!(case, "copy" | "edit") {
        let receipt =
            published.ok_or_else(|| format!("publisher failed: {}", facts["publisher_error"]))?;
        assert!(publish_error.is_none() && wrong_hash_refused && duplicate_refusal.is_some());
        let bytes = read_bounded(&root.join(candidate), 72 * 1024).unwrap();
        let meta = fs::symlink_metadata(root.join(candidate)).unwrap();
        assert!(meta.is_file() && !meta.file_type().is_symlink());
        assert_eq!(
            (meta.dev(), meta.ino(), bytes.len()),
            (
                receipt.candidate_device,
                receipt.candidate_inode,
                receipt.candidate_bytes
            )
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&bytes)),
            receipt.candidate_sha256
        );
        assert_eq!(receipt.program_edited, case == "edit");
        target
            .with_observation_budget(|_, budget| {
                budget.reserve_storage(receipt.retained_storage_bytes())
            })
            .map_err(|e| e.to_string())?;
        let observed = cpu(target.materialized().executable(), false, started);
        Ok(
            json!({"stage":"actual_source_publication","facts":facts,"cpu":observed,
            "semantic_sha256":super::super::super::lower_hex_v1(&semantic_identity),
            "canonical_identity":super::super::super::lower_hex_v1(canonical_identity.digest()),
            "canonical_bytes_sha256":canonical_before,"source_ledger":target.resource_usage(),
            "normal_checked_handoff_qualified":false}),
        )
    } else {
        assert!(published.is_none());
        let error = publish_error.ok_or("negative source unexpectedly published")?;
        assert_eq!(error.effect, Effect::NotAttempted);
        assert!(
            error.to_string().contains(refusal(case)),
            "wrong actual publisher boundary: {error}"
        );
        assert!(!root.join(candidate).exists());
        Ok(
            json!({"stage":"exact_hir_publication_refused","facts":facts,"diagnostic":error.to_string()}),
        )
    }
}
#[test]
fn publisher_boundaries_do_not_accept_generic_compile_or_inspection_failure() {
    for case in ["collision", "const", "local", "wrapper"] {
        assert!(!"ordinary compiler failed".contains(refusal(case)));
        assert!(!"inspection correspondence mismatch".contains(refusal(case)));
    }
    assert_ne!(refusal("local"), refusal("const"));
    assert_eq!(edited_program().count(), 1);
}
