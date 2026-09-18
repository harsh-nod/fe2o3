# Live Backing Credit Accounting Qualification

Development CPU evidence above base
`b87f30d1b87b2dca29e9f03e8b00f99a65b04391`. The only source change from the
[previous candidate](../dev-copy-accounting-corrected-2026-09-18/README.md) corrects
the fixture's live-credit oracle. No production algorithm, cache policy,
resource authority or admission threshold changed.

## Exact Native Oracle

The ignored hardware fixture requires explicit opt-in and native device identity.
Normal live backing charges are retained, not reserved or quarantined. Each
healthy snapshot now asserts retained records equal the complete live allocation
count. The two host payloads and staging buffer add exactly three retained host
records; the device payload holds exactly one. Size accounting distinguishes
staging capacity (4,194,272 bytes) from page-rounded backing (4,194,304 bytes).

The test uses the public RuntimeContext allocation and copy path with default
cache policy: three 256 MiB allocations, one H2D/D2H pair, all 256 MiB of returned
data checked after destination poisoning, all handles released, and ordinary
shutdown. It checks exact cache counts/capacity, unchanged backing while cached,
restoration to the queue-only baseline after production trim, zero host/device
backing after root release, and an inert repeated shutdown. It does not pre-trim
the cache, change pool limits, accept partial readback, or lower the host guard.

The [first](../dev-copy-accounting-mi300x-first-2026-09-18/README.md) and
[second](../dev-copy-accounting-mi300x-second-2026-09-18/README.md) native candidates
remain rejected. Their aborts preceded explicit copies and do not establish
successful accounting or a production leak. The separate
[native live-credit packet](../dev-copy-accounting-mi300x-live-credits-2026-09-18/README.md)
records this candidate's successful single-test execution, complete readback,
exact refunds and passing preflight/immediate/delayed endpoints. CPU
qualification alone cannot accept native behavior.

## CPU Campaign

Frozen before/after maps bind all 5,543 files and exact fixture SHA256
`b42daaf3293afc6da164d3c0c6c5a354b12d33fb795e4317789a7938ac79a4b9`.
Both complete GNU/musl runtime rosters pass 1,101 tests, with 18 ignored including
the hardware fixture, and equal the preceding campaign. Both executed binaries
match their recorded builds and before/after hashes. Scoped musl disables HIP,
has no interpreter or dynamic dependency, and has SHA256
`f92bbb2ec17040fff2745f2af7e897fb9488b6e6ef1fb240f98fedacf39c86fe`.

Strict all-target/all-feature Clippy, no-default-feature compilation, formatting,
and unsafe source policy (five passed, one ignored) are repeated. The 19 observer
and 75 host-guard CPU passes are inherited from the unchanged
[initial observer campaign](../dev-copy-host-observation-2026-09-18/README.md),
not rerun here. Fifteen closed receipts include packet static checks and final
read-only verification. `verify.py` also checks the preceding sealed inputs,
source delta, whole test rosters, and the current source/binaries. After future
source changes, use `SHA256SUMS` for offline archive integrity instead of claiming
that current-source verification reran successfully.

This packet does not establish a protected application receipt, machine-code
refinement, continuous shared-host isolation or matched HIP/HSA performance.
Accepted checkpoints, A1/A2 and #182 remain unchanged.
