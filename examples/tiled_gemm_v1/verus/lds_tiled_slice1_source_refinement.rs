use vstd::prelude::*;

#[path = "lds_tiled_slice1.rs"]
mod slice1;

verus! {

/// Four big-endian words of a SHA-256 identity. Rust-side evidence recomputes
/// each digest from the authenticated compiler and canonical-IR sources.
pub struct Digest256V1 {
    pub word0: nat,
    pub word1: nat,
    pub word2: nat,
    pub word3: nat,
}

pub open spec fn source_portable_mir_identity_v1() -> Digest256V1 {
    Digest256V1 {
        word0: 0x0467cd6daad414de,
        word1: 0x74b669cc223c26f3,
        word2: 0x8d36e28b8a67724d,
        word3: 0x245fea6e2f18d4fa,
    }
}

pub open spec fn reviewed_correspondence_identity_v1() -> Digest256V1 {
    Digest256V1 {
        word0: 0xd57cba1c5294d828,
        word1: 0xd2144c15b3eac450,
        word2: 0x4c22fa269bf2fa67,
        word3: 0x2ffd6ba1c994b3eb,
    }
}

pub open spec fn canonical_module_identity_v1() -> Digest256V1 {
    Digest256V1 {
        word0: 0x8d876b342378ee87,
        word1: 0x87ff34a866099b36,
        word2: 0xb3a0f4eb7fb7e790,
        word3: 0x13af86bf6a65eb60,
    }
}

/// Source-visible facts admitted only after the exact attributed portable-MIR
/// identity has matched. Event ranks model the ordinary Rust body in order.
pub struct AttributedSlice1SourceV1 {
    pub portable_mir_identity: Digest256V1,
    pub correspondence_identity: Digest256V1,
    pub a_elements: nat,
    pub b_elements: nat,
    pub c_elements: nat,
    pub lanes: nat,
    pub outputs_per_lane: nat,
    pub lds_allocations: nat,
    pub lds_elements_per_allocation: nat,
    pub stage_event: nat,
    pub publish_barrier_event: nat,
    pub lds_read_event: nat,
    pub mfma_event: nat,
    pub global_store_event: nat,
}

/// Exact closed identity of `fe2o3::tiled_gemm_lds_v1`. Generic Kernel IR is
/// not admitted by this relation.
pub struct CanonicalSlice1IrV1 {
    pub module_identity: Digest256V1,
    pub a_elements: nat,
    pub b_elements: nat,
    pub c_elements: nat,
    pub lanes: nat,
    pub outputs_per_lane: nat,
    pub lds_allocations: nat,
    pub lds_elements_per_allocation: nat,
    pub lds_bytes_per_allocation: nat,
    pub lds_alignment: nat,
    pub stage_event: nat,
    pub publish_barrier_event: nat,
    pub lds_read_event: nat,
    pub mfma_event: nat,
    pub global_store_event: nat,
}

pub open spec fn exact_attributed_slice1_source_v1() -> AttributedSlice1SourceV1 {
    AttributedSlice1SourceV1 {
        portable_mir_identity: source_portable_mir_identity_v1(),
        correspondence_identity: reviewed_correspondence_identity_v1(),
        a_elements: 256,
        b_elements: 256,
        c_elements: 256,
        lanes: 64,
        outputs_per_lane: 4,
        lds_allocations: 2,
        lds_elements_per_allocation: 256,
        stage_event: 0,
        publish_barrier_event: 1,
        lds_read_event: 2,
        mfma_event: 3,
        global_store_event: 4,
    }
}

pub open spec fn exact_canonical_slice1_ir_v1() -> CanonicalSlice1IrV1 {
    CanonicalSlice1IrV1 {
        module_identity: canonical_module_identity_v1(),
        a_elements: 256,
        b_elements: 256,
        c_elements: 256,
        lanes: 64,
        outputs_per_lane: 4,
        lds_allocations: 2,
        lds_elements_per_allocation: 256,
        lds_bytes_per_allocation: 512,
        lds_alignment: 16,
        stage_event: 0,
        publish_barrier_event: 1,
        lds_read_event: 2,
        mfma_event: 3,
        global_store_event: 4,
    }
}

pub open spec fn source_guard_accepts_v1(
    lane: nat,
    a_len: nat,
    b_len: nat,
    c_len: nat,
) -> bool {
    lane < 64 && a_len == 256 && b_len == 256 && c_len == 256
}

/// This relation is an identity-bound, bounded source/model correspondence.
/// It is not a proof that rustc, LLVM, linking, or emitted machine code refine
/// either side, and it grants no descriptor, load, or launch authority.
pub open spec fn source_to_canonical_ir_correspondence_v1(
    source: AttributedSlice1SourceV1,
    ir: CanonicalSlice1IrV1,
) -> bool {
    &&& source.portable_mir_identity == source_portable_mir_identity_v1()
    &&& source.correspondence_identity == reviewed_correspondence_identity_v1()
    &&& ir.module_identity == canonical_module_identity_v1()
    &&& source.a_elements == ir.a_elements
    &&& source.b_elements == ir.b_elements
    &&& source.c_elements == ir.c_elements
    &&& source.lanes == ir.lanes
    &&& source.outputs_per_lane == ir.outputs_per_lane
    &&& source.lds_allocations == ir.lds_allocations
    &&& source.lds_elements_per_allocation == ir.lds_elements_per_allocation
    &&& ir.lds_bytes_per_allocation == 512
    &&& ir.lds_alignment == 16
    &&& source.stage_event == ir.stage_event
    &&& source.publish_barrier_event == ir.publish_barrier_event
    &&& source.lds_read_event == ir.lds_read_event
    &&& source.mfma_event == ir.mfma_event
    &&& source.global_store_event == ir.global_store_event
    &&& source.stage_event < source.publish_barrier_event
    &&& source.publish_barrier_event < source.lds_read_event
    &&& source.lds_read_event < source.mfma_event
    &&& source.mfma_event < source.global_store_event
}

pub proof fn exact_source_guard_requires_exact_lengths_v1(
    lane: nat,
    a_len: nat,
    b_len: nat,
    c_len: nat,
)
    requires source_guard_accepts_v1(lane, a_len, b_len, c_len),
    ensures
        lane < 64,
        a_len == slice1::slice1_tile_elements_v1(),
        b_len == slice1::slice1_tile_elements_v1(),
        c_len == slice1::slice1_tile_elements_v1(),
{
}

pub proof fn exact_attributed_source_selects_canonical_identity_v1()
    ensures source_to_canonical_ir_correspondence_v1(
        exact_attributed_slice1_source_v1(),
        exact_canonical_slice1_ir_v1(),
    ),
{
}

/// One arbitrary admitted coordinate witnesses all universally quantified
/// Slice 1 obligations: exact extents, bounded global inputs, complete and
/// disjoint LDS initialization, a converged publish barrier, and a unique C
/// owner. The imported theorems cover every coordinate satisfying `requires`.
pub proof fn attributed_slice1_source_obligations_refine_canonical_ir_v1(
    a: Seq<real>,
    b: Seq<real>,
    c: Seq<real>,
    arrived: Seq<bool>,
    epoch: nat,
    row: nat,
    depth: nat,
    column: nat,
    lane: nat,
    component: nat,
    other_lane: nat,
    other_component: nat,
)
    requires
        source_guard_accepts_v1(lane, a.len(), b.len(), c.len()),
        slice1::arrivals_match_slice1_control_flow_v1(arrived),
        row < 16,
        depth < 16,
        column < 16,
        component < 4,
        other_lane < 64,
        other_component < 4,
        lane != other_lane || component != other_component,
    ensures
        source_to_canonical_ir_correspondence_v1(
            exact_attributed_slice1_source_v1(),
            exact_canonical_slice1_ir_v1(),
        ),
        slice1::a_global_index_v1(row, depth) < a.len(),
        slice1::b_global_index_v1(depth, column) < b.len(),
        slice1::a_read_initialized_same_epoch_v1(a, epoch, row, depth),
        slice1::b_read_initialized_same_epoch_v1(b, epoch, depth, column),
        arrived[lane as int],
        slice1::c_global_index_v1(lane, component)
            != slice1::c_global_index_v1(other_lane, other_component),
{
    exact_source_guard_requires_exact_lengths_v1(lane, a.len(), b.len(), c.len());
    exact_attributed_source_selects_canonical_identity_v1();
    slice1::all_slice1_global_input_indices_are_bounded_v1(
        a, b, row, depth, column,
    );
    slice1::every_a_lds_read_is_initialized_in_same_epoch_v1(a, epoch, row, depth);
    slice1::every_b_lds_read_is_initialized_in_same_epoch_v1(b, epoch, depth, column);
    slice1::slice1_barrier_converges_for_all_64_lanes_v1(arrived, lane);
    slice1::fixed_tile_c_stores_are_disjoint_v1(
        lane, component, other_lane, other_component,
    );
}

} // verus!
