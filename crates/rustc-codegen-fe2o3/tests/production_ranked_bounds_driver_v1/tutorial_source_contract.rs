struct TutorialSourceCaseV1<'a> {
    test_function: &'a str,
    feature: &'a str,
    kernel_symbol: &'a str,
    target: &'a str,
    bundle_version: u16,
    displayed_fragment: usize,
    refusal: Option<&'a str>,
}

impl TutorialSourceCaseV1<'_> {
    fn expected_row(&self) -> Value {
        let expectation = match self.refusal {
            Some(diagnostic) => json!({
                "kind": "rejected", "bundleVersion": self.bundle_version,
                "diagnosticContains": diagnostic, "outputArtifact": "absent",
            }),
            None => json!({"kind": "verified-bundle-export", "bundleVersion": self.bundle_version}),
        };
        json!({
            "features": [self.feature], "kernelSymbol": self.kernel_symbol,
            "target": self.target, "displayedFragmentOrdinal": self.displayed_fragment,
            "testFunction": self.test_function, "expectation": expectation,
        })
    }

    fn check_contract(&self, manifest: &Value) -> Result<(), &'static str> {
        if manifest["curriculum"]["schema"] != "fe2o3-tutorial-curriculum-obligations-v2"
            || manifest["curriculum"]["status"] != "pending"
        {
            return Err("expected pending curriculum V2");
        }
        let lessons = manifest["curriculum"]["lessons"]
            .as_array()
            .ok_or("missing lessons")?;
        let mut selected = None;
        for lesson in lessons {
            for tab in lesson["codeTabs"].as_array().ok_or("missing tabs")? {
                let item = &tab["sourceItem"];
                if item["driver"]["target"] != "production_ranked_bounds_driver_v1" {
                    continue;
                }
                if item["kind"] != "source-driver"
                    || tab["sourceItemStatus"] != "contract-bound"
                    || item["driver"]
                        != json!({
                            "package": "rustc-codegen-fe2o3",
                            "target": "production_ranked_bounds_driver_v1",
                            "path": "crates/rustc-codegen-fe2o3/tests/production_ranked_bounds_driver_v1.rs",
                        })
                {
                    return Err("source driver identity differs");
                }
                let input = &item["compilerInput"];
                if input["packageManifest"]
                    != "crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/Cargo.toml"
                    || input["defaultFeatures"] != true
                    || input["sourcePaths"]
                        != json!([
                            "crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/src/lib.rs"
                        ])
                    || input["cargoTarget"]
                        != json!({
                            "kind": "lib", "name": "fe2o3_production_ranked_bounds_fixture",
                            "sourcePath": "src/lib.rs",
                        })
                {
                    return Err("source driver compiler input differs");
                }
                for row in item["cases"].as_array().ok_or("missing cases")? {
                    if row["kernelSymbol"] == self.kernel_symbol {
                        if selected.replace(row).is_some() {
                            return Err("duplicate source case");
                        }
                    }
                }
            }
        }
        if selected != Some(&self.expected_row()) {
            return Err("independent source case differs from curriculum contract");
        }
        Ok(())
    }

    fn export_command(&self, output: &Path, target_dir: &Path) -> Command {
        let manifest: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../config/tutorial-kernel-manifest-v1.json"
        )))
        .expect("decode compiler-owned tutorial source contract");
        self.check_contract(&manifest)
            .expect("bind actual source test to tutorial contract");
        simulation_export_command_for_feature(
            self.target,
            output,
            target_dir,
            Some(self.bundle_version),
            self.feature,
        )
    }

    fn assert_kernel(&self, target: &str, module: &fe2o3_kernel_ir::Module) {
        assert!(
            self.refusal.is_none(),
            "a refusal is not an observed exported root"
        );
        assert_eq!(target, format!("{}:xnack-", self.target));
        let [kernel] = module.kernels.as_slice() else {
            panic!("tutorial export must contain exactly its selected kernel");
        };
        assert_eq!(kernel.id.as_str(), self.kernel_symbol);
    }
}

#[test]
fn tutorial_source_case_rejects_independent_selection_substitution() {
    let original: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../config/tutorial-kernel-manifest-v1.json"
    )))
    .unwrap();
    let case = TutorialSourceCaseV1 {
        test_function: "ordinary_rust_struct_argument_exports_exact_v4_components",
        feature: "aggregate_pair_struct",
        kernel_symbol: "aggregate_pair_struct",
        target: "gfx942",
        bundle_version: 4,
        displayed_fragment: 1,
        refusal: None,
    };
    assert_eq!(case.check_contract(&original), Ok(()));
    for (field, value) in [
        ("features", json!(["aggregate_pair_tuple"])),
        ("kernelSymbol", json!("aggregate_pair_tuple")),
        (
            "testFunction",
            json!("ordinary_recursive_aggregates_export_and_unsafe_shapes_fail_typed"),
        ),
        ("target", json!("gfx950")),
        ("displayedFragmentOrdinal", json!(0)),
        (
            "expectation",
            json!({"kind":"verified-bundle-export","bundleVersion":5}),
        ),
        (
            "expectation",
            json!({"kind":"rejected","bundleVersion":4,"diagnosticContains":"error","outputArtifact":"absent"}),
        ),
    ] {
        let mut altered = original.clone();
        let item = altered["curriculum"]["lessons"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|lesson| lesson["lessonId"] == "cpu-semantic-simulation")
            .unwrap()["codeTabs"][0]["sourceItem"]
            .as_object_mut()
            .unwrap();
        let row = item
            .get_mut("cases")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["kernelSymbol"] == "aggregate_pair_struct")
            .unwrap();
        row[field] = value;
        item.insert(
            "contractSha256".to_owned(),
            json!("recomputed-digests-cannot-change-the-test-expectation"),
        );
        assert!(
            case.check_contract(&altered).is_err(),
            "{field} substitution"
        );
    }
}
