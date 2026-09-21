# Production Settlement Bodies

Development qualification of scalar return admission, scratch scanning/staging and
ordered settlement commit on actual production declarations. This advances gate 1,
not full-wrapper refinement, native admission or HIP/HSA parity/performance.

## Scope

The ordinary Rust scalar storage record and Verus now share the same declaration
tokens, preserving all seven fields, derives and visibility. The journal already
shares its production declarations and lossless sequence views. The new executable
proof functions expand the unchanged production return, scratch and commit macros.
Runtime behavior and public APIs are unchanged.

Admission proves checked arithmetic and all bounds for supplied observations.
Scanning examines only the requested prefix. Staging proves every intermediate
plan field and preserves non-scratch contents and dirty tails. Commit preserves
sequential allocation updates, repeated member returns, burned attempt epochs and
untouched contents. Success is last-write-wins for aliased allocation destinations;
NoEffect preserves current lineage, not the staged prior lineage. Count-zero commit
still clears and returns the writer slot.

Raw readiness remains unchanged: no valid-state, canonicality, uniqueness,
terminal-chain, stored-writer-identity or physical-spare-capacity premise is added.
Pure sequence helpers do not assume an allocated historical journal exists.
Historical bridges prove exact decisions, readiness, plans and ordered prefixes.
Their relation equivalences explicitly retain six opaque Vec identities for
staging, allocation-free identity for successful commit, and whole-value identity
for rejected execution. Equal content views alone do not imply opaque Vec equality.

The composition proves header, evidence, chain, return-capacity and scratch error
precedence, followed by stage and commit. It uses the proved explicit key comparison
and supplied capacity observations. It is not the unchanged public wrapper: derived
`Eq`, actual `Vec::capacity()` calls, and wrapper correspondence remain separate.

## Qualification

- Two default-limit whole-crate positives: **478 verified, 0 errors** each.
- Fourteen scoped negative controls: twelve executable changes and two historical
  identity changes. The scalar and identity controls report 0 verified/1 error;
  looped-function controls report 1 verified/1 error. Unselected callee contracts
  are assumed at calls; these are not whole-crate negatives.
- Controls cover ignored physical-storage observations, wrong/skipped scratch
  rejection, staged slot/lineage/epoch corruption, wrong success and NoEffect
  lineage, uncleared backlinks/scratch, missing member returns, wrong writer
  returns, and omitted rejection/success identity requirements.
- **861 unit tests**, two ignored, and **27 doctests** pass, as do formatting and
  Clippy with all targets and warnings denied. Added tests check shared declaration
  wiring and nonadjacent allocation aliases through both staging and commit.
  Existing tests cover repeated slots, zero counts, malformed untouched state,
  error precedence and an independent widened scalar boundary oracle.

The root includes the existing logical storage root and required shared macros,
not the unused historical-type retained/scratch/commit wrapper layers. Those
wrappers retain their earlier published qualification; the current root still
includes the historical Begin/settlement/custody theorems used by its bridges.
Counts overlap previous packets and are not coverage ratios. The checker pins
the new inputs and inherited staging policy, brackets source/tool identities,
authenticates the 190-file Verus closure and records owned-process completion.
Exact diagnostic kinds, typed spans, macro expansions and exits are required.
Timeouts, resource limits, compiler errors and partial positives do not qualify.

`SOURCE` binds the packet to a signed source commit. The offline auditor rebuilds
cases from Git objects and checks exact manifests, rosters and CPU/solver receipts.
Selftests reject changed inputs, altered diagnostics and rehashed corrupt packets
after checking the pristine packet. Runs began in a development worktree; later
source binding is not a claim of execution from a clean signed checkout, and
hashes alone do not prove execution. Exploratory probes are excluded. Earlier
whole-crate attempts exhausted the default resource limit in the inherited
Begin-custody theorem, including with the smaller include closure; they are not
accepted evidence. A proof-only decomposition isolates allocation, member and
writer custody into derived lemmas and summarizes writer occupancy/key preservation.
The original theorem composes those lemmas. Its contract and solver limits do not
change. Required historical theorems remain included.

The checker authenticates the original Begin-custody source from its published
packet, verifies the unchanged theorem contract and surrounding source, and
applies the inherited source policy to the original bytes before overlaying and
checking the pinned current proof decomposition. Both source versions are input-bound. Earlier checker
pins and signed evidence are unchanged; the current source, not the old body, is
what this campaign verifies.

## Reproduce

```sh
python3 -I -B docs/evidence/dev-journal-settlement-execution-2026-09-21/check.py \
  --repo "$PWD" --verus /path/to/pinned/verus --output /new/owned/proof
python3 -I -B docs/evidence/dev-journal-settlement-execution-2026-09-21/selftest.py \
  --repo "$PWD" --proof docs/evidence/dev-journal-settlement-execution-2026-09-21/proof \
  --packet docs/evidence/dev-journal-settlement-execution-2026-09-21
python3 -I -B docs/evidence/dev-journal-settlement-execution-2026-09-21/audit.py --repo "$PWD"
```

Offline auditing needs Git objects and this packet, not the original temporary
paths, solver installation, build products or GPUs. New campaigns retain their
own path identity and cannot replace these receipts.

## Boundaries

Construction/enrollment, Begin, reader admission/release and unread guards still
need actual-typed operation correspondence. Physical capacity, pointer stability,
allocation failure, panic/unwind, test instrumentation, derived traits and public
wrappers remain outside these content proofs. Tested pointer/capacity stability
with reserved space is not a universal storage theorem.

Context event/member binding, independent producer-result custody, audited
success-gated backend support, bounded producer-first progress and pending native
XGMI qualification remain open. Pending consumers remain fail-closed. No GPU or
performance test ran for this packet. See the
[producer-read roadmap](../../runtime-producer-read-reservations-v1.md).
