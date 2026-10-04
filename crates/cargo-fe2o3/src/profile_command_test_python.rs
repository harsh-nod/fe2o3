//! Test-only prerequisite selection; production discovery remains unchanged.
#![cfg(test)]

use std::env;
use std::path::PathBuf;

use super::{FilePin, MAX_INTERPRETER_BYTES, require_absolute_python, require_elf};

const TEST_NATIVE_PYTHON: &str = "FE2O3_TEST_NATIVE_PYTHON";

pub(super) fn configured_python() -> Result<PathBuf, String> {
    select_python(
        env::var_os(TEST_NATIVE_PYTHON).map(PathBuf::from),
        super::discover_python,
    )
}

fn select_python(
    explicit: Option<PathBuf>,
    discover: impl FnOnce() -> Option<PathBuf>,
) -> Result<PathBuf, String> {
    // A present but invalid override must fail, never fall back or skip a test.
    let selected = explicit.or_else(discover).ok_or_else(|| {
        format!("native Python prerequisite unavailable; set {TEST_NATIVE_PYTHON} to an absolute python3.12 or python3.13 executable")
    })?;
    require_absolute_python(&selected)?;
    let pin = FilePin::open(
        &selected,
        "test Python interpreter",
        MAX_INTERPRETER_BYTES,
        true,
    )?;
    require_elf(&pin, "test Python interpreter")?;
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let root = env::temp_dir().join(format!(
                "cargo-fe2o3-test-python-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }

        fn file(&self, name: &str) -> PathBuf {
            assert!(matches!(name, "python3.12" | "python3.13"));
            self.0.join(name)
        }

        fn native(&self, name: &str) -> PathBuf {
            let file = self.file(name);
            // Native-file admission fixture only, not Python behavior. Never execute it.
            fs::copy("/bin/true", &file).unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o700)).unwrap();
            file
        }
    }

    impl Drop for Directory {
        fn drop(&mut self) {
            for name in ["python3.12", "python3.13"] {
                let _ = fs::remove_file(self.file(name));
            }
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn explicit(path: &Path) -> Result<PathBuf, String> {
        select_python(Some(path.to_path_buf()), || {
            panic!("explicit path must not use discovery")
        })
    }

    #[test]
    fn explicit_native_file_is_checked_for_both_supported_names() {
        let directory = Directory::new();
        for name in ["python3.12", "python3.13"] {
            let file = directory.native(name);
            assert_eq!(explicit(&file).unwrap(), file);
        }
    }

    #[test]
    fn absent_override_uses_only_supplied_discovery_and_missing_default_fails() {
        let directory = Directory::new();
        let file = directory.native("python3.12");
        let calls = Cell::new(0);
        assert_eq!(
            select_python(None, || {
                calls.set(calls.get() + 1);
                Some(file.clone())
            })
            .unwrap(),
            file
        );
        assert_eq!(calls.get(), 1);
        assert!(
            select_python(None, || None)
                .unwrap_err()
                .contains(TEST_NATIVE_PYTHON)
        );
    }

    #[test]
    fn relative_empty_and_wrong_name_overrides_fail_without_discovery() {
        for path in [
            "",
            "python3.12",
            "/test-only/python3",
            "/test-only/python3.10",
        ] {
            assert!(
                explicit(Path::new(path))
                    .unwrap_err()
                    .contains("absolute native python3.12 or python3.13")
            );
        }
    }

    #[test]
    fn missing_override_fails_without_discovery() {
        let directory = Directory::new();
        assert!(
            explicit(&directory.file("python3.12"))
                .unwrap_err()
                .contains("failed to inspect test Python interpreter")
        );
    }

    #[test]
    fn script_and_empty_overrides_fail_without_discovery() {
        let directory = Directory::new();
        let script = directory.file("python3.12");
        fs::write(&script, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            explicit(&script)
                .unwrap_err()
                .contains("must be a native ELF executable")
        );
        let empty = directory.file("python3.13");
        fs::write(&empty, b"").unwrap();
        fs::set_permissions(&empty, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            explicit(&empty)
                .unwrap_err()
                .contains("must be a nonempty regular file")
        );
    }

    #[test]
    fn symlink_override_is_not_resolved_or_replaced_by_discovery() {
        let directory = Directory::new();
        let native = directory.native("python3.12");
        let link = directory.file("python3.13");
        symlink(&native, &link).unwrap();
        assert!(
            explicit(&link)
                .unwrap_err()
                .contains("must not be a symbolic link")
        );
    }

    #[test]
    fn nonexecutable_override_fails_without_discovery() {
        let directory = Directory::new();
        let file = directory.native("python3.12");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(explicit(&file).unwrap_err().contains("is not executable"));
    }
}
