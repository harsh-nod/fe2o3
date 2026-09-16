#[test]
fn every_production_entry_crosses_the_same_pre_ranked_materialization_stage() {
    let source = include_str!("production_pipeline.rs");
    let entries = source
        .split("impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {")
        .nth(1)
        .expect("collector-owned production entry stage")
        .split("impl<'tcx> ProductionCompilation<'tcx, AdmittedSemanticMirStage> {")
        .next()
        .expect("bounded production entry stage");
    let names = [
        "verify_general_kernel_checks",
        "lower_production_target",
        "export_simulation_bundle_v1",
        "export_simulation_bundle_v2",
        "export_simulation_bundle_v3",
        "export_simulation_bundle_v4",
        "export_simulation_bundle_v5",
        "export_simulation_bundle_v6",
    ];
    assert_eq!(
        entries.matches(".materialize_target_neutral()?").count(),
        names.len()
    );
    for name in names {
        let signature = format!("pub(crate) fn {name}(");
        let route = entries
            .split(signature.as_str())
            .nth(1)
            .unwrap_or_else(|| panic!("missing {name}"))
            .split("pub(crate) fn ")
            .next()
            .unwrap();
        let stages = [
            ".import_semantic_mir()?",
            ".construct_semantic_middle_end()?",
            ".construct_semantic_ssa()?",
            ".materialize_target_neutral()?",
            ".verify_general_kernel_checks()",
        ];
        let mut previous = None;
        for stage in stages {
            assert_eq!(route.matches(stage).count(), 1, "{name}: {stage}");
            let position = route.find(stage).unwrap();
            if let Some(previous) = previous {
                assert!(previous < position, "{name}: reordered {stage}");
            }
            previous = Some(position);
        }
        let ranked = previous.unwrap();
        if name == "verify_general_kernel_checks" {
            assert!(!route.contains(".attach_target_neutral_checks()"));
            assert!(!route.contains(".admit_formal_memory()"));
        } else {
            let attach = route
                .find(".attach_target_neutral_checks()?")
                .expect("checked attachment");
            assert!(ranked < attach, "{name}: attachment preceded ranked checks");
            if name == "lower_production_target" {
                let formal = route
                    .find(".admit_formal_memory()?")
                    .expect("unchanged formal gate");
                let target = route
                    .find(".lower_production_target()")
                    .expect("target lowering");
                assert!(attach < formal && formal < target);
            } else {
                let suffix = name.strip_prefix("export_simulation_bundle_").unwrap();
                let export = format!(".into_simulation_bundle_{suffix}(");
                assert!(
                    attach
                        < route
                            .find(export.as_str())
                            .expect("same explicit simulation endpoint")
                );
                assert!(!route.contains(".admit_formal_memory()"));
                assert!(!route.contains(".lower_production_target()"));
            }
        }
        assert!(!route.contains("try_lower_after_ranked"));
        assert!(!route.contains("project_and_verify_ranked_semantic_mir_v1("));
    }
}
