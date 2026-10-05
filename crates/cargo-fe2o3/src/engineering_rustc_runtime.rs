//! Observation of the existing canonical rustc library-tree digest, without admission authority.

use crate::project::PinnedDirectory;
use crate::rustc_lib_tree::PinnedRustcLibTree;
use serde::Serialize;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str =
    "cargo fe2o3 engineering rustc-runtime --lib-tree <absolute-canonical-directory>";
const MAX_PATH_BYTES: usize = 4096;

pub(crate) fn command(args: &[OsString]) -> ExitCode {
    match observe(args) {
        Ok(observation) => {
            println!("{observation}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("cargo fe2o3 engineering rustc-runtime: {error}");
            ExitCode::FAILURE
        }
    }
}

fn parse_path(args: &[OsString]) -> Result<PathBuf, String> {
    let [option, argument] = args else {
        return Err(USAGE.to_owned());
    };
    if option != "--lib-tree" {
        return Err(USAGE.to_owned());
    }
    let text = argument
        .to_str()
        .ok_or_else(|| "library-tree path must be UTF-8".to_owned())?;
    let path = Path::new(text);
    if text.is_empty() || text.len() > MAX_PATH_BYTES || text.contains('\0') || !path.is_absolute()
    {
        return Err("library-tree path must be a bounded absolute directory path".to_owned());
    }
    if path
        .components()
        .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
        || path.components().collect::<PathBuf>().as_os_str() != argument
    {
        return Err("library-tree path must be lexically canonical".to_owned());
    }
    Ok(path.to_path_buf())
}

fn observe(args: &[OsString]) -> Result<String, String> {
    let path = parse_path(args)?;
    let directory = PinnedDirectory::open_existing(path, "observed rustc library tree")?;
    let tree = PinnedRustcLibTree::pin(directory)?;
    encode_observation(&tree)
}

#[derive(Serialize)]
struct RustcLibTreeObservationV1<'a> {
    schema: &'static str,
    lib_tree: &'a Path,
    sha256: String,
    authority: bool,
    complete_elf_closure: bool,
}

fn encode_observation(tree: &PinnedRustcLibTree) -> Result<String, String> {
    tree.revalidate()?;
    let observation = RustcLibTreeObservationV1 {
        schema: "RustcLibTreeObservationV1",
        lib_tree: tree.directory().display_path(),
        sha256: tree
            .sha256()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        authority: false,
        complete_elf_closure: false,
    };
    serde_json::to_string(&observation)
        .map_err(|error| format!("cannot encode rustc library-tree observation: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pinned_executable_test_directory::TestDirectory;
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::symlink;

    fn args(path: &Path) -> [OsString; 2] {
        [OsString::from("--lib-tree"), path.as_os_str().to_owned()]
    }

    #[test]
    fn observation_uses_canonical_production_digest_without_authority() {
        let root = TestDirectory::new();
        let path = root.path().join("lib");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("librustc_driver.so"), b"observed library bytes").unwrap();
        let tree = PinnedRustcLibTree::pin(
            PinnedDirectory::open_existing(path.clone(), "independent production pin").unwrap(),
        )
        .unwrap();
        let actual: serde_json::Value =
            serde_json::from_str(&observe(&args(&path)).unwrap()).unwrap();
        assert_eq!(actual["schema"], "RustcLibTreeObservationV1");
        assert_eq!(actual["lib_tree"], path.to_str().unwrap());
        let expected: String = tree
            .sha256()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual["sha256"], expected);
        assert_eq!(actual["authority"], false);
        assert_eq!(actual["complete_elf_closure"], false);
        assert_eq!(actual.as_object().unwrap().len(), 5);
        tree.revalidate().unwrap();
    }

    #[test]
    fn observation_rejects_ambiguous_arguments_and_paths_before_pinning() {
        for invalid in [
            vec![],
            vec!["--lib-tree"],
            vec!["/tmp"],
            vec!["--other", "/tmp"],
            vec!["--lib-tree", "/tmp", "extra"],
            vec!["--lib-tree", "relative"],
            vec!["--lib-tree", "/tmp/../lib"],
            vec!["--lib-tree", "/tmp/./lib"],
            vec!["--lib-tree", "/tmp//lib"],
            vec!["--lib-tree", "/tmp/"],
            vec!["--lib-tree", ""],
            vec!["--lib-tree", "/tmp\0/lib"],
        ] {
            let invalid: Vec<_> = invalid.into_iter().map(OsString::from).collect();
            assert!(parse_path(&invalid).is_err(), "{invalid:?}");
        }
        assert!(parse_path(&["--lib-tree".into(), OsString::from_vec(vec![b'/', 0xff])]).is_err());
        assert!(
            parse_path(&[
                "--lib-tree".into(),
                format!("/{}", "a".repeat(MAX_PATH_BYTES)).into()
            ])
            .is_err()
        );
    }

    #[test]
    fn observation_rejects_symlink_and_non_directory_inputs() {
        let root = TestDirectory::new();
        let library = root.path().join("lib");
        fs::create_dir(&library).unwrap();
        let alias = root.path().join("alias");
        symlink(&library, &alias).unwrap();
        assert!(
            observe(&args(&alias))
                .unwrap_err()
                .contains("symlink or non-directory")
        );
        let file = root.path().join("file");
        fs::write(&file, b"not a directory").unwrap();
        assert!(observe(&args(&file)).is_err());
        assert!(observe(&args(&root.path().join("absent"))).is_err());
    }

    #[test]
    fn observation_refuses_mutation_and_named_tree_replacement_before_output() {
        for replace_tree in [false, true] {
            let root = TestDirectory::new();
            let path = root.path().join("lib");
            fs::create_dir(&path).unwrap();
            fs::write(path.join("library"), b"before").unwrap();
            let tree = PinnedRustcLibTree::pin(
                PinnedDirectory::open_existing(path.clone(), "observed test tree").unwrap(),
            )
            .unwrap();
            if replace_tree {
                fs::rename(&path, root.path().join("old")).unwrap();
                fs::create_dir(&path).unwrap();
                fs::write(path.join("library"), b"before").unwrap();
            } else {
                fs::write(path.join("library"), b"during").unwrap();
            }
            assert!(encode_observation(&tree).is_err());
        }
    }
}
