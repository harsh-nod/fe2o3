use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use quote::{ToTokens, quote};

use super::*;

struct RustcFixture(PathBuf);

impl RustcFixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-constant-for-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn compile(&self, source: &str) -> std::process::Output {
        let path = self.0.join("fixture.rs");
        std::fs::write(&path, source).unwrap();
        Command::new("rustc")
            .args(["--edition=2024", "-Cdebuginfo=0", "-Copt-level=0"])
            .arg(path)
            .arg("-o")
            .arg(self.0.join("fixture"))
            .output()
            .expect("run rustc on the production loop expansion")
    }

    fn run(&self, source: &str) {
        let output = self.compile(source);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(self.0.join("fixture")).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for RustcFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove owned constant-loop fixture");
    }
}

fn lower(source: &str, bounds: &[u32]) -> ItemFn {
    let mut input: ItemFn = syn::parse_str(source).unwrap();
    let declaration = ParsedControlFlowOptionsV1 {
        loop_bounds: bounds.to_vec(),
        integer_switches: Vec::new(),
    };
    analyze_kernel_control_flow_v1(&input, Some(&declaration)).unwrap();
    lower_bounded_for_loops_v1(&mut input, Some(&declaration)).unwrap();
    input
}

#[test]
fn named_constant_ranges_preserve_every_primitive_integer_and_extreme_values() {
    let mut source = String::new();
    let mut calls = String::from("fn main() {");
    for ty in [
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
    ] {
        let name = format!("kernel_{ty}");
        source.push_str(&format!(
            "mod {ty}_case {{
            const START: {ty} = {ty}::MAX - 3;
            const END: {ty} = {ty}::MAX;"
        ));
        let input = lower(
            &format!(
                "pub fn {name}() -> u32 {{
            let mut count = 0;
            for value in START..END {{
                if value != START + count as {ty} {{ return u32::MAX; }}
                count += 1;
            }}
            count
        }}"
            ),
            &[8],
        );
        source.push_str(&input.to_token_stream().to_string());
        source.push('}');
        calls.push_str(&format!("assert_eq!({ty}_case::{name}(), 3);"));
    }
    calls.push('}');
    source.push_str(&calls);
    RustcFixture::new().run(&source);
}

#[test]
fn constant_ranges_preserve_break_continue_and_nested_loops() {
    let input = lower(
        "fn kernel() -> i32 {
        let mut sum = 0;
        for outer in START..END {
            for mut inner in 0..INNER_END {
                if inner == 1 { continue; }
                if outer == 1 { break; }
                inner += 1;
                sum += outer * inner;
            }
            if outer == 1 { break; }
        }
        sum
    }",
        &[8, 4],
    );
    RustcFixture::new().run(&format!(
        "
        const START: i32 = -2;
        const END: i32 = 2;
        const INNER_END: i32 = 3;
        {input}
        fn main() {{ assert_eq!(kernel(), -12); }}",
        input = quote!(#input)
    ));
}

#[test]
fn named_ranges_admit_empty_reversed_associated_and_generic_constants() {
    let mut source = String::from(
        "struct Limits;
        impl Limits { const START: usize = 3; const END: usize = 3; }",
    );
    for (name, range) in [
        ("empty", "Limits::START..Limits::END"),
        ("reversed", "Limits::START..0"),
        ("associated", "0..Limits::END"),
    ] {
        let input = lower(
            &format!(
                "fn {name}() -> usize {{
            let mut count = 0; for _i in {range} {{ count += 1; }} count
        }}"
            ),
            &[100],
        );
        source.push_str(&input.to_token_stream().to_string());
    }
    let input = lower(
        "fn generic<const END: usize>() -> usize {
        let mut count = 0; for _i in 0..END { count += 1; } count
    }",
        &[4],
    );
    source.push_str(&input.to_token_stream().to_string());
    source.push_str(
        "fn main() { assert_eq!(empty(), 0); assert_eq!(reversed(), 0);
        assert_eq!(associated(), 3); assert_eq!(generic::<4>(), 4); }",
    );
    RustcFixture::new().run(&source);
}

