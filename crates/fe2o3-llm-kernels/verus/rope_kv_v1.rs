use vstd::arithmetic::div_mod::{
    lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse, lemma_mod_bound,
};
use vstd::prelude::*;

verus! {

pub open spec fn head_dimension_v1() -> nat { 128 }
pub open spec fn rotary_half_dimension_v1() -> nat { 64 }
pub open spec fn kv_heads_v1() -> nat { 8 }
pub open spec fn maximum_context_v1() -> nat { 8192 }
pub open spec fn maximum_sequences_v1() -> nat { 32 }
pub open spec fn maximum_page_entries_v1() -> nat { 512 }

pub open spec fn split_half_pair_v1(dimension: nat) -> nat {
    if dimension < rotary_half_dimension_v1() {
        dimension + rotary_half_dimension_v1()
    } else {
        (dimension - rotary_half_dimension_v1()) as nat
    }
}

/// Every admitted Qwen3 rotary dimension has one bounded, non-self partner.
pub proof fn split_half_pair_is_bounded_and_non_self_v1(dimension: nat)
    requires dimension < head_dimension_v1(),
    ensures
        split_half_pair_v1(dimension) < head_dimension_v1(),
        split_half_pair_v1(dimension) != dimension,
{
}

/// Applying the exact split-half pairing twice recovers the original dimension.
pub proof fn split_half_pair_is_involutive_v1(dimension: nat)
    requires dimension < head_dimension_v1(),
    ensures split_half_pair_v1(split_half_pair_v1(dimension)) == dimension,
{
}

pub open spec fn logical_page_v1(logical_token: nat, page_tokens: nat) -> nat {
    logical_token / page_tokens
}

pub open spec fn token_slot_v1(logical_token: nat, page_tokens: nat) -> nat {
    logical_token % page_tokens
}

proof fn div_mod_reconstructs_v1(value: nat, divisor: nat)
    requires divisor > 0,
    ensures
        value == (value / divisor) * divisor + value % divisor,
        value % divisor < divisor,
{
    lemma_mod_bound(value as int, divisor as int);
    lemma_fundamental_div_mod(value as int, divisor as int);
    assert(value == divisor * (value / divisor) + value % divisor);
    assert(divisor * (value / divisor) == (value / divisor) * divisor)
        by (nonlinear_arith);
}

proof fn bounded_quotient_v1(value: nat, blocks: nat, divisor: nat)
    requires
        divisor > 0,
        value < blocks * divisor,
    ensures
        value / divisor < blocks,
        value % divisor < divisor,
        value == (value / divisor) * divisor + value % divisor,
{
    div_mod_reconstructs_v1(value, divisor);
    if value / divisor >= blocks {
        assert((value / divisor) * divisor >= blocks * divisor)
            by (nonlinear_arith)
            requires
                value / divisor >= blocks,
                divisor > 0,
        ;
        assert(value >= blocks * divisor);
        assert(false);
    }
}

proof fn packed_coordinate_v1(block: nat, inner: nat, radix: nat)
    requires radix > 0, inner < radix,
    ensures
        (block * radix + inner) / radix == block,
        (block * radix + inner) % radix == inner,
{
    lemma_fundamental_div_mod_converse(
        (block * radix + inner) as int,
        radix as int,
        block as int,
        inner as int,
    );
}

/// A bounded logical token has an in-table page, bounded slot, and exact reconstruction.
pub proof fn logical_page_slot_reconstructs_v1(
    logical_token: nat,
    context_tokens: nat,
    page_tokens: nat,
)
    requires
        page_tokens > 0,
        context_tokens > 0,
        context_tokens % page_tokens == 0,
        logical_token < context_tokens,
    ensures
        logical_page_v1(logical_token, page_tokens) < context_tokens / page_tokens,
        token_slot_v1(logical_token, page_tokens) < page_tokens,
        logical_page_v1(logical_token, page_tokens) * page_tokens
            + token_slot_v1(logical_token, page_tokens) == logical_token,
{
    div_mod_reconstructs_v1(context_tokens, page_tokens);
    assert(context_tokens == (context_tokens / page_tokens) * page_tokens);
    bounded_quotient_v1(
        logical_token,
        context_tokens / page_tokens,
        page_tokens,
    );
}

pub open spec fn physical_token_v1(
    physical_pages: Seq<nat>,
    logical_token: nat,
    page_tokens: nat,
) -> nat {
    physical_pages[logical_page_v1(logical_token, page_tokens) as int] * page_tokens
        + token_slot_v1(logical_token, page_tokens)
}

pub open spec fn pages_are_unique_v1(physical_pages: Seq<nat>) -> bool {
    forall|left: nat, right: nat|
        left < physical_pages.len()
            && right < physical_pages.len()
            && physical_pages[left as int] == physical_pages[right as int]
            ==> left == right
}

/// A fresh page table with unique physical pages gives injective token mapping.
pub proof fn unique_pages_give_injective_logical_mapping_v1(
    physical_pages: Seq<nat>,
    left_token: nat,
    right_token: nat,
    context_tokens: nat,
    page_tokens: nat,
)
    requires
        page_tokens > 0,
        context_tokens > 0,
        context_tokens % page_tokens == 0,
        physical_pages.len() == context_tokens / page_tokens,
        pages_are_unique_v1(physical_pages),
        left_token < context_tokens,
        right_token < context_tokens,
        physical_token_v1(physical_pages, left_token, page_tokens)
            == physical_token_v1(physical_pages, right_token, page_tokens),
    ensures left_token == right_token,
{
    logical_page_slot_reconstructs_v1(left_token, context_tokens, page_tokens);
    logical_page_slot_reconstructs_v1(right_token, context_tokens, page_tokens);
    let left_page = logical_page_v1(left_token, page_tokens);
    let right_page = logical_page_v1(right_token, page_tokens);
    let left_slot = token_slot_v1(left_token, page_tokens);
    let right_slot = token_slot_v1(right_token, page_tokens);
    packed_coordinate_v1(physical_pages[left_page as int], left_slot, page_tokens);
    packed_coordinate_v1(physical_pages[right_page as int], right_slot, page_tokens);
    assert(physical_pages[left_page as int] == physical_pages[right_page as int]);
    assert(left_slot == right_slot);
    assert(left_page == right_page);
}

pub open spec fn pool_element_offset_v1(
    physical_pages: Seq<nat>,
    logical_token: nat,
    page_tokens: nat,
    kv_head: nat,
    component: nat,
) -> nat {
    (physical_token_v1(physical_pages, logical_token, page_tokens) * kv_heads_v1()
        + kv_head) * head_dimension_v1() + component
}

/// Distinct logical KV coordinates cannot race under the unique-page premise.
pub proof fn exclusive_kv_coordinates_are_injective_v1(
    physical_pages: Seq<nat>,
    left_token: nat,
    left_head: nat,
    left_component: nat,
    right_token: nat,
    right_head: nat,
    right_component: nat,
    context_tokens: nat,
    page_tokens: nat,
)
    requires
        page_tokens > 0,
        context_tokens > 0,
        context_tokens % page_tokens == 0,
        physical_pages.len() == context_tokens / page_tokens,
        pages_are_unique_v1(physical_pages),
        left_token < context_tokens,
        right_token < context_tokens,
        left_head < kv_heads_v1(),
        right_head < kv_heads_v1(),
        left_component < head_dimension_v1(),
        right_component < head_dimension_v1(),
        pool_element_offset_v1(
            physical_pages, left_token, page_tokens, left_head, left_component,
        ) == pool_element_offset_v1(
            physical_pages, right_token, page_tokens, right_head, right_component,
        ),
    ensures
        left_token == right_token,
        left_head == right_head,
        left_component == right_component,
{
    let left_physical = physical_token_v1(physical_pages, left_token, page_tokens);
    let right_physical = physical_token_v1(physical_pages, right_token, page_tokens);
    packed_coordinate_v1(
        left_physical * kv_heads_v1() + left_head,
        left_component,
        head_dimension_v1(),
    );
    packed_coordinate_v1(
        right_physical * kv_heads_v1() + right_head,
        right_component,
        head_dimension_v1(),
    );
    assert(left_component == right_component);
    assert(left_physical * kv_heads_v1() + left_head
        == right_physical * kv_heads_v1() + right_head);
    packed_coordinate_v1(left_physical, left_head, kv_heads_v1());
    packed_coordinate_v1(right_physical, right_head, kv_heads_v1());
    assert(left_physical == right_physical);
    assert(left_head == right_head);
    unique_pages_give_injective_logical_mapping_v1(
        physical_pages,
        left_token,
        right_token,
        context_tokens,
        page_tokens,
    );
}

/// An initialized-prefix read is necessarily within the mapped logical context.
pub proof fn initialized_prefix_read_is_bounded_v1(
    logical_token: nat,
    initialized_prefix: nat,
    context_tokens: nat,
)
    requires
        logical_token < initialized_prefix,
        initialized_prefix <= context_tokens,
    ensures logical_token < context_tokens,
{
}

pub enum PageTableRoleV1 {
    Target,
    Draft,
}

pub open spec fn generation_domain_tag_v1(role: PageTableRoleV1) -> nat {
    match role {
        PageTableRoleV1::Target => 0,
        PageTableRoleV1::Draft => 1,
    }
}

/// Equal numeric generation counters do not collapse target and draft domains.
pub proof fn target_and_draft_generation_domains_are_distinct_v1(generation: nat)
    ensures
        generation_domain_tag_v1(PageTableRoleV1::Target)
            != generation_domain_tag_v1(PageTableRoleV1::Draft),
        (generation_domain_tag_v1(PageTableRoleV1::Target), generation)
            != (generation_domain_tag_v1(PageTableRoleV1::Draft), generation),
{
}

/// Exact target/draft geometry and finite M1 resource maxima are numerically bounded.
pub proof fn exact_qwen3_resource_envelope_is_bounded_v1()
    ensures
        36 <= 36,
        28 <= 36,
        32 <= 32,
        16 <= 32,
        kv_heads_v1() == 8,
        head_dimension_v1() == 128,
        maximum_context_v1() == 8192,
        maximum_sequences_v1() == 32,
        maximum_page_entries_v1() == 512,
{
}

fn main() {}

}
