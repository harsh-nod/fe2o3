use super::*;

fn valid<'a>(path: &'a str, integer: &'a str) -> WrapperContractV1<'a> {
    WrapperContractV1 {
        item: true,
        core: true,
        generic_arguments: 0,
        mir_available: true,
        path,
        safe: true,
        rust_abi: true,
        variadic: false,
        input_count: 2,
        first: Some(integer),
        second: Some(integer),
        result: Some(integer),
        target_integer: true,
    }
}

#[test]
fn every_supported_primitive_wrapper_is_source_only() {
    for integer in [
        "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize",
    ] {
        for method in ["saturating_add", "saturating_sub"] {
            let path = format!("core::num::<impl {integer}>::{method}");
            assert!(authenticate_contract_v1(valid(&path, integer)));
        }
    }
}

#[test]
fn untrusted_same_names_methods_or_owners_do_not_get_the_exemption() {
    for path in [
        "user::saturating_sub",
        "user::num::<impl u32>::saturating_sub",
        "core::num::<impl i32>::saturating_sub",
        "core::num::<impl u32>::saturating_mul",
        "core::num::<impl u32>::saturating_add_signed",
        "core::num::<impl u32>::saturating_sub_unsigned",
        "core::num::<impl u32>::saturating_sub::shim",
        "saturating_sub",
    ] {
        assert!(!authenticate_contract_v1(valid(path, "u32")), "{path}");
    }
    let contract = valid("core::num::<impl u32>::saturating_sub", "u32");
    for altered in [
        WrapperContractV1 {
            core: false,
            ..contract
        },
        WrapperContractV1 {
            item: false,
            ..contract
        },
        WrapperContractV1 {
            mir_available: false,
            ..contract
        },
        WrapperContractV1 {
            generic_arguments: 1,
            ..contract
        },
    ] {
        assert!(!authenticate_contract_v1(altered));
    }
}

#[test]
fn unsupported_types_and_mismatched_signature_are_closed() {
    for integer in ["i128", "u128", "f32", "f64", "bool", "&u32", "*mut u32"] {
        let path = format!("core::num::<impl {integer}>::saturating_add");
        assert!(!authenticate_contract_v1(valid(&path, integer)));
    }
    let contract = valid("core::num::<impl u32>::saturating_add", "u32");
    for altered in [
        WrapperContractV1 {
            safe: false,
            ..contract
        },
        WrapperContractV1 {
            rust_abi: false,
            ..contract
        },
        WrapperContractV1 {
            variadic: true,
            ..contract
        },
        WrapperContractV1 {
            input_count: 1,
            ..contract
        },
        WrapperContractV1 {
            first: None,
            ..contract
        },
        WrapperContractV1 {
            second: Some("i32"),
            ..contract
        },
        WrapperContractV1 {
            result: Some("u64"),
            ..contract
        },
        WrapperContractV1 {
            target_integer: false,
            ..contract
        },
    ] {
        assert!(!authenticate_contract_v1(altered));
    }
}

#[test]
fn exemption_does_not_claim_a_compiler_terminal_or_body_elision() {
    let source = include_str!("../collector.rs");
    let start = source
        .find("authenticate_reviewed_safe_core_saturating_integer_helper_v1")
        .unwrap();
    let before = &source[..start];
    assert!(before.rfind("let Some(local_def_id)").is_some());
    assert!(
        source[start..]
            .split("authenticate_reviewed_safe_core_f32_is_finite_helper_v1")
            .next()
            .unwrap()
            .contains("continue;")
    );
    let terminal = include_str!("../production_semantic_terminal_v1.rs");
    assert!(!terminal.contains("authenticate_reviewed_safe_core_saturating_integer_helper_v1"));
}
