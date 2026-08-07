use cap_primitives::ambient_authority;
use cap_primitives::fs::{DirOptions, create_dir, open_ambient_dir, open_dir_nofollow};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, BorrowedFd, IntoRawFd, RawFd};
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const MANIFEST_PATH: &str = "--manifest-path";
const TARGET_DIR: &str = "--target-dir";
const CONFIG: &str = "--config";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CargoRouting {
    manifest_path: Option<OsString>,
    target_dir: Option<OsString>,
    metadata_args: Vec<OsString>,
    config_args: Vec<OsString>,
}

impl CargoRouting {
    pub(crate) fn parse(args: &[OsString]) -> Result<Self, String> {
        let mut manifest_path = None;
        let mut target_dir = None;
        let mut metadata_args = Vec::new();
        let mut config_args = Vec::new();
        let mut index = 0;

        while index < args.len() {
            let argument = &args[index];
            if argument == "--" {
                break;
            }

            if argument == MANIFEST_PATH || argument == TARGET_DIR {
                let option = argument.clone();
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    format!("{} requires a path argument", option.to_string_lossy())
                })?;
                if looks_like_option(value) {
                    return Err(format!(
                        "{} requires a path argument; found option {value:?}",
                        option.to_string_lossy()
                    ));
                }
                set_once(
                    if option == MANIFEST_PATH {
                        &mut manifest_path
                    } else {
                        &mut target_dir
                    },
                    value.clone(),
                    &option,
                )?;
                if option == MANIFEST_PATH {
                    metadata_args.push(option);
                    metadata_args.push(value.clone());
                }
            } else if let Some(value) = split_joined_option(argument, MANIFEST_PATH)? {
                set_once(&mut manifest_path, value, OsStr::new(MANIFEST_PATH))?;
                metadata_args.push(argument.clone());
            } else if let Some(value) = split_joined_option(argument, TARGET_DIR)? {
                set_once(&mut target_dir, value, OsStr::new(TARGET_DIR))?;
            } else if argument == CONFIG || argument == "-Z" {
                let option = argument.clone();
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{} requires an argument", option.to_string_lossy()))?;
                if value.is_empty() || looks_like_option(value) {
                    return Err(format!(
                        "{} requires an argument; found {value:?}",
                        option.to_string_lossy()
                    ));
                }
                metadata_args.push(option);
                metadata_args.push(value.clone());
                config_args.push(argument.clone());
                config_args.push(value.clone());
            } else if split_joined_option(argument, CONFIG)?.is_some()
                || os_bytes(argument).starts_with(b"-Z") && os_bytes(argument).len() > 2
                || matches!(
                    argument.to_str(),
                    Some("--locked" | "--offline" | "--frozen")
                )
            {
                metadata_args.push(argument.clone());
                config_args.push(argument.clone());
            }
            index += 1;
        }

        Ok(Self {
            manifest_path,
            target_dir,
            metadata_args,
            config_args,
        })
    }
}

