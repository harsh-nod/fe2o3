# Standalone Striped SDMA Retained Release: CPU Development

Scope: the production retained-primary release driver now admits standalone
balanced 2..16-owner Striped SDMA sets. This packet is CPU development evidence,
not native GPU execution, formal correspondence, performance parity, R126
acceptance, or closure of issue #182. Combined secondary sets, LogicalMux and
terminal creation profiles remain excluded.

The seven new test functions cover:

- All eight admitted counts, with/without dispatch and a nonzero cursor, exact
  original vector/cursor/token ownership, forward destroy IDs, reverse resource
  identities, resource counts and ordinary-host completion-page account refunds.
- Seven malformed count/cursor rosters, an empty-roster corruption, and 21
  scoped structural/pending corruptions on the last of 16 original owners.
  Removed genuine owners remain
  in test-local custody until restoration. Pending payloads are metadata-only;
  no ordinary, XGMI or persistent work is submitted.
- Every topology/currentness/destroy/doorbell/resource callback occurrence on
  16 owners, for returned error and panic, retaining exact requests, raw outcomes,
  destroyed/completed prefixes, poisoned owners and an unconfirmed gate.
- First/middle/last modified destroy input, plus late native cleanup injection
  at each resource owner, reusing the lower real-memory/model cleanup oracle.
- Inert retry, a 32 KiB bound on the inline SDMA custody and a 128 KiB bound on
  the public primary release root, not total stack use.

The preserved `prelint` cohort passed the full 113-test constructed release
subtree (including
existing Generic/Directional, allocation, creation, pool, and dispatch cases)
and six SDMA cleanup tests (five lower two-phase tests and one original-certificate
retention test) on GNU and musl. Each target has
1,424 total library tests: 1,311 and 1,418 are filtered out respectively. These
selected suites are not a claim that every KFD or runtime test was rerun.

That cohort then failed strict Clippy on one test-helper idiom: case 20 replaced
an `Option` with `None` instead of using the existing `take` macro arm. The complete
failed lint output and matching before/after source inventories remain in
`prelint`; no failed command is counted as a pass. `qualify-prelint.sh` preserves
the original command sequence. The final `qualify.sh` cohort reruns strict
all-feature/all-target KFD Clippy, all twelve Generic/Striped test functions
(1,412 filtered out) and all six cleanup tests on GNU and musl, workspace
formatting, whitespace, and before/after hashes of 5,556 inputs.

The only source difference between cohorts is that one test-helper line; every
production input is byte-identical. The verifier reconstructs the prelint
fixture from the archived final fixture and checks both against their recorded
hashes. Thus the broader regression evidence is retained with an explicit
test-only source bridge, not described as a full-suite rerun on the final fixture.
A subsequent
`runtime-smoke.sh` run covers four existing GNU CPU runtime tests for shutdown
fallback/retry and error/panic terminalization. It does not install a genuine
public retained root or qualify the runtime's native striped selection branch.

`qualify.sh` records commands, timestamps, complete combined output and exit
codes without overwriting receipts. `verify.py --seal --live` checks qualification
and creates the one-time file seal; subsequent `verify.py` validates the historical
archive without requiring the worktree to remain at its qualified revision.
`--live` additionally recomputes the complete qualified source roster and hashes.
The inventory and strict harness parser are pinned existing evidence helpers.
Executable hashes and deployment binaries are not captured in this CPU packet;
native qualification needs its own exact-source/executable binding.
Raw `.command` receipts retain the recorder's final argument separator;
the final staged whitespace check excludes only those generated receipt files.
Test logs retain the harness's blank final line. The first staged check rejected
exactly those nine raw-log endings; its receipt remains preserved. Local
attributes disable only `blank-at-eof` for logs in this packet, and the final
staged check is repeated without altering any raw output.

The initial development compile found a test-only `u32`/`usize` account-counter
mismatch, corrected before qualification. The focused seven-test development
run passed before removing redundant `usize` casts identified by independent
review. Qualification claims are limited to the two recorded source cohorts
and the exact one-line bridge above. No SSH commands, GPU allocations, or remote
temporary files
were needed for this CPU extension.
