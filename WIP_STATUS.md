# Issue 271 Initializer And Local-Frame WIP

Review snapshot, not a main-ready or milestone-complete claim. Source basis:
`17f786cca73585a6ca39554f3d25595a3d9eb828`. Current source manifest:
`29617043d5096d24f83c39efd9be51541f158fd8e96b345d121e355da85d2cde`
(5,129 files). Snapshot publication preserves the source checkout HEAD/index.

## Changes

Seven source/test paths change from the basis. The opt-in local-frame chain
analysis retains physical selected control, simultaneous scalar edge bindings,
allocation/access rows and latest preceding Store information. It preserves the
old single-block entry, uses one live resource ledger, and rejects unresolved
predicates, unsupported effects, malformed sinks and incomplete initialization.
It does not admit effectful source helpers or grant artifact/runtime authority.

Private-array output occurrence keys now include the initializer component from
the sealed source effect. This prevents distinct initializer elements at one
statement from colliding. Ordinary write queries still require the original
source index relation. Whole-initializer queries through that facade and
unsupported Read proofs retain their exact rejection. No public initializer or
Load-value theorem is introduced.

The retained-read regression uses a genuine initializer, indexed write and read.
It independently checks nonpromotion, ten actual source/N effects, nine Stores
and one Load before using the normal seven-pass optimizer. Existing promoted
cases are unchanged.

## Exact Qualification

On preceding source `d05fb3e38a0c307e73f08acbae8c6408e42caa2c254229e353fcb9b920d72d05`:

- All 822 KIR tests across 41 suites passed, with one existing ignored stress
  test. This includes all 32 local-frame tests, which also passed separately.
- All four local-frame compile-fail documents passed with their intended private
  construction and E0521 callback-borrow-escape errors.
- The first output-only run was 9 passed / 1 failed. The failed read test assumed
  a read-only array would remain materialized, contrary to existing promotion
  behavior. The subsequent correction changes only its fixture and test.

On the current `29617043` source:

- The wider private-array owner run was **34 passed / 1 failed**. All ten output
  tests passed, including the corrected retained Read refusal, actual initializer
  matrix, duplicate/substitution negatives, and literal resource boundaries.
- The remaining failure is the unchanged
  `private_array_initializer_uses_exact_sealed_helper_instance_and_promoted_absence`:
  its effectful-helper fixture fails with `HelperEffectsUnavailable { function: 1 }`
  before owner sealing. That positive is unfinished integration work. It was not
  skipped, rewritten as a negative, or enabled by weakening admission. A separate
  clean-baseline run has not been performed for this snapshot.

All listed runs completed their source/tool/config guards. The only changes
between these two source snapshots are the reviewed test-only fixture successor.
Broader lowerer/backend qualification and formatting of this combined WIP remain
pending. No full-repository, tutorial, hardware or approved Verus runtime success
is claimed.

ROOT logs on `XSJHARMENON01`, under
`/home/harsh/work/fe2o3-issue271-diagnostics-20260910`:

- `v362-helper-v363-chain-module.2m7Z5oLA`
- `v362-helper-v363-chain-docs.mb9l8zve`
- `v362-helper-v363-kir-all.GUVGZ5FJ`
- `v362-helper-v363-array-output.uhWExqJE` (first fixture failure)
- `v362-helper-v363-retained-read-owner-tests.gZUgdgCd` (current 34/1 result)

## Remaining Work

Retain chain control and substitutions in the pre-ranked helper owner; establish
genuine source/call/initializer/Load/latest-Store correspondence; transport those
obligations through normal N/B/O optimization; then close formal, region, target,
per-call frame and artifact-policy obligations. The retained-helper consumer is
being developed separately and is not included in this snapshot.

The narrow guarded-memory production batch is already on both mains at
`6771738c03879c6a266d186333f4c66c1e693f37`. A separate main-compatible local-frame
producer port is undergoing its own qualification, including Execution V15
refusal coverage. Do not merge this historical WIP tree wholesale over main.

M5/M7 remain active. M8 and whole-compiler verification remain unqualified.
Approved Verus runtime qualification is deferred, not passed.
