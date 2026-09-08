use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct Scratch(PathBuf);

impl Scratch {
    fn new(case: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-flash-capability-{case}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(path.join("src")).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn check(case: &str, source: &str, should_pass: bool) {
    let scratch = Scratch::new(case);
    let device = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/fe2o3-device");
    std::fs::write(
        scratch.0.join("Cargo.toml"),
        format!(
            "[package]\nname='flash-capability-{case}'\nversion='0.0.0'\nedition='2024'\n\n[workspace]\n\n[dependencies]\nfe2o3-device={{path={device:?}}}\n"
        ),
    )
    .unwrap();
    std::fs::write(scratch.0.join("src/lib.rs"), source).unwrap();
    let target = std::env::var_os("FE2O3_GENERAL_UI_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.0.join("target"));
    let output = Command::new(env!("CARGO"))
        .current_dir(&scratch.0)
        .args(["check", "--offline", "--quiet", "--target-dir"])
        .arg(target)
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        should_pass,
        "unexpected {case} result:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn global_roles_and_brands_are_enforced_by_rust_types() {
    check(
        "pass",
        "use fe2o3_device::{ExclusiveReadWrite,Global,ReadOnly};\nfn body<B>(input:&Global<'_,f32,ReadOnly,B>,output:&mut Global<'_,f32,ExclusiveReadWrite,B>){if let Some(value)=input.load(0){let _=output.store(0,value);}}",
        true,
    );
    check(
        "role-substitution",
        "use fe2o3_device::{ExclusiveReadWrite,Global,ReadOnly};\nfn read<B>(_:&Global<'_,f32,ReadOnly,B>){}\nfn body<B>(output:&Global<'_,f32,ExclusiveReadWrite,B>){read(output);}",
        false,
    );
    check(
        "brand-substitution",
        "use fe2o3_device::{Global,ReadOnly};\nfn take<B>(_:&Global<'_,f32,ReadOnly,B>){}\nfn body<A,B>(input:&Global<'_,f32,ReadOnly,A>){take::<B>(input);}",
        false,
    );
}