#[test]
fn executable_const_arguments_fail_closed_before_a_following_loop() {
    let mut input: ItemFn = syn::parse_quote! {
        fn kernel() {
            for i in 0..Limits::<{
                let mut n = 0;
                while n < 1 { n += 1; }
                n
            }>::END { let _ = i; }
            for j in 0..3 { let _ = j; }
        }
    };
    let declaration = ParsedControlFlowOptionsV1 {
        loop_bounds: vec![1, 3, 1],
        integer_switches: Vec::new(),
    };
    analyze_kernel_control_flow_v1(&input, Some(&declaration)).unwrap();
    let original = quote!(#input).to_string();
    let error = lower_bounded_for_loops_v1(&mut input, Some(&declaration)).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("without executable const arguments")
    );
    assert_eq!(quote!(#input).to_string(), original);
}

#[test]
fn executable_const_arguments_are_rejected_inside_nested_types_and_qself() {
    for range in [
        "0..Limits::<{ 1 + 1 }>::END",
        "0..Limits::<[u8; { 1 + 1 }]>::END",
        "0..<[u8; { 1 + 1 }] as RangeEnd>::END",
    ] {
        let range = syn::parse_str(range).unwrap();
        let error = UnrolledRangeV1::parse(&range, 4).err().unwrap();
        assert!(
            error
                .to_string()
                .contains("without executable const arguments")
        );
    }
}

#[test]
fn associated_constant_paths_preserve_following_loop_bounds() {
    let input = lower(
        "fn kernel<const N: usize>() -> usize {
            let mut count = 0;
            for i in 0..Limits::END { count += i; }
            for i in 0..<Limits as RangeEnd>::END { count += i; }
            for i in 0..GenericLimits::<N>::END { count += i; }
            for i in 0..GenericLimits::<3>::END { count += i; }
            for i in 0..<[u8; N] as RangeEnd>::END { count += i; }
            for i in 0..2 { count += i; }
            count
        }",
        &[3, 3, 3, 3, 3, 2],
    );
    RustcFixture::new().run(&format!(
        "struct Limits;
        impl Limits {{ const END: usize = 3; }}
        trait RangeEnd {{ const END: usize; }}
        impl RangeEnd for Limits {{ const END: usize = 3; }}
        impl<const N: usize> RangeEnd for [u8; N] {{ const END: usize = N; }}
        struct GenericLimits<const N: usize>;
        impl<const N: usize> GenericLimits<N> {{ const END: usize = N; }}
        {input}
        fn main() {{ assert_eq!(kernel::<3>(), 16); }}",
        input = quote!(#input)
    ));
}

#[test]
fn rustc_rejects_over_bound_dynamic_noninteger_and_mismatched_endpoints() {
    let cases = [
        (
            "const END: usize = 5;",
            "fn kernel() { for i in 0..END { let _ = i; } }",
            4,
            "exceeds its declared control_flow bound",
        ),
        (
            "const END: usize = 33;",
            "fn kernel() { for i in 0..END { let _ = i; } }",
            100,
            "supports at most 32 iterations",
        ),
        (
            "",
            "fn kernel() { let end = 3; for i in 0..end { let _ = i; } }",
            4,
            "non-constant value",
        ),
        (
            "const START: f32 = 0.0; const END: f32 = 2.0;",
            "fn kernel() { for i in START..END { let _ = i; } }",
            4,
            "Integer",
        ),
        (
            "const START: char = 'a'; const END: char = 'c';",
            "fn kernel() { for i in START..END { let _ = i; } }",
            4,
            "Integer",
        ),
        (
            "const START: u32 = 0; const END: u64 = 2;",
            "fn kernel() { for i in START..END { let _ = i; } }",
            4,
            "mismatched types",
        ),
        (
            "const START: i128 = i128::MIN; const END: i128 = i128::MAX;",
            "fn kernel() { for i in START..END { let _ = i; } }",
            4,
            "exceeds its declared control_flow bound",
        ),
        (
            "const END: u128 = u128::MAX;",
            "fn kernel() { for i in 0..END { let _ = i; } }",
            4,
            "exceeds its declared control_flow bound",
        ),
    ];
    for (prefix, function, bound, expected) in cases {
        let input = lower(function, &[bound]);
        let fixture = RustcFixture::new();
        let output = fixture.compile(&format!(
            "{prefix} {input} fn main() {{ kernel(); }}",
            input = quote!(#input)
        ));
        assert!(!output.status.success(), "unexpectedly compiled {function}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "missing {expected}: {stderr}");
    }
}

#[test]
fn generic_constant_bound_is_checked_at_monomorphization() {
    let input = lower(
        "fn kernel<const END: usize>() {
        for i in 0..END { let _ = i; }
    }",
        &[4],
    );
    let output = RustcFixture::new().compile(&format!(
        "{input}
        fn main() {{ kernel::<5>(); }}",
        input = quote!(#input)
    ));
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("exceeds its declared control_flow bound")
    );
}

#[test]
fn nested_for_expansion_has_an_aggregate_budget() {
    for range in ["0..32", "0..END"] {
        let mut input: ItemFn = syn::parse_str(&format!(
            "fn kernel() {{
            for outer in {range} {{ for middle in {range} {{
                for inner in {range} {{ consume(outer, middle, inner); }}
            }} }}
        }}"
        ))
        .unwrap();
        let declaration = ParsedControlFlowOptionsV1 {
            loop_bounds: vec![32, 32, 32],
            integer_switches: Vec::new(),
        };
        let error = lower_bounded_for_loops_v1(&mut input, Some(&declaration)).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("aggregate 4096-copy expansion budget")
        );
    }
}

