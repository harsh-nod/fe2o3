use std::collections::{BTreeMap, BTreeSet};
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

#[derive(Default)]
struct DirectCallAudit {
    callees: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for DirectCallAudit {
    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = expression.func.as_ref()
            && path.qself.is_none()
            && let Some(segment) = path.path.segments.last()
        {
            self.callees.insert(segment.ident.to_string());
        }
        visit::visit_expr_call(self, expression);
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

fn collect_reachable_unsafe(
    root: &str,
    current: &str,
    functions: &BTreeMap<String, &syn::ItemFn>,
    visited: &mut BTreeSet<String>,
    violations: &mut BTreeSet<String>,
) {
    if !visited.insert(current.to_owned()) {
        return;
    }
    let Some(function) = functions.get(current) else {
        return;
    };

    let mut unsafe_audit = UnsafeKernelAudit::default();
    unsafe_audit.visit_block(&function.block);
    if function.sig.unsafety.is_some() || unsafe_audit.unsafe_blocks != 0 {
        let location = if current == root {
            root.to_owned()
        } else {
            format!("{root} -> {current}")
        };
        violations.insert(location);
    }

    let mut call_audit = DirectCallAudit::default();
    call_audit.visit_block(&function.block);
    for callee in call_audit.callees {
        collect_reachable_unsafe(root, &callee, functions, visited, violations);
    }
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
        let functions = syntax
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Fn(function) => Some((function.sig.ident.to_string(), function)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        for function in functions.values().filter(|function| is_kernel(function)) {
            kernels += 1;
            let kernel = function.sig.ident.to_string();
            let mut reachable = BTreeSet::new();
            collect_reachable_unsafe(
                &kernel,
                &kernel,
                &functions,
                &mut BTreeSet::new(),
                &mut reachable,
            );
            for location in reachable {
                violations.push(format!("{}::{location}", relative.display()));
            }
        }
    }

    assert!(kernels >= 30, "positive kernel catalog unexpectedly shrank");
    assert_eq!(
        violations,
        [
            "examples/moe_expert_v1/src/kernel.rs::moe_expert_gemm_bf16_m16_n16_k16_v1",
            "examples/tiled_gemm_v1/src/kernel.rs::tiled_gemm_lds_slice1",
            "examples/wave64_collectives_v1/src/kernel.rs::wave64_collectives_v1",
            "examples/workgroup_sync_v1/src/kernel.rs::lds_publish_read_reduce_i32_v1",
        ],
        "positive kernel unsafe debt changed; migrate removed entries and reject new ones"
    );
}
