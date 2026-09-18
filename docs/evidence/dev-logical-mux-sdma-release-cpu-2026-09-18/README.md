# LogicalMux Retained SDMA Release: CPU Development

Qualification passed: 136 selected KFD tests on each of GNU and musl, with
zero failures or ignored tests and 1,302 filtered tests per target. Four GNU
runtime smoke tests passed. Five unsafe-policy tests passed; its explicit
inventory-refresh maintenance test was ignored. Strict Clippy, no-default-
feature builds, formatting, whitespace and source-consistency checks passed.

The source cohort starts at `27843725eda2f13198b865643b7739a89433e501`
plus the exact per-file identities in `raw/source-before.log`.
The selector covers 5,558 Cargo, toolchain, source and release-contract inputs;
evidence and build products are excluded. Before/after inventories must match.

The bounded campaign selects 136 KFD tests on GNU and musl: all constructed
primary release fixture users, public retained-release selector tests and lower
SDMA cleanup tests. It additionally runs four GNU runtime retained-release
smoke tests, the unsafe-source policy (five tests plus its ignored explicit
maintenance command), strict all-feature/all-target Clippy for KFD/runtime,
both crates without default features, formatting and whitespace checks.

Seven new constructed-parent tests cover standalone LogicalMux lane counts
2/4/8/14/16 over exactly two real fixture owners. They check physical ordering,
original vector/resource/lane/cursor identity, zero-effect malformed metadata
and owner rejection, every callback error/panic prefix, mutated destroy inputs,
both late native-fixture cleanup failures, primary failures and inert retries.
A public selector negative prevents a malformed roster from being hidden by
an outstanding-buffer state. Combined composition remains Directional plus
even Striped 2..14 only, including rejection of either LogicalMux orientation.

The initial unarchived development run passed six new tests and failed one
new primary-failure oracle: it incorrectly required shared-memory quarantine
for every primary failure. That assertion was removed because the actual
contract poisons the parent, retained owners and gate, not necessarily the
memory session. The same test's resource-progress expectation was corrected:
shadow completion precedes SDMA resource release. The frozen qualification
reruns all seven new tests and their shared fixture dependencies.

This is CPU development evidence, not R126 acceptance or HIP/HSA parity.
Fixture-native cleanup means real local mappings/model/accounting with scripted
native calls, not GPU execution. No native LogicalMux constructor/selector,
submitted work, native fault, runtime-facade exposure, formal refinement,
aggregate residency or matched performance claim follows. No SSH work or remote
cleanup was needed for this packet. Existing sealed native archives qualify
their historical source only.

`verify.py` checks exact named test rosters/outcomes, commands, statuses,
chronology, source inventory equality and sealed membership. It imports only
SHA-pinned prior receipt/harness helpers. Five calibration tests cover thirteen
malformed roster/transcript variants and absent, matching, mismatched and
already-existing seals. An existing seal cannot be bypassed or overwritten.
Historical/default verification does not re-execute qualification commands.
`--live` deliberately reruns only
the SHA-pinned source selector to compare the current source cohort.

Review identified a missing final calibration step in the replay script and
an allow-unsealed seal-check bypass. Both were corrected. After the initial
qualification process exited, the final five calibration tests were recorded
separately; the complete replay script now includes that same last command.

```sh
python3 -B docs/evidence/dev-logical-mux-sdma-release-cpu-2026-09-18/verify.py
python3 -B docs/evidence/dev-logical-mux-sdma-release-cpu-2026-09-18/verify.py --live
```

`--live` requires the source tree to match the qualified cohort. Historical
verification does not assert that later edits are qualified. `qualify.sh` and
`record.sh` refuse to overwrite receipts or append to a sealed archive.
