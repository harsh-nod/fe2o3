# First Accounting Correction: CPU Qualification

Development CPU evidence for base
`b87f30d1b87b2dca29e9f03e8b00f99a65b04391` plus the captured source map. The only
source change from the [initial campaign](../dev-copy-host-observation-2026-09-18/README.md)
separates SDMA pool capacity (4,194,272 bytes) from page-rounded backing
(4,194,304 bytes). No production policy or runtime algorithm changed.

**This is a rejected native candidate.** Its subsequent distinct GPU attempt
passed the corrected size assertions, then aborted at the fixture's incorrect
expectation that live backing has zero retained records. The ledger instead
classifies normal live backing as retained. Explicit H2D/D2H copies, readback
and shutdown-refund assertions were not reached. See the
[second failed native packet](../dev-copy-accounting-mi300x-second-2026-09-18/README.md).
CPU success does not qualify these ignored native assertions.

## Recorded Scope

- Frozen before/after maps cover the same 5,543 source files; the single changed
  fixture hash is `bded7480cdf174d91756b5cbf1566aee8acd7da9af122cbc9bbea0ba5925cdf8`.
- GNU and scoped static musl each pass 1,101 runtime libtests with 18 ignored;
  both complete rosters equal the initial campaign. The accounting test is ignored.
- Strict all-target/all-feature Clippy, no-default-feature check and runtime
  formatting pass. Unsafe source policy passes five tests with one ignored.
- Both executed test binaries match their recorded build outputs and hashes
  before and after the campaign. Musl has no interpreter or dynamic dependencies;
  HIP is disabled for that build. Its SHA256 is
  `84e91a5bd81a339d5fb74d0e4c8e5fa90e4bec0d959c0d13c42a6de0c6c2b312`.
- Observer and host-guard tests were not rerun here; the unchanged source and
  their 19/75 passes are recorded in the initial campaign.
- Fifteen actual command receipts close successfully, including static checks
  for this packet and the read-only final audit. No performance claim is made.

`verify.py` checks source, exact candidate, prior sealed inputs, full harness
rosters, binaries and receipt closure. It checks the current source and binaries
when run; the archived final verification is the historical result. A later
candidate intentionally fails that current-source binding and must have its own
qualification packet. `SHA256SUMS` permits an offline integrity check without
using the current source or rerunning tests.

Accepted checkpoints, A1/A2 and #182 are unchanged. Neither this CPU campaign nor
the native attempts establish formal refinement, protected application execution,
continuous device isolation, HIP/HSA performance parity or an explanation for
external VRAM observations.
