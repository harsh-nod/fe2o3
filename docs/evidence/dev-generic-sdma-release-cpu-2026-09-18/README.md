# Single SDMA Retained Release CPU Checks

This packet covers the new retained primary teardown for one Generic SDMA queue:
untargeted, engine 0 and engine 1. It remains R126 development, not acceptance of
R126, A1/A2, issue #182 or full HIP/HSA parity.

## Final Results

| Check | GNU | Musl |
| --- | --- | --- |
| Selected KFD library tests | 66 passed, 1,351 filtered | 66 passed, 1,351 filtered |
| Full runtime library tests | 1,105 passed, 20 ignored | 1,105 passed, 20 ignored |

All selected KFD tests ran; none were ignored. The selection includes the five
new Generic groups, existing directional and ordinary primary release matrices,
public primary-root negative tests and lower retained memory-cleanup matrices.
GNU/musl named outcome rosters match. The ignored runtime tests remain native
qualification requirements, not executed GPU coverage.

Strict all-feature/all-target Clippy, no-default-feature checks, 73 GNU doctests,
formatting and diff-whitespace checks passed. The unsafe-source policy passed
five tests with its explicit inventory-refresh maintenance test ignored. The
Clippy, build, doctest, formatting and diff receipts are exit-checked; the
verifier strictly parses library and unsafe-policy harnesses, not doctest rosters.

## Source And History

The final campaign repeats both target suites after correcting test-local
escrow idioms to use `take` and `Option::replace`. The preliminary four suites
passed, but Clippy rejected ten `mem::replace` style diagnostics. Those original
receipts remain intact. The verifier checks their commands, chronology, exits
and complete rosters, and pins the reviewed failed Clippy transcript. Exactly
`generic_fixture.rs` changed between the two source inventories; production
source did not change between attempts.

Final `*-final` receipts have one ordered command chain and an unchanged
5,555-file source inventory. Its base field records parent commit
`c38b8a23d3ad01ab9ceb2b2ad3fa9fedf8e5aad6`; its file hashes describe this changed
source, not the parent. The selected source bytes are also checked against the
Git index before signing the containing commit. Evidence helpers are outside
that source inventory and are covered by this packet's checksum manifest.

`verify.py` reuses hash-pinned inventory and harness helpers from earlier sealed
packets. Its live-worktree check passed and rejected 25 mutated harnesses. It
requires the repository and matching source; it is not a portable archive-only
auditor, hermetic build closure, binary-identity binding or formal proof.
Raw shell-command receipts preserve their recorder's trailing spaces.

## Remaining Scope

Pending ordinary, XGMI and window payloads in the new admission tests are
metadata-only structural fixtures. Genuine queue resources remain retained;
this is not submitted-work qualification. Existing lower cleanup matrices and
the new final-native-call parent tests do not establish every integrated native
failure path. No SSH/GPU work or remote temporary files were created here.

The next native check is three separately admitted public primary/single-SDMA
creation and retained-release processes, checking eight reported resources,
complete account refunds, inert retry and completed Drop. Native fault injection,
formal implementation correspondence and matched HIP/HSA performance remain
open, as do Striped, LogicalMux and terminal-creation retained teardown profiles.
