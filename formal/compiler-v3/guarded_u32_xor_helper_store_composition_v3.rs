#[path = "../../crates/fe2o3-lower-mir-kernel/verus/formal_compiler_v3_spec_generated.rs"]
mod contract;
#[path = "../../crates/fe2o3-lower-mir-kernel/verus/mir_kir_structured_cfg_v3.rs"]
mod cfg;
#[path = "../../crates/fe2o3-lower-mir-kernel/verus/source_mir_kir_memory_refinement_v3.rs"]
mod memory;
#[path = "../../crates/fe2o3-proof-contracts/verus/dynamic_constrained_affine_bounds_v3.rs"]
mod bounds;

use vstd::prelude::*;

verus! {

/// One runtime-extent certificate in the exact shape accepted by the V3
/// Presburger checker. The aggregate theorem uses three distinct values of
/// this type, one for each input/input/output extent.
pub struct DynamicSiteCertificateV3 {
    pub index_constant: int,
    pub index_coefficients: Seq<int>,
    pub extent_constant: int,
    pub extent_coefficients: Seq<int>,
    pub slack_constant: int,
    pub slack_coefficients: Seq<int>,
    pub component_ceiling: int,
    pub row_constants: Seq<int>,
    pub rows: Seq<Seq<int>>,
    pub domain_witness: Seq<int>,
    pub index_lower_multipliers: Seq<int>,
    pub index_upper_multipliers: Seq<int>,
    pub slack_lower_multipliers: Seq<int>,
    pub slack_upper_multipliers: Seq<int>,
}

pub open spec fn site_checker_accepts_v3(site: DynamicSiteCertificateV3) -> bool {
    bounds::dynamic_checker_accepts(
        site.index_constant,
        site.index_coefficients,
        site.extent_constant,
        site.extent_coefficients,
        site.slack_constant,
        site.slack_coefficients,
        site.component_ceiling,
        site.row_constants,
        site.rows,
        site.domain_witness,
        site.index_lower_multipliers,
        site.index_upper_multipliers,
        site.slack_lower_multipliers,
        site.slack_upper_multipliers,
    )
}

pub open spec fn site_rows_hold_v3(
    site: DynamicSiteCertificateV3,
    point: Seq<int>,
) -> bool {
    bounds::dynamic_rows_hold_v3(site.row_constants, site.rows, point)
}

pub open spec fn site_index_v3(
    site: DynamicSiteCertificateV3,
    point: Seq<int>,
) -> int {
    bounds::dynamic_affine_value_v3(
        site.index_constant,
        site.index_coefficients,
        point,
    )
}

pub open spec fn site_extent_v3(
    site: DynamicSiteCertificateV3,
    point: Seq<int>,
) -> int {
    bounds::dynamic_affine_value_v3(
        site.extent_constant,
        site.extent_coefficients,
        point,
    )
}

/// Value of the exact three-predicate source guard.
pub open spec fn ordered_guard_v3(gid: int, extents: Seq<int>) -> bool {
    &&& extents.len() == 3
    &&& gid < extents[0]
    &&& gid < extents[1]
    &&& gid < extents[2]
}

/// Observable predicate-evaluation order of the source short-circuit chain.
pub open spec fn ordered_guard_trace_v3(gid: int, extents: Seq<int>) -> Seq<int> {
    if extents.len() != 3 {
        Seq::empty()
    } else if !(gid < extents[0]) {
        seq![0]
    } else if !(gid < extents[1]) {
        seq![0, 1]
    } else {
        seq![0, 1, 2]
    }
}

/// Exact shared three-row domain used by every access certificate. The three
/// explicit row indices form a permutation of the shared row sequence while
/// source/input/output extent order remains fixed.
pub open spec fn exact_shared_guard_domain_v3(
    first: DynamicSiteCertificateV3,
    second: DynamicSiteCertificateV3,
    output: DynamicSiteCertificateV3,
    first_row: nat,
    second_row: nat,
    output_row: nat,
) -> bool {
    &&& first_row < 3
    &&& second_row < 3
    &&& output_row < 3
    &&& first_row != second_row
    &&& first_row != output_row
    &&& second_row != output_row
    &&& first.index_constant == second.index_constant
    &&& first.index_constant == output.index_constant
    &&& first.index_coefficients =~= second.index_coefficients
    &&& first.index_coefficients =~= output.index_coefficients
    &&& first.extent_coefficients.len() == first.index_coefficients.len()
    &&& second.extent_coefficients.len() == first.index_coefficients.len()
    &&& output.extent_coefficients.len() == first.index_coefficients.len()
    &&& first.component_ceiling == second.component_ceiling
    &&& first.component_ceiling == output.component_ceiling
    &&& first.row_constants.len() == 3
    &&& first.rows.len() == 3
    &&& first.row_constants =~= second.row_constants
    &&& first.row_constants =~= output.row_constants
    &&& first.rows =~= second.rows
    &&& first.rows =~= output.rows
    &&& first.domain_witness =~= second.domain_witness
    &&& first.domain_witness =~= output.domain_witness
    &&& first.row_constants[first_row as int]
        == first.index_constant - first.extent_constant + 1
    &&& first.row_constants[second_row as int]
        == second.index_constant - second.extent_constant + 1
    &&& first.row_constants[output_row as int]
        == output.index_constant - output.extent_constant + 1
    &&& first.rows[first_row as int] =~= Seq::new(first.index_coefficients.len(), |dimension: int|
        first.index_coefficients[dimension] - first.extent_coefficients[dimension])
    &&& first.rows[second_row as int] =~= Seq::new(first.index_coefficients.len(), |dimension: int|
        second.index_coefficients[dimension] - second.extent_coefficients[dimension])
    &&& first.rows[output_row as int] =~= Seq::new(first.index_coefficients.len(), |dimension: int|
        output.index_coefficients[dimension] - output.extent_coefficients[dimension])
}

proof fn ordered_guard_establishes_shared_rows_v3(
    gid: int,
    first: DynamicSiteCertificateV3,
    second: DynamicSiteCertificateV3,
    output: DynamicSiteCertificateV3,
    point: Seq<int>,
    first_row: nat,
    second_row: nat,
    output_row: nat,
)
    requires
        exact_shared_guard_domain_v3(
            first, second, output, first_row, second_row, output_row,
        ),
        point.len() == first.index_coefficients.len(),
        gid == site_index_v3(first, point),
        gid == site_index_v3(second, point),
        gid == site_index_v3(output, point),
        ordered_guard_v3(gid, seq![
            site_extent_v3(first, point),
            site_extent_v3(second, point),
            site_extent_v3(output, point),
        ]),
    ensures
        site_rows_hold_v3(first, point),
        site_rows_hold_v3(second, point),
        site_rows_hold_v3(output, point),
{
    bounds::dynamic_strict_affine_guard_implies_normalized_row_v3(
        first.index_constant,
        first.index_coefficients,
        first.extent_constant,
        first.extent_coefficients,
        point,
    );
    bounds::dynamic_strict_affine_guard_implies_normalized_row_v3(
        second.index_constant,
        second.index_coefficients,
        second.extent_constant,
        second.extent_coefficients,
        point,
    );
    bounds::dynamic_strict_affine_guard_implies_normalized_row_v3(
        output.index_constant,
        output.index_coefficients,
        output.extent_constant,
        output.extent_coefficients,
        point,
    );
    assert forall|row: int| 0 <= row < first.rows.len() implies
        bounds::dynamic_affine_value_v3(
            first.row_constants[row], first.rows[row], point,
        ) <= 0 by {
        assert(row == first_row || row == second_row || row == output_row) by {
            if row != first_row && row != second_row && row != output_row {
                assert(false);
            }
        }
        if row == first_row {
        } else if row == second_row {
        } else {
            assert(row == output_row);
        }
    }
}

proof fn checked_dynamic_site_is_in_bounds_v3(
    site: DynamicSiteCertificateV3,
    point: Seq<int>,
)
    requires
        site_checker_accepts_v3(site),
        point.len() == site.index_coefficients.len(),
        site_rows_hold_v3(site, point),
    ensures
        0 <= site_index_v3(site, point),
        site_index_v3(site, point) < site_extent_v3(site, point),
{
    bounds::accepted_dynamic_constrained_affine_certificate_is_sound(
        site.index_constant,
        site.index_coefficients,
        site.extent_constant,
        site.extent_coefficients,
        site.slack_constant,
        site.slack_coefficients,
        site.component_ceiling,
        site.row_constants,
        site.rows,
        site.domain_witness,
        site.index_lower_multipliers,
        site.index_upper_multipliers,
        site.slack_lower_multipliers,
        site.slack_upper_multipliers,
        point,
    );
}

proof fn checked_u32_read_fits_v3(allocation: memory::AllocationV3, offset: int)
    requires
        memory::allocation_valid_v3(allocation),
        0 <= offset,
        offset + 4 <= allocation.bytes.len(),
    ensures
        0 <= memory::read_u32_v3(allocation, offset) < 4294967296,
{
}

proof fn memory_and_cfg_pow2_agree_v3(exponent: nat)
    ensures memory::pow2_v3(exponent) == cfg::pow2_v3(exponent),
    decreases exponent,
{
    if exponent > 0 {
        memory_and_cfg_pow2_agree_v3((exponent - 1) as nat);
    }
}

proof fn memory_and_cfg_xor_agree_v3(left: int, right: int, width: nat)
    ensures
        memory::xor_v3(left, right, width)
            == cfg::mir_bitwise_xor_v3(left, right, width, 4294967296),
        memory::xor_v3(left, right, width)
            == cfg::kir_bitwise_xor_v3(left, right, width, 4294967296),
    decreases width,
{
    if width > 0 {
        let bit = (width - 1) as nat;
        memory_and_cfg_pow2_agree_v3(bit);
        memory_and_cfg_xor_agree_v3(left, right, bit);
        assert(memory::bit_v3(left, bit) == cfg::bit_v3(left, bit, 4294967296));
        assert(memory::bit_v3(right, bit) == cfg::bit_v3(right, bit, 4294967296));
    }
}

/// Composes the three production-connected V3 claims for one lane of the
/// exact guarded two-load/XOR-helper/store kernel. The source observation is
/// the bounded source model authenticated by the live checker, not general
/// Rust or HIR semantics.
pub proof fn fe2o3_guarded_u32_xor_helper_store_composes_v3(
    source_memory: memory::MemoryV3,
    mir_memory: memory::MemoryV3,
    kir_memory: memory::MemoryV3,
    guard: bool,
    guard_trace: Seq<int>,
    gid: int,
    first_site: DynamicSiteCertificateV3,
    second_site: DynamicSiteCertificateV3,
    output_site: DynamicSiteCertificateV3,
    point: Seq<int>,
    first_row: nat,
    second_row: nat,
    output_row: nat,
    source_opcode: int,
    mir_opcode: int,
    kir_opcode: int,
    fallback: int,
    source_identity: int,
    semantic_mir_identity: int,
    kir_identity: int,
    model_identity: int,
)
    requires
        source_identity != 0,
        semantic_mir_identity != 0,
        kir_identity != 0,
        model_identity != 0,
        gid >= 0,
        memory::opcode_relation_v3(source_opcode, mir_opcode, kir_opcode),
        memory::environments_related_v3(source_memory, mir_memory, kir_memory),
        memory::memory_valid_v3(source_memory),
        0 <= fallback < 4294967296,
        site_checker_accepts_v3(first_site),
        site_checker_accepts_v3(second_site),
        site_checker_accepts_v3(output_site),
        point.len() == first_site.index_coefficients.len(),
        point.len() == second_site.index_coefficients.len(),
        point.len() == output_site.index_coefficients.len(),
        first_row < 3,
        second_row < 3,
        output_row < 3,
        first_row != second_row,
        first_row != output_row,
        second_row != output_row,
        exact_shared_guard_domain_v3(
            first_site, second_site, output_site, first_row, second_row, output_row,
        ),
        gid == site_index_v3(first_site, point),
        gid == site_index_v3(second_site, point),
        gid == site_index_v3(output_site, point),
        {
            let extents = seq![
                site_extent_v3(first_site, point),
                site_extent_v3(second_site, point),
                site_extent_v3(output_site, point),
            ];
            &&& guard == ordered_guard_v3(gid, extents)
            &&& guard_trace == ordered_guard_trace_v3(gid, extents)
            &&& source_memory.first.bytes.len() == 4 * extents[0]
            &&& source_memory.second.bytes.len() == 4 * extents[1]
            &&& source_memory.output.bytes.len() == 4 * extents[2]
        },
    ensures
        guard ==> gid < site_extent_v3(first_site, point),
        guard ==> gid < site_extent_v3(second_site, point),
        guard ==> gid < site_extent_v3(output_site, point),
        memory::source_step_v3(source_memory, guard, gid, source_opcode, fallback)
            == memory::mir_step_v3(mir_memory, guard, gid, mir_opcode, fallback),
        memory::mir_step_v3(mir_memory, guard, gid, mir_opcode, fallback)
            == memory::kir_step_v3(kir_memory, guard, gid, kir_opcode, fallback),
        !guard ==> memory::source_step_v3(
            source_memory, guard, gid, source_opcode, fallback,
        ).trace.len() == 0,
        !guard ==> memory::source_step_v3(
            source_memory, guard, gid, source_opcode, fallback,
        ).memory == source_memory,
        guard ==> memory::source_step_v3(
            source_memory, guard, gid, source_opcode, fallback,
        ).trace.len() == 3,
        guard ==> memory::source_step_v3(
            source_memory, guard, gid, source_opcode, fallback,
        ).result == memory::selected_result_v3(source_memory, gid, fallback),
        guard ==> {
            let addresses = memory::addresses_for_gid_v3(source_memory, gid);
            let left = memory::read_u32_v3(source_memory.first, addresses.0.offset);
            let right = memory::read_u32_v3(source_memory.second, addresses.1.offset);
            &&& memory::selected_result_v3(source_memory, gid, fallback)
                == cfg::mir_xor_diamond_call_observation_v3(left, right, fallback)[6]
            &&& memory::selected_result_v3(source_memory, gid, fallback)
                == cfg::kir_xor_diamond_call_observation_v3(left, right, fallback)[6]
        },
        guard ==> memory::source_step_v3(
            source_memory, guard, gid, source_opcode, fallback,
        ).memory.output == memory::write_u32_v3(
            source_memory.output,
            4 * gid,
            memory::selected_result_v3(source_memory, gid, fallback),
        ),
{
    assert(contract::formal_compiler_v3_word_bits() == 32);
    assert(contract::formal_compiler_v3_byte_width() == 4);
    assert(contract::formal_compiler_v3_guard_predicates() == 3);
    assert(contract::formal_compiler_v3_dynamic_extents() == 3);
    assert(contract::formal_compiler_v3_uses_ordered_short_circuit_guard());
    assert(contract::formal_compiler_v3_uses_gid_times_byte_width());

    if guard {
        ordered_guard_establishes_shared_rows_v3(
            gid,
            first_site,
            second_site,
            output_site,
            point,
            first_row,
            second_row,
            output_row,
        );
        checked_dynamic_site_is_in_bounds_v3(first_site, point);
        checked_dynamic_site_is_in_bounds_v3(second_site, point);
        checked_dynamic_site_is_in_bounds_v3(output_site, point);
        let addresses = memory::addresses_for_gid_v3(source_memory, gid);
        assert(gid >= 0);
        assert(4 * gid + 4 <= source_memory.first.bytes.len());
        assert(4 * gid + 4 <= source_memory.second.bytes.len());
        assert(4 * gid + 4 <= source_memory.output.bytes.len());
        vstd::arithmetic::div_mod::lemma_mod_multiples_basic(gid, 4);
        assert(4 * gid == gid * 4) by (nonlinear_arith);
        assert(addresses.0.offset == 4 * gid);
        assert(addresses.1.offset == 4 * gid);
        assert(addresses.2.offset == 4 * gid);
        assert(memory::access_ok_v3(source_memory.first, addresses.0));
        assert(memory::access_ok_v3(source_memory.second, addresses.1));
        assert(memory::access_ok_v3(source_memory.output, addresses.2));
        checked_u32_read_fits_v3(source_memory.first, addresses.0.offset);
        checked_u32_read_fits_v3(source_memory.second, addresses.1.offset);
        memory_and_cfg_xor_agree_v3(
            memory::read_u32_v3(source_memory.first, addresses.0.offset),
            memory::read_u32_v3(source_memory.second, addresses.1.offset),
            32,
        );
        cfg::fe2o3_mir_kir_xor_diamond_call_refines_v3(
            memory::read_u32_v3(source_memory.first, addresses.0.offset),
            memory::read_u32_v3(source_memory.second, addresses.1.offset),
            memory::read_u32_v3(source_memory.first, addresses.0.offset),
            memory::read_u32_v3(source_memory.second, addresses.1.offset),
            fallback,
        );
    }
    assert(guard == memory::guard_for_gid_v3(source_memory, gid));
    assert(guard ==> {
        let addresses = memory::addresses_for_gid_v3(source_memory, gid);
        memory::access_ok_v3(source_memory.first, addresses.0)
            && memory::access_ok_v3(source_memory.second, addresses.1)
            && memory::access_ok_v3(source_memory.output, addresses.2)
    });
    memory::fe2o3_guarded_two_load_xor_store_refines_v3(
        source_memory,
        mir_memory,
        kir_memory,
        guard,
        gid,
        source_opcode,
        mir_opcode,
        kir_opcode,
        fallback,
        source_identity,
        semantic_mir_identity,
        kir_identity,
        model_identity,
    );
}

}
