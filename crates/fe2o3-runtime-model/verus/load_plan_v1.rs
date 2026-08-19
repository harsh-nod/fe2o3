use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum LoadPermissionV1 {
    ReadOnly,
    ReadExecute,
    ReadWrite,
}

pub struct LoadSegmentV1 {
    pub file_start: nat,
    pub file_size: nat,
    pub memory_start: nat,
    pub memory_size: nat,
    pub mapping_start: nat,
    pub mapping_size: nat,
    pub permission: LoadPermissionV1,
}

pub struct LoadPlanV1 {
    pub first: LoadSegmentV1,
    pub second: LoadSegmentV1,
    pub third: LoadSegmentV1,
    pub image_start: nat,
    pub image_end: nat,
}

pub struct LoadDescriptorV1 {
    pub file_start: nat,
    pub file_size: nat,
    pub memory_start: nat,
    pub memory_size: nat,
    pub required_permission: LoadPermissionV1,
}

pub open spec fn page_size_v1() -> nat {
    4096
}

pub open spec fn max_u64_v1() -> nat {
    0xffffffffffffffff
}

pub open spec fn max_image_span_v1() -> nat {
    64 * 1024 * 1024
}

pub open spec fn page_align_down_v1(value: nat) -> nat {
    (value - value % page_size_v1()) as nat
}

pub open spec fn page_align_up_v1(value: nat) -> nat {
    if value % page_size_v1() == 0 {
        value
    } else {
        (value + page_size_v1() - value % page_size_v1()) as nat
    }
}

pub open spec fn checked_u64_range_v1(start: nat, size: nat) -> bool {
    start <= max_u64_v1() && size <= max_u64_v1() - start
}

pub open spec fn exact_page_rounding_v1(segment: LoadSegmentV1) -> bool {
    &&& segment.mapping_start == page_align_down_v1(segment.memory_start)
    &&& segment.mapping_size == page_align_up_v1(
        (segment.memory_start - segment.mapping_start + segment.memory_size) as nat,
    )
}

pub open spec fn checked_segment_v1(segment: LoadSegmentV1) -> bool {
    &&& segment.file_size > 0
    &&& segment.memory_size > 0
    &&& segment.file_size <= segment.memory_size
    &&& checked_u64_range_v1(segment.file_start, segment.file_size)
    &&& checked_u64_range_v1(segment.memory_start, segment.memory_size)
    &&& exact_page_rounding_v1(segment)
    &&& segment.mapping_size > 0
    &&& checked_u64_range_v1(segment.mapping_start, segment.mapping_size)
    &&& segment.mapping_start <= segment.memory_start
    &&& segment.memory_start + segment.memory_size
        <= segment.mapping_start + segment.mapping_size
}

pub open spec fn ranges_disjoint_v1(
    left_start: nat,
    left_size: nat,
    right_start: nat,
    right_size: nat,
) -> bool {
    left_start + left_size <= right_start
        || right_start + right_size <= left_start
}

pub open spec fn segment_file_disjoint_v1(
    left: LoadSegmentV1,
    right: LoadSegmentV1,
) -> bool {
    ranges_disjoint_v1(
        left.file_start,
        left.file_size,
        right.file_start,
        right.file_size,
    )
}

pub open spec fn segment_memory_disjoint_v1(
    left: LoadSegmentV1,
    right: LoadSegmentV1,
) -> bool {
    ranges_disjoint_v1(
        left.memory_start,
        left.memory_size,
        right.memory_start,
        right.memory_size,
    )
}

pub open spec fn segment_mapping_disjoint_v1(
    left: LoadSegmentV1,
    right: LoadSegmentV1,
) -> bool {
    ranges_disjoint_v1(
        left.mapping_start,
        left.mapping_size,
        right.mapping_start,
        right.mapping_size,
    )
}

