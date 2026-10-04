//! Synthetic report controls only; these do not establish source execution.
use super::*;
use serde_json::{Value, json};

fn report_fixture() -> Observation8 {
    let case = Case {
        integer: Integer::U32,
        target: Target::Gfx942,
    };
    let values = [
        0_u64,
        1,
        2,
        0x7fff_ffff,
        0x8000_0000,
        0x8000_0001,
        0xffff_fffe,
        0xffff_ffff,
        0xaaaa_aaaa,
        0x5555_5555,
    ];
    let mut scenarios = Vec::new();
    for name in ROOTS {
        for len in [0_usize, 1, 63, 64, 65, 129] {
            for (i, lhs) in values.into_iter().enumerate() {
                for choose in [0_u32, u32::MAX] {
                    let grid = [(len.max(1) as u64).div_ceil(64) * 64, 1, 1];
                    scenarios.push(json!({"root": name, "len": len, "lhs": lhs,
                        "rhs": values[(i + 3) % values.len()], "choose": choose,
                        "grid": grid, "workgroup": [64, 1, 1], "steps": 1,
                        "invocations": grid[0], "checked_bytes": (len + 8) * 4,
                        "replays": 2, "conflicts_incomplete": true, "races_incomplete": true}));
                }
            }
        }
    }
    let roots = ROOTS
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            json!({
                "name": name, "source_function": ([6u8; 32]), "source_body": ([7u8; 32]),
                "source_binding": ([8u8; 32]), "function": name, "source_binary_count": 2,
                "source_divide_count": 1, "original_binary_count": 2, "component_binary_count": 1,
                "anchor": [i, 0, 0], "removed": [i, 1, 0], "descendant": [i, 0, 0],
                "preserved_dynamic_divides": 1, "preserved_traps": 1, "preserved_global_stores": 2
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({
        "case": case,
        "source": {"semantic": ([1u8; 32]), "roots": ROOTS.map(|name|
            json!({"name": name, "function": ([6u8; 32]), "body": ([7u8; 32])}))},
        "original": ([2u8; 32]), "original_bytes": 123,
        "historical_i": ([3u8; 32]), "historical_j": ([4u8; 32]), "historical_j_bytes": 124,
        "final_k": ([5u8; 32]), "final_k_bytes": 125,
        "original_order": ROOTS, "j_order": ROOTS, "k_order": ROOTS, "roots": roots,
        "fresh_k_formal": ROOTS.map(|name| (name, [9u8; 32])),
        "historical_p7_execution": ([10u8; 32]), "proved_pairs": 3, "changed": true,
        "j_traps": 3, "k_traps": 3,
        "entry_work": 0, "stage_work": 10, "after_replay_work": 20, "replay_work": 10,
        "stage_floor": 100, "after_replay_floor": 100,
        "original_sim": {"case": case, "identity": ([2u8; 32]), "canonical_bytes": 123, "scenarios": scenarios},
        "final_sim": {"case": case, "identity": ([5u8; 32]), "canonical_bytes": 125, "scenarios": scenarios},
        "compared_scenarios": SCENARIOS, "native_policy": 8, "native_subject": ([5u8; 32]),
        "native_llvm_sha256": ([11u8; 32]), "native_llvm_bytes": 100, "grants_authority": false
    })).unwrap()
}

#[test]
fn actual_k_report_binds_mutation_source_fresh_reports_and_actual_native_subject() {
    let report = report_fixture();
    validate8(&report, report.case).unwrap();
    let value = serde_json::to_value(&report).unwrap();
    let changes: &[fn(&mut Value)] = &[
        |v| v["changed"] = false.into(),
        |v| v["proved_pairs"] = 0.into(),
        |v| v["historical_j"] = v["final_k"].clone(),
        |v| v["native_subject"] = v["historical_j"].clone(),
        |v| v["native_policy"] = 7.into(),
        |v| v["grants_authority"] = true.into(),
        |v| v["stage_work"] = v["entry_work"].clone(),
        |v| v["after_replay_work"] = v["stage_work"].clone(),
        |v| v["after_replay_floor"] = 101.into(),
        |v| v["k_traps"] = 0.into(),
        |v| v["roots"][0]["preserved_dynamic_divides"] = 0.into(),
        |v| v["roots"][0]["preserved_global_stores"] = 0.into(),
        |v| v["roots"][0]["source_body"] = json!(([19u8; 32])),
        |v| v["fresh_k_formal"].as_array_mut().unwrap().swap(0, 1),
        |v| {
            v["fresh_k_formal"].as_array_mut().unwrap().pop();
        },
        |v| v["k_order"].as_array_mut().unwrap().swap(0, 1),
    ];
    for change in changes {
        let mut changed = value.clone();
        change(&mut changed);
        let changed: Observation8 = serde_json::from_value(changed).unwrap();
        assert!(validate8(&changed, report.case).is_err());
    }
}

#[test]
fn actual_k_report_refuses_unknown_nested_source_and_observation_fields() {
    let value = serde_json::to_value(report_fixture()).unwrap();
    for pointer in ["", "/source"] {
        let mut changed = value.clone();
        changed
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), true.into());
        assert!(serde_json::from_value::<Observation8>(changed).is_err());
    }
}

#[test]
fn actual_k_report_requires_both_exact_sim_subjects_and_matching_experiments() {
    let report = report_fixture();
    validate8(&report, report.case).unwrap();
    let value = serde_json::to_value(&report).unwrap();
    for subject in ["original_sim", "final_sim"] {
        for field in ["identity", "canonical_bytes", "scenarios"] {
            let mut changed = value.clone();
            changed[subject][field] = match field {
                "identity" => json!(([20u8; 32])),
                "canonical_bytes" => 999.into(),
                "scenarios" => json!([]),
                _ => unreachable!(),
            };
            let changed: Observation8 = serde_json::from_value(changed).unwrap();
            assert!(validate8(&changed, report.case).is_err());
        }
        let mut changed = value.clone();
        changed[subject]["scenarios"][0]["grid"] = json!([1, 1, 1]);
        let changed: Observation8 = serde_json::from_value(changed).unwrap();
        assert!(validate8(&changed, report.case).is_err());
        let mut changed = value.clone();
        changed[subject]["scenarios"]
            .as_array_mut()
            .unwrap()
            .swap(0, 1);
        let changed: Observation8 = serde_json::from_value(changed).unwrap();
        assert!(validate8(&changed, report.case).is_err());
    }
}
