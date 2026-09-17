# Issue 271 Guarded Consumer Checkpoint

Reviewable WIP, not production activation or tutorial qualification. M5 exact
optimized-program verification and M7 production integration remain active.
M8 is unqualified; no acceptance milestone is newly complete. Do not merge
this historical tree wholesale over current main.

## Exact Source

- Host: XSJHARMENON01.
- Checkout: /home/harsh/work/fe2o3-issue271-canonical-inventory-v257-20260913.
- Checkout HEAD: 10b190b8267c0c4011b4ef6bbb52680a3c44b391.
- Source: 5,075 files, manifest SHA256
  fe16d0f16366d6a46ccc2187a3832deefe9006dfc3aa791a5b0fde6075402832.
- Previous published WIP: 8fdfcedbd30019757b54cf24ebb7fc0a85b92051.

Snapshot construction verifies every source Git blob and preserves the source
checkout HEAD/index. This status is outside the source manifest. This batch
changes exactly nine source/test paths plus this status.

## Implemented

Production formal admission now partitions guarded reads using a fresh report
from the same live semantic owner. One body-order cursor consumes exact report
rows while preserving a future row when a pending access has no row. Core-proved
reads and remaining ranked reasons form an exact disjoint partition. Foreign
owners, duplicate or excess pending locations, and unconsumed or out-of-order
rows reject. The existing capped work budget is unchanged.

Live V4 evidence pairs outer legacy policy 1 only with the legacy V1 payload,
and guarded policy 2 only with current V3/policy 2, Bits64, and an exact positive
witness. Lineage and multiroot consumers preserve exact inert format identities.
Legacy inert Bits32 compatibility is not a live Bits32 memory proof. The
canonical same-output transaction can serialize guarded reports without
bypassing source, transition, artifact, or runtime proof gates.

Tests cover both mixed-access orders, actual owner custody, pending-row closure,
body order, exact work limits, policy substitution, and public proof reimport.
The test fixtures retain exact rustc slice layout and strict canonical codecs.

## Qualification

Final source above, all source/helper guards passing:

- 566 lowerer tests passed across seven suites, zero failed or ignored.
  Logs: v257-clean-v362-guarded-consumer-lowerer-all.6eL43y5I.
- 6 public V3 and 13 public V4 proof-binding tests passed, zero failed or ignored.
  Logs: v257-clean-v362-guarded-consumer-public-binding.rz3qo8E4.
- Full-workspace formatting passed.
  Logs: v257-clean-v362-guarded-consumer-format.OGqHbwIo.

Immediately preceding source
0cc578c86ac1d88e99704b44a7741de8cf248f0b1c9996e28039818888a1fe91:

- 435 lowerer library tests passed, zero failed or ignored.
  Logs: v257-clean-v361-guarded-consumer-fixtures-repaired.Xqcx9Zzj.
- 897 backend library tests and 98 verifier library tests passed, zero failed.
  Four existing verifier ignores remain: three subprocess helper entrypoints
  and the pinned real Verus runtime proof test.
  Logs: v257-clean-v361-guarded-consumer-backend-verifier.LjH4wQvL.

The only subsequent source change is a public test accessor correction: decode
the retained canonical V8 bytes before inspecting GuardedLoad, instead of calling
a nonexistent module accessor. It changes no production code. Earlier library
results are recorded against their actual snapshot, not presented as a rerun.

The preceding codec-only checkpoint separately passed 820 KIR, 426 lowerer and
896 backend tests on source a809aed1, with one pre-existing KIR stress ignore.
Those are not additional final-consumer test executions.

## Remaining Work

Current main needs its own narrow, jointly tested guarded producer/consumer
port. Preserve Execution KIR V15, semantic MIR V29, shared-slice ABI, RustCall,
aggregate restrictions and structured errors. Historical WIP tests do not
qualify that composition. No main publication is claimed by this snapshot.

Separate initializer WIP 17f786cca73585a6ca39554f3d25595a3d9eb828 still has
the genuine HelperEffectsUnavailable positive failure. Six actual Rust shape
probes reproduced the complete-and-pure helper refusal; they did not establish
successful compilation. An opt-in bounded local-frame chain analysis is a
prerequisite only. Source value/latest-store/call proofs and retained N/B/O
control, memory and initializer transport remain required before admission.

Broader source rules, managed metadata custody, target/runtime binding,
production activation, legacy retirement and all-tutorial compilation remain
open. Simulation, target-matched hardware and website qualification remain
open. Approved Verus runtime qualification is deferred, not passed. This
checkpoint does not change separate issue 272 work.
