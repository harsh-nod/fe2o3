//! Opt-in diagnostics only. These files are not compiler receipts or authority.
use super::Owner;
use serde::Serialize;
use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub(crate) const ROOT_ENV: &str = "FE2O3_TEST_CHECKED_OUTPUT_ENDPOINTS_V1";
const CHILD_ENV: &str = "FE2O3_TEST_CHECKED_OUTPUT_ENDPOINT_DIRECTORY_V1";
const CANONICAL_LIMIT: usize = 2 * 1024 * 1024;
const GRAPH_LIMIT: usize = 8 * 1024 * 1024;
const INDEX_LIMIT: usize = 16 * 1024;

struct Observer {
    directory: PathBuf,
    attempted: bool,
}

thread_local! {
    static OBSERVER: RefCell<Option<Observer>> = const { RefCell::new(None) };
}

struct Restore(Option<Observer>);
impl Drop for Restore {
    fn drop(&mut self) {
        OBSERVER.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

/// The parent opts in to a durable external directory; the compiler has no knob.
pub(crate) fn configure_child(command: &mut Command, case: &str) {
    configure_child_with_root(command, case, std::env::var_os(ROOT_ENV).as_deref());
}

fn configure_child_with_root(command: &mut Command, case: &str, root: Option<&std::ffi::OsStr>) {
    command.env_remove(CHILD_ENV);
    let Some(root) = root else { return };
    let mut components = Path::new(case).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        let _ = writeln!(
            io::stderr(),
            "P4 endpoint snapshots disabled: invalid diagnostic case component"
        );
        return;
    }
    let directory = Path::new(root).join(case);
    command.env(CHILD_ENV, &directory);
    let _ = writeln!(
        io::stderr(),
        "P4 endpoint snapshots: {}",
        directory.display()
    );
}

pub(crate) fn child_directory() -> Option<PathBuf> {
    std::env::var_os(CHILD_ENV).map(PathBuf::from)
}

/// Install on the callback thread, not the thread that starts rustc_driver.
pub(crate) fn with_directory<R>(directory: Option<&Path>, action: impl FnOnce() -> R) -> R {
    let next = directory.map(|directory| Observer {
        directory: directory.to_owned(),
        attempted: false,
    });
    let _restore = Restore(OBSERVER.with(|slot| slot.replace(next)));
    action()
}

pub(crate) fn observe(bound: &Owner, intermediate: &Owner, output: &Owner) {
    let directory = OBSERVER.with(|slot| {
        let mut state = slot.borrow_mut();
        let state = state.as_mut()?;
        if state.attempted {
            return None;
        }
        state.attempted = true;
        Some(state.directory.clone())
    });
    if let Some(directory) = directory
        && let Err(error) = write_snapshots(&directory, [bound, intermediate, output])
    {
        let _ = writeln!(io::stderr(), "P4 endpoint snapshots unavailable: {error}");
    }
}

#[derive(Debug, Serialize)]
struct Endpoint {
    role: &'static str,
    identity_sha256: String,
    canonical_bytes: usize,
    state: &'static str,
    diagnostic: Option<String>,
}

#[derive(Serialize)]
struct Index {
    schema: &'static str,
    point: &'static str,
    identifies_failing_endpoint: bool,
    canonical_limit: usize,
    graph_limit: usize,
    endpoints: [Endpoint; 3],
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn external_destination(directory: &Path) -> io::Result<PathBuf> {
    let name = directory
        .file_name()
        .ok_or_else(|| io::Error::other("missing directory name"))?;
    let parent = directory
        .parent()
        .ok_or_else(|| io::Error::other("missing external parent"))?;
    let parent = parent.canonicalize()?;
    if parent.starts_with(workspace().canonicalize()?) {
        return Err(io::Error::other(
            "snapshot directory must be outside the checkout",
        ));
    }
    Ok(parent.join(name))
}

fn write_snapshots(directory: &Path, owners: [&Owner; 3]) -> io::Result<()> {
    let directory = external_destination(directory)?;
    // Never replace a previous run or follow an existing destination symlink.
    std::fs::create_dir(&directory)?;
    let endpoints = [
        write_endpoint(&directory, "B", owners[0], CANONICAL_LIMIT, GRAPH_LIMIT),
        write_endpoint(&directory, "C", owners[1], CANONICAL_LIMIT, GRAPH_LIMIT),
        write_endpoint(&directory, "O", owners[2], CANONICAL_LIMIT, GRAPH_LIMIT),
    ];
    let index = Index {
        schema: "fe2o3-policy4-endpoint-snapshot-v1",
        point: "after-checked-reservation-before-ranked-receipt-and-admission",
        identifies_failing_endpoint: false,
        canonical_limit: CANONICAL_LIMIT,
        graph_limit: GRAPH_LIMIT,
        endpoints,
    };
    let temporary = directory.join("index.partial");
    let result = (|| {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let mut writer = Limited {
            inner: file,
            remaining: INDEX_LIMIT,
        };
        serde_json::to_writer_pretty(&mut writer, &index).map_err(io::Error::other)?;
        writer.flush()?;
        drop(writer);
        std::fs::rename(&temporary, directory.join("index.json"))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

fn write_endpoint(
    directory: &Path,
    role: &'static str,
    owner: &Owner,
    canonical_limit: usize,
    graph_limit: usize,
) -> Endpoint {
    let canonical = owner.canonical();
    let bytes = canonical.canonical_bytes();
    let mut identity = String::with_capacity(64);
    for byte in canonical.identity().digest() {
        use std::fmt::Write as _;
        // Formatting a byte into an owned String cannot return an I/O error.
        let _ = write!(&mut identity, "{byte:02x}");
    }
    let wire_partial = directory.join(format!("{role}.kir-v12.partial"));
    let graph_partial = directory.join(format!("{role}.module.partial"));
    let wire_final = directory.join(format!("{role}.kir-v12"));
    let graph_final = directory.join(format!("{role}.module.txt"));
    let result = (|| {
        if bytes.len() > canonical_limit {
            return Err(io::Error::other(
                "complete canonical bytes exceed diagnostic cap",
            ));
        }
        let mut wire = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&wire_partial)?;
        wire.write_all(bytes)?;
        drop(wire);
        {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&graph_partial)?;
            let mut graph = Limited {
                inner: std::io::BufWriter::new(file),
                remaining: graph_limit,
            };
            // Full actual module: definitions, consumers, terminators and edge arguments.
            writeln!(&mut graph, "{:#?}", owner.module())?;
            graph.flush()?;
        }
        std::fs::rename(&wire_partial, &wire_final)?;
        std::fs::rename(&graph_partial, &graph_final)
    })();
    let diagnostic = result.err().map(|error| error.to_string());
    if diagnostic.is_some() {
        for path in [&wire_partial, &graph_partial, &wire_final, &graph_final] {
            let _ = std::fs::remove_file(path);
        }
    }
    Endpoint {
        role,
        identity_sha256: identity,
        canonical_bytes: bytes.len(),
        state: if diagnostic.is_none() {
            "complete"
        } else {
            "unavailable"
        },
        diagnostic,
    }
}

struct Limited<W> {
    inner: W,
    remaining: usize,
}

impl<W: Write> Write for Limited<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(io::Error::other("complete graph exceeds diagnostic cap"));
        }
        let written = self.inner.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_temp_dir::TestTempDir;

