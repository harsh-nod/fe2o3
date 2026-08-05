use std::cmp::Ordering;
use std::ffi::OsString;
use std::process::Command;

use fe2o3_amd_target::AmdTargetId;

use crate::ValidationError;

/// Maximum size of one complete encoded V2 descriptor.
pub const MAX_DESCRIPTOR_BYTES_V2: usize = 256 * 1024;
/// Maximum byte length of a V2 environment key.
pub const MAX_NAME_BYTES_V2: usize = 128;
/// Maximum byte length of a canonical V2 path.
pub const MAX_PATH_BYTES_V2: usize = 4096;
/// Maximum byte length of one V2 rustc argument.
pub const MAX_ARGUMENT_BYTES_V2: usize = 4096;
/// Maximum byte length of one V2 compile-environment value.
pub const MAX_ENVIRONMENT_VALUE_BYTES_V2: usize = 4096;
/// Maximum number of rustc arguments in V2.
pub const MAX_RUSTC_ARGUMENTS_V2: usize = 4096;
/// Maximum number of compile-environment entries in V2.
pub const MAX_COMPILE_ENVIRONMENT_ENTRIES_V2: usize = 1024;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Name(String);

impl Name {
    pub(crate) fn new(
        value: impl Into<String>,
        field: &'static str,
    ) -> Result<Self, ValidationError> {
        let value = value.into();
        validate_nonempty_text(&value, field, MAX_NAME_BYTES_V2)?;
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
        validate_optional_text(&value, "rustc argument", MAX_ARGUMENT_BYTES_V2)?;
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
            MAX_ENVIRONMENT_VALUE_BYTES_V2,
        )?;
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
        validate_absolute_path(&value, field)?;
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// rustc's canonical working directory and exact final argument stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcUnitV2 {
    pub(crate) working_directory: AbsolutePath,
    pub(crate) argv: Vec<Argument>,
}

impl RustcUnitV2 {
    /// Constructs rustc's process inputs.
    ///
    /// `working_directory` is the canonical absolute directory in which rustc
    /// runs. `argv` includes argument zero and preserves exact order and
    /// repetitions after wrapper-owned argument injection.
    pub fn new(
        working_directory: impl Into<String>,
        argv: Vec<String>,
    ) -> Result<Self, ValidationError> {
        if argv.is_empty() {
            return Err(ValidationError::Empty {
                field: "rustc arguments",
            });
        }
        if argv.len() > MAX_RUSTC_ARGUMENTS_V2 {
            return Err(ValidationError::TooMany {
                field: "rustc arguments",
                max: MAX_RUSTC_ARGUMENTS_V2,
            });
        }
        let argv = argv
            .into_iter()
            .map(Argument::new)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            working_directory: AbsolutePath::new(working_directory, "rustc working directory")?,
            argv,
        })
    }

    /// Returns rustc's canonical absolute working directory.
    pub fn working_directory(&self) -> &str {
        self.working_directory.as_str()
    }

    /// Iterates over the exact ordered final rustc arguments, including `argv[0]`.
    pub fn argv(&self) -> impl ExactSizeIterator<Item = &str> {
        self.argv.iter().map(Argument::as_str)
    }
}

/// One exact compile-environment key/value entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileEnvironmentEntryV2 {
    pub(crate) key: Name,
    pub(crate) value: EnvironmentValue,
}

