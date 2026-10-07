// The unchanged arena include also defines the unused resource_domain_facts_body_v1.
#![allow(unused_macros)]
// Actual post-lock field observation, not an Arc or Mutex refinement.
// Existing node/path and record bodies are verified again in this root.
// Scalar/slice parameters are the production caller's locked-field projections.
// Pinned vstd equality, array/slice specifications and derive lowering remain
// compiler trust boundaries. No freshness, custody, conservation or native
// authority is established, and no frozen credit_ok result is supplied.
include!("retained_credit_record_v1.rs");
use vstd::prelude::verus as resource_domain_arena_declarations_v1;

verus! { type ResourceVectorV1 = R67ResourceVectorV1; }
include!("../../fe2o3-resource-accounting/src/domain/arena_declarations.rs");
include!("../../fe2o3-resource-accounting/src/domain/arena_bodies.rs");
include!("../../fe2o3-resource-accounting/src/domain/retained_observation_body.rs");

verus! {

impl vstd::std_specs::cmp::PartialEqSpecImpl for Key {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

#[derive(Clone, Copy)]
struct DomainRecord {
    leaf: Key,
    credit: Record,
}

spec fn live(nodes: Seq<Option<Node>>, key: Key) -> bool {
    key.slot < nodes.len() && nodes[key.slot as int].is_some()
        && nodes[key.slot as int]->Some_0.key == key
}

spec fn at(nodes: Seq<Option<Node>>, key: Key) -> Node {
    nodes[key.slot as int]->Some_0
}

#[verifier::opaque]
spec fn walk(nodes: Seq<Option<Node>>, next: Option<Key>, budget: int) -> Option<Seq<Key>>
    decreases budget,
{
    match next {
        None => Some(Seq::empty()),
        Some(key) => if budget <= 0 || !live(nodes, key) { None }
            else { match walk(nodes, at(nodes, key).parent, budget - 1) {
                None => None,
                Some(tail) => Some(seq![key] + tail),
            } },
    }
}

spec fn linked(nodes: Seq<Option<Node>>, path: Seq<Key>) -> bool {
    &&& path.len() > 0
    &&& forall|i: int| 0 <= i < path.len() ==> live(nodes, path[i])
    &&& forall|i: int| 0 <= i < path.len() - 1 ==> at(nodes, path[i]).parent == Some(path[i + 1])
    &&& at(nodes, path.last()).parent.is_none()
}

proof fn walk_shape(nodes: Seq<Option<Node>>, next: Option<Key>, budget: int)
    requires budget >= 0,
    ensures match walk(nodes, next, budget) {
        None => true,
        Some(path) => {
            &&& path.len() <= budget
            &&& match next { None => path.len() == 0,
                Some(key) => path.len() > 0 && path[0] == key && linked(nodes, path) }
        },
    },
    decreases budget,
{
    reveal(walk);
    if let Some(key) = next {
        if budget > 0 && live(nodes, key) {
            walk_shape(nodes, at(nodes, key).parent, budget - 1);
            if let Some(tail) = walk(nodes, at(nodes, key).parent, budget - 1) {
                let path = seq![key] + tail;
                assert forall|i: int| 0 <= i < path.len() implies live(nodes, path[i]) by {
                    if i > 0 { assert(path[i] == tail[i - 1]); }
                }
                assert forall|i: int| 0 <= i < path.len() - 1 implies
                    at(nodes, path[i]).parent == Some(path[i + 1]) by {
                    if i > 0 { assert(path[i] == tail[i - 1]); assert(path[i + 1] == tail[i]); }
                }
                if tail.len() > 0 { assert(path.last() == tail.last()); }
                else { assert(path.last() == key); }
            }
        }
    }
}

proof fn linked_no_duplicate(nodes: Seq<Option<Node>>, path: Seq<Key>, i: int, j: int)
    requires linked(nodes, path), 0 <= i < j < path.len(), path[i] == path[j],
    ensures false,
    decreases path.len() - j,
{
    if j == path.len() - 1 {
        assert(at(nodes, path[i]).parent == Some(path[i + 1]));
        assert(at(nodes, path[j]).parent.is_none());
    } else {
        assert(at(nodes, path[i]).parent == Some(path[i + 1]));
        assert(at(nodes, path[j]).parent == Some(path[j + 1]));
        linked_no_duplicate(nodes, path, i + 1, j + 1);
    }
}

proof fn linked_slots_unique(nodes: Seq<Option<Node>>, path: Seq<Key>)
    requires linked(nodes, path),
    ensures forall|i: int, j: int| 0 <= i < j < path.len() ==> path[i].slot != path[j].slot,
{
    assert forall|i: int, j: int| 0 <= i < j < path.len() implies path[i].slot != path[j].slot by {
        if path[i].slot == path[j].slot {
            assert(at(nodes, path[i]).key == path[i]);
            assert(at(nodes, path[j]).key == path[j]);
            linked_no_duplicate(nodes, path, i, j);
        }
    }
}

spec fn accepted(nodes: Seq<Option<Node>>, profile: usize, leaf: Key) -> bool {
    (profile == 3 || profile == 4) && match walk(nodes, Some(leaf), profile as int) {
        Some(path) => path.len() > 0 && path.last() == ROOT,
        None => false,
    }
}

fn domain_node_v1(nodes: &[Option<Node>], key: Key) -> (result: Option<&Node>)
    ensures
        result.is_some() == live(nodes@, key),
        match result { Some(node) => *node == at(nodes@, key), None => true },
{
    resource_domain_node_body_v1!(nodes, key)
}

spec fn exact_path_result(nodes: Seq<Option<Node>>, profile: usize, leaf: Key,
    result: Option<([Key; 4], usize)>) -> bool {
    &&& result.is_some() == accepted(nodes, profile, leaf)
    &&& match result {
            None => true,
            Some((path, depth)) => {
                &&& 0 < depth <= profile <= 4
                &&& path@[depth as int - 1] == ROOT
                &&& walk(nodes, Some(leaf), profile as int) == Some(path@.take(depth as int))
                &&& forall|i: int| depth <= i < 4 ==> path@[i] == ROOT
            },
        }
}

proof fn exact_path_implies_properties(nodes: Seq<Option<Node>>, profile: usize, leaf: Key,
    result: Option<([Key; 4], usize)>)
    requires exact_path_result(nodes, profile, leaf, result),
    ensures match result { None => true, Some((path, depth)) => {
        &&& path@[0] == leaf
        &&& linked(nodes, path@.take(depth as int))
        &&& forall|i: int, j: int| 0 <= i < j < depth ==> path@[i].slot != path@[j].slot
    } },
{
    if let Some((path, depth)) = result {
        walk_shape(nodes, Some(leaf), profile as int);
        let selected = path@.take(depth as int);
        linked_slots_unique(nodes, selected);
        assert forall|i: int, j: int| 0 <= i < j < depth implies path@[i].slot != path@[j].slot by {
            assert(selected[i] == path@[i]);
            assert(selected[j] == path@[j]);
        }
    }
}

fn domain_path_v1(nodes: &[Option<Node>], profile: usize, leaf: Key)
    -> (result: Option<([Key; MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1], usize)>)
    ensures exact_path_result(nodes@, profile, leaf, result),
{
    resource_domain_path_extract_body_v1!(verus_exec_expr, nodes, profile, leaf, path, next, depth, [
        invariant
            profile == 3 || profile == 4,
            0 <= depth <= profile <= 4,
            depth == 0 ==> next == Some(leaf),
            next.is_none() ==> walk(nodes@, Some(leaf), profile as int) == Some(path@.take(depth as int)),
            depth == profile && next.is_some() ==> walk(nodes@, Some(leaf), profile as int).is_none(),
            forall|i: int| depth <= i < 4 ==> path@[i] == ROOT,
            walk(nodes@, Some(leaf), profile as int) == match walk(nodes@, next, profile as int - depth as int) {
                None => None,
                Some(tail) => Some(path@.take(depth as int) + tail),
            },
        decreases profile - depth,
    ], [
        let ghost previous_path = path;
        let ghost previous_next = next;
        let ghost previous_depth = depth;
        proof { reveal(walk); }
    ], [
        proof {
            assert(path@.take(depth as int) =~= previous_path@.take(previous_depth as int) + seq![path@[depth as int - 1]]);
            reveal(walk);
            if let Some(tail) = walk(nodes@, next, profile as int - depth as int) {
                assert((previous_path@.take(previous_depth as int) + seq![path@[depth as int - 1]]) + tail
                    =~= previous_path@.take(previous_depth as int) + (seq![path@[depth as int - 1]] + tail));
            }
        }
    ], [
        proof {
            reveal(walk);
            assert(walk(nodes@, next, profile as int - depth as int) == Some(Seq::empty()));
            assert(path@.take(depth as int) + Seq::empty() =~= path@.take(depth as int));
        }
    ])
}

spec fn observation_matches(nodes: Seq<Option<Node>>, profile: usize,
    records: Seq<Option<DomainRecord>>, poisoned: bool, key: Key, slot: usize,
    owner: u64, expected: ResourceVectorV1) -> bool
{
    &&& !poisoned
    &&& accepted(nodes, profile, key)
    &&& slot < records.len()
    &&& records[slot as int].is_some()
    &&& records[slot as int]->Some_0.leaf == key
    &&& owner != 0
    &&& records[slot as int]->Some_0.credit.owner == owner
    &&& records[slot as int]->Some_0.credit.phase == Phase::Retained
    &&& records[slot as int]->Some_0.credit.charge.counts@ == expected.counts@
}

fn domain_retained_observation_v1(
    nodes: &[Option<Node>],
    profile: usize,
    records: &[Option<DomainRecord>],
    poisoned: bool,
    key: Key,
    slot: usize,
    owner: u64,
    expected: ResourceVectorV1,
) -> (out: bool)
    ensures out == observation_matches(nodes@, profile, records@, poisoned, key, slot, owner, expected),
{
    domain_retained_observation_body_v1!(nodes, profile, records, poisoned, key, slot, owner, expected)
}

}

// D's crate-inner attribute and private declarations prevent direct include or
// module reuse without changing D. The exact preceding prefix is checker-bound
// declaration reuse, not an independently authored domain specification.
// Serialized dispatch adapter boundary:
// - Identities below are concrete scalar projections, not a proof of Arc addresses.
// - Locked stores a borrowed-state projection plus raw-lock failure, not a Mutex.
// - Lookup stores the entry for one exact key, not a proof of HashMap semantics.
// The immutable projections are not transferable tokens or native owners. All
// validity after those boundaries is computed by the included production bodies;
// no credit/path/slot validity Boolean or callback is supplied. This says nothing
// about concurrent freshness, native liveness, custody, conservation or authority.
include!("../../fe2o3-resource-accounting/src/independent_retained_observation_body.rs");
include!("../src/request_charge_body.rs");
include!("../../fe2o3-resource-accounting/src/retained_dispatch_body.rs");
include!("../../fe2o3-resource-accounting/src/domain/retained_dispatch_body.rs");
include!("../../fe2o3-kfd/src/resource_domains/composed/retained_dispatch_body.rs");
include!("../../fe2o3-runtime/src/retained_credit_dispatch_body.rs");
include!("../../fe2o3-runtime/src/context/allocation_admission/retained_lookup_body.rs");

verus! {

spec fn independent_observation_matches(records: Seq<Option<Record>>, poisoned: bool,
    slot: usize, owner: u64, expected: R67ResourceVectorV1) -> bool
{
    &&& !poisoned
    &&& slot < records.len()
    &&& records[slot as int].is_some()
    &&& owner != 0
    &&& records[slot as int]->Some_0.owner == owner
    &&& records[slot as int]->Some_0.phase == Phase::Retained
    &&& records[slot as int]->Some_0.charge.counts@ == expected.counts@
}

fn independent_retained_observation_v1(records: &[Option<Record>], poisoned: bool,
    slot: usize, owner: u64, expected: R67ResourceVectorV1) -> (out: bool)
    ensures out == independent_observation_matches(records@, poisoned, slot, owner, expected),
{
    independent_retained_observation_body_v1!(records, poisoned, slot, owner, expected)
}

type RuntimeResourceVectorV1 = R67ResourceVectorV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
enum RuntimeResourceKindV1 {
    LogicalPayloadBytes,
    RequestedAllocationBytes,
    ResidentHostAllocationBytes,
    ResidentDeviceAllocationBytes,
    ExecutableHostImageBytes,
    ExecutableDeviceBytes,
    ControlResidentBytes,
    QueueResidentBytes,
    SignalResidentBytes,
    KernargResidentBytes,
    QueueSlots,
    SignalSlots,
    KernargSlots,
    OperationSlots,
    ReplyBytes,
    ReplyCells,
    TerminalRecordBytes,
    QuarantineBookkeepingBytes,
    AllocationRecords,
}

impl R67ResourceVectorV1 {
    fn get(self, kind: RuntimeResourceKindV1) -> (out: u64)
        ensures out == self.counts@[kind as int],
    {
        self.counts[kind as usize]
    }
}

spec fn request_profile(bytes: u64) -> R67ResourceVectorV1 {
    R67ResourceVectorV1 {
        counts: [0, bytes, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    }
}

fn request_charge(bytes: u64) -> (out: R67ResourceVectorV1)
    ensures
        out == request_profile(bytes),
        forall|i: int| 0 <= i < 19 ==> out.counts@[i]
            == (if i == 1 { bytes } else if i == 18 { 1u64 } else { 0u64 }),
{
    resource_request_charge_body_v1!(bytes)
}

struct Locked<T> {
    unavailable: bool,
    value: T,
}

impl<T> Locked<T> {
    fn lock(&self) -> (out: Result<&T, ()>)
        ensures
            out.is_ok() == !self.unavailable,
            match out { Ok(value) => *value == self.value, Err(_) => true },
    {
        if self.unavailable { Err(()) } else { Ok(&self.value) }
    }
}

// The three implementations are exact identity-field projections. This private
// proof-only trait represents Arc::ptr_eq's identity boundary, not credit validity.
trait IdentityProjection {
    spec fn identity(&self) -> u64;
    fn identity_value(&self) -> (out: u64)
        ensures out == self.identity();
}

struct Arc;

impl Arc {
    fn ptr_eq<T: IdentityProjection>(left: &T, right: &T) -> (out: bool)
        ensures out == (left.identity() == right.identity()),
    {
        left.identity_value() == right.identity_value()
    }
}

struct AccountState<'a> {
    records: &'a [Option<Record>],
    poisoned: bool,
}

struct IndependentAccount<'a> {
    identity: u64,
    state: Locked<AccountState<'a>>,
}

impl<'a> IdentityProjection for IndependentAccount<'a> {
    spec fn identity(&self) -> u64 { self.identity }
    fn identity_value(&self) -> (out: u64) { self.identity }
}

struct DomainState<'a> {
    nodes: &'a [Option<Node>],
    max_depth: usize,
    records: &'a [Option<DomainRecord>],
    poisoned: bool,
}

