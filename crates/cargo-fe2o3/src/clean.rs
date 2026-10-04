use crate::generation::{GenerationLock, validate_owned_artifact};
use crate::project::{CargoProject, PinnedDirectory};
#[cfg(test)]
use cap_primitives::ambient_authority;
#[cfg(unix)]
use cap_primitives::fs::remove_open_dir_all;
#[cfg(test)]
use cap_primitives::fs::{FollowSymlinks, open_ambient_dir, open_dir_nofollow, stat};
use std::ffi::{OsStr, OsString};
#[cfg(test)]
use std::fs;
use std::fs::File;
#[cfg(test)]
use std::io;
use std::path::{Path, PathBuf};

const GENERATED_COMPONENTS: &[&str] = &["target", "fe2o3"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CleanOptions {
    pub(crate) dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProjectCleanOptions {
    pub(crate) dry_run: bool,
}

#[derive(Debug)]
pub(crate) struct ProjectCleanPlan {
    project: CargoProject,
    target_path: PathBuf,
    target_dir: Option<PinnedDirectory>,
    artifact_dir: Option<PinnedDirectory>,
    _lock: Option<GenerationLock>,
}

#[derive(Debug)]
#[cfg(test)]
pub(crate) struct CleanPlan {
    workspace_root: PathBuf,
    workspace_dir: File,
    target: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CleanAction {
    Missing(PathBuf),
    WouldRemove(PathBuf),
    Removed(PathBuf),
}

impl CleanAction {
    pub(crate) fn diagnostic(&self) -> String {
        match self {
            Self::Missing(path) => format!(
                "cargo fe2o3 clean: no generated artifacts at {}",
                path.display()
            ),
            Self::WouldRemove(path) => {
                format!(
                    "cargo fe2o3 clean: would remove the opened artifact directory resolved from {}",
                    path.display()
                )
            }
            Self::Removed(path) => {
                format!(
                    "cargo fe2o3 clean: removed the opened artifact directory originally resolved from {}",
                    path.display()
                )
            }
        }
    }
}

pub(crate) fn parse_options(args: &[String]) -> Result<CleanOptions, String> {
    let mut dry_run = false;
    for arg in args {
        if arg == "--dry-run" && !dry_run {
            dry_run = true;
        } else {
            return Err(format!(
                "cargo fe2o3 clean: unexpected argument `{arg}`; expected only optional `--dry-run`"
            ));
        }
    }

    Ok(CleanOptions { dry_run })
}

pub(crate) fn parse_project_options(args: &[OsString]) -> Result<ProjectCleanOptions, String> {
    let mut dry_run_args = Vec::new();
    let mut index = 0;

    while index < args.len() {
        let argument = &args[index];
        if argument == "--dry-run" {
            dry_run_args.push("--dry-run".to_string());
        } else if argument == "--all-target" {
            return Err(
                "cargo fe2o3 clean: --all-target is unsupported because fe2o3 cannot authorize deleting sibling Cargo outputs; use an explicit `cargo clean` for broader cleanup"
                    .to_string(),
            );
        } else if matches!(
            argument.to_str(),
            Some("--manifest-path" | "--target-dir" | "--config" | "-Z")
        ) {
            index += 1;
            if args
                .get(index)
                .is_none_or(|value| value.is_empty() || os_bytes(value).first() == Some(&b'-'))
            {
                return Err(format!(
                    "cargo fe2o3 clean: {} requires a non-empty path",
                    argument.to_string_lossy()
                ));
            }
        } else if is_joined_path_option(argument, "--manifest-path")
            || is_joined_path_option(argument, "--target-dir")
            || is_joined_path_option(argument, "--config")
            || os_bytes(argument).starts_with(b"-Z") && os_bytes(argument).len() > 2
            || matches!(
                argument.to_str(),
                Some("--locked" | "--offline" | "--frozen")
            )
        {
        } else {
            return Err(format!(
                "cargo fe2o3 clean: unexpected or ambiguous argument {argument:?}; expected cleanup options and Cargo routing/configuration options"
            ));
        }
        index += 1;
    }

    let dry_run = parse_options(&dry_run_args)?.dry_run;
    Ok(ProjectCleanOptions { dry_run })
}

pub(crate) fn plan_project(project: CargoProject) -> Result<ProjectCleanPlan, String> {
    let target_path = project.target_path().to_path_buf();
    let target_dir = project.open_target()?;
    let mut artifact_dir = None;
    if let Some(target_dir) = &target_dir {
        artifact_dir =
            target_dir.open_child(GENERATED_COMPONENTS[1], "fe2o3 artifact directory")?;
    }
    let lock = artifact_dir
        .as_ref()
        .map(|_| GenerationLock::acquire(target_dir.as_ref().expect("artifact has target parent")))
        .transpose()?;
    if let Some(artifact) = &artifact_dir {
        artifact.validate_path("fe2o3 artifact directory")?;
        validate_owned_artifact(artifact)?;
    }
    Ok(ProjectCleanPlan {
        project,
        target_path,
        target_dir,
        artifact_dir,
        _lock: lock,
    })
}

pub(crate) fn execute_project(
    plan: ProjectCleanPlan,
    options: ProjectCleanOptions,
) -> Result<Vec<CleanAction>, String> {
    plan.project.validate_paths()?;
    let Some(target_dir) = plan.target_dir else {
        return Ok(vec![CleanAction::Missing(
            plan.target_path.join(GENERATED_COMPONENTS[1]),
        )]);
    };
    target_dir.validate_path("Cargo target directory")?;

    let generated_path = plan.target_path.join(GENERATED_COMPONENTS[1]);
    let Some(generated_dir) = plan.artifact_dir else {
        return Ok(vec![CleanAction::Missing(generated_path)]);
    };
    generated_dir.validate_path("fe2o3 artifact directory")?;
    validate_owned_artifact(&generated_dir)?;
    if options.dry_run {
        return Ok(vec![CleanAction::WouldRemove(generated_path)]);
    }
    remove_generated_dir(generated_dir.into_file(), &generated_path)?;
    Ok(vec![CleanAction::Removed(generated_path)])
}

fn is_joined_path_option(argument: &OsStr, option: &str) -> bool {
    let bytes = os_bytes(argument);
    let option = option.as_bytes();
    bytes.len() > option.len() + 1
        && bytes.starts_with(option)
        && bytes.get(option.len()) == Some(&b'=')
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
        .expect("clean options must be UTF-8 off Unix")
        .as_bytes()
}

#[cfg(test)]
pub(crate) fn plan(workspace_root: &Path) -> Result<CleanPlan, String> {
    let workspace_root = fs::canonicalize(workspace_root).map_err(|error| {
        format!(
            "failed to resolve Cargo project/workspace root {}: {error}",
            workspace_root.display()
        )
    })?;
    let workspace_dir =
        open_ambient_dir(&workspace_root, ambient_authority()).map_err(|error| {
            format!(
                "failed to open Cargo project/workspace root {}: {error}",
                workspace_root.display()
            )
        })?;

    let target = GENERATED_COMPONENTS
        .iter()
        .fold(workspace_root.clone(), |path, component| {
            path.join(component)
        });

    Ok(CleanPlan {
        workspace_root,
        workspace_dir,
        target,
    })
}

#[cfg(test)]
pub(crate) fn execute(plan: &CleanPlan, options: CleanOptions) -> Result<Vec<CleanAction>, String> {
    let Some(generated_dir) = open_generated_dir(plan)? else {
        return Ok(vec![CleanAction::Missing(plan.target.clone())]);
    };

    if options.dry_run {
        return Ok(vec![CleanAction::WouldRemove(plan.target.clone())]);
    }

    #[cfg(unix)]
    {
        remove_generated_dir(generated_dir, &plan.target)?;
        Ok(vec![CleanAction::Removed(plan.target.clone())])
    }

    #[cfg(not(unix))]
    {
        drop(generated_dir);
        Err(
            "cargo fe2o3 clean: destructive cleanup is unsupported on this platform; use --dry-run"
                .to_string(),
        )
    }
}

#[cfg(test)]
fn open_generated_dir(plan: &CleanPlan) -> Result<Option<File>, String> {
    let target_path = plan.workspace_root.join(GENERATED_COMPONENTS[0]);
    let Some(target_dir) =
        open_component_nofollow(&plan.workspace_dir, GENERATED_COMPONENTS[0], &target_path)?
    else {
        return Ok(None);
    };

    open_component_nofollow(&target_dir, GENERATED_COMPONENTS[1], &plan.target)
}

// A successful no-follow open is the authority. Metadata is consulted only
// after failure to produce a fail-closed diagnostic.
#[cfg(test)]
fn open_component_nofollow(
    parent: &File,
    component: &str,
    display_path: &Path,
) -> Result<Option<File>, String> {
    match open_dir_nofollow(parent, Path::new(component)) {
        Ok(directory) => Ok(Some(directory)),
        Err(open_error) => match stat(parent, Path::new(component), FollowSymlinks::No) {
            Err(stat_error)
                if open_error.kind() == io::ErrorKind::NotFound
                    && stat_error.kind() == io::ErrorKind::NotFound =>
            {
                Ok(None)
            }
            Ok(metadata) if metadata.is_symlink() => Err(format!(
                "refusing to clean symlinked path component {}",
                display_path.display()
            )),
            Ok(metadata) if !metadata.is_dir() => Err(format!(
                "refusing to clean non-directory path component {}",
                display_path.display()
            )),
            Ok(_) => Err(format!(
                "failed to open cleanup path component without following symlinks {}: {open_error}",
                display_path.display()
            )),
            Err(stat_error) => Err(format!(
                "failed to open cleanup path component {}: {open_error}; diagnostic inspection failed: {stat_error}",
                display_path.display()
            )),
        },
    }
}

#[cfg(unix)]
fn remove_generated_dir(generated_dir: File, display_path: &Path) -> Result<(), String> {
    remove_open_dir_all(generated_dir).map_err(|error| {
        format!(
            "failed to remove opened fe2o3-generated directory {}: {error}",
            display_path.display()
        )
    })
}

#[cfg(not(unix))]
fn remove_generated_dir(generated_dir: File, _display_path: &Path) -> Result<(), String> {
    drop(generated_dir);
    Err(
        "cargo fe2o3 clean: destructive cleanup is unsupported on this platform; use --dry-run"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::remove_generated_dir;
    use super::{
        CleanAction, CleanOptions, ProjectCleanOptions, execute, open_component_nofollow,
        open_generated_dir, parse_options, parse_project_options, plan,
    };
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            loop {
                let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
                let path =
                    env::temp_dir().join(format!("cargo-fe2o3-clean-test-{}-{id}", process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => return Self { path },
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("failed to create temporary directory: {error}"),
                }
            }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn workspace(temp: &TempDir) -> PathBuf {
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).expect("create workspace");
        workspace
    }

    fn generated_dir(workspace: &Path) -> PathBuf {
        workspace.join("target").join("fe2o3")
    }

    fn options(dry_run: bool) -> CleanOptions {
        CleanOptions { dry_run }
    }

    #[test]
    fn parses_clean_options_strictly() {
        assert_eq!(parse_options(&[]), Ok(options(false)));
        assert_eq!(parse_options(&["--dry-run".to_string()]), Ok(options(true)));

        for args in [
            vec!["--force".to_string()],
            vec!["target/fe2o3".to_string()],
            vec!["--dry-run=true".to_string()],
            vec!["--dry-run".to_string(), "--dry-run".to_string()],
        ] {
            assert!(parse_options(&args).is_err(), "accepted {args:?}");
        }

        assert_eq!(
            parse_options(&["--dry-run".to_string(), "--force".to_string()]),
            Err(
                "cargo fe2o3 clean: unexpected argument `--force`; expected only optional `--dry-run`"
                    .to_string()
            )
        );
    }

    #[test]
    fn parses_project_cleanup_options_and_rejects_broad_selectors() {
        assert_eq!(
            parse_project_options(&[
                OsString::from("--dry-run"),
                OsString::from("--manifest-path=member/Cargo.toml"),
                OsString::from("--target-dir"),
                OsString::from("custom-target"),
            ]),
            Ok(ProjectCleanOptions { dry_run: true })
        );

        for args in [
            vec![OsString::from("--all-target")],
            vec![OsString::from("--package"), OsString::from("member")],
            vec![OsString::from("--workspace")],
            vec![OsString::from("--manifest-path=")],
            vec![OsString::from("--target-dir")],
        ] {
            assert!(parse_project_options(&args).is_err(), "accepted {args:?}");
        }
    }

    #[test]
    #[cfg(unix)]
    fn removes_generated_directory_and_reports_action() {
        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let generated = generated_dir(&workspace);
        fs::create_dir_all(generated.join("nested")).expect("create generated directory");
        fs::write(generated.join("nested/kernel.hsaco"), b"artifact").expect("write artifact");

        let plan = plan(&workspace).expect("plan cleanup");
        let actions = execute(&plan, options(false)).expect("execute cleanup");

        assert_eq!(actions, [CleanAction::Removed(generated.clone())]);
        assert!(!generated.exists());
        assert_eq!(
            actions[0].diagnostic(),
            format!(
                "cargo fe2o3 clean: removed the opened artifact directory originally resolved from {}",
                generated.display()
            )
        );
    }

    #[test]
    fn dry_run_preserves_generated_directory_and_reports_intent() {
        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let generated = generated_dir(&workspace);
        fs::create_dir_all(&generated).expect("create generated directory");
        fs::write(generated.join("kernel.hsaco"), b"artifact").expect("write artifact");

        let actions = execute(&plan(&workspace).expect("plan cleanup"), options(true))
            .expect("execute dry run");

        assert_eq!(actions, [CleanAction::WouldRemove(generated.clone())]);
        assert!(generated.join("kernel.hsaco").is_file());
        assert_eq!(
            actions[0].diagnostic(),
            format!(
                "cargo fe2o3 clean: would remove the opened artifact directory resolved from {}",
                generated.display()
            )
        );
    }

    #[test]
    fn missing_directory_is_idempotent() {
        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let generated = generated_dir(&workspace);
        let plan = plan(&workspace).expect("plan cleanup");

        let first = execute(&plan, options(false)).expect("first cleanup");
        let second = execute(&plan, options(false)).expect("second cleanup");

        let expected = [CleanAction::Missing(generated.clone())];
        assert_eq!(first, expected);
        assert_eq!(second, expected);
        assert_eq!(
            first[0].diagnostic(),
            format!(
                "cargo fe2o3 clean: no generated artifacts at {}",
                generated.display()
            )
        );
    }

    #[test]
    #[cfg(unix)]
    fn preserves_unrelated_target_artifacts() {
        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let generated = generated_dir(&workspace);
        let unrelated = workspace.join("target/debug/unrelated");
        fs::create_dir_all(&generated).expect("create generated directory");
        fs::create_dir_all(unrelated.parent().expect("unrelated parent"))
            .expect("create unrelated directory");
        fs::write(generated.join("kernel.hsaco"), b"artifact").expect("write artifact");
        fs::write(&unrelated, b"keep").expect("write unrelated artifact");

        execute(&plan(&workspace).expect("plan cleanup"), options(false)).expect("execute cleanup");

        assert!(!generated.exists());
        assert_eq!(
            fs::read(&unrelated).expect("read unrelated artifact"),
            b"keep"
        );
        assert!(workspace.join("target").is_dir());
    }

    #[test]
    fn refuses_file_at_cleanup_root() {
        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let generated = generated_dir(&workspace);
        fs::create_dir_all(generated.parent().expect("generated parent"))
            .expect("create target directory");
        fs::write(&generated, b"not a directory").expect("write cleanup-root file");

        let error = execute(&plan(&workspace).expect("plan cleanup"), options(false))
            .expect_err("file cleanup root must be rejected");

        assert!(error.contains("refusing to clean non-directory path component"));
        assert_eq!(
            fs::read(generated).expect("file remains"),
            b"not a directory"
        );
    }

    #[test]
    fn refuses_file_in_cleanup_path() {
        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let target = workspace.join("target");
        fs::write(&target, b"not a directory").expect("write target file");

        let error = execute(&plan(&workspace).expect("plan cleanup"), options(false))
            .expect_err("file in cleanup path must be rejected");

        assert!(error.contains("refusing to clean non-directory path component"));
        assert_eq!(
            fs::read(target).expect("parent file remains"),
            b"not a directory"
        );
    }

    #[cfg(not(unix))]
    #[test]
    fn destructive_cleanup_fails_closed_off_unix() {
        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let generated = generated_dir(&workspace);
        fs::create_dir_all(&generated).expect("create generated directory");
        fs::write(generated.join("kernel.hsaco"), b"artifact").expect("write artifact");

        let error = execute(&plan(&workspace).expect("plan cleanup"), options(false))
            .expect_err("destructive cleanup must fail closed");

        assert_eq!(
            error,
            "cargo fe2o3 clean: destructive cleanup is unsupported on this platform; use --dry-run"
        );
        assert!(generated.join("kernel.hsaco").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_cleanup_root_to_outside() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let outside = temp.path().join("outside");
        fs::create_dir_all(
            generated_dir(&workspace)
                .parent()
                .expect("generated parent"),
        )
        .expect("create target directory");
        fs::create_dir(&outside).expect("create outside directory");
        fs::write(outside.join("keep"), b"outside").expect("write outside file");
        symlink(&outside, generated_dir(&workspace)).expect("create cleanup-root symlink");

        let error = execute(&plan(&workspace).expect("plan cleanup"), options(false))
            .expect_err("symlinked cleanup root must be rejected");

        assert!(error.contains("refusing to clean symlinked path component"));
        assert_eq!(
            fs::read(outside.join("keep")).expect("outside file remains"),
            b"outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_parent_resolving_outside_workspace() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let outside_target = temp.path().join("outside-target");
        fs::create_dir_all(outside_target.join("fe2o3")).expect("create outside cleanup root");
        fs::write(outside_target.join("fe2o3/keep"), b"outside").expect("write outside file");
        symlink(&outside_target, workspace.join("target")).expect("create target symlink");

        let error = execute(&plan(&workspace).expect("plan cleanup"), options(false))
            .expect_err("outside cleanup root must be rejected");

        assert!(error.contains("refusing to clean symlinked path component"));
        assert_eq!(
            fs::read(outside_target.join("fe2o3/keep")).expect("outside file remains"),
            b"outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_parent_resolving_inside_workspace() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let alternate_target = workspace.join("alternate-target");
        fs::create_dir_all(alternate_target.join("fe2o3")).expect("create alternate cleanup root");
        fs::write(alternate_target.join("fe2o3/keep"), b"inside").expect("write inside file");
        symlink(&alternate_target, workspace.join("target")).expect("create target symlink");

        let error = execute(&plan(&workspace).expect("plan cleanup"), options(false))
            .expect_err("in-workspace parent symlink must be rejected");

        assert!(error.contains("refusing to clean symlinked path component"));
        assert_eq!(
            fs::read(alternate_target.join("fe2o3/keep")).expect("inside file remains"),
            b"inside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_internal_symlink_during_removal() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let generated = generated_dir(&workspace);
        let outside = temp.path().join("outside");
        fs::create_dir_all(&generated).expect("create generated directory");
        fs::create_dir(&outside).expect("create outside directory");
        fs::write(outside.join("keep"), b"outside").expect("write outside file");
        symlink(&outside, generated.join("outside-link")).expect("create internal symlink");

        execute(&plan(&workspace).expect("plan cleanup"), options(false)).expect("execute cleanup");

        assert!(!generated.exists());
        assert_eq!(
            fs::read(outside.join("keep")).expect("outside file remains"),
            b"outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn opened_directory_substitution_cannot_redirect_removal() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let generated = generated_dir(&workspace);
        let relocated = workspace.join("target/relocated-fe2o3");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&generated).expect("create generated directory");
        fs::write(generated.join("artifact"), b"generated").expect("write generated artifact");
        fs::create_dir(&outside).expect("create outside directory");
        fs::write(outside.join("keep"), b"outside").expect("write outside file");

        let plan = plan(&workspace).expect("plan cleanup");
        let opened = open_generated_dir(&plan)
            .expect("open generated directory")
            .expect("generated directory exists");
        fs::rename(&generated, &relocated).expect("relocate opened directory");
        symlink(&outside, &generated).expect("substitute outside symlink");

        remove_generated_dir(opened, &generated).expect("remove opened directory");

        assert!(!relocated.exists());
        assert!(
            fs::symlink_metadata(&generated)
                .expect("replacement remains")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read(outside.join("keep")).expect("outside file remains"),
            b"outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn opened_workspace_root_cannot_be_redirected_by_path_substitution() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let relocated_workspace = temp.path().join("relocated-workspace");
        let outside = temp.path().join("outside");
        let generated = generated_dir(&workspace);
        fs::create_dir_all(&generated).expect("create generated directory");
        fs::write(generated.join("artifact"), b"generated").expect("write generated artifact");
        fs::create_dir_all(outside.join("target/fe2o3")).expect("create outside directory");
        fs::write(outside.join("target/fe2o3/keep"), b"outside").expect("write outside file");

        let plan = plan(&workspace).expect("plan cleanup and open workspace root");
        fs::rename(&workspace, &relocated_workspace).expect("relocate opened workspace");
        symlink(&outside, &workspace).expect("substitute outside workspace symlink");

        let actions = execute(&plan, options(false)).expect("execute anchored cleanup");

        assert_eq!(actions, [CleanAction::Removed(generated)]);
        assert!(!relocated_workspace.join("target/fe2o3").exists());
        assert_eq!(
            fs::read(outside.join("target/fe2o3/keep")).expect("outside file remains"),
            b"outside"
        );
    }

    #[cfg(unix)]
    #[test]
    fn ordinary_directory_substitution_after_open_cannot_switch_target() {
        let temp = TempDir::new();
        let workspace = workspace(&temp);
        let original_target = workspace.join("target");
        let relocated_target = workspace.join("relocated-target");
        let replacement_generated = workspace.join("target/fe2o3");
        fs::create_dir_all(original_target.join("fe2o3")).expect("create original generated dir");
        fs::write(original_target.join("fe2o3/original"), b"original")
            .expect("write original artifact");

        let plan = plan(&workspace).expect("plan cleanup");
        let opened_target =
            open_component_nofollow(&plan.workspace_dir, "target", &original_target)
                .expect("open target")
                .expect("target exists");
        fs::rename(&original_target, &relocated_target).expect("relocate opened target");
        fs::create_dir_all(&replacement_generated).expect("create replacement generated dir");
        fs::write(replacement_generated.join("keep"), b"replacement")
            .expect("write replacement artifact");

        let opened_generated =
            open_component_nofollow(&opened_target, "fe2o3", &relocated_target.join("fe2o3"))
                .expect("open generated directory relative to anchored target")
                .expect("generated directory exists");
        remove_generated_dir(opened_generated, &relocated_target.join("fe2o3"))
            .expect("remove opened generated directory");

        assert!(!relocated_target.join("fe2o3").exists());
        assert_eq!(
            fs::read(replacement_generated.join("keep")).expect("replacement remains"),
            b"replacement"
        );
    }
}
