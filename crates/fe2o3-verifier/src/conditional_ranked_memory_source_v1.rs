//! D1 single-write, little-endian byte-memory model. This is not an ISA model.
//! IEEE operations retain the shared interpretation of the checked effect DAG.

use super::{BoundedSource, Budget, Error, source, source_limit};
use crate::functional_refinement_receipt_v2::{
    ConditionalMemoryFormulaV1, ConditionalMemoryLeafV1,
};
use fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1 as Domain;
use fe2o3_pliron::{ProductionConditionalRuntimePremiseV1 as Premise, ProductionSemanticLoadV2};
use std::fmt::Write as _;

const MAX_READS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MemoryLoad {
    pub(super) parameter: u32,
    pub(super) width: u64,
}

pub(super) fn append_memory_theorem(
    out: &mut BoundedSource,
    premises: &[Premise],
    output: u32,
    formula: &ConditionalMemoryFormulaV1<'_>,
    mut resolve: impl FnMut(&ProductionSemanticLoadV2, &mut Budget<'_>) -> Result<MemoryLoad, Error>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(1)?;
    if premises.len() < 4 || premises.len() > 4 + 3 * MAX_READS {
        return Err(Error::Subject("conditional memory premise limit"));
    }
    // Also validates the closed roster, including repeated inputs and tails.
    source::append_premise_theorem(out, premises, output, budget)?;
    formula.visit_leaves(|leaf| {
        budget.charge_work(1)?;
        match leaf {
            ConditionalMemoryLeafV1::Symbol(0) | ConditionalMemoryLeafV1::Node => Ok(()),
            ConditionalMemoryLeafV1::Symbol(_) => {
                Err(Error::Subject("unbound conditional memory symbol"))
            }
            ConditionalMemoryLeafV1::Load(load) => {
                let bound = resolve(load, budget)?;
                require_load_premise(premises, output, bound, budget)
            }
        }
    })?;
    out.write_str(&formula.render().map_err(Error::Execution)?)
        .map_err(source_limit)?;
    out.write_str(MEMORY_PRELUDE).map_err(source_limit)?;
    let a = source::address(&premises[3], output)?;
    formula.require_value_width(a.width)?;
    writeln!(out, "pub open spec fn fe2o3_memory_base_v1(c: Fe2o3MemoryV1) -> int {{ (c.base)({output}) }}\npub open spec fn fe2o3_memory_width_v1() -> int {{ {} }}", a.width).map_err(source_limit)?;
    append_runtime(out, premises, output, budget)?;
    for side in ["gpu", "reference"] {
        out.write_str("#[verifier::opaque]\n")
            .map_err(source_limit)?;
        write!(out, "pub open spec fn fe2o3_memory_{side}_value_v1(c: Fe2o3MemoryV1, m: spec_fn(int) -> u8, i: int) -> int {{ fe2o3_memory_{side}_role3_v1(c.ieee").map_err(source_limit)?;
        append_arguments(out, formula, &mut resolve, budget)?;
        out.write_str(") }\n").map_err(source_limit)?;
    }
    out.write_str("proof fn fe2o3_memory_effect_join_v1(c: Fe2o3MemoryV1, m: spec_fn(int) -> u8, i: int) ensures fe2o3_memory_gpu_value_v1(c,m,i) == fe2o3_memory_reference_value_v1(c,m,i), { reveal(fe2o3_memory_gpu_value_v1); reveal(fe2o3_memory_reference_value_v1); fe2o3_conditional_effect_v1(c.ieee").map_err(source_limit)?;
    append_arguments(out, formula, &mut resolve, budget)?;
    out.write_str("); }\n").map_err(source_limit)?;
    append_reads(out, premises, output, budget)?;
    out.write_str(TRANSITIONS).map_err(source_limit)?;
    Ok(())
}

fn require_load_premise(
    premises: &[Premise],
    output: u32,
    load: MemoryLoad,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let mut found = false;
    for row in premises[4..].chunks_exact(3) {
        budget.charge_work(3)?;
        let (parameter, _, address) = source::read_row(row, output)?;
        if parameter == load.parameter && address.width == load.width {
            found = true;
        }
    }
    if found {
        Ok(())
    } else {
        Err(Error::Subject("load outside conditional memory premises"))
    }
}

fn append_arguments(
    out: &mut BoundedSource,
    formula: &ConditionalMemoryFormulaV1<'_>,
    resolve: &mut impl FnMut(&ProductionSemanticLoadV2, &mut Budget<'_>) -> Result<MemoryLoad, Error>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    for symbol in formula.symbols() {
        budget.charge_work(1)?;
        if symbol == 0 {
            out.write_str(", i").map_err(source_limit)?;
            continue;
        }
        let mut binding = None;
        formula.visit_leaves(|leaf| -> Result<(), Error> {
            budget.charge_work(1)?;
            if let ConditionalMemoryLeafV1::Load(load) = leaf {
                if load.proof_symbol() == symbol {
                    let next = resolve(load, budget)?;
                    if binding.is_some_and(|prior| prior != next) {
                        return Err(Error::Subject("conflicting memory load symbol"));
                    }
                    binding = Some(next);
                }
            }
            Ok(())
        })?;
        let MemoryLoad { parameter, width } =
            binding.ok_or(Error::Subject("missing memory load symbol"))?;
        write!(
            out,
            ", fe2o3_memory_read_v1(m, (c.base)({parameter}) + {width} * i, {width})"
        )
        .map_err(source_limit)?;
    }
    Ok(())
}

fn bound(domain: Domain) -> &'static str {
    match domain {
        Domain::GlobalLaunch => "g",
        Domain::GuardedOutput => "n",
    }
}

