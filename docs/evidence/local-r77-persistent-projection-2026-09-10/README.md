# Persistent Generated Projection

Local R77 / GEN-2B-3 projection evidence. Baseline:
`e43e6a98dfc625374f8badee24174e2bcb8ebd95`, branch
`codex/r65-runtime-drain-versions`. The
[contract](../../runtime-persistent-generated-projection-v1.md) records the exact
nonexecuting scope and remaining native, compiler and proof requirements.

## Scope

The new consuming projection retains original image, complete ordered buffers,
fixups, policies, contract identity and timeout. It compares every source
coordinate and independently checks native hidden-argument derivation before
normalizing to the existing fixed queue template. The actual coherent-host
layout routine validates every length, including unused data, before the existing
fixed planner validates the recipe. It performs no native operation.

Context-generated preparation uses the projection inside the retained-device
scope. Its private closed storage transition keeps encoded payload before
decoder credits and authority. No second full image or buffer copy, duplicate
R73 reservation, decoder exposure or generated publication is introduced.

Three read-only agents reviewed native policy reuse, runtime/source identity and
host custody. Native review identified that the existing planner assumes native
validated layouts and could accept invalid unused future lengths. The canonical
coherent-host layout check and exact-index rejection tests address that finding.
The final native and runtime reviews found no remaining blockers. Agents ran no
builds, tests or SSH jobs.

Twelve new CPU tests cover one KFD extraction, eight runtime projection and three
host custody cases; an existing read-only credit test now also checks projection.
Two runtime compile-fail examples check non-Clone and private storage. Coverage
includes allocation pointers/capacities, complete unused tails, reordered fixups,
13 request-coordinate and nine description/policy mutations, every one of 256
hidden-suffix bytes, invalid unused lengths, aliases, dynamic LDS and timeout.

## Final Gates

All seventeen gates in `r77-sharedfixture` passed against the final source.
The runner checked 5,592 non-documentation source identities before and after;
the retained snapshot and per-gate commands/results are under `raw/`.

| Gate | Result |
| --- | --- |
| GNU runtime, five crates/all features/all targets | 2,261 passed, five existing ignores, 48 harnesses |
| musl runtime, same scope | 2,261 passed, five existing ignores, 48 harnesses |
| GNU runtime plus host doctests | 94 passed |
| musl runtime doctests / default host doctests | 78 / 15 passed |
| GNU all-feature host / musl default host | 236 passed, four existing ignores / 119 passed |
| Generated macro fixture harness | Seven passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Both pass with warnings denied |
| Runner/checker Python suite | 151 passed |
| Formatting, whitespace, dependency policy and its regressions | Pass |
| Local CI gate regression / standalone lockfiles | Pass / all 32 pass |

Cargo used `nightly-2026-04-03`, locked/offline resolution, four build jobs and
disabled incremental compilation, with `XDG_RUNTIME_DIR` removed. The local
Markdown check passes all 57 links/anchors. Twelve new CPU test functions and
two new compile-fail examples are included in the totals, not additional runs.

The current production musl dependency audit passes 43 packages and eight
permitted build scripts. Retained metadata SHA-256:
`6c0cac53358224a31c356d91228b602ae8545f7c156a0c695f103533238054ca`.
The negative-proof inventory check passes 686 files, with the solver boundary
described below. No incomplete sequence is acceptance evidence.

## Earlier Attempts

An initial compile found a missing intermediate re-export for the new request
parts. A subsequent compile rejected ABI rows borrowing the kernel being consumed;
the bounded source-name roster now owns those strings during reconciliation.

The initial focused runtime run passed three tests. After coverage expanded, a
host rejection fixture failed before projection because its queue-pointer hidden
field used the wrong COV6 offset. The fixture now uses the inspector's canonical
offset; the focused run passes all eight runtime and three new host tests. These
terminal-only outputs are not retained or substituted for final exact-source
gates. The fixtures remain structurally valid but nonexecutable test inputs.

The first full sequence (`r77-final`) passed both runtime targets, doctests,
host suites and generated fixtures, then stopped at the all-target Clippy gate:
the structural fixture was declared as a module twice in the runtime test crate.
One test-only crate module now supplies both suites. That attempt's logs and
source identities are retained separately; only the complete rerun qualifies the
final source.

## Proof And Native Boundaries

Model, resource-accounting and completion sources remain unchanged from signed
R73. No theorem was added and the Verus solver was not rerun. The current
expected-negative inventory/quality check passes 686 files; this is not proof of
the new projection, native extraction or whole-executor refinement.

Current production musl metadata and its dependency audit are retained under
`raw/`, with exact commands in `raw/r77-auxiliary-gates.py`. This checks Cargo's
production graph, not a native executable's symbol closure or GPU behavior.

No compiler-backed generated constructor, native adoption/publication, timeout
enforcement, complete readback, charged completion driver, whole-executor proof,
HIP/HSA parity or performance gain was established. Fixed-path unsupported
cardinality/alias/hidden-service profiles remain explicit compatibility work.

## Retention

`source-files.sha256` is repository-relative; `retained-files.sha256` is relative
to this evidence directory. Verbatim tool logs are not changed to satisfy
whitespace checks; the post-retention staged check excludes `raw/` for that reason.
No manifest or lockfile changed.

No SSH session, remote stage or hardware workload was started. No remote cleanup
was required. The next execution packet is consuming Context/native adoption
and publication-time authority, not another standalone executor.
