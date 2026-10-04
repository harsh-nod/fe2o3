// Cargo may check a package's library before its selected bin. This context
// permits only ordinary rustc macro binding, never extraction or publication.
#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrimaryPackage {
    manifest: PathBuf,
    name: String,
    version: String,
    manifest_sha256: [u8; 32],
    libraries: Vec<LibraryTarget>,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryTarget {
    name: String,
    source: PathBuf,
    crate_types: Vec<String>,
}

const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

fn canonical_file(path: &std::path::Path) -> Result<PathBuf, String> {
    let path = std::fs::canonicalize(path).map_err(|error| format!("canonicalize Cargo input: {error}"))?;
    if !path.is_file() { return Err("Cargo input must be a regular file".into()); }
    Ok(path)
}

fn manifest_digest(path: &std::path::Path) -> Result<[u8; 32], String> {
    use std::io::Read as _;
    use sha2::Digest as _;
    let file = std::fs::File::open(path).map_err(|error| format!("open Cargo manifest: {error}"))?;
    if !file.metadata().map_err(|error| error.to_string())?.is_file() {
        return Err("Cargo manifest must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES { return Err("Cargo manifest exceeds byte bound".into()); }
    Ok(sha2::Sha256::digest(&bytes).into())
}

fn cargo_manifest_argument(args: &[OsString]) -> Result<Option<OsString>, String> {
    let mut result = None;
    let mut index = 0;
    while index < args.len() {
        let value = if args[index] == "--manifest-path" {
            index += 1;
            Some(args.get(index).ok_or("missing Cargo manifest path")?.clone())
        } else {
            args[index].to_str().and_then(|arg| arg.strip_prefix("--manifest-path=")).map(OsString::from)
        };
        if let Some(value) = value
            && (value.is_empty() || result.replace(value).is_some())
        {
            return Err("Cargo manifest path is empty or repeated".into());
        }
        index += 1;
    }
    Ok(result)
}

impl Selection {
    pub(super) fn with_primary_package(
        mut self,
        cargo: &OsStr,
        cargo_args: &[OsString],
    ) -> Result<Self, String> {
        use std::io::Read as _;
        use std::process::{Command, Stdio};
        let mut command = Command::new(cargo);
        command.args(["metadata", "--format-version=1", "--no-deps", "--locked", "--offline"])
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::inherit());
        if let Some(manifest) = cargo_manifest_argument(cargo_args)? {
            command.arg("--manifest-path").arg(manifest);
        }
        let mut child = command.spawn().map_err(|error| format!("start Cargo target metadata: {error}"))?;
        let mut bytes = Vec::new();
        let Some(stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cargo metadata stdout is missing".into());
        };
        let read = stdout.take(MAX_METADATA_BYTES + 1).read_to_end(&mut bytes);
        if read.is_err() || bytes.len() as u64 > MAX_METADATA_BYTES {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cargo target metadata could not be read within its byte bound".into());
        }
        let status = child.wait().map_err(|error| format!("wait for Cargo target metadata: {error}"))?;
        if !status.success() { return Err(format!("Cargo target metadata failed with {status}")); }
        let metadata: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("invalid Cargo target metadata: {error}"))?;
        self.bind_primary_metadata(&metadata)?;
        Ok(self)
    }

    fn bind_primary_metadata(&mut self, metadata: &serde_json::Value) -> Result<(), String> {
        let packages = metadata["packages"].as_array().filter(|rows| rows.len() <= 4096)
            .ok_or("missing or oversized Cargo package roster")?;
        let mut selected = None;
        for package in packages {
            let targets = package["targets"].as_array().filter(|rows| rows.len() <= 256)
                .ok_or("missing or oversized Cargo target roster")?;
            for target in targets {
                if target["name"] != self.name || target["kind"] != serde_json::json!(["bin"]) {
                    continue;
                }
                let source = target["src_path"].as_str().ok_or("Cargo bin source is missing")?;
                if canonical_file(std::path::Path::new(source))? != self.source { continue; }
                if selected.replace(package).is_some() { return Err("selected Cargo bin is ambiguous".into()); }
            }
        }
        let package = selected.ok_or("selected bin is absent from actual Cargo metadata")?;
        let field = |name: &str| package[name].as_str().filter(|value| !value.is_empty() && value.len() <= 4096)
            .ok_or_else(|| format!("invalid Cargo package {name}"));
        let manifest = canonical_file(std::path::Path::new(field("manifest_path")?))?;
        let mut libraries = Vec::new();
        for target in package["targets"].as_array().ok_or("Cargo targets disappeared")? {
            let kinds = target["kind"].as_array().ok_or("invalid Cargo target kinds")?;
            if kinds.is_empty() || !kinds.iter().all(|kind| kind.as_str().is_some_and(library_kind)) { continue; }
            let name = target["name"].as_str().filter(|value| !value.is_empty() && value.len() <= 256)
                .ok_or("invalid Cargo library name")?.to_owned();
            let source = canonical_file(std::path::Path::new(target["src_path"].as_str().ok_or("missing Cargo library source")?))?;
            let crate_types = target["crate_types"].as_array().filter(|rows| !rows.is_empty() && rows.len() <= 5)
                .ok_or("invalid Cargo library crate types")?.iter().map(|value| {
                    value.as_str().filter(|kind| library_kind(kind)).map(str::to_owned).ok_or("invalid Cargo library crate type")
                }).collect::<Result<Vec<_>, _>>()?;
            libraries.push(LibraryTarget { name, source, crate_types });
        }
        self.primary = Some(PrimaryPackage {
            manifest_sha256: manifest_digest(&manifest)?,
            manifest,
            name: field("name")?.to_owned(),
            version: field("version")?.to_owned(),
            libraries,
        });
        self.validate_primary()
    }

    fn validate_primary(&self) -> Result<(), String> {
        let Some(package) = &self.primary else { return Ok(()); };
        if canonical_file(&package.manifest)? != package.manifest
            || manifest_digest(&package.manifest)? != package.manifest_sha256
            || package.name.is_empty() || package.name.len() > 4096
            || package.version.is_empty() || package.version.len() > 4096
            || package.libraries.len() > 256
        { return Err("captured Cargo package identity changed".into()); }
        for library in &package.libraries {
            if library.name.is_empty() || library.name.len() > 256
                || canonical_file(&library.source)? != library.source
                || library.crate_types.is_empty() || library.crate_types.len() > 5
                || !library.crate_types.iter().all(|kind| library_kind(kind))
            { return Err("captured Cargo library identity changed".into()); }
        }
        Ok(())
    }

    pub(super) fn binding_only_identity(
        &self,
        compile: RustcCompileInvocationV2<'_>,
    ) -> Result<Option<fe2o3_rustc_invocation::PortablePackageIdentityV1>, String> {
        let Some(package) = &self.primary else { return Ok(None); };
        if std::env::var_os("CARGO_PRIMARY_PACKAGE").as_deref() != Some(OsStr::new("1")) {
            return Ok(None);
        }
        let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").ok_or("primary Cargo manifest directory is missing")?;
        let manifest = canonical_file(&PathBuf::from(manifest_dir).join("Cargo.toml"))?;
        if manifest != package.manifest { return Ok(None); }
        let identity = fe2o3_rustc_invocation::capture_cargo_package_identity_v1()
            .map_err(|error| error.to_string())?;
        if self.binding_only_matches(compile, Some(OsStr::new("1")), &manifest, &identity)? {
            Ok(Some(identity))
        } else { Ok(None) }
    }

    fn binding_only_matches(
        &self,
        compile: RustcCompileInvocationV2<'_>,
        primary: Option<&OsStr>,
        manifest: &std::path::Path,
        identity: &fe2o3_rustc_invocation::PortablePackageIdentityV1,
    ) -> Result<bool, String> {
        let Some(package) = &self.primary else { return Ok(false); };
        if primary != Some(OsStr::new("1")) || canonical_file(manifest)? != package.manifest {
            return Ok(false);
        }
        self.validate_primary()?;
        if identity.package_name() != package.name.as_str() || identity.package_version() != package.version.as_str()
            || identity.manifest_sha256() != &package.manifest_sha256
        { return Err("actual Cargo package differs from selected bin package".into()); }
        let Some(kind) = compile_crate_type(compile)? else { return Ok(false); };
        if !kind.split(',').all(library_kind) { return Ok(false); }
        let source = canonical_file(compile.source_path())?;
        Ok(package.libraries.iter().any(|library| {
            compile.crate_name() == library.name.replace('-', "_") && source == library.source
                && kind.split(',').eq(library.crate_types.iter().map(String::as_str))
        }))
    }
}

fn library_kind(kind: &str) -> bool {
    matches!(kind, "lib" | "rlib" | "dylib" | "cdylib" | "staticlib")
}