struct DomainRoot<'a> {
    identity: u64,
    state: Locked<DomainState<'a>>,
}

impl<'a> IdentityProjection for DomainRoot<'a> {
    spec fn identity(&self) -> u64 { self.identity }
    fn identity_value(&self) -> (out: u64) { self.identity }
}

struct DomainAccount<'a> {
    root: DomainRoot<'a>,
    key: Key,
}

spec fn domain_account_observes(account: &DomainAccount, root: &DomainRoot,
    slot: usize, owner: u64, expected: ResourceVectorV1) -> bool
{
    &&& account.root.identity == root.identity
    &&& !account.root.state.unavailable
    &&& observation_matches(account.root.state.value.nodes@,
        account.root.state.value.max_depth, account.root.state.value.records@,
        account.root.state.value.poisoned, account.key, slot, owner, expected)
}

impl<'a> DomainAccount<'a> {
    fn matches_retained_charge(&self, root: &DomainRoot, slot: usize, owner: u64,
        expected: ResourceVectorV1) -> (out: bool)
        ensures out == domain_account_observes(self, root, slot, owner, expected),
    {
        domain_retained_credit_dispatch_body_v1!(self, root, slot, owner, expected)
    }
}

enum AccountHandle<'a> {
    Independent(IndependentAccount<'a>),
    Domain(DomainAccount<'a>),
}

enum TokenAccount<'a> {
    Independent(IndependentAccount<'a>),
    Domain(DomainRoot<'a>),
}

struct Token<'a> {
    account: TokenAccount<'a>,
    slot: usize,
    owner: u64,
}

struct RetainedResourceCreditsV1<'a> {
    token: Option<Token<'a>>,
}

