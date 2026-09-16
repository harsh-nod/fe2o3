#[derive(Clone, Copy)]
struct TutorialSourceInputV1<'a> {
    lesson_id: &'a str,
    tab_ordinal: usize,
    source_path: &'a str,
    lib_source_path: &'a str,
    export: SimulationExportPackageV1<'a>,
}

const CPU_TUTORIAL_SOURCE_INPUT_V1: TutorialSourceInputV1<'static> = TutorialSourceInputV1 {
    lesson_id: "cpu-semantic-simulation",
    tab_ordinal: 0,
    source_path: "crates/rustc-codegen-fe2o3/tests/fixtures/production-ranked-bounds-device/src/lib.rs",
    lib_source_path: "src/lib.rs",
    export: RANKED_BOUNDS_EXPORT_PACKAGE_V1,
};

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
        self.check_contract_for_input(manifest, CPU_TUTORIAL_SOURCE_INPUT_V1)
    }

    fn check_contract_for_input(
        &self,
        manifest: &Value,
        source: TutorialSourceInputV1<'_>,
    ) -> Result<(), &'static str> {
        if manifest["curriculum"]["schema"] != "fe2o3-tutorial-curriculum-obligations-v2"
            || manifest["curriculum"]["status"] != "pending"
        {
            return Err("expected pending curriculum V2");
        }
        let lessons = manifest["curriculum"]["lessons"]
            .as_array()
            .ok_or("missing lessons")?;
        let mut matching = lessons
            .iter()
            .filter(|lesson| lesson["lessonId"] == source.lesson_id);
        let lesson = matching.next().ok_or("missing source lesson")?;
        if matching.next().is_some() || lesson["role"] != "executable" {
            return Err("source lesson identity differs");
        }
        let tabs = lesson["codeTabs"].as_array().ok_or("missing tabs")?;
        let tab = tabs.get(source.tab_ordinal).ok_or("missing source tab")?;
        let ordinal = json!(source.tab_ordinal);
        if tab["ordinal"] != ordinal
            || tabs.iter().filter(|tab| tab["ordinal"] == ordinal).count() != 1
            || tab["kind"] != "kernel"
            || tab["language"] != "rust"
            || tab["sourcePath"] != source.source_path
        {
            return Err("source tab identity differs");
        }
        let item = &tab["sourceItem"];
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
        if input["packageManifest"] != source.export.manifest_path
            || input["defaultFeatures"] != source.export.default_features
            || input["sourcePaths"] != json!([source.source_path])
            || input["cargoTarget"]
                != json!({
                    "kind": "lib", "name": source.export.rustc_crate,
                    "sourcePath": source.lib_source_path,
                })
        {
            return Err("source driver compiler input differs");
        }
        let mut selected = None;
        for row in item["cases"].as_array().ok_or("missing cases")? {
            if row["kernelSymbol"] == self.kernel_symbol {
                if selected.replace(row).is_some() {
                    return Err("duplicate source case");
                }
            }
        }
        if selected != Some(&self.expected_row()) {
            return Err("independent source case differs from curriculum contract");
        }
        Ok(())
    }

    fn export_command(&self, output: &Path, target_dir: &Path) -> Command {
        self.export_command_for_input(CPU_TUTORIAL_SOURCE_INPUT_V1, output, target_dir)
    }

    fn export_command_for_input(
        &self,
        source: TutorialSourceInputV1<'_>,
        output: &Path,
        target_dir: &Path,
    ) -> Command {
        let manifest: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../config/tutorial-kernel-manifest-v1.json"
        )))
        .expect("decode compiler-owned tutorial source contract");
        self.check_contract_for_input(&manifest, source)
            .expect("bind actual source test to tutorial contract");
        bind_tutorial_source_package(source)
            .expect("bind tutorial Cargo package to its manifest and library source");
        simulation_export_command_for_package_with_exporter(
            Path::new(env!("CARGO_BIN_EXE_fe2o3-export-sim")),
            source.export,
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

fn tutorial_source_metadata_command(manifest: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace())
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(manifest);
    for variable in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_BUILD_TARGET",
        "RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(variable);
    }
    command
}

fn bind_tutorial_source_package(source: TutorialSourceInputV1<'_>) -> Result<(), String> {
    let canonicalize = |path: &Path| {
        std::fs::canonicalize(path).map_err(|error| {
            format!(
                "canonicalize tutorial source path {}: {error}",
                path.display()
            )
        })
    };
    let manifest = canonicalize(&workspace().join(source.export.manifest_path))?;
    let library = canonicalize(
        &manifest
            .parent()
            .ok_or("source manifest has no parent")?
            .join(source.lib_source_path),
    )?;
    let metadata = tutorial_source_metadata_output(tutorial_source_metadata_command(&manifest))?;
    check_tutorial_source_package_metadata(&metadata, source, &manifest, &library, canonicalize)
}

