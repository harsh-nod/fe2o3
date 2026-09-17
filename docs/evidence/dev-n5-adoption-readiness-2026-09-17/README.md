# N5 Adoption Readiness Development Evidence

This packet adds immutable, payload-free readiness before the generated native
adoption transition above `ad622fe331cd97d6d2da5db260221dd8c9f76432`.
It is CPU development evidence, not N5/R126 acceptance. R125 remains the accepted
Native CPU/test checkpoint. A1/A2, #182, hardware/formal qualification, aggregate
memory closure and matched HIP/HSA performance remain open.

## Behavior

Activation retains source/binding validation but no longer rejects a busy lane.
Advancement authenticates the hold and empty native prefix, then queries backend
health, persistent exclusion and lane capacity without modifying native state.
Clean contention retains the original carrier/ticket/hold in Adopting. The next
round-robin turn can retry that read-only query without rehashing payload bytes.
The query neither polls other work nor promises bounded waiting.

Readiness and adoption are synchronous in one owner turn. The native-entry
capacity guard remains in place. The phase is armed before readiness, so errors
and panics cannot retry or release accepted custody. Stop/drain retire a waiting
operation's authentic empty prefix without inventing a submission or completion.

## Coverage And Reproduction

`source-base.txt`, `source.patch` and `source-files.sha256` pin the final Rust
cohort. `record.sh` records exact commands, UTC timestamps, complete output and
exit codes without overwriting prior runs. `audit.sh` checks source identity and
final result assertions. `SHA256SUMS` closes this evidence directory.

Six new tests cover false-to-true readiness, exactly-once preflight/adoption,
unit-budget fairness in both registry orders, error/panic custody retention,
owned Stop/drain while permanently busy, exact/foreign/stale holds, unchanged
credits and shell identity, all-leased lanes, active/pipelined lanes and terminal
backend health. The existing persistent-compute exclusion test also checks that
readiness remains false without consuming the active owner or changing leases,
then becomes true after that work is actually retired.

The initial focused run exposed two test-fixture mistakes: repeated unit-budget
retirement without scheduler rotation did not scan the second entry, and dropping
a deliberately poisoned backend correctly aborted. Separate reproductions are
preserved as `preliminary-fairness` (101) and `preliminary-terminal-drop` (134),
with their binary hash. They precede the final source cohort; the tests now use a
complete retirement scan and retain the terminal mock backend, respectively.
Neither preliminary result is a qualification pass.

Final GNU and musl campaigns each passed **869 runtime tests**, with 13 ignored.
Their 882-test rosters match. Strict all-feature/all-target Clippy, formatting,
33 doctests, no-default compilation and the unsafe inventory (five passes, one
explicit maintenance ignore) passed. Binary hashes and per-test results are
recorded separately for each target. The audit requires the exact command set,
vectors and successful final exits, timestamp ordering, current binary linkage,
unique per-test result/roster agreement, and unchanged source hashes/patch.

Two read-only source/test reviews found no blocking issue in this scope. The
scheduler, Context and backend tests are compositional, not one concrete
production carrier-to-native end-to-end execution or a hardware-fault campaign.
No `fe2o3-kfd` crate source changes are included and no full lower-suite rerun is claimed.
No native probe, remote staging, process signal or remote deletion is performed
by this packet. Native tests remain ignored, not passed. No new formal proof or
performance measurement is claimed.

## Separate Follow-Up

A read-only review identified an inherited SDMA creation custody gap in
`crates/fe2o3-kfd/src/queue_live.rs`,
`with_sdma_queue_creation_custody_v1`: if lower creation returns an owned result
and model retake then panics, the local returned result is not installed in the
parent's terminal SDMA owner. This packet does not modify or qualify that path.
The next repair must retain returned success/failure owners before poisoning,
preserve the original panic and keep no-returned-output distinct. Local mapping
fixtures can test custody, not real KFD CREATE_QUEUE or hardware teardown.
