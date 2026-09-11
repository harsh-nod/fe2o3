# Local R87 Pending Allocation Evidence

Implementation baseline: signed R86
`1fa69f17e526e2b96ba69a22047f209c261c367e`, followed by the signed dispatch-only
commit `d2e65f47d942af8307d3d54ffa6ee82bef5d2c30`.
This packet implements the
[pending GTT allocation contract](../../runtime-pending-gtt-allocation-v1.md),
a pre-record allocation prerequisite of NATIVE-1-CONTROL.

## Status

All seventeen corrected frozen-source gates and the auxiliary checks pass,
with 5,622 non-documentation source identities unchanged. Three read-only
source reviews found no blocking issue within this packet.
No SSH, GPU workload, remote staging or Verus solver was started.

## Final Acceptance

| Gate | Accepted result |
| --- | --- |
| GNU and musl runtime crates, all features/targets | 2,415 passed and five ignored per target, across 48 harnesses each. |
| GNU all-feature host / musl default host | 258 passed, four ignored / 141 passed. |
| GNU runtime and host doctests | 108 passed. |
| Musl runtime / default host doctests | 91 / 16 passed. |
| Generated macro fixtures | Seven passed. |
| Focused pending allocation / shared memory | Fourteen / 175 passed; the allocation selection is a subset of shared memory. |
| Existing retention / readback / charged / shell / storage / adoption / async selections | Thirteen / ten / fifty / 19 / nine / fifteen / 233 passed. These are subsets; storage and shell overlap by one runtime test. |
| Lint, formatting and policy | All-feature/all-target and production-library Clippy pass with warnings denied; formatting, whitespace, dependency policy/tests, local CI gate and 32 standalone lockfiles pass. |
| Runner/checker tests and production closure | 151 Python tests pass; dependency audit passes for 43 packages and eight build scripts. |
| Proof inventory | 686 negative files inventoried; no Verus solver rerun. |

R87 adds fourteen KFD CPU test functions and updates the existing completion-arena
OOM regression. It adds no runtime/host test function, doctest or theorem.
Model, resource-accounting, completion and manifest/lockfile inputs remain
unchanged from the R73 proof checkpoint. That historical
62-source/1,374-obligation proof result does not cover R87.

The [summary](test-summary.json), [changed-source hashes](source-files.sha256),
[retained-file hashes](retained-files.sha256),
[accepted gate record](raw/r87-accepted-source-gate.json) and
[complete source identities](raw/r87-accepted-source-inputs.json) retain the
accepted packet. The [failed first gate record](raw/r87-final-source-gate.json)
is separate. Helpers, production metadata and raw logs are retained unchanged.

## Intermediate Checks

Fourteen new KFD CPU tests exercise the production allocation sequencer with
actual fake-native records and optional backing accounts. Preflight and restored
focused runs pass. The first full source-gate attempt (`r87-final`) stopped after
the GNU suite rejected an existing completion-arena OOM assertion: it expected
zero retained VA after the first native attempt. R87 deliberately reserves the
attempt's VA capacity and identity before that attempt. The updated regression
checks this capacity, the exact retained reservation and untrusted raw ALLOC
output, and absence of a usable record. Its focused rerun passes. The failed
full attempt remains separate from corrected acceptance.

Two isolated expected-negative mutations fail their targeted tests: retaining
raw ALLOC output only after checking errno loses failed output, and clearing the
pending owner on quarantine loses a failed released-slot reuse attempt. Both
mutations were restored before the corrected acceptance suite. The helper and
the targeted new test file are unchanged since those mutations; only the old
completion-arena regression was subsequently updated.
The restored helper SHA-256 is
`d1f4d9badb99f56e9afd498c1a457620fc2a31ce7bb4af94d46cf4d3d14f8e22`.

## Open Boundaries

The pending owner retains returned reservations, raw ALLOC output and mappings
before validation, with original panic propagation and conservative quarantine.
It does not turn malformed output into allocation authority, establish physical
residency, or fabricate a mapping that the backend never returned. Linux mapping
descriptors can be inactive after explicit guard restoration; their destructors
do not implicitly unmap or free native backing.

Successful-token model projection, seal/map/retain transitions, full preparation
and constructor ownership, outer closing-retake/validation custody and native
generated adoption remain open. Kernarg/executable and remaining control-memory
budgets are not implemented by this packet. Historical accounting/model proofs
do not prove this adapter. No Linux execution, new executable refinement,
performance result, A1/A2 completion or HIP/HSA parity is claimed.
