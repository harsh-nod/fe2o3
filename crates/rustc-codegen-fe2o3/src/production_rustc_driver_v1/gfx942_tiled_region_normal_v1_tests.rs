//! Separate actual-source checked continuation; no numerical simulation or
//! native execution. Copied evidence cannot reacquire the live compiler owner.
use super::*;

pub(in super::super) const GATE: &str = "normal-two-sessions";
pub(in super::super) const CHILD: &str = "production_rustc_driver_v1::gfx942_tiled_region_qualification_v1_tests::observation::normal::actual_bf16_normal_child";
pub(in super::super) const PREFIX: &str = "FE2O3_TILED_REGION_NORMAL_OBSERVATION_V1 ";
pub(in super::super) const SCHEMA: &str = "fe2o3-test-tiled-region-normal-observation-v1";
const LLVM_CAP: usize = 8 * 1024 * 1024;
const HANDOFF_CAP: usize = 16 * 1024 * 1024;
const ARTIFACTS: [(&str, usize); 3] = [
    ("direct.normal.ll", LLVM_CAP),
    ("direct.worker.ll", LLVM_CAP),
    ("direct.handoff-v2.bin", HANDOFF_CAP),
];

fn bounded_bytes(bytes: usize, cap: usize) -> Result<(), &'static str> {
    (bytes > 0 && bytes <= cap)
        .then_some(())
        .ok_or("normal artifact byte cap")
}

