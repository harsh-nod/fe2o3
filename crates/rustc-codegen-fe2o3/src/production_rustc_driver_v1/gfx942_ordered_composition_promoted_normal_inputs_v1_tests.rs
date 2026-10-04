//! Source bytes are checked as inputs, never admitted as source custody.
use super::*;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
const DYNAMIC: &str = "#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]";
const FINITE: &str =
    "#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1]))]";
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Snapshot {
    pub(super) bytes: usize,
    pub(super) sha256: String,
    device: u64,
    inode: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}
fn stamp(m: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
pub(super) fn snapshot(path: &Path, cap: usize) -> Snapshot {
    let before = fs::symlink_metadata(path).unwrap();
    assert!(before.is_file() && !before.file_type().is_symlink());
    assert!(before.len() > 0 && before.len() <= cap as u64);
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .unwrap();
    assert_eq!(stamp(&before), stamp(&file.metadata().unwrap()));
    let mut bytes = Vec::new();
    (&mut file)
        .take(cap as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len() as u64, before.len());
    assert!(bytes.len() <= cap);
    assert_eq!(stamp(&before), stamp(&file.metadata().unwrap()));
    assert_eq!(stamp(&before), stamp(&fs::symlink_metadata(path).unwrap()));
    Snapshot {
        bytes: bytes.len(),
        sha256: digest(&bytes),
        device: before.dev(),
        inode: before.ino(),
        mtime: before.mtime(),
        mtime_ns: before.mtime_nsec(),
        ctime: before.ctime(),
        ctime_ns: before.ctime_nsec(),
    }
}
pub(super) fn finite_seed(bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    if bytes.len() > 64 * 1024 {
        return Err("seed byte cap");
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "seed UTF8")?;
    if text.matches(DYNAMIC).count() != 1 || text.contains("max_grid") {
        return Err("exact original dynamic launch attribute");
    }
    let next = text.replacen(DYNAMIC, FINITE, 1).into_bytes();
    if next.len() > 64 * 1024 {
        return Err("finite seed byte cap");
    }
    Ok(next)
}
pub(super) fn package(variant: &str) -> &'static str {
    match variant {
        "original" => "package-original",
        "copy" => "package-promoted-copy",
        "preserve" => "package-promoted-preserve",
        "edit" => "package-promoted-edit",
        _ => panic!("closed promoted package"),
    }
}
pub(super) fn source(root: &Path, variant: &str) -> PathBuf {
    root.join(package(variant)).join(staging::LEAF)
}
pub(super) fn feature(case: &str) -> &'static str {
    match case {
        "capture-const" | "diagnostic-const" => "ordered-composition-publish-const",
        "capture-local" | "diagnostic-local" => "ordered-composition-publish-local",
        "capture-wrapper" | "diagnostic-wrapper" => "ordered-composition-publish-wrapper",
        _ => "ordered-composition-publish-direct",
    }
}
pub(super) fn invocation(root: &Path, case: &str, variant: &str) -> Invocation {
    publisher::derive_package(root, case, package(variant), feature(case))
}
pub(super) fn diagnostic(root: &Path, case: &str) -> PathBuf {
    match case {
        "diagnostic" | "copy" | "preserve" | "edit" | "stale-semantic" | "stale-canonical"
        | "stale-source" => root.join("diagnostic-initial"),
        "diagnostic-const" | "capture-const" => root.join("diagnostic-const"),
        "diagnostic-local" | "capture-local" => root.join("diagnostic-local"),
        "diagnostic-wrapper" | "capture-wrapper" => root.join("diagnostic-wrapper"),
        _ => panic!("closed diagnostic source"),
    }
}
pub(super) fn output(root: &Path, case: &str) -> PathBuf {
    if case.starts_with("diagnostic") {
        diagnostic(root, case)
    } else {
        root.join(format!("action-{case}"))
    }
}
pub(super) fn candidate(case: &str) -> String {
    match case {
        "copy" | "preserve" | "edit" => format!("{}/{}", package(case), staging::LEAF),
        "stale-semantic" | "stale-canonical" | "stale-source" | "capture-const"
        | "capture-local" | "capture-wrapper" => format!("refused-{case}.rs"),
        _ => panic!("closed candidate case"),
    }
}
pub(super) fn environment(command: &mut Command, root: &Path, record: &Invocation) {
    super::super::common_environment(command, root, record);
}
pub(super) fn write_invocation(root: &Path, name: &str, record: &Invocation) {
    let bytes = serde_json::to_vec_pretty(record).unwrap();
    assert!(bytes.len() <= 256 * 1024);
    publisher::create(
        &root.join(format!("{name}.invocation.json")),
        &bytes,
        256 * 1024,
    );
}
pub(super) fn recheck(
    root: &Path,
    case: &str,
    variant: &str,
    record: &Invocation,
    source: &Snapshot,
) {
    assert_eq!(invocation(root, case, variant), *record);
    assert_eq!(snapshot(&self::source(root, variant), 72 * 1024), *source);
    assert!(
        fs::read_dir(publisher::preparation(root, package(variant)).join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
}
#[test]
fn finite_input_is_one_explicit_prepublication_edit_only() {
    let original = staging::fixture_source();
    let next = finite_seed(&original).unwrap();
    assert_ne!(original, next);
    assert_eq!(
        String::from_utf8(next.clone())
            .unwrap()
            .replace(FINITE, DYNAMIC)
            .as_bytes(),
        original
    );
    assert!(finite_seed(&next).is_err());
    assert!(finite_seed(b"not a launch").is_err());
    let doubled = [original.as_slice(), original.as_slice()].concat();
    assert!(finite_seed(&doubled).is_err());
    assert!(finite_seed(&vec![b' '; 64 * 1024 + 1]).is_err());
}

/// Closed task-output setup: all caller-path refusals precede mkdir. This is
/// a filesystem currentness check under the root-owned no-concurrent-mutation
/// workflow, not an adversarial filesystem isolation or whole-family claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryIdentity {
    device: u64,
    inode: u64,
}
fn directory_identity(metadata: &fs::Metadata) -> Result<DirectoryIdentity, &'static str> {
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("output directory type");
    }
    Ok(DirectoryIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}
