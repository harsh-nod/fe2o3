use vstd::prelude::*;

verus! {
    uninterp spec fn fe2o3_semantic_op_v2(tag: int, a: int, b: int, c: int) -> int;

    proof fn fe2o3_functional_refinement_v2(s7: int) {
        let v0: int = fe2o3_semantic_op_v2(
            1,
            fe2o3_semantic_op_v2(
                4132,
                fe2o3_semantic_op_v2(1132, s7, 0, 0),
                fe2o3_semantic_op_v2(2132, 9, 0, 0),
                0,
            ),
            0,
            0,
        );
        let v1: int = fe2o3_semantic_op_v2(
            1,
            fe2o3_semantic_op_v2(
                4132,
                fe2o3_semantic_op_v2(1132, s7, 0, 0),
                fe2o3_semantic_op_v2(2132, 9, 0, 0),
                0,
            ),
            0,
            0,
        );
        assert(v0 == v1);
    }
}

fn main() {}