// This is a bounded test observation domain after the original live callback;
// it is not a new compiler ledger or a way to bypass production accounting.
pub(in super::super) fn observe<'tcx>(
    transaction: crate::production_pipeline::ProductionCompilation<
        'tcx,
        crate::production_pipeline::CollectedRustStage<'tcx>,
    >,
    directory: &Path,
    started: std::time::Instant,
) -> Value {
    let continued = transaction.continue_bf16_mfma_source_for_test_v1(snapshot);
    let copied = continued.snapshot.map(|snapshot| {
        json!({"stage":"actual_pre_ranked_source_region", "snapshot":snapshot,
            "phase":continued.phase})
    });
    let mut row = json!({"stage":"normal_continuation_refused", "failed_stage":continued.stage,
        "inspection":copied, "phase":continued.phase,
        "original_v12_identity":continued.original_v12_identity,
        "normal_ranked_formal_target_handoff_qualified":false,
        "output_publication_attempted":false,"numerical_cpu_qualified":false,
        "grants_artifact_or_launch_authority":false,"hardware_observed":false});
    let target = match continued.result {
        Ok(target) => target,
        Err(error) => {
            row["diagnostic"] = json!(error.to_string());
            return row;
        }
    };
    row["target"] = json!({
        "name":target.target_name(), "canonical_version":target.canonical_kernel_ir_version(),
        "semantic_functions":target.semantic_function_count(),
        "correspondence_blocks":target.correspondence_block_count(),
        "formal_witness_extent":target.formal_witness_extent(),
        "formal_allocations":target.formal_allocation_count(),
        "formal_accesses":target.formal_access_count(),
        "ranked_discharged_bounds":target.ranked_dynamic_index_discharge_count(),
        "runtime_bounds_requirements":target.runtime_bounds_requirement_count(),
        "runtime_alias_requirements":target.runtime_alias_requirement_count(),
        "inter_invocation_conflicts":target.inter_invocation_conflict_count(),
        "retained_identity_transaction_bindings":target.retained_identity_and_transaction_binding_count(),
        "grants_artifact_or_launch_authority":target.grants_artifact_or_launch_authority(),
    });
    let mut stage = "normal-LLVM-observation";
    let result = (|| -> Result<Value, String> {
        timely(started.elapsed(), 300)?;
        bounded_bytes(target.llvm_ir().len(), LLVM_CAP)?;
        // Capture the ordinary emitted LLVM before consuming the actual target.
        // The worker LLVM is recorded separately; descriptor insertion may differ.
        let llvm = target.llvm_ir().as_bytes().to_vec();
        let calls = target
            .llvm_ir()
            .matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k")
            .count();
        stage = "inert-worker-handoff-v2";
        let handoff = target
            .into_inert_worker_handoff_for_extraction()
            .map_err(|e| e.to_string())?;
        bounded_bytes(handoff.canonical_bytes().len(), HANDOFF_CAP)?;
        bounded_bytes(handoff.module_bytes().len(), LLVM_CAP)?;
        stage = "handoff-roundtrip-observation";
        let decoded =
            fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(handoff.canonical_bytes())
                .map_err(|e| e.to_string())?;
        if decoded.canonical_bytes() != handoff.canonical_bytes()
            || decoded.module_bytes() != handoff.module_bytes()
            || decoded.target() != handoff.target()
            || decoded.kind() != handoff.kind()
            || decoded.authenticates_compiler_origin()
            || decoded.grants_compiler_authority()
            || decoded.grants_worker_authority()
        {
            return Err("inert handoff roundtrip/authority differs".into());
        }
        let buffers = [
            llvm.as_slice(),
            handoff.module_bytes(),
            handoff.canonical_bytes(),
        ];
        let mut pins = Vec::with_capacity(3);
        for ((name, cap), bytes) in ARTIFACTS.into_iter().zip(buffers) {
            timely(started.elapsed(), 300)?;
            stage = "create-new-normal-output";
            row["output_publication_attempted"] = json!(true);
            crate::production_rustc_driver_v1::publish_new_inert_output(
                &directory.join(name),
                bytes,
                cap,
                "BF16 normal qualification artifact",
            )
            .map_err(|e| e.to_string())?;
            let readback = read_bounded(&directory.join(name), cap).map_err(|e| e.to_string())?;
            if readback != bytes {
                return Err("normal output readback differs".into());
            }
            pins.push(json!({"name":name,"bytes":bytes.len(),"sha256":digest(bytes)}));
            timely(started.elapsed(), 300)?;
        }
        stage = "final-normal-observation";
        let result = json!({"artifacts":pins,"mfma_llvm_calls":calls,
            "handoff":{"target":handoff.target().to_string(),
                "authenticates_compiler_origin":handoff.authenticates_compiler_origin(),
                "grants_compiler_authority":handoff.grants_compiler_authority(),
                "grants_worker_authority":handoff.grants_worker_authority(),
                "descriptor_validation":"existing prepare_production_worker_handoff + into_validated_parts; no separate descriptor authority"}});
        timely(started.elapsed(), 300)?;
        Ok(result)
    })();
    match result {
        Ok(output) => {
            row["stage"] = json!("actual_same_compilation_normal_handoff");
            row["failed_stage"] = Value::Null;
            row["outputs"] = output;
            row["normal_ranked_formal_target_handoff_qualified"] = json!(true);
        }
        Err(error) => {
            row["failed_stage"] = json!(stage);
            row["diagnostic"] = json!(error);
        }
    }
    row
}

