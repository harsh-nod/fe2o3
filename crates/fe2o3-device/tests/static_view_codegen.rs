use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

#[test]
fn constant_access_has_no_runtime_bounds_path() {
    let executable = std::env::current_exe().expect("locate integration test executable");
    let deps = executable
        .parent()
        .expect("integration test dependency directory");
    let device = newest_rlib(deps, "fe2o3_device");
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = manifest.join("tests/codegen/static_view_shape.rs");
    let output = std::env::temp_dir().join(format!(
        "fe2o3-static-view-shape-{}-{}.ll",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let compiled = Command::new(rustc)
        .current_dir(
            manifest
                .parent()
                .and_then(Path::parent)
                .expect("workspace root"),
        )
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
        .expect("run rustc LLVM-shape fixture");
    assert!(
        compiled.status.success(),
        "LLVM-shape compilation failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );

    let ir = fs::read_to_string(&output).expect("read generated LLVM IR");
    fs::remove_file(&output).expect("remove generated LLVM IR");
    for symbol in ["static_view_read_index_2", "static_view_write_index_1"] {
        let body = function_body(&ir, symbol);
        assert!(
            body.contains("getelementptr"),
            "{symbol} lost its constant offset:\n{body}"
        );
        for forbidden in ["panic_bounds_check", " br i1 ", " switch ", "icmp "] {
            assert!(
                !body.contains(forbidden),
                "{symbol} contains runtime bounds machinery `{forbidden}`:\n{body}"
            );
        }
    }
}
