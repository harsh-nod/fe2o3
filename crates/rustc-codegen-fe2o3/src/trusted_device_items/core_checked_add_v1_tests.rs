use super::*;

fn valid<'a>(path: &'a str, integer: &'a str) -> Contract<'a> {
    Contract {
        item: true,
        core: true,
        generics: 0,
        mir: true,
        path,
        safe: true,
        rust_abi: true,
        variadic: false,
        inputs: 2,
        first: Some(integer),
        second: Some(integer),
        core_option: true,
        payload: Some(integer),
        target_unsigned: true,
    }
}

#[test]
fn supported_unsigned_contracts_require_the_exact_method() {
    for integer in ["u8", "u16", "u32", "u64", "usize"] {
        let path = format!("core::num::<impl {integer}>::checked_add");
        assert!(authenticate_contract(valid(&path, integer)));
    }
    for integer in [
        "i8", "i16", "i32", "i64", "isize", "u128", "i128", "f32", "&u32",
    ] {
        let path = format!("core::num::<impl {integer}>::checked_add");
        assert!(!authenticate_contract(valid(&path, integer)));
    }
    for path in [
        "user::num::<impl u32>::checked_add",
        "core::num::<impl u64>::checked_add",
        "core::num::<impl u32>::unchecked_add",
        "core::num::<impl u32>::checked_add_signed",
        "core::num::<impl u32>::checked_sub",
        "core::num::<impl u32>::checked_add::shim",
        "checked_add",
    ] {
        assert!(!authenticate_contract(valid(path, "u32")), "{path}");
    }
}

#[test]
fn every_identity_and_signature_condition_is_required() {
    let contract = valid("core::num::<impl u32>::checked_add", "u32");
    for altered in [
        Contract {
            item: false,
            ..contract
        },
        Contract {
            core: false,
            ..contract
        },
        Contract {
            generics: 1,
            ..contract
        },
        Contract {
            mir: false,
            ..contract
        },
        Contract {
            safe: false,
            ..contract
        },
        Contract {
            rust_abi: false,
            ..contract
        },
        Contract {
            variadic: true,
            ..contract
        },
        Contract {
            inputs: 1,
            ..contract
        },
        Contract {
            inputs: 3,
            ..contract
        },
        Contract {
            first: None,
            ..contract
        },
        Contract {
            second: Some("u64"),
            ..contract
        },
        Contract {
            core_option: false,
            ..contract
        },
        Contract {
            payload: None,
            ..contract
        },
        Contract {
            payload: Some("u64"),
            ..contract
        },
        Contract {
            target_unsigned: false,
            ..contract
        },
    ] {
        assert!(!authenticate_contract(altered), "{altered:?}");
    }
}

#[test]
fn source_safety_recognition_never_becomes_a_call_terminal() {
    let collector = include_str!("../collector.rs");
    let name = "authenticate_reviewed_safe_core_checked_add_helper_v1";
    let start = collector.find(name).unwrap();
    assert!(collector[..start].rfind("let Some(local_def_id)").is_some());
    assert!(
        collector[..start]
            .rfind("self.reachable_unsafe_calls")
            .is_some()
    );
    assert!(!include_str!("../production_semantic_terminal_v1.rs").contains(name));
}