    fn owner(name: &str) -> Owner {
        let module = fe2o3_kernel_ir::Module::new(name);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work, 1_000_000,
        );
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget)
            .unwrap()
            .0
    }

    fn active() -> Option<PathBuf> {
        OBSERVER.with(|slot| slot.borrow().as_ref().map(|state| state.directory.clone()))
    }

    #[test]
    fn nested_observer_restores_scope_on_return_refusal_and_panic() {
        assert!(active().is_none());
        with_directory(Some(Path::new("outer")), || {
            assert_eq!(active(), Some(PathBuf::from("outer")));
            let result: Result<(), &str> = with_directory(Some(Path::new("inner")), || {
                assert_eq!(active(), Some(PathBuf::from("inner")));
                Err("original refusal")
            });
            assert_eq!(result, Err("original refusal"));
            assert_eq!(active(), Some(PathBuf::from("outer")));
            let panic = std::panic::catch_unwind(|| {
                with_directory(Some(Path::new("panic")), || std::panic::panic_any(73_u32));
            })
            .unwrap_err();
            assert_eq!(panic.downcast_ref::<u32>(), Some(&73));
            assert_eq!(active(), Some(PathBuf::from("outer")));
        });
        assert!(active().is_none());
    }

    #[test]
    fn actual_three_owner_bytes_full_graphs_and_metadata_are_deterministic() {
        let scratch = TestTempDir::create("policy4-endpoint-snapshots");
        let owners = [owner("actual-b"), owner("actual-c"), owner("actual-o")];
        for case in ["first", "second"] {
            let destination = scratch.path().join(case);
            with_directory(Some(&destination), || {
                observe(&owners[0], &owners[1], &owners[2])
            });
            for (role, owner) in ["B", "C", "O"].into_iter().zip(&owners) {
                assert_eq!(
                    std::fs::read(destination.join(format!("{role}.kir-v12"))).unwrap(),
                    owner.canonical().canonical_bytes(),
                );
                assert_eq!(
                    std::fs::read_to_string(destination.join(format!("{role}.module.txt")))
                        .unwrap(),
                    format!("{:#?}\n", owner.module()),
                );
            }
        }
        let first = std::fs::read(scratch.path().join("first/index.json")).unwrap();
        assert_eq!(
            first,
            std::fs::read(scratch.path().join("second/index.json")).unwrap()
        );
        let index: serde_json::Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(index["identifies_failing_endpoint"], false);
        assert!(
            index["endpoints"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["state"] == "complete")
        );
        assert_ne!(
            index["endpoints"][0]["identity_sha256"],
            index["endpoints"][1]["identity_sha256"]
        );
    }

    #[test]
    fn disabled_once_only_and_io_failures_do_not_change_result_or_owners() {
        let scratch = TestTempDir::create("policy4-endpoint-io");
        let owner = owner("unchanged");
        let digest = *owner.canonical().identity().digest();
        with_directory(None, || observe(&owner, &owner, &owner));
        let destination = scratch.path().join("once");
        let result: Result<(), &str> = with_directory(Some(&destination), || {
            observe(&owner, &owner, &owner);
            observe(&owner, &owner, &owner);
            Err("same refusal")
        });
        assert_eq!(result, Err("same refusal"));
        let before = std::fs::read(destination.join("index.json")).unwrap();
        let result = with_directory(Some(&destination), || {
            observe(&owner, &owner, &owner); // Existing destination refuses overwrite.
            91
        });
        assert_eq!(result, 91);
        assert_eq!(
            std::fs::read(destination.join("index.json")).unwrap(),
            before
        );
        let missing = scratch.path().join("missing/child");
        assert!(!with_directory(Some(&missing), || {
            observe(&owner, &owner, &owner);
            false
        }));
        assert_eq!(owner.canonical().identity().digest(), &digest);
        assert!(active().is_none());
    }

    #[test]
    fn oversized_endpoint_is_unavailable_and_never_retains_partial_graphs() {
        let scratch = TestTempDir::create("policy4-endpoint-caps");
        let owner = owner("bounded");
        for (wire_limit, graph_limit) in [(0, GRAPH_LIMIT), (CANONICAL_LIMIT, 1)] {
            let result = write_endpoint(scratch.path(), "B", &owner, wire_limit, graph_limit);
            assert_eq!(result.state, "unavailable");
            assert!(result.diagnostic.is_some());
            assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn external_path_check_rejects_checkout_destinations() {
        assert!(external_destination(&workspace().join("diagnostic-snapshot")).is_err());
    }

    #[test]
    fn parent_configuration_is_opt_in_and_does_not_change_semantic_arguments() {
        let mut command = Command::new("unused-test-only");
        command
            .env("RUSTFLAGS", "-Ctarget-cpu=gfx942")
            .arg("--edition=2024");
        configure_child_with_root(&mut command, "case", None);
        assert!(
            !command
                .get_envs()
                .any(|(key, value)| key == CHILD_ENV && value.is_some())
        );
        configure_child_with_root(
            &mut command,
            "../escape",
            Some(std::ffi::OsStr::new("/external")),
        );
        assert!(
            !command
                .get_envs()
                .any(|(key, value)| key == CHILD_ENV && value.is_some())
        );
        configure_child_with_root(
            &mut command,
            "case",
            Some(std::ffi::OsStr::new("/external")),
        );
        assert!(command.get_envs().any(|(key, value)| key == CHILD_ENV
            && value == Some(std::ffi::OsStr::new("/external/case"))));
        assert!(command.get_envs().any(|(key, value)| key == "RUSTFLAGS"
            && value == Some(std::ffi::OsStr::new("-Ctarget-cpu=gfx942"))));
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["--edition=2024"]);
    }
}
