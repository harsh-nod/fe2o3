use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};

#[derive(Default)]
struct UnsafeKernelAudit {
    unsafe_blocks: usize,
}

impl<'ast> Visit<'ast> for UnsafeKernelAudit {
    fn visit_expr_unsafe(&mut self, expression: &'ast syn::ExprUnsafe) {
        self.unsafe_blocks += 1;
        visit::visit_expr_unsafe(self, expression);
    }
}

fn is_kernel(function: &syn::ItemFn) -> bool {
    function.attrs.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "kernel")
    })
}

fn collect_rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
        .map(|entry| entry.expect("example source entry must be readable").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("cargo-fe2o3 must remain under the repository crates directory")
        .to_path_buf()
}

#[test]
fn positive_example_kernel_unsafe_debt_is_frozen() {
    let root = repository_root();
    let examples = root.join("examples");
    let mut sources = Vec::new();
    for entry in fs::read_dir(&examples).expect("repository examples must be readable") {
        let package = entry
            .expect("example package entry must be readable")
            .path();
        let source = package.join("src");
        if source.is_dir() {
            collect_rust_sources(&source, &mut sources);
        }
    }
    sources.sort();

    let mut kernels = 0_usize;
    let mut violations = Vec::new();
    for source in sources {
        let relative = source
            .strip_prefix(&root)
            .expect("example source must be under the repository root");
        let bytes = fs::read_to_string(&source)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", relative.display()));
        let syntax = syn::parse_file(&bytes)
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", relative.display()));
        for item in syntax.items {
            let syn::Item::Fn(function) = item else {
                continue;
            };
            if !is_kernel(&function) {
                continue;
            }
            kernels += 1;
            let mut audit = UnsafeKernelAudit::default();
            audit.visit_block(&function.block);
            if function.sig.unsafety.is_some() || audit.unsafe_blocks != 0 {
                violations.push(format!("{}::{}", relative.display(), function.sig.ident));
            }
        }
    }

    assert!(kernels >= 30, "positive kernel catalog unexpectedly shrank");
    assert_eq!(
        violations,
        [
            "examples/flash_attention_v1/src/kernel.rs::flash_attention_causal_f32_b1_h1_n8_d16_v1",
            "examples/moe_expert_v1/src/kernel.rs::moe_expert_gemm_bf16_m16_n16_k16_v1",
            "examples/raw_disjoint_inplace_shift/src/main.rs::raw_disjoint_inplace_shift",
            "examples/raw_disjoint_shift/src/main.rs::raw_disjoint_shift",
            "examples/row_softmax_v1/src/kernel.rs::row_softmax_v1",
            "examples/tiled_gemm_v1/src/kernel.rs::tiled_gemm_lds_slice1",
            "examples/wave64_collectives_v1/src/kernel.rs::wave64_collectives_v1",
            "examples/workgroup_sync_v1/src/kernel.rs::lds_publish_read_reduce_i32_v1",
            "examples/workgroup_sync_v1/src/scoped_atomic.rs::scoped_atomic_add_u32_v1",
        ],
        "positive kernel unsafe debt changed; migrate removed entries and reject new ones"
    );
}
