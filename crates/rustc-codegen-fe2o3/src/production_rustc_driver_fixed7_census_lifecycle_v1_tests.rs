//! Separate public-runner roster, not ordinary-source DSE mutation qualification.
use super::*;

#[test]
#[ignore = "real fixed7 public runner, pinned nightly and AMD dependencies; 26 compiler children"]
fn fixed7_source_routes_and_census_lifecycles_are_observational() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-fixed7-census-lifecycle");
    let mut direct_gfx942 = None;
    let mut completed = 0;
    for target in ["gfx942", "gfx950"] {
        for input in [Input::Fill, Input::RetainedUnit] {
            let case = capture(&workspace, scratch.path(), input, target);
            let disabled = run(
                &case,
                Policy::Seven,
                Diagnostic::Disabled,
                &scratch
                    .path()
                    .join(format!("route-{target}-{input:?}-disabled")),
            );
            let enabled = run(
                &case,
                Policy::Seven,
                Diagnostic::Fresh,
                &scratch
                    .path()
                    .join(format!("route-{target}-{input:?}-fresh")),
            );
            unchanged(&disabled, &enabled);
            for run in [&disabled, &enabled] {
                let stderr = String::from_utf8_lossy(&run.output.stderr);
                assert!(stderr.contains("historical I ") && stderr.contains("actual J "));
                assert!(!stderr.contains("actual I "));
            }
            completed += 2;
            if target == "gfx942" && input == Input::Fill {
                direct_gfx942 = Some(case);
            }
        }
    }
    let direct = direct_gfx942.unwrap();
    let refusal = capture(&workspace, scratch.path(), Input::WaveRefusal, "gfx942");
    let fatal = parse_fatal(&workspace, scratch.path(), &direct);
    for case in [&direct, &refusal, &fatal] {
        let mut baseline = None;
        for diagnostic in Diagnostic::ALL {
            let result = run(
                case,
                Policy::Seven,
                diagnostic,
                &scratch
                    .path()
                    .join(format!("lifecycle-{:?}-{diagnostic:?}", case.input)),
            );
            if let Some(baseline) = &baseline {
                unchanged(baseline, &result);
            } else {
                baseline = Some(result);
            }
            completed += 1;
        }
    }
    assert_eq!(completed, 26);
    eprintln!(
        "FIXED7 CENSUS: 4 source/route cases and 18 lifecycle controls; 26 actual public invocations; no source DSE mutation claim"
    );
}
