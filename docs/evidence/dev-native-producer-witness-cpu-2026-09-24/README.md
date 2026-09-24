# Native producer witness CPU qualification

Status: all thirteen corrected CPU qualification stages pass. GNU and musl each
pass 1,400 runtime tests with 22 hardware-only ignores and 35 focused passes
(a subset), plus 46 GNU runtime doctests. Default-feature checks, all-target
strict Clippy, formatting and unchanged source/tool brackets pass. No GPU
execution, new proof or performance result is established by this packet.

The three test-only source paths add two ignored hardware-qualification cases
using the unchanged exact R57 two-launch authority. One admits the consumer
behind a queued producer; the other requires the native published persistent
variant before consumer admission. Both retain exact C/B inputs, release the
public event before progress and observe only the consumer after admission.
Backend producer retirement restores inputs before explicit consumer flush;
Context reconciles the producer only after retaining child device success.
The oracle compares every byte of A/B/C/D (1,048,576 bytes per case), checks
two authority calls and no user-data materialization, then checks empty input,
writer, allocation, module and completion custody and explicit native shutdown.
Native publication does not establish simultaneous physical GPU execution.

`qualify.py` reuses the authenticated previous controller's thirteen bounded
stages and recorder. It binds all 3,961 source inputs and the exact three-path
delta. Final GNU/musl rosters are 1,400 passed plus 22 hardware ignores;
the two new names remain ignored, never counted as native results. The 35-test
focus is a subset; 46 runtime doctests are separate. Formatting, all-target
strict Clippy and both default-feature target checks are included. KFD/model
implementation is unchanged; their earlier suite results are not rerun claims.

## Attempts

- `cpu1`: explicitly stopped during Clippy after review found a logical progress
  deadlock in the witness. The Context producer cannot settle before consumer
  physical success; awaiting its logical state before consumer flush was wrong.
- `cpu2`: Clippy compilation rejected LowerHex formatting of the digest array.
  The correction uses the existing byte-wise hexadecimal formatting pattern.
- `cpu3`: explicitly stopped during the GNU default check after review found
  indexing of completion-only backend maps before settlement. Both reads now
  handle absent pending entries; the producer also rejects failure explicitly.
- `cpu4`: corrected qualification. Earlier cache contents are reused; no cold
  build claim is made.

Each stopped campaign retains its terminal process receipts, unchanged source
bracket and the exact rejected witness source. Review is not hardware evidence.
The CPU evidence verifier requires exact named rosters, source/tool continuity,
reaped command groups and captured outputs. Mutation tests exercise its real
entry points. `audit.py` records replay and rejection tests, checks the stopped
attempts, and can remove only the exact owned Cargo cache after all groups are
confirmed absent. It uses separate immutable before/after cleanup receipts.
Eight verifier-test groups and the first recorded audit pass. The final cleanup
audit retains the three unsuccessful attempts and records exact cache removal
in `raw/cleanup-before.json` and `raw/cleanup-after.json`; these are separate from
any subsequent native build or remote campaign.

Native campaigns must independently bind signed source, the actual tested ELF,
strict fresh preflight and immediate/delayed postflight, and collected receipts
before marker-owned remote cleanup. A1/A2, production refinement, generated
graphs, aggregate memory and matched HIP/HSA performance remain open.