fn check_tutorial_source_package_metadata(
    metadata: &Value,
    source: TutorialSourceInputV1<'_>,
    manifest: &Path,
    library: &Path,
    mut canonicalize: impl FnMut(&Path) -> Result<PathBuf, String>,
) -> Result<(), String> {
    if metadata["version"] != 1 {
        return Err("unsupported Cargo metadata version".into());
    }
    let packages = metadata["packages"]
        .as_array()
        .ok_or("missing Cargo packages")?;
    let mut selected = None;
    for package in packages {
        let name = package["name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .ok_or("invalid Cargo package name")?;
        let path = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("invalid Cargo package manifest path")?,
        );
        if !path.is_absolute() {
            return Err("Cargo package manifest path must be absolute".into());
        }
        let path = canonicalize(path)?;
        if path == manifest || name == source.export.package {
            if path != manifest
                || name != source.export.package
                || selected.replace(package).is_some()
            {
                return Err(
                    "Cargo package name/manifest identity is substituted or duplicated".into(),
                );
            }
        }
    }
    let selected = selected.ok_or("bound Cargo package is absent")?;
    let targets = selected["targets"]
        .as_array()
        .ok_or("missing Cargo package targets")?;
    let mut library_count = 0;
    for target in targets {
        let kind = target["kind"]
            .as_array()
            .ok_or("invalid Cargo target kind")?;
        if kind.is_empty() || kind.iter().any(|kind| kind.as_str().is_none()) {
            return Err("invalid Cargo target kind member".into());
        }
        let name = target["name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .ok_or("invalid Cargo target name")?;
        let path = Path::new(
            target["src_path"]
                .as_str()
                .ok_or("invalid Cargo target source path")?,
        );
        if !path.is_absolute() {
            return Err("Cargo target source path must be absolute".into());
        }
        if kind.iter().any(|kind| kind == "lib") {
            library_count += 1;
            if kind.len() != 1
                || name != source.export.rustc_crate
                || canonicalize(path)? != library
            {
                return Err("Cargo library target identity differs".into());
            }
        }
    }
    if library_count != 1 {
        return Err("expected exactly one bound Cargo library target".into());
    }
    Ok(())
}

fn tutorial_source_read_bounded(
    mut pipe: impl std::io::Read,
    limit: usize,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(limit)
        .map_err(|error| format!("reserve Cargo metadata output: {error}"))?;
    let mut buffer = [0_u8; 8192];
    let mut overflow = false;
    loop {
        let count = match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("read Cargo metadata output: {error}")),
        };
        let retained = limit.saturating_sub(bytes.len()).min(count);
        bytes.extend_from_slice(&buffer[..retained]);
        overflow |= retained != count;
    }
    if overflow {
        return Err(format!("Cargo metadata output exceeds {limit} bytes"));
    }
    Ok(bytes)
}

fn tutorial_source_metadata_output(mut command: Command) -> Result<Value, String> {
    // Match the production Cargo metadata capture bounds. The outer source-test
    // runner supplies the deadline; drain both pipes even after an overflow.
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("spawn Cargo metadata: {error}"))?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("missing Cargo metadata output pipes".into());
    };
    std::thread::scope(|scope| {
        let stdout = match std::thread::Builder::new().spawn_scoped(scope, move || {
            tutorial_source_read_bounded(stdout, 32 * 1024 * 1024)
        }) {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("spawn Cargo metadata stdout reader: {error}"));
            }
        };
        let stderr = match std::thread::Builder::new().spawn_scoped(scope, move || {
            tutorial_source_read_bounded(stderr, 1024 * 1024)
        }) {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout.join();
                return Err(format!("spawn Cargo metadata stderr reader: {error}"));
            }
        };
        let status = child.wait();
        if status.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let stdout = stdout
            .join()
            .map_err(|_| "Cargo metadata stdout reader panicked".to_owned());
        let stderr = stderr
            .join()
            .map_err(|_| "Cargo metadata stderr reader panicked".to_owned());
        let status = status.map_err(|error| format!("wait for Cargo metadata: {error}"))?;
        let stdout = stdout??;
        let stderr = stderr??;
        if !status.success() {
            return Err(format!(
                "Cargo metadata failed ({status}): {}",
                String::from_utf8_lossy(&stderr)
            ));
        }
        serde_json::from_slice(&stdout).map_err(|error| format!("decode Cargo metadata: {error}"))
    })
}

