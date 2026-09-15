use super::*;

pub(super) fn source(guard: bool) -> String {
    let upper = if guard { "|| grid_x > GRID[0]" } else { "" };
    format!(
        r#"#![no_std]
const GRID: [u32; 3] = [4, 1, 1];
const WORKGROUP: [u32; 3] = [256, 1, 1];
pub const fn batch_count_for_launch_v1(grid_x: u32, invocations_per_item: u32) -> Option<usize> {{
    if grid_x == 0 {upper} || invocations_per_item == 0 || invocations_per_item > WORKGROUP[0]
        || WORKGROUP[0] % invocations_per_item != 0 {{ return None; }}
    (grid_x as usize).checked_mul((WORKGROUP[0] / invocations_per_item) as usize)
}}
pub fn helper_range_root(grid_x: u32, first_extent: usize, out: &mut usize) {{
    let Some(batches) = batch_count_for_launch_v1(grid_x, 16) else {{ return; }};
    if first_extent != batches * 16 {{ return; }}
    *out = batches * 4 * 16;
}}
"#
    )
}

pub(super) fn verify(
    function: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1,
    guard: bool,
    proofs: &[bool],
    ranges: &[(usize, usize, u128, u128)],
) {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticCheckedBinaryOpV1, SemanticConstantValueV1, SemanticOperandV1,
    };
    let mut observed = Vec::new();
    let mut byte_constants = 0;
    let mut index_projections = 0;
    for (index, block) in function.blocks().iter().enumerate() {
        for statement in block.statements() {
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                assignment.value().kind().try_visit_operands::<()>(|operand| {
                    match operand {
                        SemanticOperandV1::Constant(value) if matches!(value.value(), SemanticConstantValueV1::Bytes(_)) => byte_constants += 1,
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                            index_projections += place.projections().iter().filter(|p| matches!(p.kind(), fe2o3_mir_model::semantic_mir_v1::SemanticProjectionKindV1::Index(_))).count();
                        }
                        _ => {},
                    }
                    Ok(())
                }).unwrap();
            }
        }
        let SemanticTerminatorKindV1::Assert {
            message:
                SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Multiply,
                    right: SemanticOperandV1::Constant(right),
                    ..
                },
            ..
        } = block.terminator().kind()
        else {
            continue;
        };
        let SemanticConstantValueV1::Scalar(factor) = right.value() else {
            panic!("scalar factor");
        };
        assert!(proofs[index], "retained checked product in bb{index}");
        let (_, _, low, high) = ranges
            .iter()
            .find(|(b, _, _, _)| *b == index)
            .expect("exact borrowed operand/site range");
        assert!(block.statements().iter().any(|s| matches!(s.kind(), SemanticStatementKindV1::Assign(a) if matches!(a.value().kind(), SemanticRvalueKindV1::CheckedBinary(c) if c.operation() == SemanticCheckedBinaryOpV1::Multiply))));
        observed.push((factor.bits(), *low, *high));
    }
    assert!(
        byte_constants >= 3,
        "real constant-array memory bytes must survive import"
    );
    assert!(
        index_projections >= 3,
        "real array index projections must survive import"
    );
    observed.sort_unstable();
    assert_eq!(observed.len(), 3, "first extent and both chained products");
    let expected = vec![(4, 16, 64), (16, 16, 64), (16, 64, 256)];
    assert_eq!(
        observed == expected,
        guard,
        "only the exact source guard supplies these bounds: {observed:?}"
    );
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn helper_result_ranges_actual_amdgpu_array_guard_and_chained_products() {
    run_profile(true, true);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn helper_result_ranges_actual_amdgpu_array_guard_bypass_loses_exact_bound() {
    run_profile(false, true);
}