fn open_directory(path: &Path) -> Result<(fs::File, DirectoryIdentity), &'static str> {
    let before =
        directory_identity(&fs::symlink_metadata(path).map_err(|_| "output directory stat")?)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "output directory open")?;
    if directory_identity(&file.metadata().map_err(|_| "output directory fd stat")?)? != before {
        return Err("output directory changed during open");
    }
    Ok((file, before))
}
struct OutputRoot {
    intended: PathBuf,
    parent: PathBuf,
    parent_fd: fs::File,
    parent_identity: DirectoryIdentity,
    excluded: PathBuf,
    excluded_fd: fs::File,
    excluded_identity: DirectoryIdentity,
}
impl OutputRoot {
    fn current(&self) -> Result<(), &'static str> {
        for (path, file, expected) in [
            (&self.parent, &self.parent_fd, self.parent_identity),
            (&self.excluded, &self.excluded_fd, self.excluded_identity),
        ] {
            if directory_identity(
                &file
                    .metadata()
                    .map_err(|_| "output retained directory stat")?,
            )? != expected
                || directory_identity(
                    &fs::symlink_metadata(path).map_err(|_| "output current directory stat")?,
                )? != expected
                || path
                    .canonicalize()
                    .map_err(|_| "output directory canonical")?
                    != *path
            {
                return Err("output directory identity changed");
            }
        }
        Ok(())
    }
}
fn output_root(requested: &Path, excluded: &Path) -> Result<OutputRoot, &'static str> {
    use std::path::Component;
    if !requested.is_absolute()
        || requested.as_os_str().len() > 4096
        || requested
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err("output root path");
    }
    let leaf = requested.file_name().ok_or("output root leaf")?;
    let parent = requested
        .parent()
        .ok_or("output root parent")?
        .canonicalize()
        .map_err(|_| "output root parent missing")?;
    let intended = parent.join(leaf);
    let excluded = excluded
        .canonicalize()
        .map_err(|_| "excluded candidate parent")?;
    if intended.starts_with(&excluded) || parent.starts_with(&excluded) {
        return Err("output root inside candidates");
    }
    if intended != requested {
        return Err("output root is not its canonical intended location");
    }
    match fs::symlink_metadata(&intended) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err("output root is not fresh"),
    }
    let (parent_fd, parent_identity) = open_directory(&parent)?;
    let (excluded_fd, excluded_identity) = open_directory(&excluded)?;
    let plan = OutputRoot {
        intended,
        parent,
        parent_fd,
        parent_identity,
        excluded,
        excluded_fd,
        excluded_identity,
    };
    plan.current()?;
    Ok(plan)
}
fn create_output_root(plan: OutputRoot) -> Result<PathBuf, &'static str> {
    // Recheck immediately before the sole mutation; never operate on the
    // caller's alias after validating a different canonical parent.
    plan.current()?;
    fs::create_dir(&plan.intended).map_err(|_| "fresh output root creation")?;
    let (created, identity) = open_directory(&plan.intended)?;
    plan.current()?;
    if plan
        .intended
        .canonicalize()
        .map_err(|_| "created output canonical")?
        != plan.intended
        || directory_identity(&created.metadata().map_err(|_| "created output fd stat")?)?
            != identity
        || directory_identity(
            &fs::symlink_metadata(&plan.intended).map_err(|_| "created output current stat")?,
        )? != identity
    {
        return Err("created output root identity differs");
    }
    // Any post-creation refusal intentionally leaves the new directory retained.
    // It does not claim no effect, retry, remove it, or publish positive evidence.
    Ok(plan.intended)
}
pub(super) fn fresh_output_root(
    requested: &Path,
    excluded: &Path,
) -> Result<PathBuf, &'static str> {
    create_output_root(output_root(requested, excluded)?)
}

