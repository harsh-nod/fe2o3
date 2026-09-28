# Ordered Publication Outer-Close Join

Signed source: `c08b3c8bb5fd99268395bce3c23664d7dd31d963`.
Tree: `1891ad59bc36aae20972bc8027649c39439dfabd`.
Implementation parent: `1e02cd3c2922d71033adbdb5707d417222f90c07`;
the final source commit only formats a test expression.

Both remotes' `codex/r65-runtime-drain-versions` heads were observed at this source
after publication at 18:16:04 UTC (harsh-nod) and 18:16:06 UTC (powderluv).

## Runtime Change

Ordered publication now returns a private, non-Copy, must-use object containing
an exclusive mutable loan of the exact pipeline and its staged identity. The
native path creates it only after the complete lane result is `Ok(Ok(()))`;
scripted outer failures likewise cannot return it. Settlement consumes the loan
without accepting another arena or identity. The batch and profile remain indexed
throughout the native operation. Equal numeric identities in different arenas
cannot redirect the consuming caller to another arena.

Both submission paths preflight the exact entry, Publishing phase and Unattempted
root before the lower submission cell can overwrite anything. Settlement retains
the prior effect order: publication duration, Retryable withdrawal or Published
timestamp, metadata confirmation, exact batch deposit, then observer emission.
Failed confirmation preserves the indexed Published root and the updated timestamp.
Dropping an unused returned object preserves custody but does not finish settlement;
the production caller consumes it immediately inside the existing unwind boundary.

No public API, native queue callback or lower receipt implementation changed.
The loan and observation object add no heap allocation or arena scan. No latency
or throughput improvement is claimed.

## Qualification Scope

The real CPU A/B/C chain now compares each installed receipt against the identity
captured from that exact lower submit, while framing earlier receipts. Outer-fault
tests compare exact next/frontier/staged heads; quarantine alone could conceal an
earlier erroneous confirmation. Scripted profile unwind checks exactly one epoch
advance and the expected frontier, with the staged marker cleared.

Read-only swarm review found no runtime correctness blockers. Qualification logs,
compiler-negative probes and a deliberate premature-confirmation mutation are
retained in the campaign archive after final qualification. Compiler ownership
checks establish only their stated Rust loan/consumption properties, not formal
native behavior or the complete runtime contract. Profile-unwind evidence remains
scripted rather than a genuine native receipt fault.

## Results

Final qualification completed at 2026-09-28T18:15:25Z with aggregate status zero:

| Gate | Result |
| --- | --- |
| CPU receipt / ordered-publication focused groups | 10 / 5 passed |
| All-feature runtime library | 1816 passed, 0 failed, 28 existing ignored |
| Runtime doctests | 8 compile-positive and 44 compile-fail passed |
| Strict all-feature/all-target Clippy | Passed |
| No-default-feature compilation and workspace formatting | Passed |
| Signature, implementation continuity and diff check | Passed |
| Double settlement / conflicting arena mutation | Intended E0382 / E0499 refusals |
| Premature metadata confirmation | Compiled; exact head-frame assertion failed |

Focused counts overlap the full suite. The first duplicate-settlement probe also
produced a borrow conflict and was rejected by the strict diagnostic classifier;
the isolated probe passes without relaxing that classifier. The archive retains
both attempts and an earlier formatting failure corrected before final qualification.
All mutations were removed before the final run; no implementation edits occurred
during it. Issue #182 was observed open through the API. No ignored test is waived.

## Remaining Work

Share the complete settlement body with Verus and compose the existing metadata
confirm/withdraw contracts with opaque receipt/profile movement and exact neighbor
frames. Source-audited outer-return authentication and Rust unwind boundaries
remain explicit premises; the prior 44 metadata obligations do not prove this
caller join. Pending/retained-resource settlement and Context composition also
remain open. Native binding/currentness, protected Worker/compiler, multi-device,
atomic/collective authority and matched HIP/HSA performance require separate work.

No solver, MI300X, native fault campaign, GPU benchmark or remote cleanup was run
in this packet. A1/A2, issue #182 and accepted lane checkpoints are not promoted.

## Reproduction

Raw campaign: `/home/harsh/.codex-tmp/fe2o3-ordered-outer-join-20260928-PohBg69f`.
The archived `qualify.sh` records commands, statuses and source continuity. The
negative patches apply to the signed source and are removed before the final
frozen run. `compile-probe.sh` validates specific Rust diagnostics and their source
location, not merely a nonzero command status. Source publication belongs to the
frozen archive; later documentation publication is recorded separately.

Verify `receipts.tar.xz` with the adjacent checksum file. Its SHA-256 is
`37c024998999a8f652bd0d15d3dd690d2987dee6ce8369b924a41c112fccfd07`.
Raw campaign files are frozen after archive creation.
