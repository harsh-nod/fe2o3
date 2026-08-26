use vstd::prelude::*;

// Exact compiler-derived ranked formulas are appended in a single `verus!`
// block. This file deliberately supplies no axioms, external bodies, or
// caller-provided relation premises.

verus! {

    pub open spec fn fe2o3_pow2_v2(exponent: nat) -> nat
        decreases exponent,
    {
        if exponent == 0 {
            1
        } else {
            2 * fe2o3_pow2_v2((exponent - 1) as nat)
        }
    }

    pub open spec fn fe2o3_bv_modulus_v2(width: nat) -> int {
        if width == 1 { 2 }
        else if width == 8 { 256 }
        else if width == 16 { 65536 }
        else if width == 32 { 4294967296 }
        else if width == 64 { 18446744073709551616 }
        else { fe2o3_pow2_v2(width) as int }
    }

    pub open spec fn fe2o3_bv_norm_v2(value: int, width: nat) -> int {
        value % fe2o3_bv_modulus_v2(width)
    }

    pub open spec fn fe2o3_bv_signed_v2(value: int, width: nat) -> int
        recommends width > 0,
    {
        let normalized = fe2o3_bv_norm_v2(value, width);
        let sign = fe2o3_bv_modulus_v2(width) / 2;
        if normalized >= sign {
            normalized - fe2o3_bv_modulus_v2(width)
        } else {
            normalized
        }
    }

    pub open spec fn fe2o3_signed_div_v2(lhs: int, rhs: int) -> int
        recommends rhs != 0,
    {
        if lhs < 0 {
            if rhs < 0 { (-lhs) / (-rhs) } else { -((-lhs) / rhs) }
        } else if rhs < 0 {
            -(lhs / (-rhs))
        } else {
            lhs / rhs
        }
    }

    pub open spec fn fe2o3_signed_rem_v2(lhs: int, rhs: int) -> int
        recommends rhs != 0,
    {
        lhs - fe2o3_signed_div_v2(lhs, rhs) * rhs
    }

    pub open spec fn fe2o3_bit_v2(value: int, bit: nat) -> int {
        (value / (fe2o3_pow2_v2(bit) as int)) % 2
    }

    pub open spec fn fe2o3_bitwise_v2(kind: nat, lhs: int, rhs: int, width: nat) -> int
        decreases width,
    {
        if width == 0 {
            0
        } else {
            let bit = (width - 1) as nat;
            let lhs_set = fe2o3_bit_v2(lhs, bit) == 1;
            let rhs_set = fe2o3_bit_v2(rhs, bit) == 1;
            let result_set =
                if kind == 0 { lhs_set != rhs_set }
                else if kind == 1 { lhs_set && rhs_set }
                else { lhs_set || rhs_set };
            fe2o3_bitwise_v2(kind, lhs, rhs, bit)
                + if result_set { fe2o3_pow2_v2(bit) as int } else { 0 }
        }
    }

    pub open spec fn fe2o3_shift_left_v2(value: int, shift: nat, width: nat) -> int {
        if shift >= width {
            0
        } else {
            fe2o3_bv_norm_v2(value * fe2o3_pow2_v2(shift) as int, width)
        }
    }

    pub open spec fn fe2o3_shift_right_v2(
        value: int,
        shift: nat,
        width: nat,
        signed: bool,
    ) -> int {
        if shift >= width {
            0
        } else {
            let normalized = fe2o3_bv_norm_v2(value, width);
            let logical = normalized / (fe2o3_pow2_v2(shift) as int);
            if signed && fe2o3_bv_signed_v2(value, width) < 0 {
                logical + (fe2o3_pow2_v2(width) - fe2o3_pow2_v2((width - shift) as nat)) as int
            } else {
                logical
            }
        }
    }

    // This symbol models congruence of identical compiler-side operator DAG
    // applications only. It grants no IEEE real-value, lowering, or target
    // instruction semantics.
    uninterp spec fn fe2o3_ieee_operator_congruence_v2(tag: int, a: int, b: int, c: int) -> int;

    proof fn fe2o3_output_0_effect_formula_v1() {
        let v8: int = fe2o3_bv_norm_v2(0, 64);
        let v7: int = fe2o3_bv_norm_v2(1, 1);
        let v3: int = fe2o3_bv_norm_v2(7, 32);
        let v4: int = fe2o3_bv_norm_v2(7, 32);
        assert(v8 == v8);
        assert(v7 == v7);
        assert(v7 == v7);
        assert(v3 == v4);
    }

    proof fn fe2o3_output_1_effect_formula_v1() {
        let v8: int = fe2o3_bv_norm_v2(0, 64);
        let v7: int = fe2o3_bv_norm_v2(1, 1);
        let v5: int = fe2o3_bv_norm_v2(9, 32);
        let v6: int = fe2o3_bv_norm_v2(9, 32);
        assert(v8 == v8);
        assert(v7 == v7);
        assert(v7 == v7);
        assert(v5 == v6);
    }

    proof fn fe2o3_replay_all_output_effect_formulas_v1() {
        fe2o3_output_0_effect_formula_v1();
        fe2o3_output_1_effect_formula_v1();
    }
}

fn fe2o3_contract_instantiations_v1() {}
