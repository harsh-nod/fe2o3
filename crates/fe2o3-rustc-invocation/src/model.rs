use std::cmp::Ordering;

use fe2o3_amd_target::AmdTargetId;

use crate::ValidationError;

/// Maximum size of one complete encoded V1 descriptor.
pub const MAX_DESCRIPTOR_BYTES: usize = 256 * 1024;
/// Maximum byte length of a package, target, crate, feature, triple, or
/// environment-key name.
pub const MAX_NAME_BYTES: usize = 128;
/// Maximum byte length of a version or general text field.
pub const MAX_TEXT_BYTES: usize = 4096;
/// Maximum byte length of a canonical path.
pub const MAX_PATH_BYTES: usize = 4096;
/// Maximum byte length of one rustc argument.
pub const MAX_ARGUMENT_BYTES: usize = 4096;
/// Maximum byte length of one compile-environment value.
pub const MAX_ENVIRONMENT_VALUE_BYTES: usize = 4096;
/// Maximum number of rustc arguments in V1.
pub const MAX_RUSTC_ARGUMENTS: usize = 4096;
/// Maximum number of compile-environment entries in V1.
pub const MAX_COMPILE_ENVIRONMENT_ENTRIES: usize = 1024;
/// Maximum number of activated Cargo features in V1.
pub const MAX_FEATURES: usize = 1024;
pub(crate) const MAX_CRATE_TYPES: usize = 7;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Name(String);

impl Name {
    pub(crate) fn new(
        value: impl Into<String>,
        field: &'static str,
    ) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_nonempty_text(&value, field, MAX_NAME_BYTES)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Text(String);

impl Text {
    pub(crate) fn new(
        value: impl Into<String>,
        field: &'static str,
    ) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_nonempty_text(&value, field, MAX_TEXT_BYTES)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Argument(String);

impl Argument {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_optional_text(&value, "rustc argument", MAX_ARGUMENT_BYTES)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct EnvironmentValue(String);

impl EnvironmentValue {
    pub(crate) fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_optional_text(
            &value,
            "compile environment value",
            MAX_ENVIRONMENT_VALUE_BYTES,
        )?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct RelativePath(String);

impl RelativePath {
    pub(crate) fn new(
        value: impl Into<String>,
        field: &'static str,
    ) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_path(&value, field, false)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct AbsolutePath(String);

impl AbsolutePath {
    pub(crate) fn new(
        value: impl Into<String>,
        field: &'static str,
    ) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_path(&value, field, true)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// The version and executable digest for one structurally assigned tool role.
///
/// The digest is SHA-256 over the exact executable bytes selected by the
/// caller. This type does not read, resolve, or execute the tool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolIdentityV1 {
    pub(crate) version: Text,
    pub(crate) executable_sha256: [u8; 32],
}

impl ToolIdentityV1 {
    /// Constructs a tool identity from a nonempty version and executable
    /// SHA-256 digest.
    pub fn new(
        version: impl Into<String>,
        executable_sha256: [u8; 32],
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            version: Text::new(version, "tool version")?,
            executable_sha256,
        })
    }

    /// Returns the version string captured by the producer.
    pub fn version(&self) -> &str {
        self.version.as_str()
    }

    /// Returns the SHA-256 digest of the executable bytes.
    pub const fn executable_sha256(&self) -> &[u8; 32] {
        &self.executable_sha256
    }
}

/// A workspace-local Cargo package identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoPackageV1 {
    pub(crate) name: Name,
    pub(crate) version: Text,
    pub(crate) manifest_path: RelativePath,
}

impl CargoPackageV1 {
    /// Constructs a package identity.
    ///
    /// `manifest_path` is relative to the canonical workspace root and must
    /// use `/` separators.
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        manifest_path: impl Into<String>,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            name: Name::new(name, "Cargo package name")?,
            version: Text::new(version, "Cargo package version")?,
            manifest_path: RelativePath::new(manifest_path, "Cargo manifest path")?,
        })
    }

    /// Returns the Cargo package name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns the Cargo package version.
    pub fn version(&self) -> &str {
        self.version.as_str()
    }

    /// Returns the canonical workspace-relative manifest path.
    pub fn manifest_path(&self) -> &str {
        self.manifest_path.as_str()
    }
}

