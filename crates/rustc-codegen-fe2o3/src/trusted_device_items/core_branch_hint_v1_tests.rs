use super::*;

fn valid(path: &str) -> Contract<'_> {
    Contract {
        item: true,
        core: true,
        generics: 0,
        mir: true,
        intrinsic: false,
        path,
        safe: true,
        rust_abi: true,
        variadic: false,
        inputs: 1,
        boolean_input: true,
        boolean_output: true,
    }
}

#[test]
fn only_exact_core_branch_hint_wrapper_paths_are_recognized() {
    for path in ["core::intrinsics::likely", "core::intrinsics::unlikely"] {
        assert!(authenticate_contract(valid(path)));
    }
    for path in [
        "likely",
        "user::intrinsics::likely",
        "core::hint::likely",
        "core::intrinsics::cold_path",
        "core::intrinsics::likely::shim",
        "core::intrinsics::unlikely_bool",
    ] {
        assert!(!authenticate_contract(valid(path)), "{path}");
    }
}

#[test]
fn every_branch_hint_identity_and_signature_condition_is_required() {
    let contract = valid("core::intrinsics::unlikely");
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
            intrinsic: true,
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
            inputs: 0,
            ..contract
        },
        Contract {
            inputs: 2,
            ..contract
        },
        Contract {
            boolean_input: false,
            ..contract
        },
        Contract {
            boolean_output: false,
            ..contract
        },
    ] {
        assert!(!authenticate_contract(altered), "{altered:?}");
    }
}

#[test]
fn branch_hint_source_safety_does_not_create_a_terminal() {
    let collector = include_str!("../collector.rs");
    let name = "authenticate_reviewed_safe_core_branch_hint_helper_v1";
    let start = collector.find(name).unwrap();
    assert!(collector[..start].rfind("let Some(local_def_id)").is_some());
    assert!(
        collector[..start]
            .rfind("self.reachable_unsafe_calls")
            .is_some()
    );
    let terminal = include_str!("../production_semantic_terminal_v1.rs");
    assert!(!terminal.contains(name));
    assert!(terminal.contains("intrinsic.name == sym::cold_path"));
}