fn set_once(slot: &mut Option<OsString>, value: OsString, option: &OsStr) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!(
            "{} requires a non-empty path",
            option.to_string_lossy()
        ));
    }
    if slot.replace(value).is_some() {
        return Err(format!(
            "{} may be specified only once",
            option.to_string_lossy()
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn split_joined_option(argument: &OsStr, option: &str) -> Result<Option<OsString>, String> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let bytes = argument.as_bytes();
    let mut prefix = option.as_bytes().to_vec();
    prefix.push(b'=');
    if !bytes.starts_with(&prefix) {
        return Ok(None);
    }
    let value = &bytes[prefix.len()..];
    if value.is_empty() {
        return Err(format!("{option} requires a non-empty path"));
    }
    Ok(Some(OsString::from_vec(value.to_vec())))
}

#[cfg(not(unix))]
fn split_joined_option(argument: &OsStr, option: &str) -> Result<Option<OsString>, String> {
    let Some(argument) = argument.to_str() else {
        return Ok(None);
    };
    let Some(value) = argument
        .strip_prefix(option)
        .and_then(|value| value.strip_prefix('='))
    else {
        return Ok(None);
    };
    if value.is_empty() {
        return Err(format!("{option} requires a non-empty path"));
    }
    Ok(Some(OsString::from(value)))
}

#[derive(Debug)]
pub(crate) struct CargoProject {
    invocation_dir: PinnedDirectory,
    workspace_root: PinnedDirectory,
    target_path: PathBuf,
}

impl CargoProject {
    pub(crate) fn discover(args: &[OsString]) -> Result<Self, String> {
        let routing = CargoRouting::parse(args)?;
        let invocation_path = std::env::current_dir()
            .map_err(|error| format!("failed to resolve the invocation directory: {error}"))?;
        let invocation_dir = PinnedDirectory::open_existing(
            lexical_absolute(&invocation_path, Path::new("/"))?,
            "Cargo invocation directory",
        )?;
        let output = metadata_output(&invocation_dir, &routing.metadata_args)?;
        if !output.status.success() {
            return Err(format!(
                "could not resolve Cargo project metadata: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let record: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("failed to parse cargo metadata output: {error}"))?;
        let workspace_root = metadata_path(&record, "workspace_root")?;
        let metadata_target = metadata_path(&record, "target_directory")?;
        let target_path = match routing.target_dir {
            Some(path) => lexical_absolute(Path::new(&path), invocation_dir.display_path())?,
            None => lexical_absolute(&metadata_target, invocation_dir.display_path())?,
        };
        let workspace_root_path = lexical_absolute(&workspace_root, invocation_dir.display_path())?;
        let workspace_root =
            PinnedDirectory::open_existing(workspace_root_path, "Cargo workspace root")?;

        Ok(Self {
            invocation_dir,
            workspace_root,
            target_path,
        })
    }

    pub(crate) fn invocation_dir(&self) -> &PinnedDirectory {
        &self.invocation_dir
    }

    pub(crate) fn target_path(&self) -> &Path {
        &self.target_path
    }

    pub(crate) fn open_or_create_target(&self) -> Result<PinnedDirectory, String> {
        PinnedDirectory::open_or_create(self.target_path.clone(), "Cargo target directory")
            .map(|(directory, _)| directory)
    }

    pub(crate) fn open_target(&self) -> Result<Option<PinnedDirectory>, String> {
        PinnedDirectory::open_optional(self.target_path.clone(), "Cargo target directory")
    }

    pub(crate) fn validate_paths(&self) -> Result<(), String> {
        self.invocation_dir
            .validate_path("Cargo invocation directory")?;
        self.workspace_root.validate_path("Cargo workspace root")
    }

    pub(crate) fn cargo_config_value(
        &self,
        args: &[OsString],
        key: &str,
    ) -> Result<Option<serde_json::Value>, String> {
        let routing = CargoRouting::parse(args)?;
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
        let mut command = Command::new(cargo);
        command.args([
            "config",
            "get",
            "-Z",
            "unstable-options",
            "--format=json-value",
        ]);
        command.args(&routing.config_args);
        command.arg(key);
        command.current_dir(self.invocation_dir.child_path());
        let output = command
            .output()
            .map_err(|error| format!("failed to query Cargo configuration `{key}`: {error}"))?;
        if output.status.success() {
            return serde_json::from_slice(&output.stdout)
                .map(Some)
                .map_err(|error| {
                    format!("failed to parse Cargo configuration `{key}` as JSON: {error}")
                });
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("config value") && stderr.contains("is not set") {
            return Ok(None);
        }
        Err(format!(
            "could not resolve Cargo configuration `{key}`: {}",
            stderr.trim()
        ))
    }

    pub(crate) fn semantic_configuration(&self, args: &[OsString]) -> Result<Vec<u8>, String> {
        let mut snapshot = b"fe2o3-cargo-configuration-v1\0".to_vec();
        for argument in args.iter().take_while(|argument| *argument != "--") {
            append_snapshot_field(&mut snapshot, os_bytes(argument));
        }
        for key in ["build", "target", "profile"] {
            append_snapshot_field(&mut snapshot, key.as_bytes());
            let value = self.cargo_config_value(args, key)?;
            let encoded = serde_json::to_vec(&value).map_err(|error| {
                format!("failed to encode Cargo configuration `{key}`: {error}")
            })?;
            append_snapshot_field(&mut snapshot, &encoded);
        }
        Ok(snapshot)
    }
}

fn append_snapshot_field(snapshot: &mut Vec<u8>, value: &[u8]) {
    snapshot.extend_from_slice(&(value.len() as u64).to_le_bytes());
    snapshot.extend_from_slice(value);
}

fn metadata_output(
    invocation_dir: &PinnedDirectory,
    routing_args: &[OsString],
) -> Result<Output, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let mut command = Command::new(cargo);
    command.args(["metadata", "--no-deps", "--format-version", "1"]);
    command.args(routing_args);
    command.current_dir(invocation_dir.child_path());
    command
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))
}

fn metadata_path(record: &serde_json::Value, key: &str) -> Result<PathBuf, String> {
    record
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| format!("cargo metadata output did not contain a string `{key}`"))
}

fn looks_like_option(value: &OsStr) -> bool {
    os_bytes(value).first() == Some(&b'-')
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value
        .to_str()
        .expect("Cargo routing values must be UTF-8 off Unix")
        .as_bytes()
}

#[derive(Debug)]
pub(crate) struct PinnedDirectory {
    display_path: PathBuf,
    file: File,
    identity: DirectoryIdentity,
}

impl PinnedDirectory {
    pub(crate) fn open_existing(display_path: PathBuf, kind: &str) -> Result<Self, String> {
        let (file, _) = open_absolute_directory(&display_path, false)?
            .ok_or_else(|| format!("{kind} does not exist: {}", display_path.display()))?;
        Self::from_open_file(display_path, file, kind)
    }

    pub(crate) fn open_optional(display_path: PathBuf, kind: &str) -> Result<Option<Self>, String> {
        let Some((file, _)) = open_absolute_directory(&display_path, false)? else {
            return Ok(None);
        };
        Self::from_open_file(display_path, file, kind).map(Some)
    }

    pub(crate) fn open_or_create(
        display_path: PathBuf,
        kind: &str,
    ) -> Result<(Self, bool), String> {
        let (file, created) = open_absolute_directory(&display_path, true)?
            .ok_or_else(|| format!("failed to create {kind}: {}", display_path.display()))?;
        Self::from_open_file(display_path, file, kind).map(|directory| (directory, created))
    }

    fn from_open_file(display_path: PathBuf, file: File, kind: &str) -> Result<Self, String> {
        let identity = DirectoryIdentity::from_file(&file)
            .map_err(|error| format!("failed to inspect retained {kind}: {error}"))?;
        Ok(Self {
            display_path,
            file,
            identity,
        })
    }

    pub(crate) fn from_transferred_file(file: File, kind: &str) -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        {
            let descriptor_flags = rustix::io::fcntl_getfd(&file)
                .map_err(|error| format!("failed to inspect transferred {kind}: {error}"))?;
            if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC) {
                return Err(format!(
                    "transferred {kind} descriptor is not close-on-exec"
                ));
            }
            let status_flags = rustix::fs::fcntl_getfl(&file)
                .map_err(|error| format!("failed to inspect transferred {kind}: {error}"))?;
            if status_flags & rustix::fs::OFlags::ACCMODE != rustix::fs::OFlags::RDONLY {
                return Err(format!("transferred {kind} descriptor is writable"));
            }
            if !file
                .metadata()
                .map_err(|error| format!("failed to inspect transferred {kind}: {error}"))?
                .is_dir()
            {
                return Err(format!("transferred {kind} descriptor is not a directory"));
            }
        }
        Self::from_open_file(
            PathBuf::from("<cargo-fe2o3 capability broker artifact directory>"),
            file,
            kind,
        )
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn try_clone_for_transfer(&self) -> Result<File, String> {
        let descriptor = rustix::fs::openat(
            &self.file,
            Path::new("."),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|error| format!("failed to normalize retained directory descriptor: {error}"))?;
        let file = File::from(descriptor);
        let identity = DirectoryIdentity::from_file(&file)
            .map_err(|error| format!("failed to inspect transferred directory: {error}"))?;
        if identity != self.identity {
            return Err("transferred directory identity changed".to_string());
        }
        Ok(file)
    }

    pub(crate) fn open_or_create_child(&self, component: &str, kind: &str) -> Result<Self, String> {
        validate_component(component)?;
        let path = self.display_path.join(component);
        let file = match open_dir_nofollow(&self.file, Path::new(component)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                create_dir(&self.file, Path::new(component), &DirOptions::new()).map_err(
                    |error| format!("failed to create {kind} {}: {error}", path.display()),
                )?;
                open_dir_nofollow(&self.file, Path::new(component)).map_err(|error| {
                    format!(
                        "failed to pin newly created {kind} {}: {error}",
                        path.display()
                    )
                })?
            }
            Err(error) => {
                return Err(format!(
                    "refusing unpinned or symlinked {kind} {}: {error}",
                    path.display()
                ));
            }
        };
        Self::from_open_file(path, file, kind)
    }

    pub(crate) fn open_child(&self, component: &str, kind: &str) -> Result<Option<Self>, String> {
        validate_component(component)?;
        let path = self.display_path.join(component);
        match open_dir_nofollow(&self.file, Path::new(component)) {
            Ok(file) => Self::from_open_file(path, file, kind).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!(
                "refusing unpinned or symlinked {kind} {}: {error}",
                path.display()
            )),
        }
    }

    pub(crate) fn display_path(&self) -> &Path {
        &self.display_path
    }

    pub(crate) fn file(&self) -> &File {
        &self.file
    }

    pub(crate) fn into_file(self) -> File {
        self.file
    }

    pub(crate) fn child_path(&self) -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.display_path.clone()
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn fixed_child_path(&self, target_fd: RawFd) -> Result<PathBuf, String> {
        require_unused_child_descriptor(target_fd)?;
        Ok(PathBuf::from(format!("/proc/self/fd/{target_fd}")))
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn inherit_for_child_at(
        &self,
        command: &mut Command,
        target_fd: RawFd,
    ) -> Result<(), String> {
        require_unused_child_descriptor(target_fd)?;
        let source_fd = self.file.as_raw_fd();
        let identity = self.identity;
        // SAFETY: `self.file` remains alive through spawn and the callback uses descriptor-only
        // syscalls. F_DUPFD_CLOEXEC refuses to replace an occupied target descriptor.
        unsafe {
            command.pre_exec(move || {
                let source = BorrowedFd::borrow_raw(source_fd);
                let installed =
                    rustix::io::fcntl_dupfd_cloexec(source, target_fd).map_err(io::Error::from)?;
                if installed.as_raw_fd() != target_fd {
                    return Err(io::Error::from_raw_os_error(
                        rustix::io::Errno::BUSY.raw_os_error(),
                    ));
                }
                let stat = rustix::fs::fstat(&installed).map_err(io::Error::from)?;
                if !identity.matches_stat(&stat) {
                    return Err(io::Error::from_raw_os_error(
                        rustix::io::Errno::STALE.raw_os_error(),
                    ));
                }
                rustix::io::fcntl_setfd(&installed, rustix::io::FdFlags::empty())
                    .map_err(io::Error::from)?;
                let _ = installed.into_raw_fd();
                Ok(())
            });
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn replace_for_child_at(
        &self,
        command: &mut Command,
        target_fd: RawFd,
    ) -> Result<(), String> {
        if target_fd < 3 {
            return Err("fixed child descriptor would replace a standard stream".to_string());
        }
        let source_fd = self.file.as_raw_fd();
        let identity = self.identity;
        // SAFETY: the retained File remains alive through spawn. dup3 runs only in the child and
        // may replace a descriptor inherited through Cargo without mutating the wrapper process.
        unsafe {
            command.pre_exec(move || {
                if source_fd != target_fd
                    && libc::dup3(source_fd, target_fd, libc::O_CLOEXEC) != target_fd
                {
                    return Err(io::Error::last_os_error());
                }
                let installed = BorrowedFd::borrow_raw(target_fd);
                let stat = rustix::fs::fstat(installed).map_err(io::Error::from)?;
                if !identity.matches_stat(&stat) {
                    return Err(io::Error::from_raw_os_error(
                        rustix::io::Errno::STALE.raw_os_error(),
                    ));
                }
                rustix::io::fcntl_setfd(installed, rustix::io::FdFlags::empty())
                    .map_err(io::Error::from)
            });
        }
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn fixed_child_path(
        &self,
        _target_fd: std::os::fd::RawFd,
    ) -> Result<PathBuf, String> {
        Err("fixed descriptor paths require Linux procfs".to_string())
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn inherit_for_child_at(
        &self,
        _command: &mut Command,
        _target_fd: std::os::fd::RawFd,
    ) -> Result<(), String> {
        Err("fixed descriptor inheritance requires Linux procfs".to_string())
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn replace_for_child_at(
        &self,
        _command: &mut Command,
        _target_fd: std::os::fd::RawFd,
    ) -> Result<(), String> {
        Err("fixed descriptor inheritance requires Linux procfs".to_string())
    }

    pub(crate) fn validate_path(&self, kind: &str) -> Result<(), String> {
        let (current, _) = open_absolute_directory(&self.display_path, false)?
            .ok_or_else(|| format!("{kind} path disappeared: {}", self.display_path.display()))?;
        let identity = DirectoryIdentity::from_file(&current)
            .map_err(|error| format!("failed to inspect current {kind}: {error}"))?;
        if identity != self.identity {
            return Err(format!(
                "{kind} path was substituted after it was pinned: {}",
                self.display_path.display()
            ));
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn require_unused_child_descriptor(descriptor: RawFd) -> Result<(), String> {
    if descriptor < 3 {
        return Err("fixed child descriptor would replace a standard stream".to_string());
    }
    let path = PathBuf::from(format!("/proc/self/fd/{descriptor}"));
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(format!(
            "fixed child descriptor {descriptor} is already in use"
        )),
        Err(error) => Err(format!(
            "failed to validate fixed child descriptor {descriptor}: {error}"
        )),
    }
}

fn open_absolute_directory(path: &Path, create: bool) -> Result<Option<(File, bool)>, String> {
    if !path.is_absolute() {
        return Err(format!(
            "directory path must be absolute: {}",
            path.display()
        ));
    }
    let mut current = open_ambient_dir(Path::new("/"), ambient_authority())
        .map_err(|error| format!("failed to open filesystem root: {error}"))?;
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(component) => Some(component),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut created_exact = false;

    for (index, component) in components.iter().enumerate() {
        match open_dir_nofollow(&current, Path::new(component)) {
            Ok(next) => current = next,
            Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
                create_dir(&current, Path::new(component), &DirOptions::new()).map_err(
                    |error| {
                        format!(
                            "failed to create directory component {} in {}: {error}",
                            component.to_string_lossy(),
                            path.display()
                        )
                    },
                )?;
                current = open_dir_nofollow(&current, Path::new(component)).map_err(|error| {
                    format!(
                        "failed to pin created directory component {} in {}: {error}",
                        component.to_string_lossy(),
                        path.display()
                    )
                })?;
                created_exact = index + 1 == components.len();
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "refusing directory path with a symlink or non-directory component {}: {error}",
                    path.display()
                ));
            }
        }
    }
    Ok(Some((current, created_exact)))
}

fn lexical_absolute(path: &Path, base: &Path) -> Result<PathBuf, String> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    let mut normalized = PathBuf::from("/");
    for component in joined.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(component) => normalized.push(component),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "path escapes the filesystem root: {}",
                        joined.display()
                    ));
                }
            }
            Component::Prefix(_) => {
                return Err(format!("unsupported path prefix in {}", joined.display()));
            }
        }
    }
    Ok(normalized)
}

