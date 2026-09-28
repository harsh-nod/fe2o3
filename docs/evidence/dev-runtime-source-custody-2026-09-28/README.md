# Runtime Native Source-Event Custody

Source: SSH-signed `c7031845fc504bca99d76de8c017b704a0f3b76e`, based on
`fd7a5efece4926d993025a4f62565a46705ca4d5`.

The runtime now owns genuine native source-event receipts for requested ordinary
materialized publications. This is a CPU-qualified ownership integration, not
native dependent-target execution, GPU qualification, a new formal proof, or
HIP/HSA parity. Issue #182 remains open; A1/A2 and accepted lane checkpoints
are unchanged.

## Behavior

- A live logical event at an initial or ordered publication attempt requests
  one native source event. Multiple logical handles do not duplicate native
  authority. Events recorded after publication remain logical-only.
- A Prepared owner keeps its request through retry, even if its logical events
  are subsequently released. A no-effect ordered withdrawal discards its
  temporary owner and resamples logical retains on the next attempt.
- The exact published batch and full event vector are deposited in indexed
  custody before the outer lane callback closes. No event or batch is cloned.
- Actual Ready precedes unused-event release, which precedes signal recycling.
  Logical event handles may remain live while their producer completes.
- Release occurs outside an existing native lane selection, allowing the
  session API to select and restore the event's exact lane, including AUX.
- A returned event is restored into the original vector without allocation.
  Every release error is terminal at runtime: returned custody proves no
  consumption, not transient backpressure. Uncertain release/unwind preserves
  the completed dispatch and prevents recycling or logical success.
- Phase guards prevent cancellation/withdrawal with owned native events and
  retirement with an unreleased source event. Successful recycle timing includes
  unused source-event release. Explicit producer-success checks are unchanged.

The opaque CPU fixture uses the actual lower classified source-publication and
event-release bodies, logical dispatch owners, event ledgers and completion
receipts. Its packet publication and signal storage are CPU substitutes; it
creates no native code or memory authority. Injected ring refusal is a CPU
no-effect callback, not a hardware ring-capacity observation.

## Qualification

Toolchain: `nightly-2026-04-03`, unoptimized GNU test profile, incremental disabled,
two Cargo jobs. Recorded pre-run implementation hashes match the final source;
the final 17-file source manifest also verifies against the signed commit.

| Check | Result |
| --- | --- |
| CPU receipt integration | 20 passed, including 10 new source-event groups |
| Full runtime all-feature library suite | 1828 passed, 0 failed, 30 hardware ignores |
| KFD live-queue subset | 155 passed, 1567 filtered out |
| KFD/runtime all-feature doctests | 42 + 9 + 44 = 95 passed |
| Both crates, all-feature/all-target Clippy | Passed with `-D warnings` |
| Both crates, no-default-feature production check | Passed |
| Formatting, diff and source-hash checks | Passed |

The new groups cover both lanes; early, late and repeated logical events;
Prepared retries; full lower rollback and ordered request resampling; exact
pin-blocked recycle; foreign-session returned custody; physical out-of-order
retirement without dependent authorization; initial/ordered publication outer
faults; and release/completion errors and unwinds. Four fault groups execute
36 isolated child cases. Existing receipt fault groups also pass.

Earlier failed test runs remain in the archive. They exposed an incorrect
subprocess selector and test assumptions about CPU cache replacement and stable
lane assignment. An initial borrowing-check failure is documented in the notes
without a raw log. The final suite uses actual lane assignment and preserves the
CPU provider's idle-cache checks. See `development-notes.md` and `commands.json`
after extraction.

There was **no fresh full KFD library run**. The earlier packet's broad run with
two corrected oracle failures remains historical evidence, not a green full
KFD result for this commit.

## Open Boundaries

The previously qualified shared settlement and metadata bodies remain unchanged.
This integration changes their production callers and adds native source-event
custody; it has no new Verus qualification. Historical proof results do not
establish the new request, publication, rollback, release or retirement invariants.
In particular, opaque Active framing does not prove that an ordered withdrawal
discards no native event authority.

Direct Requested cancellation and runtime-side in-submit terminal/unwind cases
still need dedicated source-event integration coverage. Native source execution,
dependent-target scheduling, physical versus semantic target settlement,
lane-local DATA composition, aggregate accounting, overlap and matched HIP/HSA
performance remain open. Inline table accounting follows the larger Active type;
the separately allocated event vector is not thereby covered by that account.

## Retention And Cleanup

`raw.tar.gz` is 79,712 bytes and contains 24 inventoried files plus `SHA256SUMS`:
logs, command/results metadata, source patch and hashes, executed ELF hashes,
toolchain, issue snapshot and signature verification. It does not contain ELFs
or claim a hermetic build. A fresh extraction passed all internal checksums and
byte-for-byte comparison with the original raw directory.

Archive SHA-256:
`6549357809d255c5c374c6c37752ce5f0a3efd34ec2c86588846778c9260dc8d`.

Only five audited stale local build ELFs were removed, reclaiming 1,376,145,408
allocated bytes. Frozen evidence, current build products and unrelated work
were preserved. The temporary restore directory was removed after verification.
No MI300X job or remote directory was created or modified in this packet.