#[test]
fn range_helper_rejects_zero_bounds_even_without_the_options_parser() {
    for range in [syn::parse_quote!(0..END), syn::parse_quote!(0..1)] {
        let error = UnrolledRangeV1::parse(&range, 0).err().unwrap();
        assert_eq!(
            error.to_string(),
            "control_flow loop bounds must be nonzero"
        );
    }
}

#[test]
fn literal_ranges_keep_body_driven_type_inference_and_wide_values() {
    let input = lower(
        "fn kernel() -> i64 {
        let mut sum = 0i64;
        for i in (0)..(4) { sum += i; }
        for i in 5_000_000_000..5_000_000_002 { sum += i; }
        sum
    }",
        &[4, 2],
    );
    let expansion = quote!(#input).to_string();
    assert!(!expansion.contains("if const"));
    RustcFixture::new().run(&format!(
        "{expansion}
        fn main() {{ assert_eq!(kernel(), 10_000_000_007); }}"
    ));
}

#[test]
fn constant_endpoint_names_are_outside_generated_helper_scopes() {
    let input = lower(
        "fn kernel() -> usize {
        let mut sum = 0;
        for i in integer..Integer { sum += i; }
        sum
    }",
        &[4],
    );
    RustcFixture::new().run(&format!(
        "#![allow(non_upper_case_globals)]
        const integer: usize = 0;
        const Integer: usize = 3;
        {input}
        fn main() {{ assert_eq!(kernel(), 3); }}",
        input = quote!(#input)
    ));
}

#[test]
fn sinkhorn_tutorial_uses_the_production_constant_lowering_without_source_edits() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/gfx950_advanced_attention/src/kernel.rs"
    ));
    let file = syn::parse_file(source).unwrap();
    let (mut input, options) = file
        .items
        .into_iter()
        .find_map(|item| {
            let syn::Item::Fn(input) = item else {
                return None;
            };
            if !quote!(#input)
                .to_string()
                .contains("0 .. SINKHORN_ITERATIONS_V1")
            {
                return None;
            }
            let options = input
                .attrs
                .iter()
                .find_map(|attr| {
                    if attr.path().is_ident("kernel") {
                        let syn::Meta::List(list) = &attr.meta else {
                            return None;
                        };
                        Some(crate::parse_kernel_options(list.tokens.clone()).unwrap())
                    } else {
                        None
                    }
                })
                .unwrap();
            Some((input, options))
        })
        .expect("attributed Sinkhorn kernel retains its named constant range");
    let declaration = options.control_flow.as_ref().unwrap();
    assert_eq!(declaration.loop_bounds, vec![3]);
    let sidecar = analyze_kernel_control_flow_v1(&input, Some(declaration))
        .unwrap()
        .unwrap();
    let contract = fe2o3_rustc_front::decode_control_flow_contract_v1(&sidecar).unwrap();
    assert_eq!(
        contract
            .nodes()
            .iter()
            .filter(|node| matches!(
                node.kind(),
                fe2o3_rustc_front::ControlFlowNodeKindV1::Loop { .. }
            ))
            .count(),
        1
    );
    lower_bounded_for_loops_v1(&mut input, Some(declaration)).unwrap();
    let lowered = quote!(#input).to_string();
    assert!(!lowered.contains("for _iteration in"));
    assert_eq!(lowered.matches("let _iteration =").count(), 3);
    assert!(lowered.contains("SINKHORN_ITERATIONS_V1"));
    assert!(lowered.contains("constant for range exceeds its declared control_flow bound"));
}
