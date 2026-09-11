# Local R85 Charged Readback Evidence

Implementation baseline: signed R84 `f346aab2e958725291229eae3fffaee127230112`.
This packet implements the private
[charged readback completion contract](../../runtime-charged-readback-completion-v1.md).

## Status

All seventeen final frozen-source gates and the auxiliary checks pass, with
5,618 non-documentation source identities unchanged. Three read-only reviewers
found no remaining blocking issue. This accepts the private host substrate on
CPU, not native generated completion. Adoption and native completion hooks remain
absent; no new Verus theorem or GPU result is claimed.

## Final Acceptance

| Gate | Accepted result |
| --- | --- |
| GNU and musl runtime crates, all features/targets | 2,388 passed and five ignored per target, across 48 harnesses each. |
| GNU all-feature host / musl default host | 258 passed, four ignored / 141 passed. |
| GNU runtime and host doctests | 108 passed. |
| Musl runtime / default host doctests | 91 / 16 passed. |
| Generated macro fixtures | Seven passed. |
| Focused actual reserved readback / charged storage | Ten / fifty passed; these are subsets, not additional full-suite tests. |
| Existing shell / storage / adoption / async selections | 19 / nine / fifteen / 233 passed. Storage and shell selections overlap by one runtime test. |
| Lint, formatting and policy | All-feature/all-target and production-library Clippy pass with warnings denied; formatting, whitespace, dependency policy/tests, local CI gate and 32 standalone lockfiles pass. |
| Runner/checker tests and production closure | 151 Python tests pass; dependency audit passes for 43 packages and eight build scripts. |
| Proof inventory | 686 negative files inventoried; no Verus solver rerun. |

R85 adds ten host CPU test functions, no runtime/KFD test functions, no new
doctests and no new theorems. Runtime model, resource-accounting, completion and
manifest/lockfile inputs remain unchanged from the R73 proof checkpoint.
Its historical 62-source/1,374-obligation proof result does not cover this adapter.

The [machine-readable summary](test-summary.json),
[changed-source hashes](source-files.sha256),
[retained-file hashes](retained-files.sha256),
[final gate record](raw/r85-final-source-gate.json) and
[complete source identities](raw/r85-final-source-inputs.json) retain the exact
accepted packet. Raw command logs are preserved without whitespace rewriting.
The helper scripts and production metadata are retained alongside them.

## Intermediate Checks

The shared charged decoder refactor passed all forty existing charged-storage
tests. Ten new reserved-readback tests passed. Read-only review found that the
initial foreign-gate fixture also had invalid read-only bytes, so it did not
isolate the new gate comparison. The fixture now fills matching original bytes
and checks that shape/data are otherwise valid before testing the foreign gate.

The corrected ten-test selection passed. Temporarily removing only the gate
comparison made the foreign-owner test fail at its forbidden decode callback,
with Cargo exit 101. The exact original readback source SHA-256
`781f1612e05e0e52e45ae8be80fc6c5e65eb1741e8686c96da6150a83148f2df`
was restored before subsequent positive checks. The mutation's unused-field
warning is not a production lint result. Logs for positive and expected-negative
attempts remain separate.

## Open Boundaries

CPU tests use actual charged storage/readback but structural compiler fixtures;
they do not establish protected construction or native completion authority.
The malformed settlement fixture preserves a real quarantined debit; it is not
an injected core mutex failure. Native construction ownership, adoption,
publication, runtime completion admission, public API, executable refinement,
Linux qualification and matched performance remain open. No SSH, GPU workload
or remote staging was started for this packet.
