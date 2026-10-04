//! Actual fresh callbacks consume the same private recipe codec as unit tests.
use super::*;
use crate::collector::source_census_v1::bitselect_feasibility::retained::local_order::recipes as codec;

#[path = "source_local_order_recipe_fixture_v1_tests.rs"]
mod cases;
#[path = "source_local_order_recipe_io_v1_tests.rs"]
mod io;
#[path = "source_local_order_recipe_ladder_v1_tests.rs"]
mod ladder;

const OUTPUT: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_RECIPE_OUTPUT";
const INPUT: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_RECIPE_INPUT";
const CASE: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_RECIPE_CASE";
const RUN: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_RECIPE_RUN";
const RECIPE: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_RECIPE_NAME";
const CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::local_order::recipes::actual_source_local_order_recipe_child";
const PREFIX: &str = "FE2O3_SOURCE_LOCAL_ORDER_RECIPE ";

struct RecipeCallbacks {
    source: Option<RetainedInput>,
    recipe: Option<io::RetainedRecipe>,
    calls: usize,
    result: Option<Result<(Value, Option<[Vec<u8>; 2]>), String>>,
}
impl Callbacks for RecipeCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let source = self.source.take().expect("one retained source descriptor");
        self.result = Some((|| {
            if let Some(recipe) = &mut self.recipe {
                recipe.recheck()?;
            }
            let transaction = crate::production_rustc_driver_v1::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let result = transaction.observe_source_local_order_recipe(
                source,
                self.recipe.as_ref().map(|file| &file.recipe),
            )?;
            if let Some(recipe) = &mut self.recipe {
                recipe.recheck()?;
            }
            Ok(result)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "actual rustc child; use actual_source_local_order_recipe_ladder"]
fn actual_source_local_order_recipe_child() {
    let directory = PathBuf::from(std::env::var_os(INPUT).expect("recipe input directory"));
    let case = std::env::var(CASE).expect("recipe case");
    let run = std::env::var(RUN).expect("recipe run");
    let name = std::env::var(RECIPE).expect("recipe selector");
    let name = (name != "none").then_some(name);
    require_current_source();
    assert_eq!(std::env::current_dir().unwrap(), repository());
    let actual = cases::derive(&directory, &case, &run, name.as_deref());
    let saved: cases::Invocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{case}-{run}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, saved, "stale recipe invocation preparation");
    assert_eq!(
        std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(),
        actual.crate_binding
    );
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        actual.cargo_observation
    );
    assert_eq!(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        fixture().to_str().unwrap()
    );
    assert_eq!(std::env::var("CARGO_PKG_NAME").unwrap(), PACKAGE);
    assert_eq!(std::env::var("CARGO_PKG_VERSION").unwrap(), "0.1.0");
    assert_eq!(std::env::var("CARGO_CRATE_NAME").unwrap(), CRATE_NAME);
    crate::production_rustc_driver_v1::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let source = RetainedInput::open(actual.source_relative.to_str().unwrap(), false).unwrap();
    let recipe = name
        .as_deref()
        .map(|name| io::RetainedRecipe::open(&io::recipe_path(&directory, name)).unwrap());
    if case == "stale-source" {
        fs::OpenOptions::new()
            .append(true)
            .open(&actual.source_relative)
            .unwrap()
            .write_all(b"\n// changed after retention, before actual rustc\n")
            .unwrap();
    }
    if case == "stale-recipe" {
        // This is a private create-new copy, never one of the four saved recipes.
        fs::OpenOptions::new()
            .append(true)
            .open(io::recipe_path(&directory, "stale"))
            .unwrap()
            .write_all(b" ")
            .unwrap();
    }
    let mut callbacks = RecipeCallbacks {
        source: Some(source),
        recipe,
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1, "actual callback count");
    let result = callbacks
        .result
        .expect("actual recipe callback did not run");
    let observation = if let Some(expected) = cases::refusal(&case, &run) {
        let diagnostic = result.expect_err("source/recipe negative must reach its exact boundary");
        assert_eq!(
            diagnostic, expected,
            "another compiler failure is not this negative"
        );
        json!({"stage":"actual_source_local_order_recipe_refused","diagnostic":diagnostic})
    } else {
        let (result, generated) = result.unwrap();
        assert_eq!(result["stage"], "actual_source_local_order_private_recipe");
        assert_eq!(result["bound_version"], "V12");
        assert_eq!(result["scheduled_version"], "V12");
        assert_eq!(result["parameter_ordinals"], json!([1, 2, 3, 4]));
        assert_eq!(result["independent_transition_replays"], 2);
        for key in [
            "previous_evidence_reused",
            "serialized_owner_read",
            "fixed_production_policy_modified",
            "public_recipe_admitted",
            "final_source_output_admitted",
            "native_emitted",
            "grants_artifact_or_launch_authority",
        ] {
            assert_eq!(result[key], false);
        }
        if name.is_none() {
            assert_eq!(result["mode"], "generate");
            assert_ne!(result["selected_identity"], result["second_identity"]);
            assert_ne!(result["selected_order"], result["second_order"]);
            assert!(matches!(
                (case.as_str(), run.as_str()),
                ("positive", "generate") | ("new-item", "regenerate")
            ));
            io::persist_pair(&directory, case == "new-item", generated.unwrap());
        } else {
            assert_eq!(result["mode"], "replay");
            assert!(generated.is_none());
            assert_eq!(result["selected_identity"], result["second_identity"]);
            assert_eq!(result["selected_order"], result["second_order"]);
            assert_eq!(result["simulation"]["scenarios"], 15);
            assert_eq!(result["simulation"]["runs"], 30);
            assert_eq!(result["simulation"]["output_and_canaries_checked"], true);
        }
        result
    };
    if case != "stale-source" {
        assert_eq!(hash(&actual.source_relative), actual.source_sha256);
    }
    if case != "stale-recipe"
        && let Some(name) = name
    {
        assert_eq!(
            Some(hash(&io::recipe_path(&directory, &name))),
            actual.recipe_sha256
        );
    }
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let report = serde_json::to_vec(&json!({
        "invocation":actual,"observation":observation,
        "recipe_io_accounting":callbacks.recipe.as_ref().map(|r| &r.accounting),
        "actual_rustc_callback":true,"grants_artifact_or_launch_authority":false,
    }))
    .unwrap();
    assert!(report.len() <= 64 * 1024);
    println!("\n{PREFIX}{}", std::str::from_utf8(&report).unwrap());
}

fn child(directory: &Path, case: &str, run: &str, recipe: Option<&str>) -> Value {
    let record = cases::derive(directory, case, run, recipe);
    paths::write_new(
        &directory.join(format!("{case}-{run}.invocation.json")),
        &serde_json::to_vec_pretty(&record).unwrap(),
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    let stdout = checked(
        sanitized(&mut command)
            .current_dir(repository())
            .args(["--exact", CHILD, "--ignored", "--nocapture"])
            .env(INPUT, directory)
            .env(CASE, case)
            .env(RUN, run)
            .env(RECIPE, recipe.unwrap_or("none"))
            .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &record.cargo_observation,
            )
            .env("CARGO_MANIFEST_DIR", fixture())
            .env("CARGO_PKG_NAME", PACKAGE)
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_CRATE_NAME", CRATE_NAME),
        directory,
        &format!("{case}-{run}"),
        None,
    );
    let stdout = std::str::from_utf8(&stdout).unwrap();
    let reports = stdout
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].len() <= 64 * 1024);
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let report: Value = serde_json::from_str(reports[0]).unwrap();
    assert_eq!(report["invocation"], serde_json::to_value(record).unwrap());
    report
}
