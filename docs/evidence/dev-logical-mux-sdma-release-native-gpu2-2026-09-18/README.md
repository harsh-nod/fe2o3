# LogicalMux GPU 2: preflight refusal before example execution

Disposition: **rejected shared-host admission, no native queue probe executed**.
This is not evidence of a native runtime failure or a passing LogicalMux
teardown. The frozen plan contained five lane counts (2, 4, 8, 14, 16), but
the first preflight refused before invoking `queue-example`. The campaign's
`cases` array is empty; there are no test or post-test observation receipts.
The local controller's `native_attempted=true` means the outer runner started,
not that a queue, ioctl or copy was attempted by the example.

## Observed outcome

GPU 2, UID `0xd2e26fef80cf5c33`, BDF `0000:46:00.0`, had new foreign work
by the authoritative preflight. All three sysfs snapshots reported 96% GPU
busy, 7% memory busy and 31,202,111,488 VRAM bytes. ROCm SMI reported 95% GPU
busy and the same VRAM count. The PID census reported attachment `1150353`.
The complete pinned observer derived nine busy/VRAM/attachment refusal reasons.
A preliminary idle observation did not grant a reservation and did not override
this refusal. No foreign process was signalled or modified.

The only inner commands were topology, placement and the first preflight.
The runner stopped permanently, preserved the refusal and rehashed its payload.
The four original owned PIDs/groups (`1156116`, `1156118`, `1156119`, `1156120`)
were absent during inventory, before/after removal and independent absence.
PID `1150353` was not in that roster. Accessible same-UID references were empty;
unreadable entries and their visibility limits remain in the raw receipts.

All 40 remote files were collected and matched the complete remote inventory.
The exact mode-0700 directory
`/home/harsh/fe2o3-logical-mux-sdma-20260918.09931d84` was re-inventoried and
removed only after collection, followed by independent path/PID/group absence.
The Git archive retains 39 collected files: only `queue-example` is omitted.
Its identity remains in the CPU receipt, export binding, payload map, remote
inventory and collection verification. No remote build or broad cleanup occurred.

## Binding and audit

- Signed containing source commit: `9afafa4176e8b0c6ff53f6892841924f3c7c9c27`.
- Complete CPU source cohort: 5,558 inputs, SHA-256
  `b61c8b42cda67dbdeba649907cf93ca23b862b055f77616fad98fcaab19c405f`.
- CPU seal: `40715bbee4e76d373862261b76cbb62f94d23808cd5ecf0c457162c881d153d9`.
- Static musl ELF: `bcb2a337dd679c31726a326ddfa99e2bb72975b6bff71a7270817148cd32e109`.
- Frozen 26-file payload: `8e5bf50389af8c9086f6707f4fdca943b1b1737ebfbb7400a975fa67b0769983`.

`verify.py` audits source/binary/export identities, actual commands and bounds,
raw observation derivation, exact stopped-before-example state, collection,
cleanup and independent absence. It uses a SHA-pinned historical helper but
does not reuse that helper's combined-SDMA native command or disposition.
The nested collected CPU `SHA256SUMS` is included in the outer manifest; only
the archive-root seal is excluded. Ten archive calibration tests cover hostile
mutations, including refusal promotion, missing/changed nested CPU seals,
command identity, cleanup, ownership and source binding. The runner's separate
local calibrations passed five protocol tests and four controller tests.

Run `python3 -B docs/evidence/dev-logical-mux-sdma-release-native-gpu2-2026-09-18/verify.py`
for a read-only historical audit. It does not invoke SSH, GPU devices, Cargo or
the ELF. Optional `--source-root` and `--binary` revalidate supplied current
inputs without execution. `--seal` creates the outer manifest once.

This result is preserved separately from any later fresh campaign on another
GPU. It proves refusal and cleanup behavior only, not native LogicalMux
construction/teardown, submitted-work correctness, scheduling, native faults,
formal refinement, physical residency, performance or HIP/HSA parity.