pub open spec fn exact_permission_profile_v1(plan: LoadPlanV1) -> bool {
    &&& plan.first.permission != plan.second.permission
    &&& plan.first.permission != plan.third.permission
    &&& plan.second.permission != plan.third.permission
    &&& (plan.first.permission == LoadPermissionV1::ReadOnly
        || plan.second.permission == LoadPermissionV1::ReadOnly
        || plan.third.permission == LoadPermissionV1::ReadOnly)
    &&& (plan.first.permission == LoadPermissionV1::ReadExecute
        || plan.second.permission == LoadPermissionV1::ReadExecute
        || plan.third.permission == LoadPermissionV1::ReadExecute)
    &&& (plan.first.permission == LoadPermissionV1::ReadWrite
        || plan.second.permission == LoadPermissionV1::ReadWrite
        || plan.third.permission == LoadPermissionV1::ReadWrite)
}

pub open spec fn canonical_load_plan_v1(plan: LoadPlanV1) -> bool {
    &&& checked_segment_v1(plan.first)
    &&& checked_segment_v1(plan.second)
    &&& checked_segment_v1(plan.third)
    &&& plan.first.memory_start < plan.second.memory_start
    &&& plan.second.memory_start < plan.third.memory_start
    &&& plan.first.mapping_start < plan.second.mapping_start
    &&& plan.second.mapping_start < plan.third.mapping_start
    &&& exact_permission_profile_v1(plan)
    &&& segment_file_disjoint_v1(plan.first, plan.second)
    &&& segment_file_disjoint_v1(plan.first, plan.third)
    &&& segment_file_disjoint_v1(plan.second, plan.third)
    &&& segment_memory_disjoint_v1(plan.first, plan.second)
    &&& segment_memory_disjoint_v1(plan.first, plan.third)
    &&& segment_memory_disjoint_v1(plan.second, plan.third)
    &&& segment_mapping_disjoint_v1(plan.first, plan.second)
    &&& segment_mapping_disjoint_v1(plan.first, plan.third)
    &&& segment_mapping_disjoint_v1(plan.second, plan.third)
    &&& plan.image_start == plan.first.mapping_start
    &&& plan.image_end == plan.third.mapping_start + plan.third.mapping_size
    &&& plan.image_start <= plan.image_end <= max_u64_v1()
    &&& plan.image_end - plan.image_start <= max_image_span_v1()
}

pub open spec fn descriptor_binds_segment_v1(
    descriptor: LoadDescriptorV1,
    segment: LoadSegmentV1,
) -> bool {
    &&& descriptor.required_permission == segment.permission
    &&& descriptor.file_size > 0
    &&& descriptor.memory_size > 0
    &&& checked_u64_range_v1(descriptor.file_start, descriptor.file_size)
    &&& checked_u64_range_v1(descriptor.memory_start, descriptor.memory_size)
    &&& segment.file_start <= descriptor.file_start
    &&& descriptor.file_start + descriptor.file_size
        <= segment.file_start + segment.file_size
    &&& segment.memory_start <= descriptor.memory_start
    &&& descriptor.memory_start + descriptor.memory_size
        <= segment.memory_start + segment.memory_size
    &&& descriptor.file_start - segment.file_start
        == descriptor.memory_start - segment.memory_start
}

pub open spec fn descriptor_binding_count_v1(
    plan: LoadPlanV1,
    descriptor: LoadDescriptorV1,
) -> nat {
    (if descriptor_binds_segment_v1(descriptor, plan.first) { 1nat } else { 0nat })
        + (if descriptor_binds_segment_v1(descriptor, plan.second) { 1nat } else { 0nat })
        + (if descriptor_binds_segment_v1(descriptor, plan.third) { 1nat } else { 0nat })
}