/// The Cargo target category selected for device compilation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum CargoTargetKindV1 {
    /// A library target.
    Library,
    /// A binary target.
    Binary,
    /// An example target.
    Example,
    /// An integration-test target.
    Test,
    /// A benchmark target.
    Benchmark,
    /// A Cargo build-script target.
    BuildScript,
    /// A procedural-macro target.
    ProcMacro,
}

/// A concrete rustc crate type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum CrateTypeV1 {
    /// rustc's inferred library crate type (`lib`).
    Lib,
    /// A Rust library (`rlib`).
    Rlib,
    /// A Rust dynamic library (`dylib`).
    Dylib,
    /// A C-compatible dynamic library (`cdylib`).
    Cdylib,
    /// A static native library (`staticlib`).
    Staticlib,
    /// A procedural-macro library (`proc-macro`).
    ProcMacro,
    /// An executable (`bin`).
    Bin,
}

/// The Rust language edition applied to the selected target.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum EditionV1 {
    /// Rust 2015.
    Rust2015,
    /// Rust 2018.
    Rust2018,
    /// Rust 2021.
    Rust2021,
    /// Rust 2024.
    Rust2024,
}

/// Cargo's selected target and its canonical set-like inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoTargetV1 {
    pub(crate) name: Name,
    pub(crate) kind: CargoTargetKindV1,
    pub(crate) crate_types: Vec<CrateTypeV1>,
    pub(crate) edition: EditionV1,
    pub(crate) source_path: RelativePath,
    pub(crate) features: Vec<Name>,
}

impl CargoTargetV1 {
    /// Constructs a selected Cargo target.
    ///
    /// `crate_types` and `features` must each be strictly sorted and unique.
    /// `source_path` is relative to the canonical workspace root.
    pub fn new(
        name: impl Into<String>,
        kind: CargoTargetKindV1,
        crate_types: Vec<CrateTypeV1>,
        edition: EditionV1,
        source_path: impl Into<String>,
        features: Vec<String>,
    ) -> Result<Self, ValidationError> {
        if crate_types.is_empty() {
            return Err(ValidationError::Empty {
                field: "crate types",
            });
        }
        if crate_types.len() > MAX_CRATE_TYPES {
            return Err(ValidationError::TooMany {
                field: "crate types",
                max: MAX_CRATE_TYPES,
            });
        }
        validate_strictly_sorted(&crate_types, "crate types")?;
        if features.len() > MAX_FEATURES {
            return Err(ValidationError::TooMany {
                field: "Cargo features",
                max: MAX_FEATURES,
            });
        }
        let features = features
            .into_iter()
            .map(|feature| Name::new(feature, "Cargo feature"))
            .collect::<Result<Vec<_>, _>>()?;
        validate_strictly_sorted(&features, "Cargo features")?;

        Ok(Self {
            name: Name::new(name, "Cargo target name")?,
            kind,
            crate_types,
            edition,
            source_path: RelativePath::new(source_path, "Cargo target source path")?,
            features,
        })
    }

    /// Returns the Cargo target name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns the Cargo target category.
    pub const fn kind(&self) -> CargoTargetKindV1 {
        self.kind
    }

    /// Returns the strictly sorted crate-type set.
    pub fn crate_types(&self) -> &[CrateTypeV1] {
        &self.crate_types
    }

    /// Returns the Rust edition.
    pub const fn edition(&self) -> EditionV1 {
        self.edition
    }

    /// Returns the canonical workspace-relative source path.
    pub fn source_path(&self) -> &str {
        self.source_path.as_str()
    }

    /// Iterates over strictly sorted activated Cargo feature names.
    pub fn features(&self) -> impl ExactSizeIterator<Item = &str> {
        self.features.iter().map(Name::as_str)
    }
}

/// Cargo-level identity for the selected compilation unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoIdentityV1 {
    pub(crate) executable: ToolIdentityV1,
    pub(crate) package: CargoPackageV1,
    pub(crate) target: CargoTargetV1,
}

impl CargoIdentityV1 {
    /// Constructs Cargo's executable and selected-unit identity.
    pub const fn new(
        executable: ToolIdentityV1,
        package: CargoPackageV1,
        target: CargoTargetV1,
    ) -> Self {
        Self {
            executable,
            package,
            target,
        }
    }

