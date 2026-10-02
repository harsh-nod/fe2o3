use super::*;

fn valid_identity() -> IdentityContract {
    IdentityContract {
        item: true,
        core: true,
        external: true,
        lang_item: true,
        mir: true,
        intrinsic: false,
        generic_arity_matches: true,
    }
}

fn valid_signature() -> SignatureContract {
    SignatureContract {
        method: true,
        inherent: true,
        core_owner: true,
        associated_owner: true,
        normalized: true,
        safe: true,
        rust_abi: true,
        variadic: false,
        inputs: 1,
        shared_slice: true,
        usize_output: true,
        matching_self_type: true,
    }
}

#[test]
fn slice_len_requires_exact_external_core_lang_item_and_mir_identity() {
    let valid = valid_identity();
    assert!(authenticate_identity(valid));
    for invalid in [
        IdentityContract {
            item: false,
            ..valid
        },
        IdentityContract {
            core: false,
            ..valid
        },
        IdentityContract {
            external: false,
            ..valid
        },
        IdentityContract {
            lang_item: false,
            ..valid
        },
        IdentityContract {
            mir: false,
            ..valid
        },
        IdentityContract {
            intrinsic: true,
            ..valid
        },
        IdentityContract {
            generic_arity_matches: false,
            ..valid
        },
    ] {
        assert!(!authenticate_identity(invalid), "{invalid:?}");
    }
}

#[test]
fn slice_len_requires_normalized_safe_shared_slice_inherent_signature() {
    let valid = valid_signature();
    assert!(authenticate_signature(valid));
    for invalid in [
        SignatureContract {
            method: false,
            ..valid
        },
        SignatureContract {
            inherent: false,
            ..valid
        },
        SignatureContract {
            core_owner: false,
            ..valid
        },
        SignatureContract {
            associated_owner: false,
            ..valid
        },
        SignatureContract {
            normalized: false,
            ..valid
        },
        SignatureContract {
            safe: false,
            ..valid
        },
        SignatureContract {
            rust_abi: false,
            ..valid
        },
        SignatureContract {
            variadic: true,
            ..valid
        },
        SignatureContract { inputs: 0, ..valid },
        SignatureContract { inputs: 2, ..valid },
        SignatureContract {
            shared_slice: false,
            ..valid
        },
        SignatureContract {
            usize_output: false,
            ..valid
        },
        SignatureContract {
            matching_self_type: false,
            ..valid
        },
    ] {
        assert!(!authenticate_signature(invalid), "{invalid:?}");
    }
}

#[test]
fn slice_len_source_authentication_does_not_create_a_terminal_or_skip_unsafe_checks() {
    let collector = include_str!("../collector.rs");
    let name = "authenticate_reviewed_safe_core_slice_len_helper_v1";
    let start = collector.find(name).unwrap();
    let prefix = &collector[..start];
    assert!(prefix.rfind("let Some(local_def_id)").is_some());
    assert!(prefix.rfind("if safety == Safety::Unsafe").is_some());
    assert!(prefix.rfind("self.reachable_unsafe_calls").is_some());
    assert!(
        collector
            .contains("self.process_terminator(&terminator.kind, mir, index, &function.instance)?")
    );
    let terminal = include_str!("../production_semantic_terminal_v1.rs");
    assert!(!terminal.contains(name));
    assert!(!terminal.contains("slice_len_fn"));
}
