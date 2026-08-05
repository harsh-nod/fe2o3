use vstd::prelude::*;

include!("../src/vecadd_body.rs");

verus! {

/// Symbolic allocation metadata supplied by the launch environment.
pub struct Allocation {
    pub id: nat,
    pub address_space: nat,
    pub base_address: nat,
    pub byte_length: nat,
    pub address_space_size: nat,
}

/// A half-open byte range retaining its allocation provenance.
pub struct ByteRegion {
    pub allocation_id: nat,
    pub address_space: nat,
    pub byte_offset: nat,
    pub byte_length: nat,
}

pub enum PermissionKind {
    SharedRead,
    ExclusiveWrite,
}

pub struct RegionPermission {
    pub kind: PermissionKind,
    pub region: ByteRegion,
}

/// Ghost initialization state attached to a modeled byte-region permission.
pub struct RegionCapability {
    pub permission: RegionPermission,
    pub initialized: bool,
}

/// A symbolic launch identity. Its nonce is supplied by the proof environment,
/// not constructed or authenticated by the executable kernel body.
pub struct LaunchBrand {
    pub nonce: nat,
}

pub struct IndexSpace1d {
    pub brand: LaunchBrand,
    pub extent: nat,
}

pub struct BrandedThreadIndex1d {
    pub brand: LaunchBrand,
    pub linear: nat,
}

/// Ghost evidence consumed by the source-level vecadd proof. This is not an
/// executable token and cannot authorize runtime loading or launching.
pub struct VecAddSourceEvidence {
    pub space: IndexSpace1d,
    pub thread: BrandedThreadIndex1d,
    pub a_allocation: Allocation,
    pub b_allocation: Allocation,
    pub output_allocation: Allocation,
    pub a_capability: RegionCapability,
    pub b_capability: RegionCapability,
    pub output_capability: RegionCapability,
    pub element_size: nat,
}

pub open spec fn thread_belongs_to_space(
    space: IndexSpace1d,
    thread: BrandedThreadIndex1d,
) -> bool {
    thread.brand == space.brand && thread.linear < space.extent
}

pub open spec fn allocation_is_representable(allocation: Allocation) -> bool {
    allocation.base_address + allocation.byte_length <= allocation.address_space_size
}

pub open spec fn region_is_in_bounds(
    allocation: Allocation,
    region: ByteRegion,
) -> bool {
    allocation.id == region.allocation_id
        && allocation.address_space == region.address_space
        && region.byte_length > 0
        && region.byte_offset + region.byte_length <= allocation.byte_length
        && allocation.base_address + region.byte_offset + region.byte_length
            <= allocation.address_space_size
}

pub open spec fn regions_overlap(left: ByteRegion, right: ByteRegion) -> bool {
    left.allocation_id == right.allocation_id
        && left.address_space == right.address_space
        && left.byte_offset < right.byte_offset + right.byte_length
        && right.byte_offset < left.byte_offset + left.byte_length
}

pub open spec fn permissions_are_compatible(
    left: RegionPermission,
    right: RegionPermission,
) -> bool {
    !regions_overlap(left.region, right.region)
        || (left.kind == PermissionKind::SharedRead
            && right.kind == PermissionKind::SharedRead)
}

pub open spec fn shared_read(region: ByteRegion) -> RegionPermission {
    RegionPermission { kind: PermissionKind::SharedRead, region }
}

pub open spec fn exclusive_write(region: ByteRegion) -> RegionPermission {
    RegionPermission { kind: PermissionKind::ExclusiveWrite, region }
}

pub open spec fn initialized_read_capability(region: ByteRegion) -> RegionCapability {
    RegionCapability {
        permission: shared_read(region),
        initialized: true,
    }
}

pub open spec fn output_write_capability(
    region: ByteRegion,
    initialized: bool,
) -> RegionCapability {
    RegionCapability {
        permission: exclusive_write(region),
        initialized,
    }
}

pub open spec fn capability_can_read(capability: RegionCapability) -> bool {
    capability.permission.kind == PermissionKind::SharedRead && capability.initialized
}

pub open spec fn capability_can_write(capability: RegionCapability) -> bool {
    capability.permission.kind == PermissionKind::ExclusiveWrite
}

pub open spec fn capability_after_write(capability: RegionCapability) -> RegionCapability {
    RegionCapability {
        permission: capability.permission,
        initialized: true,
    }
}

pub open spec fn vecadd_source_evidence_is_valid(
    evidence: VecAddSourceEvidence,
    domain_len: nat,
    thread: nat,
) -> bool {
    thread_belongs_to_space(evidence.space, evidence.thread)
        && evidence.space.extent == domain_len
        && evidence.thread.linear == thread
        && evidence.element_size > 0
        && allocation_is_representable(evidence.a_allocation)
        && allocation_is_representable(evidence.b_allocation)
        && allocation_is_representable(evidence.output_allocation)
        && evidence.a_allocation.byte_length == domain_len * evidence.element_size
        && evidence.b_allocation.byte_length == domain_len * evidence.element_size
        && evidence.output_allocation.byte_length == domain_len * evidence.element_size
        && evidence.output_allocation.id != evidence.a_allocation.id
        && evidence.output_allocation.id != evidence.b_allocation.id
        && evidence.a_capability == initialized_read_capability(element_region(
            evidence.a_allocation,
            thread,
            evidence.element_size,
        ))
        && evidence.b_capability == initialized_read_capability(element_region(
            evidence.b_allocation,
            thread,
            evidence.element_size,
        ))
        && evidence.output_capability.permission == exclusive_write(element_region(
            evidence.output_allocation,
            output_index(thread),
            evidence.element_size,
        ))
}

/// Target-neutral model of the identity write mapping used by the Rust example.
pub open spec fn output_index(thread: nat) -> nat {
    thread
}

pub open spec fn element_region(
    allocation: Allocation,
    index: nat,
    element_size: nat,
) -> ByteRegion {
    ByteRegion {
        allocation_id: allocation.id,
        address_space: allocation.address_space,
        byte_offset: index * element_size,
        byte_length: element_size,
    }
}

pub open spec fn element_byte_address(
    allocation: Allocation,
    index: nat,
    element_size: nat,
) -> nat {
    allocation.base_address + index * element_size
}

pub open spec fn element_byte_end(
    allocation: Allocation,
    index: nat,
    element_size: nat,
) -> nat {
    element_byte_address(allocation, index, element_size) + element_size
}

pub open spec fn vecadd_value(a: Seq<int>, b: Seq<int>, thread: nat) -> int
    recommends
        thread < a.len(),
        thread < b.len(),
{
    a[thread as int] + b[thread as int]
}

pub open spec fn vecadd_write(
    old_output: Seq<int>,
    a: Seq<int>,
    b: Seq<int>,
    thread: nat,
) -> Seq<int>
    recommends
        thread < old_output.len(),
        thread < a.len(),
        thread < b.len(),
{
    old_output.update(output_index(thread) as int, vecadd_value(a, b, thread))
}

pub open spec fn vecadd_value_u32(a: Seq<u32>, b: Seq<u32>, thread: nat) -> u32
    recommends
        thread < a.len(),
        thread < b.len(),
        a[thread as int] as nat + b[thread as int] as nat <= u32::MAX as nat,
{
    (a[thread as int] as nat + b[thread as int] as nat) as u32
}

pub open spec fn vecadd_postcondition_u32(
    output: Seq<u32>,
    a: Seq<u32>,
    b: Seq<u32>,
) -> bool {
    output.len() == a.len()
        && output.len() == b.len()
        && forall |index: nat| index < output.len() ==>
            output[index as int] == vecadd_value_u32(a, b, index)
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ModelLaunchDomain1d {
    pub length: usize,
}

impl ModelLaunchDomain1d {
    pub open spec fn spec_len(&self) -> usize {
        self.length
    }

    #[verifier::when_used_as_spec(spec_len)]
    pub fn len(&self) -> (length: usize)
        ensures
            length == self.length,
    {
        self.length
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ModelThreadInDomain1d {
    pub launch: ModelLaunchDomain1d,
    pub linear: usize,
}

impl ModelThreadInDomain1d {
    pub open spec fn spec_domain(&self) -> ModelLaunchDomain1d {
        self.launch
    }

    #[verifier::when_used_as_spec(spec_domain)]
    pub fn domain(&self) -> (launch: ModelLaunchDomain1d)
        ensures
            launch == self.launch,
    {
        self.launch
    }
}

#[derive(Copy, Clone)]
pub struct ModelLinearIndex {
    pub linear: usize,
}

impl ModelLinearIndex {
    pub open spec fn spec_value(&self) -> usize {
        self.linear
    }

    #[verifier::when_used_as_spec(spec_value)]
    pub fn value(&self) -> (linear: usize)
        ensures
            linear == self.linear,
    {
        self.linear
    }
}

#[derive(Copy, Clone)]
pub struct ModelIdentityWriteIndex {
    pub linear: usize,
}

impl ModelIdentityWriteIndex {
    pub fn new(
        thread: ModelThreadInDomain1d,
        output_len: usize,
    ) -> (write: Option<Self>)
        ensures
            match write {
                Some(write) => write.linear == thread.linear && thread.linear < output_len,
                None => thread.linear >= output_len,
            },
    {
        if thread.linear < output_len {
            Some(Self { linear: thread.linear })
        } else {
            None
        }
    }

    pub open spec fn spec_index(&self) -> ModelLinearIndex {
        ModelLinearIndex { linear: self.linear }
    }

    #[verifier::when_used_as_spec(spec_index)]
    pub fn index(&self) -> (index: ModelLinearIndex)
        ensures
            index.linear == self.linear,
    {
        ModelLinearIndex { linear: self.linear }
    }
}

#[derive(PartialEq, Eq)]
pub enum VecAddError {
    DomainLengthMismatch,
    ArithmeticOverflow,
}

/// Verus expands the same executable body that ordinary rustc expands in
/// `src/lib.rs`; the preconditions select the successful kernel path. This is
/// source-model evidence, not machine-code refinement or runtime authority.
pub fn same_source_vecadd_thread(
    domain: ModelLaunchDomain1d,
    thread: ModelThreadInDomain1d,
    a: &[u32],
    b: &[u32],
    output: &mut [u32],
    Ghost(evidence): Ghost<VecAddSourceEvidence>,
) -> (result: Result<(), VecAddError>)
    requires
        thread.launch == domain,
        thread.linear < domain.length,
        a@.len() == domain.length,
        b@.len() == domain.length,
        old(output)@.len() == domain.length,
        a@[thread.linear as int] as nat + b@[thread.linear as int] as nat <= u32::MAX as nat,
        vecadd_source_evidence_is_valid(
            evidence,
            domain.length as nat,
            thread.linear as nat,
        ),
    ensures
        result.is_ok(),
        final(output)@ == old(output)@.update(
            thread.linear as int,
            vecadd_value_u32(a@, b@, thread.linear as nat),
        ),
        capability_after_write(evidence.output_capability).initialized,
        region_is_in_bounds(
            evidence.a_allocation,
            evidence.a_capability.permission.region,
        ),
        region_is_in_bounds(
            evidence.b_allocation,
            evidence.b_capability.permission.region,
        ),
        region_is_in_bounds(
            evidence.output_allocation,
            evidence.output_capability.permission.region,
        ),
        capability_can_read(evidence.a_capability),
        capability_can_read(evidence.b_capability),
        capability_can_write(evidence.output_capability),
        permissions_are_compatible(
            evidence.a_capability.permission,
            evidence.output_capability.permission,
        ),
        permissions_are_compatible(
            evidence.b_capability.permission,
            evidence.output_capability.permission,
        ),
{
    assert(thread.domain().len() == domain.len());
    assert(domain.len() == domain.length);
    assert(a.len() == domain.length);
    assert(b.len() == domain.length);
    assert(output.len() == domain.length);
    proof {
        same_source_evidence_has_valid_permissions(
            evidence,
            domain.length as nat,
            thread.linear as nat,
        );
    }
    vecadd_thread_body!(
        domain,
        thread,
        a,
        b,
        output,
        ModelIdentityWriteIndex,
        VecAddError::DomainLengthMismatch,
        VecAddError::ArithmeticOverflow
    )
}

pub proof fn element_region_is_in_bounds_and_address_representable(
    allocation: Allocation,
    element_count: nat,
    index: nat,
    element_size: nat,
)
    requires
        allocation_is_representable(allocation),
        allocation.byte_length == element_count * element_size,
        element_size > 0,
        index < element_count,
    ensures
        region_is_in_bounds(allocation, element_region(allocation, index, element_size)),
        element_byte_address(allocation, index, element_size)
            < element_byte_end(allocation, index, element_size),
        element_byte_end(allocation, index, element_size) <= allocation.address_space_size,
{
    assert(index + 1 <= element_count);
    assert((index + 1) * element_size <= element_count * element_size) by (nonlinear_arith)
        requires
            index + 1 <= element_count,
            element_size > 0,
    ;
    assert(index * element_size + element_size == (index + 1) * element_size)
        by (nonlinear_arith);
}

/// Connects a branded source thread to initialized input reads and its one
/// exclusive output region. The output may start in either initialization
/// state; the successful write establishes initialization for that element.
pub proof fn same_source_evidence_has_valid_permissions(
    evidence: VecAddSourceEvidence,
    domain_len: nat,
    thread: nat,
)
    requires
        vecadd_source_evidence_is_valid(evidence, domain_len, thread),
    ensures
        region_is_in_bounds(
            evidence.a_allocation,
            evidence.a_capability.permission.region,
        ),
        region_is_in_bounds(
            evidence.b_allocation,
            evidence.b_capability.permission.region,
        ),
        region_is_in_bounds(
            evidence.output_allocation,
            evidence.output_capability.permission.region,
        ),
        capability_can_read(evidence.a_capability),
        capability_can_read(evidence.b_capability),
        capability_can_write(evidence.output_capability),
        capability_after_write(evidence.output_capability).initialized,
        permissions_are_compatible(
            evidence.a_capability.permission,
            evidence.output_capability.permission,
        ),
        permissions_are_compatible(
            evidence.b_capability.permission,
            evidence.output_capability.permission,
        ),
{
    assert(thread < domain_len);
    element_region_is_in_bounds_and_address_representable(
        evidence.a_allocation,
        domain_len,
        thread,
        evidence.element_size,
    );
    element_region_is_in_bounds_and_address_representable(
        evidence.b_allocation,
        domain_len,
        thread,
        evidence.element_size,
    );
    element_region_is_in_bounds_and_address_representable(
        evidence.output_allocation,
        domain_len,
        output_index(thread),
        evidence.element_size,
    );
}

pub proof fn per_thread_vecadd_has_valid_region_permissions(
    a: Seq<int>,
    b: Seq<int>,
    output: Seq<int>,
    a_allocation: Allocation,
    b_allocation: Allocation,
    output_allocation: Allocation,
    thread: nat,
    element_size: nat,
)
    requires
        a.len() == b.len(),
        a.len() == output.len(),
        thread < output.len(),
        element_size > 0,
        allocation_is_representable(a_allocation),
        allocation_is_representable(b_allocation),
        allocation_is_representable(output_allocation),
        a_allocation.byte_length == a.len() * element_size,
        b_allocation.byte_length == b.len() * element_size,
        output_allocation.byte_length == output.len() * element_size,
        output_allocation.id != a_allocation.id,
        output_allocation.id != b_allocation.id,
    ensures
        region_is_in_bounds(
            a_allocation,
            element_region(a_allocation, thread, element_size),
        ),
        region_is_in_bounds(
            b_allocation,
            element_region(b_allocation, thread, element_size),
        ),
        region_is_in_bounds(
            output_allocation,
            element_region(output_allocation, output_index(thread), element_size),
        ),
        permissions_are_compatible(
            shared_read(element_region(a_allocation, thread, element_size)),
            shared_read(element_region(b_allocation, thread, element_size)),
        ),
        permissions_are_compatible(
            shared_read(element_region(a_allocation, thread, element_size)),
            exclusive_write(element_region(output_allocation, output_index(thread), element_size)),
        ),
        permissions_are_compatible(
            shared_read(element_region(b_allocation, thread, element_size)),
            exclusive_write(element_region(output_allocation, output_index(thread), element_size)),
        ),
        vecadd_value(a, b, thread) == a[thread as int] + b[thread as int],
{
    element_region_is_in_bounds_and_address_representable(
        a_allocation,
        a.len(),
        thread,
        element_size,
    );
    element_region_is_in_bounds_and_address_representable(
        b_allocation,
        b.len(),
        thread,
        element_size,
    );
    element_region_is_in_bounds_and_address_representable(
        output_allocation,
        output.len(),
        thread,
        element_size,
    );
}

/// Shared reads are compatible even when both inputs name the same bytes.
pub proof fn shared_input_reads_may_alias(region: ByteRegion)
    ensures
        permissions_are_compatible(
            shared_read(region),
            shared_read(region),
        ),
{
}

pub proof fn disjoint_write_and_read_regions_are_compatible(
    write_region: ByteRegion,
    read_region: ByteRegion,
)
    requires
        !regions_overlap(write_region, read_region),
    ensures
        permissions_are_compatible(
            exclusive_write(write_region),
            shared_read(read_region),
        ),
{
}

/// Unequal identity indices yield disjoint output byte ranges.
pub proof fn distinct_threads_have_disjoint_output_regions(
    output_allocation: Allocation,
    left: nat,
    right: nat,
    thread_count: nat,
    element_size: nat,
)
    requires
        left < thread_count,
        right < thread_count,
        left != right,
        element_size > 0,
    ensures
        !regions_overlap(
            element_region(output_allocation, output_index(left), element_size),
            element_region(output_allocation, output_index(right), element_size),
        ),
        permissions_are_compatible(
            exclusive_write(element_region(
                output_allocation,
                output_index(left),
                element_size,
            )),
            exclusive_write(element_region(
                output_allocation,
                output_index(right),
                element_size,
            )),
        ),
{
    if left < right {
        assert((left + 1) * element_size <= right * element_size) by (nonlinear_arith)
            requires
                left < right,
                element_size > 0,
        ;
        assert(left * element_size + element_size == (left + 1) * element_size)
            by (nonlinear_arith);
    } else {
        assert(right < left);
        assert((right + 1) * element_size <= left * element_size) by (nonlinear_arith)
            requires
                right < left,
                element_size > 0,
        ;
        assert(right * element_size + element_size == (right + 1) * element_size)
            by (nonlinear_arith);
    }
}

/// Launch branding prevents an index witness from being silently reused in a
/// different index space, while identity indexing makes distinct writes
/// injective inside one space.
pub proof fn distinct_branded_threads_have_disjoint_output_regions(
    space: IndexSpace1d,
    left: BrandedThreadIndex1d,
    right: BrandedThreadIndex1d,
    output_allocation: Allocation,
    element_size: nat,
)
    requires
        thread_belongs_to_space(space, left),
        thread_belongs_to_space(space, right),
        left.linear != right.linear,
        element_size > 0,
    ensures
        output_index(left.linear) != output_index(right.linear),
        !regions_overlap(
            element_region(output_allocation, output_index(left.linear), element_size),
            element_region(output_allocation, output_index(right.linear), element_size),
        ),
        permissions_are_compatible(
            exclusive_write(element_region(
                output_allocation,
                output_index(left.linear),
                element_size,
            )),
            exclusive_write(element_region(
                output_allocation,
                output_index(right.linear),
                element_size,
            )),
        ),
{
    distinct_threads_have_disjoint_output_regions(
        output_allocation,
        left.linear,
        right.linear,
        space.extent,
        element_size,
    );
}

/// Composition theorem for a complete branded launch. Every active slot must
/// carry its matching branded identity and the value established by the shared
/// executable body.
pub proof fn completed_branded_vecadd_is_functionally_correct(
    space: IndexSpace1d,
    observed_threads: Seq<BrandedThreadIndex1d>,
    output: Seq<u32>,
    a: Seq<u32>,
    b: Seq<u32>,
)
    requires
        output.len() == space.extent,
        a.len() == space.extent,
        b.len() == space.extent,
        observed_threads.len() == space.extent,
        forall |slot: nat| slot < space.extent ==>
            thread_belongs_to_space(space, observed_threads[slot as int])
                && observed_threads[slot as int].linear == slot,
        forall |slot: nat| slot < space.extent ==>
            a[slot as int] as nat + b[slot as int] as nat <= u32::MAX as nat,
        forall |slot: nat| slot < space.extent ==>
            output[observed_threads[slot as int].linear as int]
                == vecadd_value_u32(a, b, observed_threads[slot as int].linear),
    ensures
        vecadd_postcondition_u32(output, a, b),
{
    assert forall |index: nat| index < output.len() implies
        output[index as int] == vecadd_value_u32(a, b, index) by {
        assert(index < space.extent);
        assert(observed_threads[index as int].linear == index);
        assert(output[observed_threads[index as int].linear as int]
            == vecadd_value_u32(a, b, observed_threads[index as int].linear));
    }
}

pub proof fn vecadd_changes_only_the_owned_output(
    old_output: Seq<int>,
    a: Seq<int>,
    b: Seq<int>,
    output_allocation: Allocation,
    thread: nat,
    other: nat,
    element_size: nat,
)
    requires
        old_output.len() == a.len(),
        old_output.len() == b.len(),
        thread < old_output.len(),
        other < old_output.len(),
        other != output_index(thread),
        element_size > 0,
    ensures
        vecadd_write(old_output, a, b, thread)[other as int] == old_output[other as int],
        !regions_overlap(
            element_region(output_allocation, output_index(thread), element_size),
            element_region(output_allocation, other, element_size),
        ),
{
    distinct_threads_have_disjoint_output_regions(
        output_allocation,
        thread,
        other,
        old_output.len(),
        element_size,
    );
}

/// A write also frames every region from another symbolic allocation.
pub proof fn output_write_frames_other_allocations(
    output_allocation: Allocation,
    framed_allocation: Allocation,
    output_index: nat,
    framed_index: nat,
    element_size: nat,
)
    requires
        output_allocation.id != framed_allocation.id,
        element_size > 0,
    ensures
        !regions_overlap(
            element_region(output_allocation, output_index, element_size),
            element_region(framed_allocation, framed_index, element_size),
        ),
        permissions_are_compatible(
            exclusive_write(element_region(output_allocation, output_index, element_size)),
            shared_read(element_region(framed_allocation, framed_index, element_size)),
        ),
{
}

/// Trusted hardware/backend boundary. The backend must refine this contract,
/// and launch composition must separately guarantee distinct IDs for distinct
/// active threads.
#[verifier::external_body]
pub fn hardware_thread_id(thread_count: usize) -> (thread: usize)
    requires
        thread_count > 0,
    ensures
        thread < thread_count,
{
    unimplemented!()
}

} // verus!