#[test]
fn tutorial_source_metadata_binds_package_manifest_and_library_identity() {
    let source = CPU_TUTORIAL_SOURCE_INPUT_V1;
    let manifest = Path::new("/workspace/package-a/Cargo.toml");
    let library = Path::new("/workspace/package-a/src/lib.rs");
    let original = json!({"version": 1, "packages": [
        {"name": source.export.package, "manifest_path": manifest,
         "targets": [{"kind": ["lib"], "name": source.export.rustc_crate, "src_path": library}]},
        {"name": "package-b", "manifest_path": "/workspace/package-b/Cargo.toml",
         "targets": [{"kind": ["lib"], "name": source.export.rustc_crate,
                      "src_path": "/workspace/package-b/src/lib.rs"}]},
    ]});
    let check = |metadata: &Value, input| {
        check_tutorial_source_package_metadata(metadata, input, manifest, library, |path| {
            Ok(path.to_owned())
        })
    };
    assert_eq!(check(&original, source), Ok(()));
    assert!(
        check(
            &original,
            TutorialSourceInputV1 {
                export: SimulationExportPackageV1 {
                    package: "package-b",
                    ..source.export
                },
                ..source
            }
        )
        .is_err(),
        "same library name cannot substitute the selected package"
    );
    for (path, value) in [
        ("/version", json!(2)),
        ("/packages", Value::Null),
        ("/packages/0/name", json!("package-b")),
        ("/packages/0/name", Value::Null),
        (
            "/packages/0/manifest_path",
            json!("/workspace/package-b/Cargo.toml"),
        ),
        ("/packages/0/manifest_path", json!("package-a/Cargo.toml")),
        ("/packages/0/targets", Value::Null),
        ("/packages/0/targets/0/kind", json!(["bin"])),
        ("/packages/0/targets/0/kind", json!(["lib", 1])),
        ("/packages/0/targets/0/name", json!("wrong_lib")),
        (
            "/packages/0/targets/0/src_path",
            json!("/workspace/package-b/src/lib.rs"),
        ),
        ("/packages/0/targets/0/src_path", Value::Null),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(path).unwrap() = value;
        assert!(check(&changed, source).is_err(), "{path}");
    }
    for path in ["/packages", "/packages/0/targets"] {
        let mut changed = original.clone();
        let records = changed.pointer_mut(path).unwrap().as_array_mut().unwrap();
        records.push(records[0].clone());
        assert!(check(&changed, source).is_err(), "duplicate {path}");
    }
    let mut missing = original.clone();
    missing["packages"].as_array_mut().unwrap().remove(0);
    assert!(check(&missing, source).is_err());
    assert!(
        check_tutorial_source_package_metadata(&original, source, manifest, library, |_| Err(
            "canonicalization failed".into()
        ),)
        .is_err()
    );
    let mut aliased = original.clone();
    aliased["packages"][0]["manifest_path"] = json!("/alias/Cargo.toml");
    assert_eq!(
        check_tutorial_source_package_metadata(&aliased, source, manifest, library, |path| Ok(
            if path == Path::new("/alias/Cargo.toml") {
                manifest
            } else {
                path
            }
            .to_owned()
        ),),
        Ok(())
    );
}

#[test]
fn tutorial_source_metadata_command_is_locked_offline_and_sanitized() {
    let command = tutorial_source_metadata_command(Path::new("/workspace/package-a/Cargo.toml"));
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "metadata",
            "--locked",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
            "/workspace/package-a/Cargo.toml",
        ]
        .iter()
        .map(std::ffi::OsStr::new)
        .collect::<Vec<_>>()
    );
    for variable in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_BUILD_TARGET",
        "RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    ] {
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == variable)
                .unwrap()
                .1,
            None
        );
    }
}

#[test]
fn tutorial_source_metadata_capture_drains_overflow_and_propagates_reads() {
    let bytes = vec![42; 20_000];
    for limit in [0, 1, 8_192, 19_999] {
        let mut pipe = std::io::Cursor::new(&bytes);
        assert!(
            tutorial_source_read_bounded(&mut pipe, limit)
                .unwrap_err()
                .contains("exceeds")
        );
        assert_eq!(pipe.position(), bytes.len() as u64);
    }
    assert_eq!(
        tutorial_source_read_bounded(bytes.as_slice(), bytes.len()).unwrap(),
        bytes
    );
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("read-failure-sentinel"))
        }
    }
    assert!(
        tutorial_source_read_bounded(Broken, 1)
            .unwrap_err()
            .contains("read-failure-sentinel")
    );
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

