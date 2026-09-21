//! Inert rendering controls; no source authentication or owner construction.
use super::*;

fn registers() -> Gfx942OrderedProgramRegistersV1 {
    Gfx942OrderedProgramRegistersV1::new(17, 3, [9, 63, 0]).unwrap()
}

#[test]
fn fixed_expression_uses_exact_names_registers_and_descriptor_order() {
    let (program, expression) =
        render_bitselect_expression_v1(["left", "right", "mask"], registers()).unwrap();
    assert_eq!(program, bitselect_program().unwrap());
    assert_eq!(program.count(), 3);
    assert_eq!(
        expression,
        "fe2o3_device::amdgpu_ordered_program! {\n\
         \x20   gfx942_xnack_off_wave64;\n\
         \x20   scratch(17); out(3);\n\
         \x20   in(9) = left;\n\
         \x20   in(63) = right;\n\
         \x20   in(0) = mask;\n\
         \x20   xor(scratch, input0, input1);\n\
         \x20   and(scratch, scratch, input2);\n\
         \x20   xor(out, input1, scratch);\n}"
    );
}

#[test]
fn formatter_rejects_duplicate_keyword_or_injected_names() {
    for inputs in [
        ["a", "a", "mask"],
        ["a", "b", "b"],
        ["a", "b", "a"],
        ["let", "b", "mask"],
        ["a", "b", "x); panic!(); //"],
        ["", "b", "mask"],
        ["a", "b", "r#type"],
        ["a", "b", "másk"],
    ] {
        assert_eq!(
            render_bitselect_expression_v1(inputs, registers()),
            Err(OrderedProgramMaterializationErrorV1::InvalidSourceIdentifier)
        );
    }
}

#[test]
fn identifier_length_boundary_remains_bounded() {
    let allowed = "a".repeat(64);
    let refused = "a".repeat(65);
    assert!(render_bitselect_expression_v1([&allowed, "b", "mask"], registers()).is_ok());
    assert_eq!(
        render_bitselect_expression_v1([&refused, "b", "mask"], registers()),
        Err(OrderedProgramMaterializationErrorV1::InvalidSourceIdentifier)
    );
}
