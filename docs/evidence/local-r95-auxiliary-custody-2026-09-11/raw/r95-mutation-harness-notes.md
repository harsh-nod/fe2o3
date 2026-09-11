# R95 Mutation Harness Corrections

The first `slot-identity` mutation compiled and failed the named vacancy test
at case 1. Its patch helper replaced a substring as a complete line, dropping
four leading spaces. The restoration SHA-256 assertion rejected the resulting
candidate before writing. Primary restored the exact original line and checked
all 5,647 non-documentation identities against `r95-corrected-kfd-source.json`.
The original log, record, inventory and descriptor remain preliminary evidence.

At that stage, the corrected campaign used the `v2-` names. During restoration of
`v2-unrooted-opening`, apply_patch rejected backward-ordered hunks without
changing the file. The helper was changed to emit one minimal full-line hunk,
preserving whitespace and eliminating hunk-order dependence. The exact restored
inventory and all eight intended mutated inventories were checked at that stage;
the current checker targets the final formatted-source campaign instead.
The completed unrooted-opening test recorded a genuine named rejection;
a restoration-tool failure is not a test result.

Both problems were in local mutation tooling. No mutated source is accepted.
The final positive gates run after exact restoration. The retained helper is
the corrected implementation, not a claim that the original attempts used it.

The subsequent full-source `r95-corrected` attempt stopped at formatting: one
assertion added after the earlier formatter pass needed line wrapping. The
formatted source is frozen in `r95-formatted-source.json`. All eight mutations
are rerun as `v3-` against it, followed by `r95-accepted-mutation-check` and the
full `r95-accepted` positive gates. Both earlier campaigns remain preliminary.

The first final-source completed-owner mutation exposed a context-free insertion
in the minimal-hunk helper. It was inserted at the module header, and compilation
failed; `r95-mutation-v3-completed-owner` is not an accepted negative test.
Primary removed the misplaced line and verified all formatted source hashes.
The helper now retains three surrounding lines and verifies the complete intended
mutated file before running any test. The corrected test is named
`v3b-completed-owner`; the other final-source entries retain their `v3-` names.