    /// Returns the Cargo executable identity.
    pub const fn executable(&self) -> &ToolIdentityV1 {
        &self.executable
    }

    /// Returns the selected package identity.
    pub const fn package(&self) -> &CargoPackageV1 {
        &self.package
    }

    /// Returns the selected target identity.
    pub const fn target(&self) -> &CargoTargetV1 {
        &self.target
    }
}

/// Whether rustc is compiling a test harness.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum TestStateV1 {
    /// The `--test` option is absent.
    NotTest,
    /// The `--test` option is present.
    Test,
}

/// rustc's selected unit and exact final argument stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcUnitV1 {
    pub(crate) crate_name: Name,
    pub(crate) host_target: Name,
    pub(crate) effective_target: Name,
    pub(crate) test_state: TestStateV1,
    pub(crate) argv: Vec<Argument>,
}

impl RustcUnitV1 {
    /// Constructs rustc's unit identity.
    ///
    /// `argv` excludes the executable at argument index zero, preserves exact
    /// order and repetitions, and is
    /// interpreted as the final stream after backend argument injection.
    pub fn new(
        crate_name: impl Into<String>,
        host_target: impl Into<String>,
        effective_target: impl Into<String>,
        test_state: TestStateV1,
        argv: Vec<String>,
    ) -> Result<Self, ValidationError> {
        if argv.len() > MAX_RUSTC_ARGUMENTS {
            return Err(ValidationError::TooMany {
                field: "rustc arguments",
                max: MAX_RUSTC_ARGUMENTS,
            });
        }
        let argv = argv
            .into_iter()
            .map(Argument::new)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            crate_name: Name::new(crate_name, "rustc crate name")?,
            host_target: Name::new(host_target, "rustc host target")?,
            effective_target: Name::new(effective_target, "rustc effective target")?,
            test_state,
            argv,
        })
    }

    /// Returns rustc's crate name.
    pub fn crate_name(&self) -> &str {
        self.crate_name.as_str()
    }

    /// Returns rustc's host target triple.
    pub fn host_target(&self) -> &str {
        self.host_target.as_str()
    }

    /// Returns the effective compilation target triple.
    pub fn effective_target(&self) -> &str {
        self.effective_target.as_str()
    }

    /// Returns whether this is a test-harness compilation.
    pub const fn test_state(&self) -> TestStateV1 {
        self.test_state
    }

    /// Iterates over the exact ordered final rustc arguments.
    pub fn argv(&self) -> impl ExactSizeIterator<Item = &str> {
        self.argv.iter().map(Argument::as_str)
    }
}

/// rustc's executable and selected-unit identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcIdentityV1 {
    pub(crate) executable: ToolIdentityV1,
    pub(crate) unit: RustcUnitV1,
}

impl RustcIdentityV1 {
    /// Constructs the rustc identity.
    pub const fn new(executable: ToolIdentityV1, unit: RustcUnitV1) -> Self {
        Self { executable, unit }
    }

    /// Returns the rustc executable identity.
    pub const fn executable(&self) -> &ToolIdentityV1 {
        &self.executable
    }

    /// Returns the selected rustc unit.
    pub const fn unit(&self) -> &RustcUnitV1 {
        &self.unit
    }
}

/// Structurally assigned identities for code-generation tools.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendToolsV1 {
    pub(crate) backend: ToolIdentityV1,
    pub(crate) clang: ToolIdentityV1,
    pub(crate) linker: ToolIdentityV1,
    pub(crate) inspector: Option<ToolIdentityV1>,
}

impl BackendToolsV1 {
    /// Constructs the backend tool identities.
    pub const fn new(
        backend: ToolIdentityV1,
        clang: ToolIdentityV1,
        linker: ToolIdentityV1,
        inspector: Option<ToolIdentityV1>,
    ) -> Self {
        Self {
            backend,
            clang,
            linker,
            inspector,
        }
    }

    /// Returns the rustc codegen backend identity.
    pub const fn backend(&self) -> &ToolIdentityV1 {
        &self.backend
    }