pub(in super::super) fn accept(case: &str, row: &Value) -> Result<(), &'static str> {
    if row["numerical_cpu_qualified"] != false
        || row["grants_artifact_or_launch_authority"] != false
        || row["hardware_observed"] != false
    {
        return Err("normal observation authority boundary");
    }
    match case {
        "direct" => {
            if row["stage"] != "actual_same_compilation_normal_handoff"
                || !row["failed_stage"].is_null()
                || row["normal_ranked_formal_target_handoff_qualified"] != true
                || row["output_publication_attempted"] != true
            {
                return Err("actual normal continuation refused");
            }
            super::accept("direct", &row["inspection"])?;
            if row["original_v12_identity"]
                .as_array()
                .is_none_or(|a| a.len() != 32)
                || row["original_v12_identity"]
                    != row["inspection"]["snapshot"]["canonical_identity"]
                || row["target"]["name"] != "gfx942:xnack-"
                || row["target"]["grants_artifact_or_launch_authority"] != false
                || row["outputs"]["mfma_llvm_calls"] != 1
                || row["outputs"]["handoff"]["target"] != "gfx942:xnack-"
            {
                return Err("same-owner normal target/LLVM relation");
            }
            for field in [
                "authenticates_compiler_origin",
                "grants_compiler_authority",
                "grants_worker_authority",
            ] {
                if row["outputs"]["handoff"][field] != false {
                    return Err("handoff granted authority");
                }
            }
            let pins = row["outputs"]["artifacts"]
                .as_array()
                .ok_or("normal artifact roster")?;
            if pins.len() != 3 {
                return Err("normal artifact roster length");
            }
            for ((name, cap), pin) in ARTIFACTS.into_iter().zip(pins) {
                if pin["name"] != name {
                    return Err("normal artifact name/order");
                }
                let bytes = pin["bytes"]
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                    .ok_or("normal artifact length")?;
                bounded_bytes(bytes, cap)?;
                let hash = pin["sha256"].as_str().ok_or("normal artifact digest")?;
                if hash.len() != 64
                    || !hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                {
                    return Err("normal artifact digest grammar");
                }
            }
        }
        "wrong-launch" => {
            if row["stage"] != "normal_continuation_refused"
                || row["failed_stage"] != "pre-ranked-source"
                || row["normal_ranked_formal_target_handoff_qualified"] != false
                || row["output_publication_attempted"] != false
                || !row["inspection"].is_null()
                || !row["original_v12_identity"].is_null()
                || !row["outputs"].is_null()
            {
                return Err("normal launch refusal crossed source boundary");
            }
            super::accept(
                "wrong-launch",
                &json!({"stage":"actual_source_or_callback_refused", "diagnostic":row["diagnostic"]}),
            )?;
        }
        _ => return Err("unknown normal case"),
    }
    Ok(())
}

pub(in super::super) fn recheck_outputs(directory: &Path, case: &str, row: &Value) {
    accept(case, row).unwrap();
    if case == "direct" {
        for ((name, cap), pin) in ARTIFACTS
            .into_iter()
            .zip(row["outputs"]["artifacts"].as_array().unwrap())
        {
            let bytes = read_bounded(&directory.join(name), cap).unwrap();
            assert_eq!(pin["bytes"], bytes.len());
            assert_eq!(pin["sha256"], digest(&bytes));
        }
    } else {
        // Direct artifacts belong to the earlier positive. The refused child
        // must not create its own output or rewrite those retained bytes.
        assert!(!directory.join("wrong-launch.normal.ll").exists());
        assert!(!directory.join("wrong-launch.worker.ll").exists());
        assert!(!directory.join("wrong-launch.handoff-v2.bin").exists());
    }
}

#[test]
#[ignore = "actual normal child; only launch via the separately bounded two-session ladder"]
fn actual_bf16_normal_child() {
    super::super::source_child(true);
}
#[test]
#[ignore = "genuine BF16 inspection then SAME original owner through mandatory normal gates; no numerical simulation"]
fn actual_bf16_normal_ladder() {
    ladder(&["direct", "wrong-launch"], GATE);
}

#[test]
fn normal_artifact_caps_are_exact_and_nonempty() {
    for cap in [LLVM_CAP, HANDOFF_CAP] {
        assert!(bounded_bytes(0, cap).is_err());
        assert!(bounded_bytes(cap, cap).is_ok());
        assert!(bounded_bytes(cap + 1, cap).is_err());
    }
}

