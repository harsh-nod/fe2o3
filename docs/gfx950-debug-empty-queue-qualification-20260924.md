# gfx950 empty-queue lifecycle qualification — 2026-09-24

One supervised, packet-incapable native run created and retired an empty debug
queue. It published **zero valid packets** and made **zero doorbell stores**.
This is local resource-lifecycle evidence, not kernel execution, an actual GPU
stop, debugger acceptance or physical-register capture. V4 remains open.

## What actually ran

The fixed observer reached `local_retired` with 190,296,064 bytes of queue backing.
Its retained kernel/trap mapping was 16,384 bytes; the separately reported
projected native total was 190,316,544 bytes. Invalid ring initialization during
queue creation is not packet publication.

The actual owner retained its runtime, trap, metadata, queue and backing through
ordered teardown. The final local observation recorded the zero-GPU-FD fence.
The independent launch family separately established exact process/start/pidfd
joins, same-PID client execution, inherited credentials (including render group
993), manager InvocationID, terminal receipt/ACK, wait/reap/ECHILD, stream EOF and
empty scope. Inner and outer cleanup both completed, with zero kill attempts,
zero adopted children and no expired deadline; afterward the scope was
`not-found`. Local retirement and launch-family cleanup are different facts.
Cleanup is not rollback or proof of process-wide sampler exclusion.

The successful attempt nonce was `7681acb99c0cf507a8453e9900493bb6`;
the independently joined manager generation was
`495f03a776b04065aba6219bd24cb329`. Before/after prerequisite and benign-evidence
snapshots matched. These historical observations do not authorize replay after
host, executable, credential or dependency changes.

## Separate prerequisites, not a stopped-wave result

Fresh batch startup of the separately built stopped-wave debugger passed its
owned-family gate. The 198,796,296-byte GDB ELF has SHA-256
`c52feb106f7b47b11b965b60d931e28e6a59944b1e46b8a0e400600d6108e9be`.
The startup snapshot contained 104 initial Python modules plus eight
collector-added modules. It is not complete import history, complete MI-runtime
coverage, GPU-thread acceptance or a transfer of the older no-queue qualification.

Separately, ordinary LLVM/LLD compilation of a future one-stop fixture passed
at O0 and O3: 13 Node controls, two positive artifacts, four input refusals and
166 artifact-mutation refusals. Both 5,312-byte HSACOs have SHA-256
`cd3daab76db347104166c898a58dcc29c197ee1eac8b258b10a2c793e8779c87`.
The full entry is 16 instructions / 84 bytes, with independently checked
executable padding, a 264-byte kernarg segment and zero LDS/private memory.
Its single `s_trap 3` is at entry+76; entry+80 is a source-derived **future
query expectation**, not an observed PC or presently authorized native gate.
Neither fixture was dispatched. Its proposed 256-byte output and canaries were
not allocated or observed by that static gate.

The empty-queue run instead retained the older 5,536-byte observer artifact.
The two artifacts and the authored gfx942 examples must not be conflated.

## Retained evidence

Hashes identify retained evidence, not signatures, live tokens or portable
authorization. A qualified marker alone is insufficient: the completed root
receipt and its joined observations are required.

| Evidence | Bytes | SHA-256 |
| --- | ---: | --- |
| Empty-queue completed root gate | 441,904 | `0d3efc22d6577f8b0d9ae901340131f24c5b878b1ac4494c19d67ad9cd3f8e8b` |
| Local observation | 1,336 | `0992c06ee4801e7f19ad7d85a39bb28c7b4e9ba7607c9acbb86c5b0244a8085b` |
| Outer family relation | 1,586 | `e70cc98571a99bca5d9f1ec60c76fd9efa0b383d8ef9243a09e8b1a968072531` |
| Local qualified marker | 1,045 | `272f89ec252690ff638d9ac5dd095a8d43655edf95157305e89d1b4c34b3fcec` |
| Fresh debugger-startup completed gate | 142,401 | `2fe2986f3e1fd3bb3ff2c9b96366949d8c1bd1411741e912425f04226c6a3dae` |
| One-stop static completed gate | 101,665 | `ea06a78526d13469c3c0be5ee727ce96f4491c9899015474593635539f0e699d` |

The unsafe [local-owner contract](unsafe-code-policy.md) still requires exclusive
caller lifetime and reviewed supervision. There is no casual native replay
command or raw-address execution API. Attached-debugger/runtime-event
acknowledgment, TTMP readback and sampler exclusion remain unestablished by this
run. No packet, kernel dispatch, actual DEBUG_TRAP stop, GPU register/memory
sample, source authority or protected publication follows from it. The older
[no-queue runtime observation](evidence/gfx950-native-runtime-observation-20260924.md)
remains a separate result. Original accepted exits remain
**M1/V1/V2/U1/U2/U3 (6/18)**.
