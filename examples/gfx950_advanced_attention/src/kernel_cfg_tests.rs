use syn::{Expr, Item, ItemFn, Local, Pat, Stmt};

// Host builds exclude the AMDGPU bodies, so inspect the actual source's cfg
// placement as well as rerunning the device compilation for each variant.
fn kernel(name: &str) -> ItemFn {
    syn::parse_file(include_str!("kernel.rs"))
        .expect("kernel source parses")
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
        .expect("kernel exists")
}

fn locals<'a>(function: &'a ItemFn, name: &str) -> Vec<&'a Local> {
    function
        .block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            Stmt::Local(local) if matches!(&local.pat, Pat::Ident(pat) if pat.ident == name) => {
                Some(local)
            }
            _ => None,
        })
        .collect()
}

fn assert_unconditional_local<'a>(function: &'a ItemFn, name: &str) -> &'a Local {
    let declarations = locals(function, name);
    assert_eq!(
        declarations.len(),
        1,
        "{name} must be shared by both variants"
    );
    assert!(
        declarations[0].attrs.is_empty(),
        "{name} must not be cfg-gated"
    );
    declarations[0]
}

fn assert_path(expression: &Expr, name: &str) {
    assert!(matches!(expression, Expr::Path(path) if path.path.is_ident(name)));
}

fn assert_shared_value_layout(function: &ItemFn) {
    let base = assert_unconditional_local(function, "value_base");
    let Expr::MethodCall(call) = base.init.as_ref().expect("base initializer").expr.as_ref() else {
        panic!("value_base must retain the batch offset calculation");
    };
    assert_path(&call.receiver, "batch");
    assert_eq!(call.method, "wrapping_mul");
    assert_eq!(call.args.len(), 1);
    let Expr::Binary(stride) = &call.args[0] else {
        panic!("value_base must retain the token/channel stride");
    };
    assert!(matches!(stride.op, syn::BinOp::Mul(_)));
    assert_path(&stride.left, "ATTENTION_TOKENS_V1");
    assert_path(&stride.right, "CHANNELS_V1");
}

fn assert_variant_gate(local: &Local, feature: &str, enabled: bool) {
    assert_eq!(local.attrs.len(), 1, "normalization must have one cfg gate");
    let attribute = &local.attrs[0];
    assert!(attribute.path().is_ident("cfg"));
    let condition = format!("feature = {feature:?}");
    let expected = if enabled {
        condition
    } else {
        format!("not ({condition})")
    };
    assert_eq!(
        attribute
            .meta
            .require_list()
            .expect("cfg predicate")
            .tokens
            .to_string(),
        expected,
    );
}

#[test]
fn sparse_value_loads_are_shared_and_only_normalization_is_feature_gated() {
    let function = kernel("gfx950_content_sparse_attention");
    assert_shared_value_layout(&function);
    for name in ["value0", "value1", "value2"] {
        assert_unconditional_local(&function, name);
    }
    let results = locals(&function, "result");
    assert_eq!(results.len(), 2);
    let feature = "kernel-content-sparse-attention-reciprocal-reuse-v1";
    assert_variant_gate(results[0], feature, false);
    assert_variant_gate(results[1], feature, true);
}

#[test]
fn compressed_value_base_is_shared_by_local_and_global_normalization_variants() {
    let function = kernel("gfx950_compressed_hybrid_attention");
    assert_shared_value_layout(&function);
    for name in ["compressed0", "compressed1", "compressed2"] {
        assert_unconditional_local(&function, name);
    }
    let feature = "kernel-compressed-hybrid-attention-division-baseline-v1";
    for name in ["local_value", "global_value"] {
        let values = locals(&function, name);
        assert_eq!(values.len(), 2);
        assert_variant_gate(values[0], feature, true);
        assert_variant_gate(values[1], feature, false);
    }
}
