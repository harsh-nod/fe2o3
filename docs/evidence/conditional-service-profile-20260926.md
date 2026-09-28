# Conditional Service And Profile Checkpoint

The later [issuer checkpoint](conditional-native-issuer-20260926.md) connects
these owners to the shared durable service and actual locked V5 observation.
Protected deployment and the full compiler-to-GPU path remain incomplete.

This continues [execution custody](conditional-execution-custody-20260926.md).
Implementation snapshot: `97f87c8de728c3ff26b76481bec1ac4d42c69bbf`.
All [#272](https://github.com/harsh-nod/fe2o3/issues/272) milestones M0-M7 remain
open. No tutorial kernel receives new protected-execution, machine-refinement
or GPU-launch credit. The 47/47 end-to-end goal is not complete.

## Implemented

- Nominal V3 service requests/responses carry actual conditional V3 leaves.
  Packet domains are distinct; decoding never retries V1 or V2. One private
  body retains the existing operations, joins and resource accounting.
- The terminal V3 client uses those packets for acquisition, recovery,
  cancellation and fresh-challenge currentness. It retains the original
  exclusive budget borrow and absolute deadline, using the existing exchange
  engine and typed `verify_native_v3` join.
- The V3 client profile embeds the full V3 issuer policy. Its sealed capability
  admits only `/etc/fe2o3/compiler-execution/client-profile-v3` through the
  existing descriptor-relative trusted-tree checks, without environment
  overrides or older-profile fallback.
- Nominal V3 launch, readiness and supervisor-handoff owners match actual V3
  policies and manifests. Their identity-only V1 wires remain unchanged.
  Structural decoding of an older-policy frame is not V3 policy admission.
- V3 launch-image custody uses the existing sealed-object implementation.
  Revalidation and transfer matching require the retained object, not merely
  an identical-byte replacement. Input reservations and returned full/delta
  charges remain explicit.

These records and primitive capabilities do not establish an authenticated
supervisor, live compiler occurrence, independently provisioned key custody,
durable Worker publication or GPU authority. Trusted-tree admission pins opened
objects, not perpetual pathname currentness or exclusion of privileged writers.
Primitive profile/launch capabilities require their caller to preserve the
original resource account across operations; only the terminal client retains
an exclusive account borrow itself.

The primary and a native worker used disjoint source ownership. The worker
implemented profile/lifecycle records in a private source-only worktree and
performed independent service/client/launch-custody reviews. No actionable
regression was found. Expanded V2 packet, client, lifecycle and launch-custody
bodies match their originals after documentation/format normalization.
This is source review, not machine-level equivalence.

## Verification

All runs use pinned nightly 2026-04-03, locked offline dependencies, one Cargo
job/test thread, hidden GPUs, the existing bounded cache and writable runtime
directory. Counts from earlier focused runs overlap and must not be added.

At the implementation snapshot:

- Full protocol suite: 222 library/integration tests, 96 compile-fail examples
  and six positive examples passed.
- Full closure-capability suite: 143 unit tests, 40 compile-fail examples and
  two positive examples passed.
- Native client admission: four socket-free refusal/accounting tests passed.
- Client documentation: all 14 compile-fail examples passed.
- Changed Rust files pass formatting and diff checks. Hygiene delta passes.
  DCO check reports 45 signed commits since the recorded integration base.

The combined protocol/capability log SHA256 is:

```text
db6331b55fc47925cde70cd5a60cbeceb31568f24aed35ced9632a7a7008183c
```

Final client admission and documentation log SHA256s, respectively:

```text
7ddc517e3108143dd505cfe945aac87a6572893679747b7eb46f3cdcc7e63074
045d40b807beff6c98f265af835f9fc29c9a31823581b7a41dbbe2b1e8851ac9
```

Client socket transcripts are **not validated** here. Both six-case native
transcript runs failed with EPERM. A focused V3 retry identifies fixture receive
timeout setup as the failing syscall boundary, before client execution.
Its failed log SHA256 is
`1660c5503f667930da061e22544888d52cf0490af15740783c8a172fe4d8c11e`.
The earlier full client run also failed pre-existing supervisor fixtures with
an overlong Unix pathname under the pinned temporary directory, EPERM and
InvalidServicePeer. No fixture timeout, production check or sandbox restriction
was removed to obtain a pass. Failed logs remain retained. The initial packet
test compile failure was a new ambiguous integer literal, corrected to u16.

At documentation snapshot `d66ea32b8`, locked offline `cargo check --all-targets`
passed for the execution client, issuer, broker authority service, closure
capabilities, Cargo driver and rustc backend. These are Cargo target kinds, not
GPU targets. Existing warnings remain; this is not whole-workspace validation.
The downstream log SHA256 is:

```text
1da48b8b8d3ec9ce16d11b9e87bb05e3a0a1381d130f5179af485d1ffc24b932
```

## Remaining Integration

1. Select the conditional family through independently pinned deployment,
   authenticated launch and actual V3 issuer inputs, not source names, packet
   tags, environment fallback or a decoded profile alone.
2. Move the original TARGET work owner into the outer synchronous compiler scope
   before early native admission. Retain peer identity and the absolute deadline
   without an exclusive borrow that prevents subsequent compiler work. Keep the
   original SOURCE account separate; do not introduce a temporary admission budget.
3. Connect the existing issuer to a real, still-locked V5 occurrence and its V3
   durable Worker/anchor records. No synthetic readiness or observation owner.
4. Consume the actual postchecked preparation, original proof/history and same
   protected invocation through conditional publication. Revalidate immediately
   before publishing; only an authenticated complete carriage can finish custody.
5. Complete trusted parent recovery/preflight and consume that same occurrence
   through the Worker/finalizer. Machine semantics, proved numerical bounds and
   the entire target-matched 47-kernel matrix remain separate requirements.

The production backend and Cargo boundary still admit V1 execution custody.
Conditional production entry still fails closed before publication. Successful
component tests do not alter these boundaries.

Both public mains were last observed at `1d8ef2462`. Normal Git fetch still
fails DNS, so concurrent public work has not been merged and this batch has not
been pushed. All three GPU SSH aliases also fail DNS resolution. No new remote
job or scratch directory was created.