fn validate_component(component: &str) -> Result<(), String> {
    if component.is_empty() || component == "." || component == ".." || component.contains('/') {
        return Err(format!("invalid directory component `{component}`"));
    }
    Ok(())
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectoryIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl DirectoryIdentity {
    fn from_file(file: &File) -> io::Result<Self> {
        use std::os::unix::fs::MetadataExt;

        let metadata = file.metadata()?;
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    fn matches_stat(self, stat: &rustix::fs::Stat) -> bool {
        self.device == stat.st_dev && self.inode == stat.st_ino
    }
}

#[cfg(not(unix))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectoryIdentity {
    len: u64,
}

#[cfg(not(unix))]
impl DirectoryIdentity {
    fn from_file(file: &File) -> io::Result<Self> {
        Ok(Self {
            len: file.metadata()?.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CargoRouting, PinnedDirectory, lexical_absolute};
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn routing_stops_at_the_application_separator() {
        let args = [
            "--manifest-path",
            "selected/Cargo.toml",
            "--package=member",
            "--",
            "--target-dir",
            "application-value",
        ]
        .map(OsString::from);

        assert_eq!(
            CargoRouting::parse(&args),
            Ok(CargoRouting {
                manifest_path: Some(OsString::from("selected/Cargo.toml")),
                target_dir: None,
                metadata_args: vec![
                    OsString::from("--manifest-path"),
                    OsString::from("selected/Cargo.toml"),
                ],
                config_args: vec![],
            })
        );
    }

    #[test]
    fn routing_rejects_ambiguous_or_missing_paths() {
        for args in [
            vec![OsString::from("--manifest-path")],
            vec![OsString::from("--target-dir=")],
            vec![
                OsString::from("--target-dir"),
                OsString::from("one"),
                OsString::from("--target-dir=two"),
            ],
        ] {
            assert!(CargoRouting::parse(&args).is_err(), "accepted {args:?}");
        }
    }

    #[test]
    fn lexical_paths_are_resolved_without_filesystem_canonicalization() {
        assert_eq!(
            lexical_absolute(Path::new("../target/custom"), Path::new("/work/member/src")),
            Ok(PathBuf::from("/work/member/target/custom"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn joined_routing_preserves_non_utf8_path_bytes() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let argument = OsString::from_vec(b"--target-dir=target/\xff".to_vec());
        let routing = CargoRouting::parse(&[argument]).expect("parse routing");

        assert_eq!(
            routing.target_dir.expect("target directory").as_bytes(),
            b"target/\xff"
        );
    }

    #[test]
    fn routing_forwards_invocation_configuration_to_metadata() {
        let args = [
            "--config",
            "build.target-dir='configured'",
            "--offline",
            "--package",
            "member",
        ]
        .map(OsString::from);
        let routing = CargoRouting::parse(&args).expect("parse routing");
        assert_eq!(
            routing.metadata_args,
            [
                OsString::from("--config"),
                OsString::from("build.target-dir='configured'"),
                OsString::from("--offline"),
            ]
        );
        assert_eq!(
            routing.config_args,
            [
                OsString::from("--config"),
                OsString::from("build.target-dir='configured'"),
                OsString::from("--offline"),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn retained_directory_rejects_rename_and_symlink_substitution() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "cargo-fe2o3-project-pin-{}-{}",
            process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let selected = root.join("selected");
        let relocated = root.join("relocated");
        let outside = root.join("outside");
        fs::create_dir_all(&selected).expect("create selected directory");
        fs::create_dir(&outside).expect("create outside directory");
        fs::write(outside.join("keep"), b"outside").expect("write outside sentinel");
        let pinned = PinnedDirectory::open_existing(selected.clone(), "selected directory")
            .expect("pin selected directory");

        fs::rename(&selected, &relocated).expect("relocate selected directory");
        symlink(&outside, &selected).expect("install replacement symlink");

        let error = pinned
            .validate_path("selected directory")
            .expect_err("substitution must be rejected");
        assert!(error.contains("symlink") || error.contains("substituted"));
        assert_eq!(
            fs::read(outside.join("keep")).expect("outside remains"),
            b"outside"
        );
        fs::remove_dir_all(&root).expect("remove test directory");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn transferred_artifact_requires_a_cloexec_directory_descriptor() {
        let root = std::env::temp_dir().join(format!(
            "cargo-fe2o3-project-transfer-{}-{}",
            process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create transfer test directory");
        let regular_path = root.join("regular");
        fs::write(&regular_path, b"not a directory").expect("write regular fixture");
        let regular = fs::File::open(&regular_path).expect("open regular fixture");
        assert!(
            PinnedDirectory::from_transferred_file(regular, "artifact")
                .expect_err("regular file must be rejected")
                .contains("not a directory")
        );

        let directory = PinnedDirectory::open_existing(root.clone(), "artifact")
            .expect("pin transfer directory")
            .into_file();
        rustix::io::fcntl_setfd(&directory, rustix::io::FdFlags::empty())
            .expect("clear close-on-exec for negative fixture");
        assert!(
            PinnedDirectory::from_transferred_file(directory, "artifact")
                .expect_err("inheritable directory must be rejected")
                .contains("not close-on-exec")
        );
        fs::remove_dir_all(&root).expect("remove transfer test directory");
    }
}
