# Guarded Native-Wait Attempt: VRAM Postflight Failure

Development evidence; the campaign failed its postflight guard. Source is exactly
`5d70cb0a6e16fb265fe224690274fdb0be2b0055`, exported with `git archive`.
The changing working tree was not used as benchmark input. This packet does
not establish a matched performance result, HIP comparison, native-wait B/C
coverage, protected execution, formal refinement, parity or milestone acceptance.

## Result

Only baseline KFD **A/1** ran. It exited 0 after thirteen full-buffer-validated
256 MiB round trips, including three warmups and ten samples, and explicit
teardown. Its postflight failed the unchanged 512 MiB VRAM threshold, so the
campaign exited 1. **B, C and HSA D did not run.** There was no retry, threshold
relaxation, timing ratio or matched summary.

GPU 4 was `0x54f88318ca05093d`, PCI `0000:85:00.0`:

| Observation, UTC on 2026-09-18 | Utilization | VRAM bytes | Result |
| --- | --- | ---: | --- |
| A/1 preflight, 07:37:26.996253914 | 0% | 298,647,552 | Admitted, no listed GPU 4 attachment |
| A/1 postflight, 07:38:07.634359191 | 0% | 648,634,368 | Rejected by VRAM threshold |
| Final guard, 07:38:08.943881562 | 0% | 298,754,048 | Admitted, no listed GPU 4 attachment |

The failing guard returned before querying PID attachments. Consequently it
establishes a VRAM-only refusal, not a GPU 4 process attachment. The later PID
snapshot lists PID 865597 on GPUs 6/7 and PID 3161403 on GPU 0, not GPU 4.
The transient is unexplained: neither cause nor overlap with measured work is
established. The later passing guard does not rehabilitate the failed postflight.
Fresh pre/post observations are not an exclusive reservation or continuous
monitoring of the shared host.

The intended unchanged protocol had orders `ABDC`, `BCAD`, `CDBA`, `DACB`, depth
1, CPU affinity 48-95 and memory node 1. A is KFD slice50us; B/C are instrumented
native sleep ceilings 1ms/25us; D is HSA fine-grained host memory, requested mask
2 and CPU agent 1. HSA allocation/mapping and engine-mask choice are not physical
equivalence to the KFD route. There is no HIP cell. These intended comparisons
were not reached and no timing interpretation is offered here.

## Provenance And Cleanup

Seventeen original command records retain exact arguments, raw stdout/stderr,
UTC timestamps and exit status. Only `benchmark` and `full-summary-refusal`
exit 1; all other original records exit 0. The source audit overlapped the
native runner; there is no claim all records ran serially. The native build,
inspection, execution, collection and cleanup dependency chain is ordered.
No receipts were reconstructed during packet assembly.

All 5,538 source file identities match the fixed Git blobs and recorded remote
manifests before/after build and execution. The source export SHA-256 is
`abb3ee3a4be431584a1303bda58828a2c89160d1d1c9461cebad95d64f7eb9b4`.
Recorded executable SHA-256 values are:

- Rust diagnostic: `883fd7cf04888bfaa3e3c539d088e94341e92841fe038caa981285930afc46d1`.
- HSA comparator, built but not executed: `5d7e0578ea8078bf10066bbd7f36fe78608449af6fb6d647f2ec4503e86d9e33`.

The exact owned remote directory was
`/tmp/fe2o3-kfd-native-wait-5d70cb0a-20260918.CgOcvGXS`. Cleanup checked live
executable/cwd references, removed only this 454 MiB directory and confirmed
absence at 07:39:38.825511783 UTC. A separate read-only process scan finished
at 07:40:22.397035159 UTC with directory absent and zero owned references.
No other user's files or jobs were modified. All campaign sessions are closed.

## Review

Historical scripts and raw paths remain unchanged. `source.tar` and binaries
are intentionally not duplicated in this compact packet. The tar remains in
the original local staging directory, and the read-only verifier independently
regenerates its exact bytes in memory from the specified Git commit, verifies
its SHA, file roster, modes, Git blob IDs and remote SHA-256 manifests. The
executables were removed with remote scratch; their identity is supported by
the captured manifests and actual before/after checker output, not new reads.

`vendor/` contains byte-identical, hash-pinned prior parsers. `audit.py` validates
the complete actual A/1 transcript and failed/final guard positions, preserves
the recorded strict full-summary refusal, and independently requires the
complete-protocol parser to reject this interruption. It emits no timing ratios.
It also checks the seventeen exact command records and cleanup observations.

Read-only verification, with no SSH, builds or workloads:

```sh
python3 -I audit.py --repo /home/harsh/.codex-tmp/fe2o3-c4-completion-20260917
```

Run from this packet directory or use an absolute path to `audit.py`.
Independent read-only review found no claim or provenance blocker. Integration
review receipts live under `review/`, separate from the seventeen unchanged
original records under `raw/`. The new audit was formatted after an unretained
exploratory format check requested changes; historical scripts were not changed.
`seal.sh` rechecks the audit and successful review receipts before recording
`SHA256SUMS`. The seal preserves this failed attempt, not performance acceptance.
