# Preliminary V4-J2 Qualification

This cohort is **not global proof-gate acceptance**. It preserves the first frozen
reader-preflight packet and the structural integration failure. The
[successor receipt](../dev-v4j2-reader-preflight-2026-09-17/README.md) records fresh
qualification after the runner repair; do not combine these cohorts' results.

Base: `ed5b5d64bf95116c21e9bf350c30132ff2bbb524`. Ten source/documentation files
are captured by `source-files.list`, `source-files.sha256` and `source.patch`.
The source manifest SHA is
`757af06213ead31f079e3bc6aece06387b38432e2e3079a5c6796160b8c3706d`;
the patch SHA is
`164aa039c143d81aa0811cfb6672e993b0b9362dae25d4c877fe7ce02fed5682`.

## Results

- GNU and scoped musl each passed 2,121 tests with 23 ignored, zero failures,
  measured or filtered tests. Runtime: 1,057/17; host: 271/4; model: 793/2.
- Complete 2,144-entry rosters match and preserve all 2,139 baseline entries,
  adding exactly five tests. Roster SHA:
  `547a5731bffbcdb3d0e5f167124d8c8c073e6f9a90d31d89cba6789a69d7c076`.
- Strict Clippy, formatting, no-default checks, unsafe-source policy (5 passed,
  1 ignored) and all 87 doctests passed. Source and six executable hashes match
  before and after qualification.
- Reader self-tests passed, including 38 adverse diagnostic/source rejections,
  timestamp-valid stale-bytecode rejection and inherited owned-process cleanup.
- The reader campaign passed: two whole-crate positive runs at 103/0 and 21
  executable mutations, each at 102/1 with the exact intended postcondition.
  Of the positive obligations, 69 are unchanged J1 and 34 are new reader work.
- The global runner exited 1 before its solver campaign: its changed bytes did
  not match the unchanged structural auditor's complete-source authentication.
  Review also identified the new tool-binding, function and preseal surfaces
  requiring explicit auditor integration. This is an integration failure, not
  a solver failure and not a waived check.

`qualify.sh` stopped at that failure. The `source-after` and `binaries-after`
records were then run explicitly before any repair or archive relocation. All
18 command records are closed; only `global-verus` has nonzero status. Candidate
source copies, raw solver JSON/diagnostics, tool identities and closed-process
records are retained under `reader-campaign`.

The archive was relocated from `dev-v4j2-reader-preflight-2026-09-17` to the
preliminary directory after closure. Absolute paths in raw records are historical
invocation paths; source bytes and relative archive manifests are unchanged.

## Boundary

This proves only the named Verus constructor contents and ordered preflight,
under the documented storage/shape premises. It does not prove acquisition or
release commits, invariant preservation, production Rust correspondence,
initialized inputs, Context composition, authenticated native quiescence or
performance. Quiescence/capacity observations remain external premises.
Accepted checkpoints and A1/A2/#182 are unchanged.

Earlier unfrozen development found a Rust `Debug` requirement in a specification
`unwrap` (replaced by matching), a cached-Python-bytecode identity gap (fixed by
compiling the verified bytes), and guard-deletion mutations with multiple exit
diagnostics (correctly rejected, replaced by single-branch corruptions without
relaxing the parser). None is counted as an accepted negative in this cohort.