#[cfg(test)]
struct OutputScratch(PathBuf);
#[cfg(test)]
impl OutputScratch {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = parent.join(format!(
            "fe2o3-promoted-output-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
#[cfg(test)]
impl Drop for OutputScratch {
    fn drop(&mut self) {
        // This is only the exact fresh directory created by this test helper.
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn output_root_refusals_have_no_candidate_directory_effect() {
    use std::os::unix::fs::symlink;
    let scratch = OutputScratch::new();
    let candidates = scratch.0.join("candidates");
    fs::create_dir(&candidates).unwrap();
    let alias = scratch.0.join("candidate-alias");
    symlink(&candidates, &alias).unwrap();
    let before = directory_identity(&fs::symlink_metadata(&candidates).unwrap()).unwrap();
    for requested in [
        candidates.join("forbidden"),
        candidates.clone(),
        alias.join("forbidden"),
        scratch.0.join("missing-parent/output"),
        PathBuf::from("relative-output"),
        scratch.0.join("other/../candidates/forbidden"),
    ] {
        assert!(fresh_output_root(&requested, &candidates).is_err());
        assert_eq!(
            directory_identity(&fs::symlink_metadata(&candidates).unwrap()).unwrap(),
            before
        );
        assert_eq!(fs::read_dir(&candidates).unwrap().count(), 0);
        assert!(!scratch.0.join("missing-parent").exists());
    }
    let existing = scratch.0.join("existing");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("sentinel"), b"unchanged").unwrap();
    assert!(fresh_output_root(&existing, &candidates).is_err());
    assert_eq!(fs::read(existing.join("sentinel")).unwrap(), b"unchanged");
}
#[test]
fn output_root_creation_joins_exact_new_directory_and_retained_parent() {
    let scratch = OutputScratch::new();
    let candidates = scratch.0.join("candidates");
    fs::create_dir(&candidates).unwrap();
    let requested = scratch.0.join("new-output");
    let created = fresh_output_root(&requested, &candidates).unwrap();
    assert_eq!(created, requested);
    assert_eq!(created.canonicalize().unwrap(), requested);
    let (fd, identity) = open_directory(&created).unwrap();
    assert_eq!(
        directory_identity(&fd.metadata().unwrap()).unwrap(),
        identity
    );
    assert_eq!(
        directory_identity(&fs::symlink_metadata(&created).unwrap()).unwrap(),
        identity
    );
    assert_eq!(fs::read_dir(&created).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&candidates).unwrap().count(), 0);
    assert!(fresh_output_root(&requested, &candidates).is_err());
}
#[test]
fn output_parent_replacement_is_refused_before_creation() {
    let scratch = OutputScratch::new();
    let candidates = scratch.0.join("candidates");
    let parent = scratch.0.join("parent");
    fs::create_dir(&candidates).unwrap();
    fs::create_dir(&parent).unwrap();
    let plan = output_root(&parent.join("output"), &candidates).unwrap();
    fs::rename(&parent, scratch.0.join("retained-parent")).unwrap();
    fs::create_dir(&parent).unwrap();
    assert_eq!(
        create_output_root(plan),
        Err("output directory identity changed")
    );
    assert!(!parent.join("output").exists());
    assert!(!scratch.0.join("retained-parent/output").exists());
    assert_eq!(fs::read_dir(&candidates).unwrap().count(), 0);
}
