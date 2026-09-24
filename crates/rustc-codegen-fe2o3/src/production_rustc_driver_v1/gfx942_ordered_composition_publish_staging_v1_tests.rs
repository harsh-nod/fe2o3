//! Task-owned standalone Cargo preparation. Never modifies a compiler candidate.
use super::*;
use std::collections::BTreeSet;

pub(super) const PACKAGE_NAME: &str = "fe2o3-ordered-composition-publish-fixture";
pub(super) const LIB_NAME: &str = "fe2o3_ordered_composition_publish_fixture";
pub(super) const LEAF: &str = "src/ordered_composition_publish_v1.rs";
const ROOT: &[u8] = b"#![no_std]\nmod ordered_composition_publish_v1;\n";
pub(super) const FEATURES: [&str; 5] = [
    "ordered-composition-publish-direct",
    "ordered-composition-publish-collision",
    "ordered-composition-publish-const",
    "ordered-composition-publish-local",
    "ordered-composition-publish-wrapper",
];
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct LockRow {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}
fn quoted(value: &str) -> Result<String, &'static str> {
    if value.len() > 4096 {
        return Err("lock string bound");
    }
    serde_json::from_str(value).map_err(|_| "generated lock requires plain quoted strings")
}
/// Only Cargo's current generated version4 plain package-array format. This is
/// not a general TOML parser; unknown keys/tables/escapes and duplicate keys fail.
fn lock_rows(bytes: &[u8]) -> Result<BTreeSet<LockRow>, &'static str> {
    if bytes.len() > 1024 * 1024 {
        return Err("lock byte bound");
    }
    let source = std::str::from_utf8(bytes).map_err(|_| "lock UTF8")?;
    let mut rows = BTreeSet::new();
    let mut current: Option<(
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = None;
    let mut dependencies = false;
    let mut saw_dependencies = false;
    let mut version = false;
    fn finish(
        current: &mut Option<(
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )>,
        rows: &mut BTreeSet<LockRow>,
    ) -> Result<(), &'static str> {
        if let Some((name, version, source, checksum)) = current.take() {
            let row = LockRow {
                name: name.ok_or("lock package name")?,
                version: version.ok_or("lock package version")?,
                source,
                checksum,
            };
            match row.source.as_deref() {
                None if row.checksum.is_none() => {}
                Some(s) if s.starts_with("registry+") && row.checksum.is_some() => {}
                Some(s) if s.starts_with("git+") && row.checksum.is_none() => {}
                _ => return Err("lock source/checksum pairing"),
            }
            if row.checksum.as_ref().is_some_and(|s| {
                s.len() != 64
                    || !s
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            }) {
                return Err("lock checksum shape");
            }
            if !rows.insert(row) || rows.len() > 4096 {
                return Err("lock duplicate or count");
            }
        }
        Ok(())
    }
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if dependencies {
            if line == "]" {
                dependencies = false;
                continue;
            }
            let item = line.strip_suffix(',').ok_or("lock dependency row")?;
            let _ = quoted(item)?;
            continue;
        }
        if line == "[[package]]" {
            finish(&mut current, &mut rows)?;
            current = Some((None, None, None, None));
            saw_dependencies = false;
            continue;
        }
        if current.is_none() {
            if line != "version = 4" || version {
                return Err("lock version/table");
            }
            version = true;
            continue;
        }
        if line == "dependencies = [" {
            if saw_dependencies {
                return Err("duplicate lock dependency list");
            }
            saw_dependencies = true;
            dependencies = true;
            continue;
        }
        let (key, value) = line.split_once(" = ").ok_or("lock key syntax")?;
        let row = current.as_mut().unwrap();
        let slot = match key {
            "name" => &mut row.0,
            "version" => &mut row.1,
            "source" => &mut row.2,
            "checksum" => &mut row.3,
            _ => return Err("unknown lock key"),
        };
        if slot.replace(quoted(value)?).is_some() {
            return Err("duplicate lock field");
        }
    }
    if !version || dependencies {
        return Err("unfinished lock");
    }
    finish(&mut current, &mut rows)?;
    if rows.is_empty() {
        return Err("empty lock");
    }
    Ok(rows)
}
pub(super) fn closure_matches(workspace: &[u8], staged: &[u8]) -> Result<usize, &'static str> {
    let base = lock_rows(workspace)?;
    let selected = lock_rows(staged)?;
    let mut own = 0;
    for row in &selected {
        if row.name == PACKAGE_NAME {
            if row.version != "0.1.0" || row.source.is_some() || row.checksum.is_some() {
                return Err("staged package lock identity");
            }
            own += 1;
        } else if !base.contains(row) {
            return Err("staged dependency version/source/checksum differs");
        }
    }
    if own != 1 {
        return Err("staged package lock count");
    }
    Ok(selected.len() - 1)
}
pub(super) fn fixture_source() -> Vec<u8> {
    let expected = include_bytes!(
        "../../tests/fixtures/production-extraction-device/src/ordered_composition_publish_v1.rs"
    );
    let actual = read_bounded(&super::super::fixture().join(LEAF), 64 * 1024).unwrap();
    assert_eq!(actual, expected);
    actual
}
pub(super) fn package(root: &Path, name: &str, original: bool) -> PathBuf {
    assert!(matches!(
        name,
        "package-original"
            | "package-promoted-copy"
            | "package-promoted-preserve"
            | "package-promoted-edit"
    ));
    let package = root.join(name);
    fs::create_dir(&package).unwrap();
    fs::create_dir(package.join("src")).unwrap();
    let device = repository()
        .join("crates/fe2o3-device")
        .canonicalize()
        .unwrap();
    let literal = serde_json::to_string(device.to_str().unwrap()).unwrap();
    let host = repository()
        .join("crates/fe2o3-host")
        .canonicalize()
        .unwrap();
    let host_literal = serde_json::to_string(host.to_str().unwrap()).unwrap();
    let mut manifest = format!(
        "[package]\nname = \"{PACKAGE_NAME}\"\nversion = \"0.1.0\"\nedition = \"2024\"\nlicense = \"Apache-2.0 OR MIT\"\nrust-version = \"1.94\"\npublish = false\n\n[workspace]\n\n[dependencies]\nfe2o3-device = {{ path = {literal}, version = \"=0.1.0\" }}\n\n[target.'cfg(not(target_arch = \"amdgpu\"))'.dependencies]\nfe2o3-host = {{ path = {host_literal}, version = \"=0.1.0\" }}\n\n[features]\n"
    );
    for feature in FEATURES {
        manifest.push_str(&format!("{feature} = []\n"));
    }
    manifest.push_str(&format!(
        "\n[lib]\nname = \"{LIB_NAME}\"\npath = \"src/lib.rs\"\n"
    ));
    create(&package.join("Cargo.toml"), manifest.as_bytes(), 64 * 1024);
    create(&package.join("src/lib.rs"), ROOT, 4096);
    if original {
        create(&package.join(LEAF), &fixture_source(), 64 * 1024);
    }
    package
}
pub(super) fn metadata_feature(
    bytes: &[u8],
    package: &Path,
    feature: &str,
) -> Result<(), &'static str> {
    if !FEATURES.contains(&feature) {
        return Err("staged feature");
    }
    let metadata: Value = serde_json::from_slice(bytes).map_err(|_| "staged metadata JSON")?;
    let packages = metadata["packages"].as_array().ok_or("staged packages")?;
    let matches = packages
        .iter()
        .filter(|p| {
            p["manifest_path"].as_str().map(Path::new) == Some(package.join("Cargo.toml").as_path())
        })
        .collect::<Vec<_>>();
    let [p] = matches.as_slice() else {
        return Err("staged metadata package count");
    };
    if p["name"] != PACKAGE_NAME || p["version"] != "0.1.0" || p["features"][feature] != json!([]) {
        return Err("actual staged feature/package differs");
    }
    Ok(())
}
/// Exact workspace predecessor bytes, not an independently synthesized lock.
fn workspace_seed(workspace: &[u8]) -> Result<&[u8], &'static str> {
    let rows = lock_rows(workspace)?;
    if rows.iter().any(|r| r.name == PACKAGE_NAME) {
        return Err("workspace already contains staged root");
    }
    let device = rows
        .iter()
        .filter(|r| r.name == "fe2o3-device")
        .collect::<Vec<_>>();
    let [device] = device.as_slice() else {
        return Err("workspace device row count");
    };
    if device.version != "0.1.0" || device.source.is_some() || device.checksum.is_some() {
        return Err("workspace device identity");
    }
    Ok(workspace)
}
pub(super) fn prepare(root: &Path, name: &str, sysroot: &Path, started: std::time::Instant) {
    let package = root.join(name);
    let prep = root.join(format!("{name}.preparation"));
    fs::create_dir(&prep).unwrap();
    create(
        &prep.join("sysroot.stdout"),
        format!("{}\n", sysroot.display()).as_bytes(),
        4096,
    );
    let workspace = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    let seed = workspace_seed(&workspace).unwrap();
    create(&prep.join("workspace-lock.seed"), seed, 1024 * 1024);
    create(&package.join("Cargo.lock"), seed, 1024 * 1024);
    let manifest = read_bounded(&package.join("Cargo.toml"), 64 * 1024).unwrap();
    let source_root = read_bounded(&package.join("src/lib.rs"), 4096).unwrap();
    let source_leaf = package.join(LEAF);
    let leaf_before = source_leaf
        .exists()
        .then(|| read_bounded(&source_leaf, 72 * 1024).unwrap());
    timely(started.elapsed(), 1200).unwrap();
    // One explicit offline lock reconciliation for the actual new root. Unlike
    // generate-lockfile, this starts with genuine previously selected versions.
    // Cargo is still allowed to refuse; changed selected tuples never pass.
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    let resolution = checked(
        sanitized(&mut cargo)
            .current_dir(&package)
            .args([
                "metadata",
                "--offline",
                "--format-version=1",
                "--manifest-path",
            ])
            .arg(package.join("Cargo.toml")),
        &prep,
        "resolve",
        None,
    );
    timely(started.elapsed(), 1200).unwrap();
    let lock = read_bounded(&package.join("Cargo.lock"), 1024 * 1024).unwrap();
    let selected = closure_matches(&workspace, &lock).unwrap();
    for feature in FEATURES {
        metadata_feature(&resolution, &package, feature).unwrap();
    }
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    let metadata = checked(
        sanitized(&mut cargo)
            .current_dir(&package)
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--format-version=1",
                "--manifest-path",
            ])
            .arg(package.join("Cargo.toml")),
        &prep,
        "metadata",
        None,
    );
    for feature in FEATURES {
        metadata_feature(&metadata, &package, feature).unwrap();
    }
    assert_eq!(
        metadata, resolution,
        "locked metadata changed actual standalone package observation"
    );
    assert_eq!(
        read_bounded(&package.join("Cargo.lock"), 1024 * 1024).unwrap(),
        lock
    );
    assert_eq!(
        read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap(),
        workspace
    );
    assert_eq!(
        read_bounded(&package.join("Cargo.toml"), 64 * 1024).unwrap(),
        manifest
    );
    assert_eq!(
        read_bounded(&package.join("src/lib.rs"), 4096).unwrap(),
        source_root
    );
    assert_eq!(
        source_leaf
            .exists()
            .then(|| read_bounded(&source_leaf, 72 * 1024).unwrap()),
        leaf_before
    );
    timely(started.elapsed(), 1200).unwrap();
    publish_json(
        &prep,
        "preparation.json",
        &json!({
            "package":name,"selected_dependency_count":selected,"workspace_lock_sha256":digest(&workspace),
            "seed_lock_sha256":digest(seed),"resolved_lock_sha256":digest(&lock),
            "staged_lock_sha256":digest(&lock),"metadata_sha256":digest(&metadata),
            "resolution_metadata_sha256":digest(&resolution),"locked_metadata_identical":true,
            "exact_workspace_lock_seed":true,"explicit_offline_metadata_reconciliations":1,
            "selected_dependency_tuples_unchanged":true,"staged_sources_unchanged":true,
            "actual_standalone_metadata":true,"network_requested":false,
        }),
    );
    timely(started.elapsed(), 1200).unwrap();
}
#[test]
fn staged_lock_refuses_version_checksum_and_unreviewed_source_changes() {
    let base=b"version = 4\n[[package]]\nname = \"dep\"\nversion = \"1\"\nsource = \"registry+https://example.invalid\"\nchecksum = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n";
    let tail = format!("\n[[package]]\nname = \"{PACKAGE_NAME}\"\nversion = \"0.1.0\"\n");
    let good = format!("{}{tail}", std::str::from_utf8(base).unwrap());
    assert_eq!(closure_matches(base, good.as_bytes()).unwrap(), 1);
    for bad in [
        good.replace("version = \"1\"", "version = \"2\""),
        good.replace("aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb"),
        good.replace(
            "registry+https://example.invalid",
            "registry+https://other.invalid",
        ),
        good.replace("version = 4", "version = 3"),
        good.replace("name = \"dep\"", "name = \"dep\"\nname = \"dep\""),
        good.replace("[[package]]", "[package]"),
    ] {
        assert!(closure_matches(base, bad.as_bytes()).is_err());
    }
}

#[test]
fn workspace_seed_is_exact_and_refuses_forged_standalone_root_or_device() {
    let workspace=b"# retained workspace bytes\nversion = 4\n[[package]]\nname = \"fe2o3-device\"\nversion = \"0.1.0\"\n";
    assert_eq!(workspace_seed(workspace).unwrap(), workspace);
    let text = std::str::from_utf8(workspace).unwrap();
    let own = format!("{text}\n[[package]]\nname = \"{PACKAGE_NAME}\"\nversion = \"0.1.0\"\n");
    assert_eq!(
        workspace_seed(own.as_bytes()),
        Err("workspace already contains staged root")
    );
    assert_eq!(
        workspace_seed(text.replace("fe2o3-device", "unrelated").as_bytes()),
        Err("workspace device row count")
    );
    assert_eq!(
        workspace_seed(text.replace("0.1.0", "0.2.0").as_bytes()),
        Err("workspace device identity")
    );
    let foreign = format!("{text}source = \"git+https://example.invalid/device#abc\"\n");
    assert_eq!(
        workspace_seed(foreign.as_bytes()),
        Err("workspace device identity")
    );
}
