# Completion Source Rollback Composition

Development evidence only. A1/A2 and issue #182 remain incomplete, accepted
lane checkpoints are unchanged, and no HIP/HSA parity or performance gain is
claimed. No remote hardware was used for this packet.

## Qualified Source

- Production, proof factoring, CPU oracle and initial qualification:
  `c4d778cab75f32345923bf2b54bc30dc56b9c749`.
- Supported eager-cancellation negative: `dca90b9e1`.
- Exact closure-diagnostic calibration, final qualified source:
  `f225e1d66e026ad3f12c71b8d82e3fd8ad5d7708`.

`queue_completion/source_rollback_body.rs` shares the actual inline cleanup
condition in `queue_live/fixed_dispatch.rs`. Proof observers expand the same
production release and cancellation call macros. `rollback_adapters_body.rs`
also shares the event forwarding and consuming cancellation adapters. The
proof observes actual reservation results and actual cancellation, rather than
assuming either succeeds. Inline expansion preserves the caller's short-circuit
ownership/drop scope. No allocation, scan or runtime validation pass is added;
this is an algorithmic observation, not a latency measurement.

The cancellation and event-release roots now include one owner schema and
their respective execution leaves. The new composed root includes both leaves
over the same retained completion owner, rather than composing unrelated states.

## Contract and Trust

The normal-return contract has three exact outcomes:

1. Event release refuses: cancellation is not called, logical owner state is
   unchanged, and the result is normalized to `StaleEventOccurrence`.
2. Event release succeeds but cancellation refuses: completed release effects
   remain, and the same normalized error is returned.
3. Both succeed: event release effects remain and only validated retention
   slots become Available. Identity counters and slot generations stay burned.

There is no healthy-owner, matching-cardinality or event/retention-provenance
premise. Constructed witnesses include a poisoned owner with a cancellable
retention, two aliasing events with N=1 retention, disjoint retention, malformed
neighbors, MAX counters and arbitrary non-Copy reader payload. Successful
reservation-dependent witnesses remain conditional on actual reservation results.

Standalone cancellation uses **`--no-cheating`**, including its consuming
adapter. Batch release and composition instead depend on exactly two unchanged,
SHA-pinned HashSet/HashMap `try_reserve` contents-preservation contracts. They do
**not** use `--no-cheating`. No allocator-success, capacity, native-authority or
owner-authenticity guarantee is inferred. Standard Verus/vstd/Z3/compiler trust
also remains. Logical HashMap equality is not allocation/hasher representation
equality, and refusal through the consuming adapter does not return a retention.
Allocation, Drop, panic and unwind behavior are not proved here.

## Qualification

All four final campaigns bind to the same signed source:

| Campaign | Stages | Whole-crate obligations | Executable negatives |
| --- | ---: | ---: | ---: |
| Bound cancellation | 31/31 | 26/0 errors | 23 |
| Event batch release | 32/32 | 29/0 errors | 24 |
| Source composition | 18/18 | 51/0 errors | 10 |
| Consuming/forwarding adapters | 11/11 | 51/0 errors | 3 |

These are **92 passing stages and 60 executable negatives**. Obligation counts
overlap across roots and include derived checks; they are not added as distinct
runtime operations. Every campaign passes original, exact relocated and closing
whole-crate positives, signed blob binding, **6212 unchanged source hashes** and
the pinned **190-file / 129019839-byte** tool closure. Exact five/six/ten-file
include graphs reject extra inputs. The two-contract supplement is audited as
raw bytes, including rejection of CRLF substitution. Wiring calibration checks
the production arguments, identity syntax adapter and forwarding call.

- Final KFD regression: **1418 passed, zero failures**, with **320 construction
  tests explicitly excluded**. This is not a full KFD-suite rerun.
- Full runtime library: **1828 passed, zero failures, 30 hardware ignores**.
- **124 doctests** pass: 42 KFD, 53 runtime (9 plus 44), 29 runtime-model.
- Strict all-feature/all-target Clippy, no-default-feature production checks,
  workspace/included-body formatting and diff checks pass.
- All **197 recorded process groups**, including initial/stopped runs and the
  repeated doctest command, are reaped and independently confirmed absent.

The focused source suite passes all ten groups. Its terminal-prefix group now
covers **eight genuine N=3 CPU cases** across primary and AUX. The added zero-pin
refusal makes event release fail while bound cancellation would succeed, so
eager cancellation changes the independently expected snapshot. Existing full
neighbor/dependency-owner frames, exact recipe cancellation identity/count,
burned counters and repeated terminal rejection checks remain intact.

## Retained Refusals

The initial composition campaign rejects the eager Boolean-bitwise-OR mutant
as an unsupported verifier frontend feature. The replacement uses two explicit
evaluations and then Boolean OR; it fails logically without losing the eager
behavior being tested.

The first adapter campaign rejects the otherwise genuine closure postcondition
failure because its exact diagnostic spelling was not recognized. The final
checker admits only `unable to prove post-condition of closure` for the exact
`*cancel_bound` selector. Hostile calibration rejects other selectors, unknown
suffixes, foreign/nested diagnostics and mixed frontend/resource failures. The
frozen base classifier is unchanged. Both stopped campaigns remain in raw data.

The first CPU script rejected successful doctests because it expected one runtime
53-test summary instead of rustdoc's separate 9 and 44 summaries. The original
script, outputs and refusal remain; the corrected script reruns doctests and
remaining checks with exact counts and the same source hashes. No test failure
was reclassified. Earlier proof-development failures are also retained.

## Remaining Work

Next is the outer post-binding failure settlement: actual recipe/dispatch
cancellation and forwarding, original retry-error preservation, terminal error
normalization and unchanged completion/resource frames. This packet does not
prove that binding established the exact retained Reserved identity, or that
native `RetryableBeforeSideEffect` preserves both owners. Guaranteed healthy
cleanup also needs Available-implies-zero-pins and exact event-issuance provenance.

Outer terminalization, process-global poisoning, unwind, native target scheduling,
physical-versus-semantic settlement, lane DATA composition, protected Worker and
generated execution, multi-device/fault campaigns and matched HIP/HSA measurement
remain open. Partial cleanup must not be presented as whole-chain atomic rollback.

## Archive

`raw.tar.gz` is **12299798 bytes**, containing **1853 manifested files plus the
inner SHA256SUMS manifest**. It retains exact relocated/mutated sources, all
campaign/development logs, CPU/static records, the issue snapshot and reproduction
commands. An independent restore passed every hash and recursive comparison;
its owned temporary directory was removed and absence checked. Only an unused
487795384-byte cached host-test executable was removed from the owned worktree
to relieve local disk pressure. No shared-machine files were touched.

The outer SHA256SUMS seals this report and archive. See `COMMANDS.md`,
`qualify.py`, `qualify-initial.py`, `audit-final.py` and `final-audit.json` inside
the archive for run order, retained refusals and process/source checks.