struct ResourceCreditAccountV1<'a>(AccountHandle<'a>);

spec fn account_observes(account: &ResourceCreditAccountV1,
    credits: &RetainedResourceCreditsV1, expected: ResourceVectorV1) -> bool
{
    match &credits.token {
        None => false,
        Some(token) => match (&account.0, &token.account) {
            (AccountHandle::Independent(actual), TokenAccount::Independent(token_account)) => {
                &&& actual.identity == token_account.identity
                &&& !actual.state.unavailable
                &&& independent_observation_matches(actual.state.value.records@,
                    actual.state.value.poisoned, token.slot, token.owner, expected)
            },
            (AccountHandle::Domain(actual), TokenAccount::Domain(root)) =>
                domain_account_observes(actual, root, token.slot, token.owner, expected),
            _ => false,
        },
    }
}

impl<'a> ResourceCreditAccountV1<'a> {
    fn matches_retained_charge_v1(&self, credits: &RetainedResourceCreditsV1,
        expected: ResourceVectorV1) -> (out: bool)
        ensures out == account_observes(self, credits, expected),
    {
        resource_retained_credit_dispatch_body_v1!(self, credits, expected)
    }
}

struct RequestInner<'a> {
    identity: u64,
    account: ResourceCreditAccountV1<'a>,
}