#[test]
fn normal_oracle_joins_identity_output_roster_and_inert_authority() {
    // Inert metadata only, never a source or target owner. Exercise the oracle
    // independently; the ignored ladder alone can supply live positive facts.
    let identity = [7u8; 32];
    let pins = ARTIFACTS.map(|(name, _)| json!({"name":name,"bytes":1,"sha256":"0".repeat(64)}));
    let good = json!({"stage":"actual_same_compilation_normal_handoff", "failed_stage":null,
        "normal_ranked_formal_target_handoff_qualified":true,"output_publication_attempted":true,
        "numerical_cpu_qualified":false,"grants_artifact_or_launch_authority":false,"hardware_observed":false,
        "original_v12_identity":identity,
        "inspection":{"stage":"actual_pre_ranked_source_region","snapshot":{"canonical_identity":identity},
            "phase":{"same_ledger":true,"result_ok":true,"failed_work":false,"failed_storage":false}},
        "target":{"name":"gfx942:xnack-","grants_artifact_or_launch_authority":false},
        "outputs":{"artifacts":pins,"mfma_llvm_calls":1,
            "handoff":{"target":"gfx942:xnack-","authenticates_compiler_origin":false,
                "grants_compiler_authority":false,"grants_worker_authority":false}}});
    assert!(accept("direct", &good).is_ok());
    let mut changed = good.clone();
    changed["original_v12_identity"][0] = json!(8);
    assert!(accept("direct", &changed).is_err());
    let mut changed = good.clone();
    changed["inspection"]["phase"]["same_ledger"] = json!(false);
    assert!(accept("direct", &changed).is_err());
    let mut changed = good.clone();
    changed["outputs"]["mfma_llvm_calls"] = json!(0);
    assert!(accept("direct", &changed).is_err());
    for field in [
        "authenticates_compiler_origin",
        "grants_compiler_authority",
        "grants_worker_authority",
    ] {
        let mut changed = good.clone();
        changed["outputs"]["handoff"][field] = json!(true);
        assert!(accept("direct", &changed).is_err());
    }
    for (field, value) in [
        ("name", json!("../worker.ll")),
        ("bytes", json!(0)),
        ("sha256", json!("not-a-pin")),
    ] {
        let mut changed = good.clone();
        changed["outputs"]["artifacts"][0][field] = value;
        assert!(accept("direct", &changed).is_err());
    }
    let mut changed = good.clone();
    changed["outputs"]["artifacts"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert!(accept("direct", &changed).is_err());
    let mut changed = good.clone();
    changed["outputs"]["artifacts"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(accept("direct", &changed).is_err());
}
#[test]
fn normal_oracle_does_not_accept_a_successful_preranked_snapshot_or_later_refusal() {
    let identity = [0u8; 32];
    let pre = json!({"stage":"actual_pre_ranked_source_region","snapshot":{"canonical_identity":identity}});
    assert!(accept("direct", &pre).is_err());
    for stage in [
        "ranked",
        "target-neutral-attachment",
        "formal-memory",
        "target",
        "inert-worker-handoff-v2",
    ] {
        let row = json!({"stage":"normal_continuation_refused","failed_stage":stage,
            "numerical_cpu_qualified":false,"grants_artifact_or_launch_authority":false,"hardware_observed":false});
        assert!(accept("direct", &row).is_err());
        assert!(accept("wrong-launch", &row).is_err());
    }
}
#[test]
fn normal_wrong_launch_requires_exact_early_refusal_without_outputs() {
    let good = json!({"stage":"normal_continuation_refused","failed_stage":"pre-ranked-source",
        "normal_ranked_formal_target_handoff_qualified":false,"output_publication_attempted":false,
        "diagnostic":"BF16 source requires explicit WG64 and one workgroup",
        "numerical_cpu_qualified":false,"grants_artifact_or_launch_authority":false,"hardware_observed":false});
    assert!(accept("wrong-launch", &good).is_ok());
    for (field, value) in [
        ("output_publication_attempted", json!(true)),
        ("hardware_observed", json!(true)),
        ("inspection", json!({})),
        ("diagnostic", json!("unrelated compiler panic")),
    ] {
        let mut bad = good.clone();
        bad[field] = value;
        assert!(accept("wrong-launch", &bad).is_err());
    }
}
