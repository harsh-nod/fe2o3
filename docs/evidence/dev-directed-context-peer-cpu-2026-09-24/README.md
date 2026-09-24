# Directed Context Peer Copies: CPU Checkpoint

Development evidence for the
[directed Context contract](../../runtime-directed-context-peer-v1.md).
The final source passes the complete development CPU campaign below. This is
not native, formal-refinement, performance or milestone acceptance.

## Implementation

The additive Context API admits exact pending-producer inputs only through the
explicit directed scalar backend profile. Producer reservations retain the
original writer/member/range and event binding, share the stable-reader budget,
and survive producer writer-slot reuse. Legacy copies, ordered batches,
generated operations and Worker protocols do not inherit this contract.

The completion planner retains the original terminal backend fact, reconciles
exact producer results before logical Success, and performs at most one backend
action per submission observation. Stream synchronization observes each of its
pending members under the existing shared deadline, not one backend action for
the entire aggregate. Local traversal is bounded and avoids repeated complete
roster validation for every cursor increment during a contiguous node visit.

Failed, quiescent and definitely cancelled consumers release their own inputs
without waiting on pending predecessors. Unknown inputs cannot become Success;
contradictory success observations seal and retain custody. Cancellation,
disposal, generated paths, quarantine and cleanup account for producer roots.

## Qualification

Pinned nightly `2026-04-03`, all features, test optimization level 1, debug
assertions enabled, incremental compilation disabled and two build jobs:

- GNU runtime library: 1,304 passed, zero failed, twenty hardware-only ignores.
- Musl runtime library: 1,304 passed, zero failed, twenty hardware-only ignores.
- Runtime doctests: 46 passed, in groups of 4 and 42.
- Formatting and all-target warnings-denied Clippy pass.
- All seven command receipts have status zero and owned process-group absence.
- All 3,814 source/configuration/fixture hashes are unchanged across the run and
  match the final source.
- Runner SHA256: `ca428b98dcef2853ca55c3d9cc9184402b0afde54bb0668917184728b6af4f3f`.

The archive contains 23 exact raw records: seven command triples and two input
brackets. Cargo's final blank lines are preserved rather than normalized.

## Scope

Nineteen new CPU test functions exercise all eight observation/progress paths,
source/event/range admission, aliases, profile exclusion, a 256-operation chain,
a diamond, released events, callback order and panic containment, immediate
nonsuccess settlement, retained-success cancellation, backend contradictions,
discarded results, source Unknown refusal, missing and malformed roots,
read-budget exhaustion and actual writer-slot reuse after Success and NoEffect.
Separate controls show that descriptive Context Success cannot substitute for
the producer reservation's result.

The fixtures script physical backend leaves and exercise the actual Context
admission, observation and journal paths. They do not qualify native XGMI
publication, physical mappings, hardware progress, faults or cleanup. No new
formal theorem or Rust/native refinement is supplied. Historical proofs retain
their captured inputs and cannot automatically qualify these changed Context
transitions. No SSH, GPU operation or HIP/HSA benchmark ran for this packet.

Two independent read-only source reviews found no remaining blocker after
corrections. Review identified Unknown-source rejection poisoning, secondary
release-unwind policy, cursor/depth validation and cancellation preflight gaps;
these were corrected and appropriate regression controls added. Review is not
formal proof or native execution evidence.

## Retained History

Private scratch root:
`/home/harsh/.codex-tmp/fe2o3-directed-context-20260923-dRfjW3ba`.

- `focused-1` failed compilation on one missing tuple type annotation.
- `focused-2` passed sixteen focused tests on its preliminary source.
- `cpu-1` passed GNU/musl (1,304 each, twenty ignored), all 46 doctests and
  formatting, then failed strict Clippy on one nested conditional.
- The conditional was changed to the equivalent let-chain without suppressing
  the lint. An independent comparison of all 3,814 source inputs confirmed this
  was the only source delta, including exact reconstruction of the prior hash.
- `lint-1` passed strict all-target Clippy. Its runner adds a lint-only mode;
  the full-campaign command roster and bounds are unchanged.
- `cpu-2` is the final successful, unchanged-input campaign archived here.

Private raw records preserve unsuccessful attempts and their original inputs. The
runner imports an owned-process runner helper, not the historical proof theorem.
Its input brackets are development source checks, not portable or hermetic
compiler/toolchain attestations.

After qualification, `cargo clean` removed only this task's private target:
1,339 files, 661.5 MiB. The target's absence was checked separately. Raw records
and the runner remain; no shared cache or remote-host files were removed.

## Remaining Gates

Dedicated async-engine operation registration, native pending-input chain and
fault qualification, executable refinement, aggregate memory accounting and
matched performance remain open. The next async adapter must avoid an implicit
operation-registry flush after a directed quantum; independent explicit stream
registrations require a separate, documented scheduling contract.

Native R125, Admission R118B C1/C2/C3 and Resources R116/V3 remain the broader
accepted checkpoints. This work does not close A1/A2, #182 or HIP/HSA parity.
