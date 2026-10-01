use super::*;

pub(super) fn build_std_vendor_fixture(
    label: &str,
    include_registry_package: bool,
    include_package: bool,
    vendor_checksum: &str,
) -> (
    PathBuf,
    crate::PinnedRustc,
    crate::rustc_lib_tree::PinnedRustcLibTree,
) {
    let root = env::temp_dir().join(format!(
        "fe2o3-engineering-build-std-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let rustc_lib = root.join("rustc-lib");
    let library = rustc_lib.join("rustlib/src/rust/library");
    let vendor = root.join("vendor");
    fs::create_dir_all(&library).unwrap();
    fs::create_dir(&vendor).unwrap();
    let mut lock =
        String::from("version = 4\n\n[[package]]\nname = \"core\"\nversion = \"0.0.0\"\n");
    if include_registry_package {
        lock.push_str(concat!(
            "\n[[package]]\n",
            "name = \"rustc-literal-escaper\"\n",
            "version = \"0.0.7\"\n",
            "source = \"registry+https://github.com/rust-lang/crates.io-index\"\n",
            "checksum = \"1111111111111111111111111111111111111111111111111111111111111111\"\n",
        ));
    }
    fs::write(library.join("Cargo.lock"), lock).unwrap();
    if include_package {
        let package = vendor.join("rustc-literal-escaper-0.0.7");
        fs::create_dir(&package).unwrap();
        fs::write(
            package.join("Cargo.toml"),
            "[package]\nname = \"rustc-literal-escaper\"\nversion = \"0.0.7\"\n",
        )
        .unwrap();
        fs::write(
            package.join(".cargo-checksum.json"),
            format!("{{\"files\":{{}},\"package\":\"{vendor_checksum}\"}}"),
        )
        .unwrap();
    }
    let rustc_tree = crate::rustc_lib_tree::PinnedRustcLibTree::pin(
        crate::project::PinnedDirectory::open_existing(rustc_lib, "test rustc lib-tree directory")
            .unwrap(),
    )
    .unwrap();
    let rustc = crate::PinnedRustc {
        executable: sealed_test_executable(),
        lib_tree: crate::RustcLibTree::Authority(rustc_tree),
    };
    let vendor = pin_vendor_tree(&fs::canonicalize(vendor).unwrap()).unwrap();
    (root, rustc, vendor)
}

pub(super) fn sealed_test_executable() -> crate::pinned_executable::PinnedExecutable {
    let executable_path = fs::canonicalize("/bin/true").unwrap();
    crate::pinned_executable::PinnedExecutable::open(&executable_path)
        .unwrap()
        .seal_executable_image()
        .unwrap()
}
