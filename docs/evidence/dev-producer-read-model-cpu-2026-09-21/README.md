# Producer-Read Model CPU Qualification

Signed source: `cba869de8794e6baf0d0e183caf740f6d5dbc1f3`.

This checkpoint adds the bounded producer-read reservation model, including
fourteen test groups. It does not enable pending journaled producer inputs in
RuntimeContext. The [model description](../../runtime-producer-read-reservations-v1.md)
records the remaining proof, Context, backend and native qualification gates.

## Results

| Check | GNU | musl |
| --- | --- | --- |
| Runtime model | 813 passed, 2 ignored | 813 passed, 2 ignored |
| Model doctests | 27 passed | 27 passed |
| All-feature runtime library | 1221 passed, 20 ignored | 1221 passed, 20 ignored |
| Ordered XGMI owner example tests | 3 passed | 3 passed |

The two ignored model tests are existing release-mode scale benchmarks. The
twenty ignored runtime tests require hardware qualification. No new reservation
test is ignored. Doctests include rejection of mutable inner-journal extraction.

GNU no-default-features library checking, warnings-denied model all-targets
Clippy, warnings-denied all-feature runtime library/test/owner-example Clippy,
selected-source rustfmt, source whitespace checks, signature verification and
tracked-source before/after checks all passed. Every recorded command exited
zero and its owned process group was absent afterward.

The run used nightly `2026-04-03`, optimization level 1, debug assertions and
overflow checks enabled, two Cargo build jobs, eight test threads and a reused
target cache. Each command's explicit replacement environment is in its receipt.

## Evidence

`raw/` contains all twelve command receipts and their byte-for-byte stdout and
stderr. `finished.json` records the source and limits of the successful campaign.
`qualify.py` is the original local driver, retained as provenance; its absolute
paths and clean-source assertions make it unsuitable for direct portable replay.
It uses the pinned recorder from the earlier settled-XGMI evidence packet.

`SHA256SUMS` covers the archive except itself. Check it from this directory with:

```sh
sha256sum --check SHA256SUMS
```

The checksum inventory checks bytes, not the truth of the test claims.
Authenticity depends on the signed archive commit. The command receipts include
verification of the signed source and tracked-source diff brackets. The driver's
additional HEAD and clean-status assertions were not separate recorded commands.
This is not an authenticated compiler/dependency closure or a hermetic build.
Raw test output intentionally retains its ending blank lines.

No Verus campaign, executable refinement, native execution, fault-injection
campaign, HIP/HSA comparison or performance qualification was run here. No new
files or processes were created on MI300X. The runtime remains unchanged, and
the full parity objective and its required gates remain open.