impl CompileEnvironmentEntryV2 {
    pub(crate) fn new(
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, ValidationError> {
        let key = Name::new(key, "compile environment key")?;
        if key.as_str().contains('=') {
            return Err(ValidationError::InvalidText {
                field: "compile environment key",
            });
        }
        if is_forbidden_environment_key(key.as_str()) {
            return Err(ValidationError::ForbiddenEnvironmentVariable {
                key: key.as_str().to_owned(),
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

/// The complete intended process environment for one rustc execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileEnvironmentV2 {
    pub(crate) entries: Vec<CompileEnvironmentEntryV2>,
}

impl CompileEnvironmentV2 {
    /// Captures and validates the complete current process environment.
    ///
    /// Non-UTF-8 keys or values and reserved fe2o3 transport variables are
    /// rejected. The resulting entries are sorted by key.
    pub fn capture_current() -> Result<Self, ValidationError> {
        Self::from_os_entries(std::env::vars_os())
    }

    /// Validates the complete environment prepared for a child rustc process.
    pub fn from_child_environment(
        entries: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Result<Self, ValidationError> {
        Self::from_os_entries(entries)
    }

    /// Returns the canonical entries sorted by key.
    pub fn entries(&self) -> &[CompileEnvironmentEntryV2] {
        &self.entries
    }

    /// Replaces a command's environment with exactly this captured set.
    pub fn configure_command(&self, command: &mut Command) {
        command.env_clear();
        command.envs(
            self.entries
                .iter()
                .map(|entry| (entry.key.as_str(), entry.value.as_str())),
        );
    }

    pub(crate) fn from_encoded_entries(
        entries: Vec<CompileEnvironmentEntryV2>,
    ) -> Result<Self, ValidationError> {
        Self::validate(entries)
    }

    fn from_os_entries(
        entries: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Result<Self, ValidationError> {
        let mut converted = Vec::new();
        for (key, value) in entries {
            if converted.len() == MAX_COMPILE_ENVIRONMENT_ENTRIES_V2 {
                return Err(ValidationError::TooMany {
                    field: "compile environment",
                    max: MAX_COMPILE_ENVIRONMENT_ENTRIES_V2,
                });
            }
            let key = key
                .into_string()
                .map_err(|_| ValidationError::NonUtf8Environment { field: "key" })?;
            let value = value
                .into_string()
                .map_err(|_| ValidationError::NonUtf8Environment { field: "value" })?;
            converted.push(CompileEnvironmentEntryV2::new(key, value)?);
        }
        converted.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        Self::validate(converted)
    }

    fn validate(entries: Vec<CompileEnvironmentEntryV2>) -> Result<Self, ValidationError> {
        if entries.len() > MAX_COMPILE_ENVIRONMENT_ENTRIES_V2 {
            return Err(ValidationError::TooMany {
                field: "compile environment",
                max: MAX_COMPILE_ENVIRONMENT_ENTRIES_V2,
            });
        }
        validate_environment_sorted(&entries)?;
        Ok(Self { entries })
    }

    #[cfg(test)]
    pub(crate) fn from_entries_for_test<K, V>(
        entries: impl IntoIterator<Item = (K, V)>,
    ) -> Result<Self, ValidationError>
    where
        K: Into<OsString>,
        V: Into<OsString>,
    {
        Self::from_os_entries(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value.into())),
        )
    }
}

/// A canonical V2 identity for one exact rustc process assigned the fe2o3 backend.
///
/// The two caller-asserted digests identify executable contents that are
/// referenced, but not embedded, by the exact argument stream. This descriptor
/// is not artifact evidence or launch authority. Trusted execution must hash
/// and execute the same pinned rustc object and keep the pinned backend object
/// available until rustc loads it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcInvocationDescriptorV2 {
    pub(crate) rustc_executable_sha256: [u8; 32],
    pub(crate) codegen_backend_sha256: [u8; 32],
    pub(crate) rustc: RustcUnitV2,
    pub(crate) compile_environment: CompileEnvironmentV2,
}

impl RustcInvocationDescriptorV2 {
    /// Constructs and structurally validates one exact V2 rustc invocation.
    ///
    /// The wrapper-assigned backend argument must be the final argv entry in
    /// canonical joined form. Query-versus-compile policy is deliberately not
    /// part of this frozen wire-schema validator.
    pub fn new(
        rustc_executable_sha256: [u8; 32],
        codegen_backend_sha256: [u8; 32],
        rustc: RustcUnitV2,
        compile_environment: CompileEnvironmentV2,
    ) -> Result<Self, ValidationError> {
        validate_compile_invocation(&rustc, &compile_environment)?;
        let descriptor = Self {
            rustc_executable_sha256,
            codegen_backend_sha256,
            rustc,
            compile_environment,
        };
        crate::encode_v2::validate_encoded_size(&descriptor)?;
        Ok(descriptor)
    }

    /// Returns the caller-asserted SHA-256 digest of the rustc executable bytes.
    pub const fn rustc_executable_sha256(&self) -> &[u8; 32] {
        &self.rustc_executable_sha256
    }

    /// Returns the caller-asserted SHA-256 digest of the codegen backend bytes.
    pub const fn codegen_backend_sha256(&self) -> &[u8; 32] {
        &self.codegen_backend_sha256
    }

    /// Returns rustc's exact working directory and final argument vector.
    pub const fn rustc(&self) -> &RustcUnitV2 {
        &self.rustc
    }

    /// Returns the complete intended rustc environment.
    pub const fn compile_environment(&self) -> &CompileEnvironmentV2 {
        &self.compile_environment
    }

    /// Returns the rustc path represented once in `argv[0]`.
    pub fn rustc_executable_path(&self) -> &str {
        self.rustc.argv[0].as_str()
    }

    /// Returns the codegen backend path represented once in rustc's arguments.
    pub fn codegen_backend_path(&self) -> &str {
        assigned_codegen_backend(&self.rustc.argv).expect("validated descriptor has a backend")
    }

    /// Returns the canonical AMD target represented once in the environment.
    pub fn amd_target(&self) -> &str {
        environment_value(&self.compile_environment, "FE2O3_TARGET")
            .expect("validated descriptor has an AMD target")
    }

    /// Returns the canonical artifact directory represented once in the environment.
    pub fn artifact_output_directory(&self) -> &str {
        environment_value(&self.compile_environment, "FE2O3_HSACO_DIR")
            .expect("validated descriptor has an artifact directory")
    }

    /// Reports whether kernel-IR verification is enabled by the environment.
    pub fn verification_required(&self) -> bool {
        environment_value(&self.compile_environment, "FE2O3_VERIFY_KERNEL_IR") == Some("1")
    }
}

fn validate_compile_invocation(
    rustc: &RustcUnitV2,
    environment: &CompileEnvironmentV2,
) -> Result<(), ValidationError> {
    AbsolutePath::new(rustc.argv[0].as_str(), "rustc executable path")?;

    let backend = assigned_codegen_backend(&rustc.argv).ok_or(ValidationError::Empty {
        field: "rustc codegen backend",
    })?;
    AbsolutePath::new(backend, "codegen backend path")?;

    let target = environment_value(environment, "FE2O3_TARGET").ok_or(ValidationError::Empty {
        field: "FE2O3_TARGET",
    })?;
    validate_amd_target(target)?;

    let output =
        environment_value(environment, "FE2O3_HSACO_DIR").ok_or(ValidationError::Empty {
            field: "FE2O3_HSACO_DIR",
        })?;
    AbsolutePath::new(output, "artifact output directory")?;

    if let Some(value) = environment_value(environment, "FE2O3_VERIFY_KERNEL_IR")
        && !matches!(value, "0" | "1")
    {
        return Err(ValidationError::InvalidText {
            field: "FE2O3_VERIFY_KERNEL_IR",
        });
    }
    Ok(())
}

fn assigned_codegen_backend(argv: &[Argument]) -> Option<&str> {
    let (last, preceding) = argv.split_last()?;
    let backend = last.as_str().strip_prefix("-Zcodegen-backend=")?;
    if backend.is_empty()
        || preceding.iter().any(|argument| {
            argument.as_str().starts_with("-Zcodegen-backend=")
                || argument.as_str().starts_with("codegen-backend=")
        })
    {
        return None;
    }
    Some(backend)
}

fn environment_value<'a>(environment: &'a CompileEnvironmentV2, key: &str) -> Option<&'a str> {
    environment
        .entries
        .iter()
        .find(|entry| entry.key.as_str() == key)
        .map(|entry| entry.value.as_str())
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

fn validate_absolute_path(value: &str, field: &'static str) -> Result<(), ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::Empty { field });
    }
    if value.len() > MAX_PATH_BYTES_V2 {
        return Err(ValidationError::TooLong {
            field,
            max: MAX_PATH_BYTES_V2,
        });
    }
    if value.contains(['\0', '\\']) || !value.starts_with('/') {
        return Err(ValidationError::InvalidPath { field });
    }
    if value == "/" {
        return Ok(());
    }
    if value[1..]
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(ValidationError::InvalidPath { field });
    }
    Ok(())
}

fn validate_environment_sorted(
    values: &[CompileEnvironmentEntryV2],
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

fn is_forbidden_environment_key(key: &str) -> bool {
    key.starts_with("FE2O3_TRANSPORT_")
}

fn validate_amd_target(value: &str) -> Result<(), ValidationError> {
    let target = AmdTargetId::parse(value).map_err(|_| ValidationError::InvalidAmdTarget)?;
    if target.to_string() != value {
        return Err(ValidationError::InvalidAmdTarget);
    }
    Ok(())
}
