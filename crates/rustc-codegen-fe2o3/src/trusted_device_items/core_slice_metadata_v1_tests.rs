use super::*;

fn valid_identity() -> IdentityContract<'static> {
    IdentityContract {
        item: true,
        core: true,
        external: true,
        free_function: true,
        path: REVIEWED_PATH,
        declared_generics: 1,
        instance_generics: 1,
        mir: true,
        intrinsic: false,
    }
}

fn valid_signature() -> SignatureContract {
    SignatureContract {
        normalized: true,
        safe: true,
        rust_abi: true,
        variadic: false,
        inputs: 1,
        generic_slice: true,
        const_slice_pointer: true,
        matching_generic_pointee: true,
        usize_output: true,
    }
}

#[test]
fn slice_metadata_requires_exact_external_core_wrapper_identity() {
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
            free_function: false,
            ..valid
        },
        IdentityContract {
            declared_generics: 0,
            ..valid
        },
        IdentityContract {
            declared_generics: 2,
            ..valid
        },
        IdentityContract {
            instance_generics: 0,
            ..valid
        },
        IdentityContract {
            instance_generics: 2,
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
    ] {
        assert!(!authenticate_identity(invalid), "{invalid:?}");
    }
    for path in [
        "metadata",
        "lookalike::core::ptr::metadata",
        "core::ptr::metadata::metadata",
        "core::ptr::from_raw_parts",
        "core::ptr::from_raw_parts_mut",
        "core::intrinsics::ptr_metadata",
        "core::ptr::metadata_extra",
    ] {
        assert!(
            !authenticate_identity(IdentityContract { path, ..valid }),
            "{path}"
        );
    }
}

#[test]
fn slice_metadata_requires_normalized_const_slice_pointer_to_usize() {
    let valid = valid_signature();
    assert!(authenticate_signature(valid));
    for invalid in [
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
            generic_slice: false,
            ..valid
        },
        SignatureContract {
            const_slice_pointer: false,
            ..valid
        },
        SignatureContract {
            matching_generic_pointee: false,
            ..valid
        },
        SignatureContract {
            usize_output: false,
            ..valid
        },
    ] {
        assert!(!authenticate_signature(invalid), "{invalid:?}");
    }
}

#[test]
fn slice_metadata_source_authentication_preserves_unsafe_and_child_checks() {
    let collector = include_str!("../collector.rs");
    let name = "authenticate_reviewed_safe_core_slice_metadata_helper_v1";
    assert_eq!(collector.matches(name).count(), 1);
    let start = collector.find(name).unwrap();
    let prefix = &collector[..start];
    let external = prefix.rfind("let Some(local_def_id)").unwrap();
    assert!(prefix.rfind("if safety == Safety::Unsafe").unwrap() < external);
    assert!(prefix.rfind("self.reachable_unsafe_calls").unwrap() < external);
    assert!(
        collector
            .contains("self.process_terminator(&terminator.kind, mir, index, &function.instance)?")
    );
    let terminal = include_str!("../production_semantic_terminal_v1.rs");
    assert!(!terminal.contains(name));
    assert!(!terminal.contains(REVIEWED_PATH));
}