fn tutorial_source_identity_test_case() -> TutorialSourceCaseV1<'static> {
    TutorialSourceCaseV1 {
        test_function: "source_export_test",
        feature: "kernel-first",
        kernel_symbol: "shared_symbol",
        target: "gfx942",
        bundle_version: 5,
        displayed_fragment: 0,
        refusal: None,
    }
}

fn tutorial_source_identity_test_document(case: &TutorialSourceCaseV1<'_>) -> Value {
    let source = CPU_TUTORIAL_SOURCE_INPUT_V1;
    json!({"curriculum": {
        "schema": "fe2o3-tutorial-curriculum-obligations-v2",
        "status": "pending",
        "lessons": [{
            "lessonId": source.lesson_id, "role": "executable",
            "codeTabs": [{
                "ordinal": 0, "kind": "kernel", "language": "rust",
                "sourcePath": source.source_path, "sourceItemStatus": "contract-bound",
                "sourceItem": {
                    "kind": "source-driver",
                    "driver": {
                        "package": "rustc-codegen-fe2o3",
                        "target": "production_ranked_bounds_driver_v1",
                        "path": "crates/rustc-codegen-fe2o3/tests/production_ranked_bounds_driver_v1.rs",
                    },
                    "compilerInput": {
                        "packageManifest": source.export.manifest_path,
                        "defaultFeatures": source.export.default_features,
                        "sourcePaths": [source.source_path],
                        "cargoTarget": {
                            "kind": "lib", "name": source.export.rustc_crate,
                            "sourcePath": source.lib_source_path,
                        },
                    },
                    "cases": [case.expected_row()],
                },
            }],
        }],
    }})
}

#[test]
fn tutorial_source_case_scopes_shared_symbols_to_exact_lesson_and_tab() {
    let first = tutorial_source_identity_test_case();
    let second = TutorialSourceCaseV1 {
        feature: "kernel-second",
        ..tutorial_source_identity_test_case()
    };
    let source = TutorialSourceInputV1 {
        tab_ordinal: 1,
        source_path: "examples/other/src/kernel.rs",
        export: SimulationExportPackageV1 {
            manifest_path: "examples/other/Cargo.toml",
            package: "other-package",
            rustc_crate: "other_lib",
            default_features: false,
        },
        ..CPU_TUTORIAL_SOURCE_INPUT_V1
    };
    let mut document = tutorial_source_identity_test_document(&first);
    let mut other = document["curriculum"]["lessons"][0]["codeTabs"][0].clone();
    other["ordinal"] = json!(1);
    other["sourcePath"] = json!(source.source_path);
    let item = &mut other["sourceItem"];
    item["compilerInput"]["packageManifest"] = json!(source.export.manifest_path);
    item["compilerInput"]["defaultFeatures"] = json!(false);
    item["compilerInput"]["sourcePaths"] = json!([source.source_path]);
    item["compilerInput"]["cargoTarget"]["name"] = json!(source.export.rustc_crate);
    item["cases"] = json!([second.expected_row()]);
    document["curriculum"]["lessons"][0]["codeTabs"]
        .as_array_mut()
        .unwrap()
        .push(other);
    assert_eq!(first.check_contract(&document), Ok(()));
    assert_eq!(second.check_contract_for_input(&document, source), Ok(()));
    assert!(second.check_contract(&document).is_err());
    assert!(first.check_contract_for_input(&document, source).is_err());

    let mut other_lesson = document["curriculum"]["lessons"][0].clone();
    other_lesson["lessonId"] = json!("another-lesson");
    document["curriculum"]["lessons"]
        .as_array_mut()
        .unwrap()
        .push(other_lesson);
    assert_eq!(first.check_contract(&document), Ok(()));
    assert_eq!(
        second.check_contract_for_input(
            &document,
            TutorialSourceInputV1 {
                lesson_id: "another-lesson",
                ..source
            }
        ),
        Ok(()),
    );
    document["curriculum"]["lessons"][0]["codeTabs"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1);
    assert!(first.check_contract(&document).is_err());
    assert!(second.check_contract_for_input(&document, source).is_err());
}

