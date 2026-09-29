# Dispatch Epoch Cancellation and Terminal Rollback Frames

Development evidence only. A1/A2 remain incomplete, accepted lane checkpoints
are unchanged, and no HIP/HSA behavioral or performance parity is claimed.
No remote machine or GPU was used for this packet.

## Qualified Source

- Production, tests and initial proof: `26de1791c2d0a5cbd77235337381cd82bdd994aa`.
- Localized out-of-bounds negative control: `5054bbaf3`.
- Explicit proof-result bounds, final qualified source:
  `e31f26eddfd82332e12890ab14fc35b65c4fbb11`.

`queue_dispatch_binding/epoch_cancel_body.rs` shares all four executed method
bodies: poison checking, exact identity checking, reserved-roster lookup and
epoch cancellation. Production retains its existing evaluation order:
`expected_roster` runs before `require_identity`. No allocation, scan, extra
runtime validation or public API is added; cancellation remains constant work.
This is an algorithmic observation, not a latency measurement.

## Contract and Trust

`dispatch_epoch_cancel_v1.rs` proves exact success/refusal without a valid-owner
precondition. Success requires an unpoisoned owner, matching recipe occurrence,
queue, in-bounds selected slot, slot generation and Reserved dispatch generation.
Only that slot's phase becomes Vacant. Every refusal is pre-mutation; Poisoned
precedes stale identity. Dispatch and slot generations remain burned.

The method does not authenticate roster internals, next-generation consistency,
nonzero identities, capacity-profile consistency, recycled metadata or sibling
state. The proof preserves this behavior instead of assuming those conditions.
A constructed second-slot witness uses arbitrary roster/neighbor/credit payloads,
zero-capable generations, inconsistent capacity metadata and a MAX counter. It
demonstrates success, stale retry refusal and poison precedence.

The proof projects numeric identity newtypes structurally and the retained
`HostMetadataTableV1` slice payload to Vec. Its accounting payload is an arbitrary
non-Copy value framed unchanged. The reviewed Deref/DerefMut forwarding,
allocation, accounting and Drop implementation are not verified here. Standard
Verus/vstd/Z3/compiler trust remains. This root uses **`--no-cheating`** and
adds no trusted contracts; it does not import the event-release reservation
supplement.

## Qualification

- Final signed campaign: **30/30 stages pass**. Original, relocated and closing
  whole-crate positives each report **16 verified obligations, zero errors**.
  Eleven obligations are derived Clone checks, not runtime operations.
- **22 executable negative controls** fail logically: omitted identity checks,
  out-of-bounds false acceptance, wrong lookup/roster/cancel slot, ignored refusal,
  omitted cancellation, generation/metadata changes and pre-refusal mutation.
- Exact two-file relocation, signed blob binding, **6204 unchanged source
  hashes**, and the pinned **190-file / 129019839-byte** tool closure pass.
  All **56 process groups** from the three runs are reaped and independently
  absent. Four new synthetic calibration groups run outside the owned stages.
- Final KFD regression: **1418 passed, zero failures**, explicitly excluding
  **320 construction-primary tests**. This is not a full KFD-suite rerun.
- Full runtime: **1828 passed, zero failures, 30 hardware ignores**.
- **124 doctests** pass: 42 KFD, 53 runtime and 29 runtime-model.
- Strict all-feature/all-target Clippy and no-default-feature production checks pass.
- Workspace/included-body formatting, source continuity and diff checks pass.

Two raw dispatch groups cover 20 hostile identity/phase/poison cases and three
selected-slot positions with malformed neighboring metadata. A new source-flow
group covers **six genuine N=3 CPU cases**, three failure stages on primary and
AUX, with older selected-lane work and live work on the other lane. Independent
full snapshots check exact release/cancel prefixes, neighbor preservation,
dispatch cancellation identity/count, consumed counters, retained generations,
dependency-owner custody and repeated entry refusal on both lanes.

The CPU recipe owner is external to `session.dispatch`: only the injected
dispatch-cancel fault poisons that fixture owner. Tests do not pretend generic
terminalization poisons it, returns consumed tokens or disposes native resources.

## Retained Failures

The initial signed run stopped on an omitted-bounds mutant: its actual logical
failure had a primary span in vstd's Vec specification, which the strict
source-authentication policy rejected. The replacement control safely returns
wrong success for an out-of-bounds identity, giving a local postcondition failure.
It targets `require_identity`, not cancellation's separate roster precheck.

The second run stopped on a wrong-slot lookup with both a postcondition failure
and an indexing recommendation diagnostic. The final proof adds an explicit
in-bounds conjunct before indexing its successful roster result. This is already
implied by the success equivalence and does not weaken accepted behavior.
The inherited classifier and its authentication policy are unchanged. Both
stopped campaigns, the initial proof frontend failure and the initial test-only
newtype compile failure remain in raw evidence.

## Remaining Work

Complete source rollback composition still needs the actual release/cancel
adapters, reservation traces, dispatch forwarding, native callback owner frames
and terminalization/unwind boundaries. A failed later cleanup must retain earlier
successful cleanup, not claim whole-chain atomic rollback. A guaranteed healthy
retry also needs the lifecycle invariant Available implies zero pins; binding
does not independently recheck it. Full native authority, generated execution,
multi-device/fault campaigns and matched HIP/HSA measurements remain open.

`raw.tar.gz` contains development and signed campaign logs, exact relocated and
mutated inputs, source/tool records, CPU/static checks and an inner SHA256SUMS
manifest. Its **3041216 bytes** contain **389 manifested files plus the manifest**.
An independent restore passed every hash and a recursive comparison; its owned
temporary directory was removed and absence checked.
`COMMANDS.md` inside it records reproducible commands and the distinction
between earlier development runs and the final source. The outer SHA256SUMS
seals this report and the archive.
