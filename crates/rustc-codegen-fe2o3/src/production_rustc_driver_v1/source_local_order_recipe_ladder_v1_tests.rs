//! Fifteen fresh callbacks, seven independent full-kernel replay observations.
use super::*;

fn observation(report: &Value) -> &Value {
    &report["observation"]
}

#[test]
#[ignore = "pinned actual source, serialized Cargo, fresh absolute output directory"]
fn actual_source_local_order_recipe_ladder() {
    let directory = PathBuf::from(std::env::var_os(OUTPUT).expect("fresh recipe output directory"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let source_root = paths::create_root(&directory);
    require_current_source();
    super::super::super::prepare(&directory);
    for case in cases::CASES {
        let case_dir = source_root.join(case);
        fs::create_dir(&case_dir).unwrap();
        paths::write_new(&case_dir.join("source.rs"), cases::source(case).as_bytes());
    }
    let generated = child(&directory, "positive", "generate", None);
    let saved_original = ["source", "reverse"].map(|name| {
        let path = io::recipe_path(&directory, name);
        let bytes = read_bounded(&path, codec::BYTE_CAP).unwrap();
        assert_eq!(
            codec::Recipe::decode(&bytes).unwrap().encode().unwrap(),
            bytes
        );
        bytes
    });
    assert_ne!(saved_original[0], saved_original[1]);
    let source = child(&directory, "positive", "source", Some("source"));
    let reverse = child(&directory, "positive", "reverse", Some("reverse"));
    let repeated = child(&directory, "positive", "repeat", Some("reverse"));
    assert_eq!(
        observation(&reverse),
        observation(&repeated),
        "unchanged fresh callbacks must replay deterministically"
    );
    assert_eq!(
        observation(&source)["selected_identity"],
        observation(&generated)["selected_identity"]
    );
    assert_eq!(
        observation(&reverse)["selected_identity"],
        observation(&generated)["second_identity"]
    );
    assert_ne!(
        observation(&source)["selected_identity"],
        observation(&reverse)["selected_identity"]
    );
    assert_ne!(
        observation(&source)["selected_order"],
        observation(&reverse)["selected_order"]
    );
    assert_eq!(observation(&source)["preference"], "source_order");
    assert_eq!(observation(&reverse)["preference"], "reverse_ready");
    assert_eq!(observation(&source)["source_unchanged_from_origin"], true);

    let renamed_source = child(&directory, "renamed", "source", Some("source"));
    let renamed_reverse = child(&directory, "renamed", "reverse", Some("reverse"));
    for renamed in [&renamed_source, &renamed_reverse] {
        assert_eq!(
            observation(renamed)["current_binding"],
            observation(&source)["current_binding"],
            "parameter/span edit must preserve all five actually measured Instance identities"
        );
        assert_eq!(observation(renamed)["source_unchanged_from_origin"], false);
        assert_eq!(
            observation(renamed)["previous_origin"],
            observation(&source)["current_origin"]
        );
        assert_ne!(
            observation(renamed)["current_origin"]["source"],
            observation(&source)["current_origin"]["source"]
        );
        assert_ne!(
            observation(renamed)["source_initializer"],
            observation(&source)["source_initializer"]
        );
        assert_eq!(observation(renamed)["previous_evidence_reused"], false);
    }
    assert_eq!(
        observation(&renamed_source)["selected_order"],
        observation(&source)["selected_order"]
    );
    assert_eq!(
        observation(&renamed_reverse)["selected_order"],
        observation(&reverse)["selected_order"]
    );
    let changed = child(&directory, "changed-operator", "source", Some("source"));
    let ambiguous = child(&directory, "ambiguous", "source", Some("source"));
    let new_item_old = child(&directory, "new-item", "old", Some("reverse"));
    let regenerated = child(&directory, "new-item", "regenerate", None);
    assert_ne!(
        observation(&regenerated)["current_binding"]["item"],
        observation(&generated)["current_binding"]["item"],
        "new item regeneration must be measured, not an automatic rebind of old intent"
    );
    let new_source = child(&directory, "new-item", "source", Some("new-source"));
    let new_reverse = child(&directory, "new-item", "reverse", Some("new-reverse"));
    assert_eq!(
        observation(&new_source)["current_binding"],
        observation(&regenerated)["current_binding"]
    );
    assert_eq!(
        observation(&new_reverse)["current_binding"],
        observation(&regenerated)["current_binding"]
    );
    assert_eq!(
        observation(&new_source)["selected_identity"],
        observation(&regenerated)["selected_identity"]
    );
    assert_eq!(
        observation(&new_reverse)["selected_identity"],
        observation(&regenerated)["second_identity"]
    );
    assert_ne!(
        read_bounded(&io::recipe_path(&directory, "new-source"), codec::BYTE_CAP).unwrap(),
        saved_original[0]
    );
    assert_ne!(
        read_bounded(&io::recipe_path(&directory, "new-reverse"), codec::BYTE_CAP).unwrap(),
        saved_original[1]
    );

    let stale = child(&directory, "stale-source", "source", Some("source"));
    let target = child(&directory, "wrong-target", "source", Some("source"));
    paths::write_new(&io::recipe_path(&directory, "stale"), &saved_original[0]);
    let stale_recipe = child(&directory, "stale-recipe", "source", Some("stale"));
    for (name, bytes) in ["source", "reverse"].into_iter().zip(saved_original) {
        assert_eq!(
            read_bounded(&io::recipe_path(&directory, name), codec::BYTE_CAP).unwrap(),
            bytes,
            "no replay/refusal/regeneration overwrites the original recipe"
        );
    }
    let observations = vec![
        generated,
        source,
        reverse,
        repeated,
        renamed_source,
        renamed_reverse,
        changed,
        ambiguous,
        new_item_old,
        regenerated,
        new_source,
        new_reverse,
        stale,
        target,
        stale_recipe,
    ];
    assert_eq!(observations.len(), 15);
    assert_eq!(
        observations
            .iter()
            .filter(|r| observation(r)["mode"] == "replay")
            .count(),
        7
    );
    assert_eq!(
        observations
            .iter()
            .filter(|r| observation(r)["mode"] == "generate")
            .count(),
        2
    );
    assert_eq!(
        observations
            .iter()
            .filter(|r| observation(r)["stage"] == "actual_source_local_order_recipe_refused")
            .count(),
        6
    );
    let runs: u64 = observations
        .iter()
        .filter_map(|r| observation(r)["simulation"]["runs"].as_u64())
        .sum();
    assert_eq!(runs, 210);

    let mut source_bytes = 0u64;
    assert_eq!(
        fs::read_dir(&source_root).unwrap().take(9).count(),
        cases::CASES.len()
    );
    for case in cases::CASES {
        let path = cases::absolute(&directory, case);
        assert_eq!(
            fs::read_dir(path.parent().unwrap())
                .unwrap()
                .take(2)
                .count(),
            1
        );
        let size = fs::metadata(path).unwrap().len();
        assert!(size <= 64 * 1024);
        source_bytes = source_bytes.checked_add(size).unwrap();
    }
    let mut recipe_bytes = 0u64;
    for name in ["source", "reverse", "new-source", "new-reverse", "stale"] {
        let path = io::recipe_path(&directory, name);
        let bytes = read_bounded(&path, codec::BYTE_CAP).unwrap();
        // Even the stale-file negative remains valid inert JSON; currentness,
        // rather than its semantics, was deliberately broken.
        codec::Recipe::decode(&bytes).unwrap();
        recipe_bytes = recipe_bytes.checked_add(bytes.len() as u64).unwrap();
    }
    assert!(recipe_bytes <= 5 * codec::BYTE_CAP as u64);
    require_current_source();
    let report = serde_json::to_vec_pretty(&json!({
        "observations":observations,"actual_callbacks":15,"successful_replays":7,
        "fresh_item_generations":2,"exact_refusals":6,"full_kernel_simulator_runs":runs,
        "source_directory":paths::relative_root(&directory),"source_files":8,"source_bytes":source_bytes,
        "source_file_limit":8,"source_byte_limit":8 * 64 * 1024,
        "recipe_files":5,"recipe_bytes":recipe_bytes,"recipe_file_limit":5,
        "recipe_byte_limit":5 * codec::BYTE_CAP,"private_recipe_draft":true,
        "public_recipe_placement_resolved":false,"public_recipe_admitted":false,
        "fixed_production_policy_modified":false,"final_source_output_admitted":false,
        "native_emitted":false,"grants_artifact_or_launch_authority":false,
    })).unwrap();
    assert!(report.len() <= 512 * 1024);
    paths::write_new(&directory.join("observation.json"), &report);
}