    /// Returns the clang identity.
    pub const fn clang(&self) -> &ToolIdentityV1 {
        &self.clang
    }

    /// Returns the linker identity.
    pub const fn linker(&self) -> &ToolIdentityV1 {
        &self.linker
    }

    /// Returns the optional code-object inspector identity.
    pub const fn inspector(&self) -> Option<&ToolIdentityV1> {
        self.inspector.as_ref()
    }
}

/// Canonical textual representation of a concrete AMD target ID.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AmdTargetIdTextV1(String);

impl AmdTargetIdTextV1 {
    /// Parses a known concrete processor with optional canonical feature
    /// modifiers.
    pub fn new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_amd_target(&value)?;
        Ok(Self(value))
    }

    /// Returns the canonical AMD target ID text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Whether result-affecting compiler verification is enabled.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum VerificationModeV1 {
    /// Verification is disabled.
    Disabled,
    /// Verification is required.
    Required,
}

/// Result-affecting device compilation configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceConfigurationV1 {
    pub(crate) amd_target: AmdTargetIdTextV1,
    pub(crate) verification: VerificationModeV1,
}

impl DeviceConfigurationV1 {
    /// Constructs the device compilation configuration.
    pub const fn new(amd_target: AmdTargetIdTextV1, verification: VerificationModeV1) -> Self {
        Self {
            amd_target,
            verification,
        }
    }

    /// Returns the canonical AMD target ID.
    pub const fn amd_target(&self) -> &AmdTargetIdTextV1 {
        &self.amd_target
    }

    /// Returns the result-affecting verification mode.
    pub const fn verification(&self) -> VerificationModeV1 {
        self.verification
    }
}

/// Canonical absolute paths that namespace generated sidecar artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputDomainV1 {
    pub(crate) workspace_root: AbsolutePath,
    pub(crate) artifact_output_directory: AbsolutePath,
}

impl OutputDomainV1 {
    /// Constructs the output domain from canonical absolute UTF-8 paths.
    pub fn new(
        workspace_root: impl Into<String>,
        artifact_output_directory: impl Into<String>,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            workspace_root: AbsolutePath::new(workspace_root, "workspace root")?,
            artifact_output_directory: AbsolutePath::new(
                artifact_output_directory,
                "artifact output directory",
            )?,
        })
    }

    /// Returns the canonical absolute workspace root.
    pub fn workspace_root(&self) -> &str {
        self.workspace_root.as_str()
    }

    /// Returns the canonical absolute artifact output directory.
    pub fn artifact_output_directory(&self) -> &str {
        self.artifact_output_directory.as_str()
    }
}

/// One compile-environment key/value entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileEnvironmentEntryV1 {
    pub(crate) key: Name,
    pub(crate) value: EnvironmentValue,
}

impl CompileEnvironmentEntryV1 {
    /// Constructs one environment entry.
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Result<Self, ValidationError> {
        let key = Name::new(key, "compile environment key")?;
        if key.as_str().contains('=') {
            return Err(ValidationError::InvalidText {
                field: "compile environment key",
            });
        }
        Ok(Self {
            key,
            value: EnvironmentValue::new(value)?,
        })
    }

    /// Returns the environment key.
    pub fn key(&self) -> &str {
        self.key.as_str()
    }

    /// Returns the environment value, which may be empty.
    pub fn value(&self) -> &str {
        self.value.as_str()
    }
}

/// A canonical V1 coordination identity for one selected rustc invocation.
///
/// This descriptor is not artifact evidence or launch authority. Its digest
/// may coordinate a build attempt, but trusted compiler output and runtime
/// launch validation require separate, stronger bindings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcInvocationDescriptorV1 {
    pub(crate) cargo: CargoIdentityV1,
    pub(crate) rustc: RustcIdentityV1,
    pub(crate) tools: BackendToolsV1,
    pub(crate) device: DeviceConfigurationV1,
    pub(crate) output: OutputDomainV1,
    pub(crate) compile_environment: Vec<CompileEnvironmentEntryV1>,
}