pub proof fn checked_page_rounding_and_image_span_v1(plan: LoadPlanV1)
    requires
        canonical_load_plan_v1(plan),
    ensures
        exact_page_rounding_v1(plan.first),
        exact_page_rounding_v1(plan.second),
        exact_page_rounding_v1(plan.third),
        plan.first.mapping_start % page_size_v1() == 0,
        plan.second.mapping_start % page_size_v1() == 0,
        plan.third.mapping_start % page_size_v1() == 0,
        plan.first.mapping_size % page_size_v1() == 0,
        plan.second.mapping_size % page_size_v1() == 0,
        plan.third.mapping_size % page_size_v1() == 0,
        checked_u64_range_v1(plan.first.file_start, plan.first.file_size),
        checked_u64_range_v1(plan.second.file_start, plan.second.file_size),
        checked_u64_range_v1(plan.third.file_start, plan.third.file_size),
        checked_u64_range_v1(plan.first.memory_start, plan.first.memory_size),
        checked_u64_range_v1(plan.second.memory_start, plan.second.memory_size),
        checked_u64_range_v1(plan.third.memory_start, plan.third.memory_size),
        checked_u64_range_v1(plan.first.mapping_start, plan.first.mapping_size),
        checked_u64_range_v1(plan.second.mapping_start, plan.second.mapping_size),
        checked_u64_range_v1(plan.third.mapping_start, plan.third.mapping_size),
        plan.first.memory_start + plan.first.memory_size
            <= plan.first.mapping_start + plan.first.mapping_size,
        plan.second.memory_start + plan.second.memory_size
            <= plan.second.mapping_start + plan.second.mapping_size,
        plan.third.memory_start + plan.third.memory_size
            <= plan.third.mapping_start + plan.third.mapping_size,
        plan.image_start == plan.first.mapping_start,
        plan.image_end == plan.third.mapping_start + plan.third.mapping_size,
        plan.image_start <= plan.image_end <= max_u64_v1(),
        plan.image_end - plan.image_start <= max_image_span_v1(),
{
}

pub proof fn canonical_segments_are_pairwise_disjoint_v1(plan: LoadPlanV1)
    requires
        canonical_load_plan_v1(plan),
    ensures
        plan.first.memory_start < plan.second.memory_start
            < plan.third.memory_start,
        plan.first.mapping_start < plan.second.mapping_start
            < plan.third.mapping_start,
        exact_permission_profile_v1(plan),
        segment_file_disjoint_v1(plan.first, plan.second),
        segment_file_disjoint_v1(plan.first, plan.third),
        segment_file_disjoint_v1(plan.second, plan.third),
        segment_memory_disjoint_v1(plan.first, plan.second),
        segment_memory_disjoint_v1(plan.first, plan.third),
        segment_memory_disjoint_v1(plan.second, plan.third),
        segment_mapping_disjoint_v1(plan.first, plan.second),
        segment_mapping_disjoint_v1(plan.first, plan.third),
        segment_mapping_disjoint_v1(plan.second, plan.third),
{
}

pub proof fn descriptor_equal_delta_binds_exactly_one_load_v1(
    plan: LoadPlanV1,
    descriptor: LoadDescriptorV1,
    selected: nat,
)
    requires
        canonical_load_plan_v1(plan),
        selected < 3,
        selected == 0 ==> descriptor_binds_segment_v1(descriptor, plan.first),
        selected == 1 ==> descriptor_binds_segment_v1(descriptor, plan.second),
        selected == 2 ==> descriptor_binds_segment_v1(descriptor, plan.third),
    ensures
        descriptor_binding_count_v1(plan, descriptor) == 1,
        selected == 0 ==> {
            &&& descriptor.file_start - plan.first.file_start
                == descriptor.memory_start - plan.first.memory_start
            &&& descriptor.file_start + descriptor.file_size
                <= plan.first.file_start + plan.first.file_size
            &&& descriptor.memory_start + descriptor.memory_size
                <= plan.first.memory_start + plan.first.memory_size
        },
        selected == 1 ==> {
            &&& descriptor.file_start - plan.second.file_start
                == descriptor.memory_start - plan.second.memory_start
            &&& descriptor.file_start + descriptor.file_size
                <= plan.second.file_start + plan.second.file_size
            &&& descriptor.memory_start + descriptor.memory_size
                <= plan.second.memory_start + plan.second.memory_size
        },
        selected == 2 ==> {
            &&& descriptor.file_start - plan.third.file_start
                == descriptor.memory_start - plan.third.memory_start
            &&& descriptor.file_start + descriptor.file_size
                <= plan.third.file_start + plan.third.file_size
            &&& descriptor.memory_start + descriptor.memory_size
                <= plan.third.memory_start + plan.third.memory_size
        },
{
}

} // verus!