#[test]
fn tutorial_source_case_rejects_input_and_location_substitution() {
    let case = tutorial_source_identity_test_case();
    let document = tutorial_source_identity_test_document(&case);
    assert_eq!(case.check_contract(&document), Ok(()));
    for (path, value) in [
        ("/lessonId", json!("other-lesson")),
        ("/role", json!("conceptual")),
        ("/codeTabs/0/ordinal", json!(1)),
        (
            "/codeTabs/0/sourcePath",
            json!("examples/other/src/kernel.rs"),
        ),
        ("/codeTabs/0/kind", json!("reference")),
        ("/codeTabs/0/language", json!("text")),
        ("/codeTabs/0/sourceItemStatus", json!("pending")),
        (
            "/codeTabs/0/sourceItem/driver/package",
            json!("other-package"),
        ),
        (
            "/codeTabs/0/sourceItem/driver/target",
            json!("other_driver"),
        ),
        (
            "/codeTabs/0/sourceItem/driver/path",
            json!("crates/other/tests/other.rs"),
        ),
        (
            "/codeTabs/0/sourceItem/compilerInput/packageManifest",
            json!("examples/other/Cargo.toml"),
        ),
        (
            "/codeTabs/0/sourceItem/compilerInput/defaultFeatures",
            json!(false),
        ),
        (
            "/codeTabs/0/sourceItem/compilerInput/sourcePaths",
            json!(["examples/other/src/kernel.rs"]),
        ),
        (
            "/codeTabs/0/sourceItem/compilerInput/cargoTarget/kind",
            json!("bin"),
        ),
        (
            "/codeTabs/0/sourceItem/compilerInput/cargoTarget/name",
            json!("other_lib"),
        ),
        (
            "/codeTabs/0/sourceItem/compilerInput/cargoTarget/sourcePath",
            json!("src/other.rs"),
        ),
        (
            "/codeTabs/0/sourceItem/cases/0/features",
            json!(["kernel-second"]),
        ),
    ] {
        let mut changed = document.clone();
        *changed["curriculum"]["lessons"][0]
            .pointer_mut(path)
            .unwrap() = value;
        assert!(case.check_contract(&changed).is_err(), "{path}");
    }
}

#[test]
fn tutorial_source_case_rejects_duplicate_selected_identities() {
    let case = tutorial_source_identity_test_case();
    let document = tutorial_source_identity_test_document(&case);
    for path in [
        "/curriculum/lessons",
        "/curriculum/lessons/0/codeTabs",
        "/curriculum/lessons/0/codeTabs/0/sourceItem/cases",
    ] {
        let mut changed = document.clone();
        let values = changed.pointer_mut(path).unwrap().as_array_mut().unwrap();
        values.push(values[0].clone());
        assert!(case.check_contract(&changed).is_err(), "{path}");
    }
}

#[test]
fn tutorial_source_exporter_uses_independent_package_selection() {
    let build = |source| {
        simulation_export_command_for_package_with_exporter(
            Path::new("unused-exporter"),
            source,
            "gfx942",
            Path::new("out.fe2sim"),
            Path::new("build-target"),
            Some(5),
            "kernel-first",
        )
    };
    for source in [
        RANKED_BOUNDS_EXPORT_PACKAGE_V1,
        SimulationExportPackageV1 {
            manifest_path: "examples/other/Cargo.toml",
            package: "other-package",
            rustc_crate: "other_lib",
            default_features: false,
        },
    ] {
        let command = build(source);
        let mut expected = vec![
            "--crate",
            source.rustc_crate,
            "--output",
            "out.fe2sim",
            "--target",
            "gfx942",
            "--bundle-version",
            "5",
            "--target-dir",
            "build-target",
            "--",
            "--manifest-path",
            source.manifest_path,
            "--package",
            source.package,
            "--features",
            "kernel-first",
            "--lib",
        ];
        if !source.default_features {
            expected.push("--no-default-features");
        }
        assert_eq!(command.get_program(), "unused-exporter");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            expected
                .iter()
                .map(std::ffi::OsStr::new)
                .collect::<Vec<_>>(),
        );
        for wrapper in [
            "RUSTC_WRAPPER",
            "CARGO_BUILD_RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        ] {
            assert_eq!(
                command
                    .get_envs()
                    .find(|(name, _)| *name == wrapper)
                    .unwrap()
                    .1,
                Some(std::ffi::OsStr::new(
                    "/fe2o3-poisoned-caller-wrapper-must-not-run"
                )),
            );
        }
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == "RUSTFLAGS")
                .unwrap()
                .1,
            None,
        );
    }
    let wrapper = simulation_export_command_for_feature_with_exporter(
        Path::new("unused-exporter"),
        "gfx942",
        Path::new("out.fe2sim"),
        Path::new("build-target"),
        Some(5),
        "kernel-first",
    );
    assert_eq!(
        wrapper.get_args().collect::<Vec<_>>(),
        build(RANKED_BOUNDS_EXPORT_PACKAGE_V1)
            .get_args()
            .collect::<Vec<_>>(),
    );
}
