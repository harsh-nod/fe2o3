use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct ScratchTarget {
    path: PathBuf,
}

impl ScratchTarget {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-static-view-host-codegen-target-{}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path).expect("remove stale scratch target");
        }
        Self { path }
    }
}

impl Drop for ScratchTarget {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn newest_rlib(directory: &Path, crate_name: &str) -> PathBuf {
    let prefix = format!("lib{crate_name}-");
    let mut candidates = fs::read_dir(directory)
        .expect("read Cargo dependency directory")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let file_name = path.file_name()?.to_str()?;
            (file_name.starts_with(&prefix) && path.extension() == Some(OsStr::new("rlib"))).then(
                || {
                    let modified = entry
                        .metadata()
                        .and_then(|metadata| metadata.modified())
                        .ok();
                    (modified, path)
                },
            )
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(modified, _)| *modified);
    candidates
        .pop()
        .map(|(_, path)| path)
        .unwrap_or_else(|| panic!("missing {crate_name} rlib in {}", directory.display()))
}

fn function_body<'a>(ir: &'a str, symbol: &str) -> &'a str {
    let marker = format!("@{symbol}(");
    let start = ir
        .find(&marker)
        .unwrap_or_else(|| panic!("missing LLVM function {symbol}"));
    let definition = ir[..start]
        .rfind("define ")
        .unwrap_or_else(|| panic!("missing LLVM definition for {symbol}"));
    let end = ir[start..]
        .find("\n}")
        .map(|offset| start + offset + 2)
        .unwrap_or_else(|| panic!("unterminated LLVM function {symbol}"));
    &ir[definition..end]
}

fn assert_exact_gep_offsets(body: &str, symbol: &str, expected: &[u64]) {
    let gep_lines = body
        .lines()
        .filter(|line| line.contains("getelementptr"))
        .collect::<Vec<_>>();
    assert_eq!(
        gep_lines.len(),
        expected.len(),
        "{symbol} has unexpected host GEPs:\n{body}"
    );
    for (line, offset) in gep_lines.into_iter().zip(expected) {
        assert!(
            line.contains(&format!(", i64 {offset}")),
            "{symbol} has the wrong host byte offset; expected {offset}:\n{line}"
        );
    }
}

#[test]
fn host_rustc_emits_exact_constant_offsets_without_bounds_paths() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let scratch = ScratchTarget::new();
    let library_build = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "build",
            "--locked",
            "-p",
            "fe2o3-device",
            "--lib",
            "--target-dir",
        ])
        .arg(&scratch.path)
        .output()
        .expect("build isolated fe2o3-device library");
    assert!(
        library_build.status.success(),
        "isolated fe2o3-device build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&library_build.stdout),
        String::from_utf8_lossy(&library_build.stderr)
    );
    let deps = scratch.path.join("debug/deps");
    let device = newest_rlib(&deps, "fe2o3_device");
    let source = manifest.join("tests/host_codegen/static_view_shape.rs");
    let output = std::env::temp_dir().join(format!(
        "fe2o3-host-static-view-shape-{}-{}.ll",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let compiled = Command::new(rustc)
        .current_dir(workspace)
        .arg("--crate-name=static_view_shape")
        .arg("--crate-type=lib")
        .arg("--edition=2024")
        .arg("-Copt-level=3")
        .arg("-Cpanic=abort")
        .arg("--emit=llvm-ir")
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("fe2o3_device={}", device.display()))
        .arg(&source)
        .arg("-o")
        .arg(&output)
        .output()
        .expect("run host rustc LLVM-shape fixture");
    assert!(
        compiled.status.success(),
        "host LLVM-shape compilation failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );

    let ir = fs::read_to_string(&output).expect("read generated LLVM IR");
    fs::remove_file(&output).expect("remove generated LLVM IR");
    assert!(
        !ir.contains("amdgcn-amd-amdhsa"),
        "host-only test must not be interpreted as AMDGPU evidence"
    );

    for symbol in [
        "host_static_view_read_index_0",
        "host_static_view_read_indices_1_3",
        "host_static_view_write_index_2",
    ] {
        let body = function_body(&ir, symbol);
        for forbidden in ["panic_bounds_check", " br i1 ", " switch ", "icmp "] {
            assert!(
                !body.contains(forbidden),
                "{symbol} contains runtime bounds machinery `{forbidden}`:\n{body}"
            );
        }
    }

    let index_zero = function_body(&ir, "host_static_view_read_index_0");
    assert_exact_gep_offsets(index_zero, "host_static_view_read_index_0", &[]);

    let indices_one_three = function_body(&ir, "host_static_view_read_indices_1_3");
    assert_exact_gep_offsets(
        indices_one_three,
        "host_static_view_read_indices_1_3",
        &[4, 12],
    );
    assert_eq!(
        indices_one_three.matches("load i32").count(),
        2,
        "multiple static accesses should remain distinct loads:\n{indices_one_three}"
    );

    let index_two = function_body(&ir, "host_static_view_write_index_2");
    assert_exact_gep_offsets(index_two, "host_static_view_write_index_2", &[8]);
    assert_eq!(
        index_two.matches("store i32").count(),
        1,
        "constant mutable access should end in a store:\n{index_two}"
    );
}
