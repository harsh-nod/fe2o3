use super::*;

fn valid() -> SaturatingContractV1 {
    SaturatingContractV1 {
        supported_integer: true,
        safe: true,
        rust_abi: true,
        variadic: false,
        nounwind: true,
        input_count: 2,
        inputs_match: true,
        result_matches: true,
    }
}

#[test]
fn only_the_two_exact_intrinsic_names_are_selected() {
    assert_eq!(
        operation_v1("saturating_add"),
        Some(SemanticSaturatingIntegerOpV1::Add)
    );
    assert_eq!(
        operation_v1("saturating_sub"),
        Some(SemanticSaturatingIntegerOpV1::Subtract)
    );
    for name in [
        "saturating_mul",
        "saturating_add_signed",
        "saturating_sub_unsigned",
        "simd_saturating_add",
        "Saturating_add",
        "user::saturating_add",
        "saturating_sub ",
        "fabs",
        "atomic_xadd",
    ] {
        assert_eq!(operation_v1(name), None, "{name}");
    }
}

#[test]
fn scalar_widths_are_closed_and_not_host_sized() {
    for bits in [8, 16, 32, 64] {
        assert!(supported_bits_v1(bits));
    }
    for bits in [0, 1, 7, 24, 65, 128, u64::MAX] {
        assert!(!supported_bits_v1(bits));
    }
    assert_eq!(validate_contract_v1(valid()), Ok(()));
}

#[test]
fn unsafe_foreign_variadic_or_unwinding_signatures_refuse() {
    for contract in [
        SaturatingContractV1 {
            safe: false,
            ..valid()
        },
        SaturatingContractV1 {
            rust_abi: false,
            ..valid()
        },
        SaturatingContractV1 {
            variadic: true,
            ..valid()
        },
    ] {
        assert_eq!(
            validate_contract_v1(contract),
            Err(SaturatingIntrinsicErrorV1::Signature)
        );
    }
    assert_eq!(
        validate_contract_v1(SaturatingContractV1 {
            nounwind: false,
            ..valid()
        }),
        Err(SaturatingIntrinsicErrorV1::Unwind)
    );
}

#[test]
fn arity_operand_result_and_domain_mismatches_refuse_separately() {
    for input_count in [0, 1, 3, usize::MAX] {
        assert_eq!(
            validate_contract_v1(SaturatingContractV1 {
                input_count,
                ..valid()
            }),
            Err(SaturatingIntrinsicErrorV1::CallArity)
        );
    }
    for (contract, error) in [
        (
            SaturatingContractV1 {
                supported_integer: false,
                ..valid()
            },
            SaturatingIntrinsicErrorV1::UnsupportedIntegerType,
        ),
        (
            SaturatingContractV1 {
                inputs_match: false,
                ..valid()
            },
            SaturatingIntrinsicErrorV1::InputType,
        ),
        (
            SaturatingContractV1 {
                result_matches: false,
                ..valid()
            },
            SaturatingIntrinsicErrorV1::ResultType,
        ),
    ] {
        assert_eq!(validate_contract_v1(contract), Err(error));
    }
}

#[test]
fn new_operations_are_never_atomic_normalization_recipes() {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAtomicAccessV1, SemanticAtomicOrderingV1, SemanticAtomicRmwOpV1,
        SemanticAtomicScopeV1,
    };
    let old = ProductionRustcIntrinsicOperationV1::AtomicRmw {
        operation: SemanticAtomicRmwOpV1::Add,
        access: SemanticAtomicAccessV1::new(
            SemanticAtomicOrderingV1::Relaxed,
            SemanticAtomicScopeV1::System,
        ),
    };
    assert_eq!(old.operation_tag(), 0);
    assert!(old.atomic_rmw().is_some());
    assert_eq!(
        ProductionRustcIntrinsicOperationV1::FabsF32.operation_tag(),
        1
    );
    for (operation, tag) in [
        (SemanticSaturatingIntegerOpV1::Add, 2),
        (SemanticSaturatingIntegerOpV1::Subtract, 3),
    ] {
        let operation = ProductionRustcIntrinsicOperationV1::SaturatingInteger(operation);
        assert_eq!(operation.operation_tag(), tag);
        assert_eq!(operation.atomic_rmw(), None);
    }
}

#[test]
fn ordinary_items_cannot_enter_the_classifier_via_name_matching() {
    // Structural regression: actual TyCtxt capture is separately exercised by
    // the ordinary-source tests. No fabricated booleans stand in for an owner.
    let source = include_str!("production_rustc_intrinsic_v1.rs");
    let start = source.find("pub(crate) fn classify<'tcx>(").unwrap();
    let function = &source[start
        ..source[start..]
            .find("\nfn is_fabs_intrinsic_name_v1")
            .unwrap()
            + start];
    let item_gate = function
        .find("let InstanceKind::Intrinsic(def_id) = instance.def else")
        .unwrap();
    let metadata = function.find(".intrinsic(def_id)").unwrap();
    let saturation = function.find("saturating_integer::operation_v1").unwrap();
    assert!(item_gate < metadata && metadata < saturation);
    assert!(function[item_gate..metadata].contains("return Ok(None)"));
    let planner = include_str!("rustc_semantic_plan_v1.rs");
    assert!(!planner.contains("expect(\"preflight retains only normalized atomic intrinsics\")"));
    assert!(planner.contains(".ok_or(ProductionSemanticPreflightErrorV1::IdentityTableMismatch)?"));
}