fn append_runtime(
    out: &mut BoundedSource,
    premises: &[Premise],
    output: u32,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let a = source::address(&premises[3], output)?;
    let (w, align, k) = (a.width, a.alignment, source::prefix(a.domain));
    write!(
        out,
        r#"
pub open spec fn fe2o3_memory_runtime_v1(c: Fe2o3MemoryV1) -> bool {{
    let n = c.n; let g = c.g; let b = (c.base)({output});
    &&& fe2o3_conditional_d1_v1(g, c.global_x, c.owner)
    &&& 0 <= n <= g <= 18446744073709551615
    &&& n == (c.len)({output})
    &&& fe2o3_conditional_span_v1(b,n,{w},c.pointer_max)
    &&& (n > 0 ==> b % {align} == 0)
    &&& fe2o3_conditional_permissions_v1(c.writable,b,{w}*n)
    &&& fe2o3_conditional_prefix_v1(b,{k},{w},{align},c.index_max,c.offset_max,c.pointer_max)
"#
    )
    .map_err(source_limit)?;
    for row in premises[4..].chunks_exact(3) {
        budget.charge_work(3)?;
        let (p, access, a) = source::read_row(row, output)?;
        let (rw, ra, rk, rb) = (
            a.width,
            a.alignment,
            source::prefix(a.domain),
            bound(access),
        );
        write!(out, r#"
    &&& {rb} <= (c.len)({p})
    &&& fe2o3_conditional_span_v1((c.base)({p}),(c.len)({p}),{rw},c.pointer_max)
    &&& ({rb} > 0 ==> (c.base)({p}) % {ra} == 0)
    &&& fe2o3_conditional_permissions_v1(|a:int| (c.readable)({p},a),(c.base)({p}),{rw}*{rb})
    &&& fe2o3_conditional_prefix_v1((c.base)({p}),{rk},{rw},{ra},c.index_max,c.offset_max,c.pointer_max)
    &&& fe2o3_conditional_separate_v1((c.base)({p}),{rw}*(c.len)({p}),b,{w}*n)
"#).map_err(source_limit)?;
    }
    out.write_str("}\n").map_err(source_limit)
}

fn append_reads(
    out: &mut BoundedSource,
    premises: &[Premise],
    output: u32,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    out.write_str("pub open spec fn fe2o3_memory_reads_equal_v1(c: Fe2o3MemoryV1, m: spec_fn(int)->u8, z: spec_fn(int)->u8, i:int) -> bool { let n=c.n; let g=c.g; true\n").map_err(source_limit)?;
    for row in premises[4..].chunks_exact(3) {
        budget.charge_work(3)?;
        let (p, access, a) = source::read_row(row, output)?;
        let (w, limit) = (a.width, bound(access));
        writeln!(out,"&& (0 <= i < {limit} ==> fe2o3_memory_read_v1(m,(c.base)({p})+{w}*i,{w}) == fe2o3_memory_read_v1(z,(c.base)({p})+{w}*i,{w}))").map_err(source_limit)?;
    }
    out.write_str("}\nproof fn fe2o3_memory_reads_stable_v1(c: Fe2o3MemoryV1, m:spec_fn(int)->u8, z:spec_fn(int)->u8, i:int) requires fe2o3_memory_runtime_v1(c), 0 <= i < c.g, forall|a:int| !fe2o3_memory_output_v1(c,a) ==> #[trigger] m(a) == z(a), ensures fe2o3_memory_reads_equal_v1(c,m,z,i), { let n=c.n; let g=c.g;\n").map_err(source_limit)?;
    for row in premises[4..].chunks_exact(3) {
        budget.charge_work(3)?;
        let (p, access, a) = source::read_row(row, output)?;
        let (w, limit) = (a.width, bound(access));
        write!(
            out,
            r#"
    if i < {limit} {{
        let b = (c.base)({p})+{w}*i;
        assert forall|a:int| b <= a < b+{w} implies #[trigger] m(a) == z(a) by {{
            assert((c.base)({p}) <= a < (c.base)({p})+{w}*(c.len)({p}));
            assert(!fe2o3_memory_output_v1(c,a));
        }};
        fe2o3_memory_read_ext_v1(m,z,b,{w});
    }}
"#
        )
        .map_err(source_limit)?;
    }
    out.write_str("}\nproof fn fe2o3_memory_values_stable_v1(c: Fe2o3MemoryV1, m:spec_fn(int)->u8, z:spec_fn(int)->u8, i:int) requires fe2o3_memory_runtime_v1(c), 0 <= i < c.n, forall|a:int| !fe2o3_memory_output_v1(c,a) ==> #[trigger] m(a) == z(a), ensures fe2o3_memory_gpu_value_v1(c,m,i) == fe2o3_memory_gpu_value_v1(c,z,i), fe2o3_memory_reference_value_v1(c,m,i) == fe2o3_memory_reference_value_v1(c,z,i), { reveal(fe2o3_memory_gpu_value_v1); reveal(fe2o3_memory_reference_value_v1); fe2o3_memory_reads_stable_v1(c,m,z,i); }\n").map_err(source_limit)?;
    append_accesses(out, premises, output, budget)
}

fn append_accesses(
    out: &mut BoundedSource,
    premises: &[Premise],
    output: u32,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    let a = source::address(&premises[3], output)?;
    let (w, align, q) = (a.width, a.alignment, source::selected_index(a.domain));
    write!(out,r#"
pub open spec fn fe2o3_memory_accesses_v1(c: Fe2o3MemoryV1,i:int) -> bool {{
    let n=c.n; let g=c.g;
    &&& fe2o3_conditional_formed_v1((c.base)({output}),{q},{w},{align},c.index_max,c.offset_max,c.pointer_max)
    &&& (i < n ==> fe2o3_conditional_permissions_v1(c.writable,(c.base)({output})+{w}*i,{w}))
"#).map_err(source_limit)?;
    for row in premises[4..].chunks_exact(3) {
        budget.charge_work(3)?;
        let (p, access, a) = source::read_row(row, output)?;
        let (w, align, q, limit) = (
            a.width,
            a.alignment,
            source::selected_index(a.domain),
            bound(access),
        );
        write!(out,r#"
    &&& fe2o3_conditional_formed_v1((c.base)({p}),{q},{w},{align},c.index_max,c.offset_max,c.pointer_max)
    &&& (i < {limit} ==> fe2o3_conditional_permissions_v1(|a:int| (c.readable)({p},a),(c.base)({p})+{w}*i,{w}))
"#).map_err(source_limit)?;
    }
    write!(out,"}}\nproof fn fe2o3_memory_accesses_proved_v1(c:Fe2o3MemoryV1,i:int) requires fe2o3_memory_runtime_v1(c), 0 <= i < c.g, ensures fe2o3_memory_accesses_v1(c,i), {{\nfe2o3_conditional_output_p{output}_v1((c.base)({output}),c.n,c.g,i,i,c.index_max,c.offset_max,c.pointer_max,c.writable,c.global_x,c.owner);\n").map_err(source_limit)?;
    for (ordinal, row) in premises[4..].chunks_exact(3).enumerate() {
        budget.charge_work(3)?;
        let (p, _, _) = source::read_row(row, output)?;
        writeln!(out,"fe2o3_conditional_read_{ordinal}_p{p}_out{output}_v1((c.base)({p}),(c.len)({p}),(c.base)({output}),c.n,c.g,i,i,c.index_max,c.offset_max,c.pointer_max,|a:int| (c.readable)({p},a));").map_err(source_limit)?;
    }
    out.write_str("}\n").map_err(source_limit)
}

const MEMORY_PRELUDE: &str = r#"
// Version 1: a finite D1 launch, scalar loads and one guarded scalar store per
// invocation, little-endian bytes, no atomics, barriers, pointer provenance or
// ISA floating-point claim. The callable expressions above are the checked DAG.
pub struct Fe2o3MemoryV1 {
    pub base: spec_fn(int)->int,
    pub len: spec_fn(int)->int,
    pub readable: spec_fn(int,int)->bool,
    pub writable: spec_fn(int)->bool,
    pub n:int, pub g:int,
    pub index_max:int, pub offset_max:int, pub pointer_max:int,
    pub global_x:spec_fn(int)->int, pub owner:spec_fn(int)->int,
    pub ieee:spec_fn(int,int,int,int)->int,
}
pub open spec fn fe2o3_memory_read_v1(m:spec_fn(int)->u8,b:int,w:nat) -> int
    decreases w,
{ if w == 0 { 0 } else { m(b) as int + 256 * fe2o3_memory_read_v1(m,b+1,(w-1) as nat) } }
proof fn fe2o3_memory_read_ext_v1(m:spec_fn(int)->u8,z:spec_fn(int)->u8,b:int,w:nat)
    requires forall|a:int| b <= a < b+w ==> #[trigger] m(a) == z(a),
    ensures fe2o3_memory_read_v1(m,b,w) == fe2o3_memory_read_v1(z,b,w),
    decreases w,
{
    if w > 0 {
        assert(m(b) == z(b));
        assert forall|a:int| b+1 <= a < b+1+(w-1) implies #[trigger] m(a) == z(a) by {};
        fe2o3_memory_read_ext_v1(m,z,b+1,(w-1) as nat);
    }
}
// State composition uses byte-function congruence, not repeated scalar unfolding.
#[verifier::opaque]
pub open spec fn fe2o3_memory_byte_v1(value:int,offset:int) -> u8 {
    ((value / (fe2o3_pow2_v2((8*offset) as nat) as int)) % 256) as u8
}
pub open spec fn fe2o3_memory_output_v1(c:Fe2o3MemoryV1,a:int) -> bool {
    fe2o3_memory_base_v1(c) <= a < fe2o3_memory_base_v1(c)+fe2o3_memory_width_v1()*c.n
}
pub open spec fn fe2o3_memory_cell_v1(c:Fe2o3MemoryV1,a:int) -> int {
    (a-fe2o3_memory_base_v1(c))/fe2o3_memory_width_v1()
}
pub open spec fn fe2o3_memory_target_v1(c:Fe2o3MemoryV1,m:spec_fn(int)->u8,a:int) -> u8 {
    if fe2o3_memory_output_v1(c,a) {
        fe2o3_memory_byte_v1(fe2o3_memory_reference_value_v1(c,m,fe2o3_memory_cell_v1(c,a)),
            (a-fe2o3_memory_base_v1(c))%fe2o3_memory_width_v1())
    } else { m(a) }
}
"#;

const TRANSITIONS: &str = r#"
pub open spec fn fe2o3_memory_gpu_state_v1(c:Fe2o3MemoryV1,m:spec_fn(int)->u8,k:nat) -> spec_fn(int)->u8
    decreases k,
{
    if k == 0 { m } else {
        let i = (c.global_x)(k-1);
        let b = fe2o3_memory_base_v1(c); let w = fe2o3_memory_width_v1();
        let prev = fe2o3_memory_gpu_state_v1(c,m,(k-1) as nat);
        |a:int| {
            if 0 <= i < c.n && b+w*i <= a < b+w*(i+1) {
                fe2o3_memory_byte_v1(fe2o3_memory_gpu_value_v1(c,prev,i),(a-b)%w)
            } else { prev(a) }
        }
    }
}
pub open spec fn fe2o3_memory_gpu_v1(c:Fe2o3MemoryV1,m:spec_fn(int)->u8,k:nat,a:int) -> u8 {
    (fe2o3_memory_gpu_state_v1(c,m,k))(a)
}
pub open spec fn fe2o3_memory_reference_state_v1(c:Fe2o3MemoryV1,m:spec_fn(int)->u8,k:nat) -> spec_fn(int)->u8
    decreases k,
{
    if k == 0 { m } else {
        let i:int = k-1;
        let b = fe2o3_memory_base_v1(c); let w = fe2o3_memory_width_v1();
        let prev = fe2o3_memory_reference_state_v1(c,m,(k-1) as nat);
        |a:int| {
            if b+w*i <= a < b+w*(i+1) {
                fe2o3_memory_byte_v1(fe2o3_memory_reference_value_v1(c,prev,i),(a-b)%w)
            } else { prev(a) }
        }
    }
}
pub open spec fn fe2o3_memory_reference_v1(c:Fe2o3MemoryV1,m:spec_fn(int)->u8,k:nat,a:int) -> u8 {
    (fe2o3_memory_reference_state_v1(c,m,k))(a)
}
proof fn fe2o3_memory_gpu_prefix_v1(c:Fe2o3MemoryV1,m:spec_fn(int)->u8,k:nat)
    requires fe2o3_memory_runtime_v1(c), k <= c.g,
    ensures forall|a:int| #[trigger] fe2o3_memory_gpu_v1(c,m,k,a) ==
        (if fe2o3_memory_output_v1(c,a) && (c.owner)(fe2o3_memory_cell_v1(c,a)) < k {
            fe2o3_memory_target_v1(c,m,a) } else { m(a) }),
    decreases k,
{
    reveal_with_fuel(fe2o3_memory_gpu_state_v1, 2);
    if k > 0 {
        fe2o3_memory_gpu_prefix_v1(c,m,(k-1) as nat);
        let i = (c.global_x)(k-1);
        assert(0 <= i < c.g && (c.owner)(i) == k-1);
        let prev = fe2o3_memory_gpu_state_v1(c,m,(k-1) as nat);
        assert forall|a:int| !fe2o3_memory_output_v1(c,a) implies #[trigger] prev(a) == m(a) by {
            assert(fe2o3_memory_gpu_v1(c,m,(k-1) as nat,a) == m(a));
        };
        if i < c.n {
            fe2o3_memory_values_stable_v1(c,prev,m,i);
            fe2o3_memory_effect_join_v1(c,m,i);
        }
        assert forall|a:int| #[trigger] fe2o3_memory_gpu_v1(c,m,k,a) ==
            (if fe2o3_memory_output_v1(c,a) && (c.owner)(fe2o3_memory_cell_v1(c,a)) < k {
                fe2o3_memory_target_v1(c,m,a) } else { m(a) }) by {
            let b = fe2o3_memory_base_v1(c); let w = fe2o3_memory_width_v1();
            let j = fe2o3_memory_cell_v1(c,a);
            assert(fe2o3_memory_gpu_v1(c,m,(k-1) as nat,a) == (if fe2o3_memory_output_v1(c,a) && (c.owner)(j) < k-1 {
                fe2o3_memory_target_v1(c,m,a) } else { m(a) }));
            assert(fe2o3_memory_gpu_v1(c,m,k,a) ==
                (if 0 <= i && i < c.n && b+w*i <= a < b+w*(i+1) {
                    fe2o3_memory_byte_v1(fe2o3_memory_gpu_value_v1(c, prev, i),(a-b)%w)
                } else { prev(a) }));
            if fe2o3_memory_output_v1(c,a) {
                assert(0 <= j < c.n);
                assert(0 <= (c.owner)(j) < c.g && (c.global_x)((c.owner)(j)) == j);
                assert(((c.owner)(j) == k-1) == (j == i));
                assert((b+w*i <= a < b+w*(i+1)) == (j == i));
                if j == i {
                    assert(fe2o3_memory_gpu_value_v1(c,prev,i) == fe2o3_memory_reference_value_v1(c,m,j));
                }
            } else if i < c.n {
                assert(!(b+w*i <= a < b+w*(i+1)));
            }
        };
    } else {
        assert forall|a:int| #[trigger] fe2o3_memory_gpu_v1(c,m,k,a) ==
            (if fe2o3_memory_output_v1(c,a) && (c.owner)(fe2o3_memory_cell_v1(c,a)) < k {
                fe2o3_memory_target_v1(c,m,a) } else { m(a) }) by {
            if fe2o3_memory_output_v1(c,a) {
                let j = fe2o3_memory_cell_v1(c,a);
                assert(0 <= j < c.n);
                assert(0 <= (c.owner)(j));
            }
        };
    }
}
proof fn fe2o3_memory_reference_prefix_v1(c:Fe2o3MemoryV1,m:spec_fn(int)->u8,k:nat)
    requires fe2o3_memory_runtime_v1(c), k <= c.n,
    ensures forall|a:int| #[trigger] fe2o3_memory_reference_v1(c,m,k,a) ==
        (if fe2o3_memory_output_v1(c,a) && fe2o3_memory_cell_v1(c,a) < k {
            fe2o3_memory_target_v1(c,m,a) } else { m(a) }),
    decreases k,
{
    reveal_with_fuel(fe2o3_memory_reference_state_v1, 2);
    if k > 0 {
        fe2o3_memory_reference_prefix_v1(c,m,(k-1) as nat);
        let i:int = k-1;
        let prev = fe2o3_memory_reference_state_v1(c,m,(k-1) as nat);
        assert forall|a:int| !fe2o3_memory_output_v1(c,a) implies #[trigger] prev(a) == m(a) by {
            assert(fe2o3_memory_reference_v1(c,m,(k-1) as nat,a) == m(a));
        };
        fe2o3_memory_values_stable_v1(c,prev,m,i);
        assert forall|a:int| #[trigger] fe2o3_memory_reference_v1(c,m,k,a) ==
            (if fe2o3_memory_output_v1(c,a) && fe2o3_memory_cell_v1(c,a) < k {
                fe2o3_memory_target_v1(c,m,a) } else { m(a) }) by {
            let b=fe2o3_memory_base_v1(c); let w=fe2o3_memory_width_v1();
            let j=fe2o3_memory_cell_v1(c,a);
            assert(fe2o3_memory_reference_v1(c,m,(k-1) as nat,a) == (if fe2o3_memory_output_v1(c,a) && j < k-1 {
                fe2o3_memory_target_v1(c,m,a) } else { m(a) }));
            assert(fe2o3_memory_reference_v1(c,m,k,a) ==
                (if b+w*i <= a < b+w*(i+1) {
                    fe2o3_memory_byte_v1(fe2o3_memory_reference_value_v1(c, prev, i),(a-b)%w)
                } else { prev(a) }));
            if fe2o3_memory_output_v1(c,a) {
                assert(0 <= j < c.n);
                assert((b+w*i <= a < b+w*(i+1)) == (j == i));
                if j == i {
                    assert(fe2o3_memory_reference_value_v1(c,prev,i) == fe2o3_memory_reference_value_v1(c,m,j));
                }
            } else { assert(!(b+w*i <= a < b+w*(i+1))); }
        };
    } else {
        assert forall|a:int| #[trigger] fe2o3_memory_reference_v1(c,m,k,a) ==
            (if fe2o3_memory_output_v1(c,a) && fe2o3_memory_cell_v1(c,a) < k {
                fe2o3_memory_target_v1(c,m,a) } else { m(a) }) by {
            if fe2o3_memory_output_v1(c,a) { assert(0 <= fe2o3_memory_cell_v1(c,a)); }
        };
    }
}
pub open spec fn fe2o3_memory_gpu_prefix_reads_v1(c:Fe2o3MemoryV1,m:spec_fn(int)->u8,t:int) -> bool {
    fe2o3_memory_reads_equal_v1(c,m,
        fe2o3_memory_gpu_state_v1(c,m,t as nat),(c.global_x)(t))
}
proof fn fe2o3_conditional_memory_transition_v1(c:Fe2o3MemoryV1,m:spec_fn(int)->u8)
    requires fe2o3_memory_runtime_v1(c),
    ensures
        forall|a:int| #[trigger] fe2o3_memory_gpu_v1(c,m,c.g as nat,a)
            == fe2o3_memory_reference_v1(c,m,c.n as nat,a),
        forall|a:int| !fe2o3_memory_output_v1(c,a) ==>
            #[trigger] fe2o3_memory_gpu_v1(c,m,c.g as nat,a) == m(a),
        forall|i:int| 0 <= i < c.g ==> #[trigger] fe2o3_memory_accesses_v1(c,i),
        forall|t:int| 0 <= t < c.g ==> #[trigger] fe2o3_memory_gpu_prefix_reads_v1(c,m,t),
{
    fe2o3_memory_gpu_prefix_v1(c,m,c.g as nat);
    fe2o3_memory_reference_prefix_v1(c,m,c.n as nat);
    assert forall|a:int| #[trigger] fe2o3_memory_gpu_v1(c,m,c.g as nat,a)
        == fe2o3_memory_reference_v1(c,m,c.n as nat,a) by {
        if fe2o3_memory_output_v1(c,a) {
            let j=fe2o3_memory_cell_v1(c,a);
            assert(0 <= j < c.n);
            assert(0 <= (c.owner)(j) < c.g);
        }
    };
    assert forall|i:int| 0 <= i < c.g implies #[trigger] fe2o3_memory_accesses_v1(c,i) by {
        fe2o3_memory_accesses_proved_v1(c,i);
    };
    assert forall|t:int| 0 <= t < c.g implies #[trigger] fe2o3_memory_gpu_prefix_reads_v1(c,m,t) by {
        fe2o3_memory_gpu_prefix_v1(c,m,t as nat);
        let prev=fe2o3_memory_gpu_state_v1(c,m,t as nat);
        assert forall|a:int| !fe2o3_memory_output_v1(c,a) implies #[trigger] m(a) == prev(a) by {
            assert(fe2o3_memory_gpu_v1(c,m,t as nat,a) == m(a));
        };
        fe2o3_memory_reads_stable_v1(c,m,prev,(c.global_x)(t));
    };
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functional_refinement_receipt_v2::with_conditional_memory_development_v1;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    fn premises() -> [Premise; 7] {
        [
            Premise::D1Launch,
            Premise::OutputWithinGlobalX { parameter: 2 },
            Premise::WritableOutput { parameter: 2 },
            Premise::RepresentableAddress {
                parameter: 2,
                domain: Domain::GuardedOutput,
                element_bytes: 4,
                alignment: 4,
            },
            Premise::ReadableInput {
                parameter: 0,
                domain: Domain::GuardedOutput,
            },
            Premise::SeparateInputOutput {
                input: 0,
                output: 2,
            },
            Premise::RepresentableAddress {
                parameter: 0,
                domain: Domain::GlobalLaunch,
                element_bytes: 4,
                alignment: 4,
            },
        ]
    }

    #[test]
    fn memory_work_denial_stays_on_original_ledger() {
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut out = BoundedSource::new().unwrap();
        let result = with_conditional_memory_development_v1(true, false, |formula| {
            append_memory_theorem(
                &mut out,
                &premises(),
                2,
                formula,
                |_, _| panic!("resolver after work denial"),
                &mut budget,
            )
        });
        assert!(matches!(result, Err(Error::Resource(_))));
        assert!(out.0.is_empty());
    }

    #[test]
    fn memory_refuses_unbound_or_wrong_width_load() {
        for width in [1, 8] {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let mut out = BoundedSource::new().unwrap();
            let result = with_conditional_memory_development_v1(true, false, |formula| {
                append_memory_theorem(
                    &mut out,
                    &premises(),
                    2,
                    formula,
                    |_, _| {
                        Ok(MemoryLoad {
                            parameter: 0,
                            width,
                        })
                    },
                    &mut budget,
                )
            });
            assert!(matches!(
                result,
                Err(Error::Subject("load outside conditional memory premises"))
            ));
        }
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut out = BoundedSource::new().unwrap();
        let result = with_conditional_memory_development_v1(true, false, |formula| {
            append_memory_theorem(
                &mut out,
                &premises(),
                2,
                formula,
                |_, _| Err(Error::Subject("test unbound load")),
                &mut budget,
            )
        });
        assert!(matches!(result, Err(Error::Subject("test unbound load"))));
    }

    #[test]
    fn memory_roster_cap_precedes_emission() {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut out = BoundedSource::new().unwrap();
        let too_many = vec![Premise::D1Launch; 5 + MAX_READS * 3];
        let result = with_conditional_memory_development_v1(false, false, |formula| {
            append_memory_theorem(
                &mut out,
                &too_many,
                2,
                formula,
                |_, _| unreachable!(),
                &mut budget,
            )
        });
        assert!(matches!(
            result,
            Err(Error::Subject("conditional memory premise limit"))
        ));
        assert!(out.0.is_empty());
    }
}