impl<'a> IdentityProjection for RequestInner<'a> {
    spec fn identity(&self) -> u64 { self.identity }
    fn identity_value(&self) -> (out: u64) { self.identity }
}

struct Gfx942RequestAccountV1<'a>(RequestInner<'a>);

struct Gfx942RetainedRequestV1<'a> {
    account: Gfx942RequestAccountV1<'a>,
    inner: RetainedResourceCreditsV1<'a>,
}

spec fn composed_observes(account: &Gfx942RequestAccountV1,
    credit: &Gfx942RetainedRequestV1, bytes: u64) -> bool
{
    account.0.identity == credit.account.0.identity
        && account_observes(&account.0.account, &credit.inner, request_profile(bytes))
}

impl<'a> Gfx942RequestAccountV1<'a> {
    fn matches_retained_charge_v1(&self, credit: &Gfx942RetainedRequestV1,
        bytes: u64) -> (out: bool)
        ensures out == composed_observes(self, credit, bytes),
    {
        composed_retained_credit_dispatch_body_v1!(self, credit, bytes)
    }
}

struct RuntimeAllocationDeviceAdmissionV1<'a> {
    account: Gfx942RequestAccountV1<'a>,
}

impl<'a> RuntimeAllocationDeviceAdmissionV1<'a> {
    fn account(&self) -> (out: &Gfx942RequestAccountV1<'a>)
        ensures *out == self.account,
    {
        &self.account
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeDeviceIdV1 {
    context_generation: u64,
    local: u64,
}

impl RuntimeDeviceIdV1 {
    fn get(self) -> (out: u64)
        ensures out == self.local,
    {
        self.local
    }
}

impl vstd::std_specs::cmp::PartialEqSpecImpl for RuntimeDeviceIdV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAllocationIdV1 {
    context_generation: u64,
    local: u64,
}

impl vstd::std_specs::cmp::PartialEqSpecImpl for RuntimeAllocationIdV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

enum RuntimeResourceCreditAccountInnerV1<'a> {
    General(ResourceCreditAccountV1<'a>),
    Composed(RuntimeAllocationDeviceAdmissionV1<'a>),
}

enum RuntimeRetainedResourceCreditsV1<'a> {
    General(RetainedResourceCreditsV1<'a>),
    Composed(Gfx942RetainedRequestV1<'a>),
}

struct RuntimeResourceCreditAccountV1<'a> {
    device: RuntimeDeviceIdV1,
    inner: RuntimeResourceCreditAccountInnerV1<'a>,
}

spec fn runtime_observes(account: &RuntimeResourceCreditAccountV1,
    device: RuntimeDeviceIdV1, credits: &RuntimeRetainedResourceCreditsV1,
    expected: RuntimeResourceVectorV1) -> bool
{
    &&& account.device == device
    &&& match (&account.inner, credits) {
        (RuntimeResourceCreditAccountInnerV1::General(actual),
            RuntimeRetainedResourceCreditsV1::General(credit)) =>
            account_observes(actual, credit, expected),
        (RuntimeResourceCreditAccountInnerV1::Composed(admission),
            RuntimeRetainedResourceCreditsV1::Composed(credit)) =>
            expected == request_profile(expected.counts@[1])
                && composed_observes(&admission.account, credit, expected.counts@[1]),
        _ => false,
    }
}

impl<'a> RuntimeResourceCreditAccountV1<'a> {
    fn matches_retained_charge_v1(&self, device: RuntimeDeviceIdV1,
        credits: &RuntimeRetainedResourceCreditsV1, expected: RuntimeResourceVectorV1)
        -> (out: bool)
        ensures out == runtime_observes(self, device, credits, expected),
    {
        runtime_retained_credit_dispatch_body_v1!(self, device, credits, expected)
    }
}

struct AccountLookup<'a> {
    key: RuntimeDeviceIdV1,
    entry: Option<&'a RuntimeResourceCreditAccountV1<'a>>,
}

impl<'a> AccountLookup<'a> {
    spec fn selected(&self, key: RuntimeDeviceIdV1)
        -> Option<&'a RuntimeResourceCreditAccountV1<'a>>
    {
        if self.key == key { self.entry } else { None }
    }

    fn get(&self, key: &RuntimeDeviceIdV1)
        -> (out: Option<&'a RuntimeResourceCreditAccountV1<'a>>)
        ensures out == self.selected(*key),
    {
        if self.key == *key { self.entry } else { None }
    }
}

struct RetainedLookup<'a> {
    key: RuntimeAllocationIdV1,
    entry: Option<&'a RuntimeRetainedResourceCreditsV1<'a>>,
}

impl<'a> RetainedLookup<'a> {
    spec fn selected(&self, key: RuntimeAllocationIdV1)
        -> Option<&'a RuntimeRetainedResourceCreditsV1<'a>>
    {
        if self.key == key { self.entry } else { None }
    }

    fn get(&self, key: &RuntimeAllocationIdV1)
        -> (out: Option<&'a RuntimeRetainedResourceCreditsV1<'a>>)
        ensures out == self.selected(*key),
    {
        if self.key == *key { self.entry } else { None }
    }
}

struct ContextAllocationAdmissionV1<'a> {
    accounts: AccountLookup<'a>,
    retained: RetainedLookup<'a>,
}

spec fn context_observes(context: &ContextAllocationAdmissionV1,
    id: RuntimeAllocationIdV1, device: RuntimeDeviceIdV1, byte_len: u64) -> bool
{
    match (context.accounts.selected(device), context.retained.selected(id)) {
        (None, None) => true,
        (Some(account), Some(credits)) =>
            runtime_observes(account, device, credits, request_profile(byte_len)),
        _ => false,
    }
}

impl<'a> ContextAllocationAdmissionV1<'a> {
    fn has_expected_credit(&self, id: RuntimeAllocationIdV1, device: RuntimeDeviceIdV1,
        byte_len: u64) -> (out: bool)
        ensures out == context_observes(self, id, device, byte_len),
    {
        context_retained_credit_lookup_body_v1!(self, id, device, byte_len)
    }
}

}
