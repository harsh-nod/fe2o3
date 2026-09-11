# Local R86 Dispatch Retention Evidence

Implementation baseline: signed R85 `9f8779faab169271d716f4a241f849fef0af3567`.
This packet implements the
[whole-roster data retention contract](../../runtime-dispatch-data-retention-v1.md),
the first production-used data portion of NATIVE-1.

## Status

All seventeen corrected frozen-source gates and the auxiliary checks pass,
with 5,620 non-documentation source identities unchanged. Three read-only
reviews found no remaining blocking issue in this data-conversion scope.
Complete code/kernarg construction ownership, outer unwind/retake custody and
native adoption remain open. No SSH, GPU workload or remote staging was started.

## Final Acceptance

| Gate | Accepted result |
| --- | --- |
| GNU and musl runtime crates, all features/targets | 2,401 passed and five ignored per target, across 48 harnesses each. |
| GNU all-feature host / musl default host | 258 passed, four ignored / 141 passed. |
| GNU runtime and host doctests | 108 passed. |
| Musl runtime / default host doctests | 91 / 16 passed. |
| Generated macro fixtures | Seven passed. |
| Focused whole-roster conversion | Thirteen passed, including the five-variant pristine-abort case. |
| Existing readback / charged / shell / storage / adoption / async selections | Ten / fifty / 19 / nine / fifteen / 233 passed. These are subsets; storage and shell overlap by one runtime test. |
| Lint, formatting and policy | All-feature/all-target and production-library Clippy pass with warnings denied; formatting, whitespace, dependency policy/tests, local CI gate and 32 standalone lockfiles pass. |
| Runner/checker tests and production closure | 151 Python tests pass; dependency audit passes for 43 packages and eight build scripts. |
| Proof inventory | 686 negative files inventoried; no Verus solver rerun. |

R86 adds thirteen KFD CPU test functions, no runtime/host test functions, no new
doctests and no new theorems. Model, resource-accounting, completion and
manifest/lockfile inputs remain unchanged from the R73 proof checkpoint.
That historical 62-source/1,374-obligation proof result does not cover R86.

The [summary](test-summary.json), [changed-source hashes](source-files.sha256),
[retained-file hashes](retained-files.sha256),
[accepted gate record](raw/r86-accepted-source-gate.json) and
[complete source identities](raw/r86-accepted-source-inputs.json) retain the
exact accepted packet. The [failed first gate record](raw/r86-final-source-gate.json)
is separate. Helpers, production metadata and raw logs are preserved without
whitespace rewriting.

## Intermediate Checks

The first compile rejected a test fixture's nonexistent VM generation field;
the fixture now mutates the actual VM ID. The first ten tests then passed and
reported an unused old single-Host conversion method. That private method was
removed after production moved to whole-roster conversion. Thirteen expanded
tests pass without that warning.

The first full source-gate attempt (`r86-final`) passed runtime/host/doc/fixture
tests but stopped at Clippy's `drop_non_drop` check in the capacity regression.
The overflow token now remains in a local binding instead of an explicit drop.
That failed attempt remains separate from the corrected acceptance run.

The preflight-panic test is exercised against a deliberate early-move mutation;
the duplicate test is exercised with only duplicate rejection disabled. Their
expected-negative logs are separate from positive source acceptance. Exact
restoration of the helper precedes the final suite.
The restored helper SHA-256 is
`bf8ba6f1f408befdfea58c5385f2b98b07ab5aba6a54a14f85207e4e7011ae1b`.
Both mutations predate the unrelated capacity-test lint fix; their targeted
test functions and the restored helper are unchanged in the accepted source.

## Open Boundaries

Tests use actual fake-native records and backing-account owners, not Linux
authority. The five-variant pristine-abort fixture uses injected content
metadata; separate initialized Host/Device tests use their fake-backed
initialization routines. They do not prove native execution or compiler admission.
Behavioral fixtures execute the common conversion helper, not the live-session
wrapper or the complete fixed-dispatch constructor.

Persistent preflight rejection now preserves all original inputs; later
code/kernarg failure reconstruction and whole-constructor unwind remain open.
The runtime model and accounting proofs are unchanged, and no solver rerun or
new adapter theorem is claimed. This packet does not complete NATIVE-1,
NATIVE-2, A1/A2, #182 or HIP/HSA parity.