impl RustcInvocationDescriptorV1 {
    /// Constructs and fully validates one V1 invocation descriptor.
    ///
    /// `compile_environment` must be strictly sorted by key and contain no
    /// duplicate keys. Transport and attempt-token variables must already
    /// have been removed by the caller.
    pub fn new(
        cargo: CargoIdentityV1,
        rustc: RustcIdentityV1,
        tools: BackendToolsV1,
        device: DeviceConfigurationV1,
        output: OutputDomainV1,
        compile_environment: Vec<CompileEnvironmentEntryV1>,
    ) -> Result<Self, ValidationError> {
        if compile_environment.len() > MAX_COMPILE_ENVIRONMENT_ENTRIES {
            return Err(ValidationError::TooMany {
                field: "compile environment",
                max: MAX_COMPILE_ENVIRONMENT_ENTRIES,
            });
        }
        validate_environment_sorted(&compile_environment)?;
        let descriptor = Self {
            cargo,
            rustc,
            tools,
            device,
            output,
            compile_environment,
        };
        crate::encode::validate_encoded_size(&descriptor)?;
        Ok(descriptor)
    }

    /// Returns Cargo's executable and selected-unit identity.
    pub const fn cargo(&self) -> &CargoIdentityV1 {
        &self.cargo
    }

    /// Returns rustc's executable and selected-unit identity.
    pub const fn rustc(&self) -> &RustcIdentityV1 {
        &self.rustc
    }

    /// Returns the structurally assigned backend tool identities.
    pub const fn tools(&self) -> &BackendToolsV1 {
        &self.tools
    }

    /// Returns the device compilation configuration.
    pub const fn device(&self) -> &DeviceConfigurationV1 {
        &self.device
    }

    /// Returns the artifact output namespace.
    pub const fn output(&self) -> &OutputDomainV1 {
        &self.output
    }

    /// Returns the strictly sorted compile environment.
    pub fn compile_environment(&self) -> &[CompileEnvironmentEntryV1] {
        &self.compile_environment
    }
}

fn validate_nonempty_text(
    value: &str,
    field: &'static str,
    max: usize,
) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::Empty { field });
    }
    validate_optional_text(value, field, max)
}

fn validate_optional_text(
    value: &str,
    field: &'static str,
    max: usize,
) -> Result<(), ValidationError> {
    if value.len() > max {
        return Err(ValidationError::TooLong { field, max });
    }
    if value.contains('\0') {
        return Err(ValidationError::InvalidText { field });
    }
    Ok(())
}

fn validate_path(value: &str, field: &'static str, absolute: bool) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::Empty { field });
    }
    if value.len() > MAX_PATH_BYTES {
        return Err(ValidationError::TooLong {
            field,
            max: MAX_PATH_BYTES,
        });
    }
    if value.contains(['\0', '\\']) || value.starts_with('/') != absolute {
        return Err(ValidationError::InvalidPath { field });
    }
    if absolute && value == "/" {
        return Ok(());
    }
    let components = if absolute { &value[1..] } else { value };
    if components
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(ValidationError::InvalidPath { field });
    }
    Ok(())
}

fn validate_strictly_sorted<T: Ord>(
    values: &[T],
    field: &'static str,
) -> Result<(), ValidationError> {
    for pair in values.windows(2) {
        match pair[0].cmp(&pair[1]) {
            Ordering::Less => {}
            Ordering::Equal => return Err(ValidationError::Duplicate { field }),
            Ordering::Greater => return Err(ValidationError::NonCanonicalOrder { field }),
        }
    }
    Ok(())
}

fn validate_environment_sorted(
    values: &[CompileEnvironmentEntryV1],
) -> Result<(), ValidationError> {
    for pair in values.windows(2) {
        match pair[0].key.cmp(&pair[1].key) {
            Ordering::Less => {}
            Ordering::Equal => {
                return Err(ValidationError::Duplicate {
                    field: "compile environment",
                });
            }
            Ordering::Greater => {
                return Err(ValidationError::NonCanonicalOrder {
                    field: "compile environment",
                });
            }
        }
    }
    Ok(())
}

fn validate_amd_target(value: &str) -> Result<(), ValidationError> {
    let target = AmdTargetId::parse(value).map_err(|_| ValidationError::InvalidAmdTarget)?;
    if target.to_string() != value {
        return Err(ValidationError::InvalidAmdTarget);
    }
    Ok(())
}
