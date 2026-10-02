# Pending Multi-GPU Readback And Capture

Increment above `dec70e410699a0fb44ffe4c287e0e2954e1575bd`, prioritizing A3.
Ten MI300X runs pass on 2/3/5/7 GPUs, including reversed seven-device ordering.
This is not A3 completion, compute sharding, HIP/HSA parity, physical overlap,
copy-performance parity or whole-adapter formal refinement.

## Implementation

The native multi-device backend admits an exact-event-bound same-device
DeviceLocal-to-HostVisible readback while its ordinary native peer producer is
still pending. It authenticates routes and retained metadata without touching
detached owners, then reuses existing bounded cooperative-copy progress. Child
execution begins only after producer success and owner restoration. D2H uses
native scratch plus host staging; it is not a zero-staging optimization.

A distinct Context copy-custody root preserves canonical producer identity and
version/epoch/lineage leases independently of public events. Consumer success
cannot commit its output until Context observes and reconciles the peer parent.
Other backends must explicitly opt into this default-disabled contract.

The owner API adds one bounded group-capture cutoff: at most 16 registered host
ranges, one caller-owned concatenated buffer, and a 128 MiB aggregate byte budget.
The existing single-range API retains its 64 MiB limit. All logical sources are
revalidated before the first read; read failure discards the entire result.
Capture bytes stay charged until disposal, even beyond owner shutdown.

## Native Results

Each profile runs changed rounds 0 and 1 in separate bounded processes. Every
round partitions the same 67,108,901-byte payload into contiguous uneven shards,
admits N peer copies and N dependent readbacks before cutoff, releases public
events, and checks all 2N exact completion receipts plus every captured byte.
No compute launches are authorized. Both whole-payload digests are independently
derived by the controller and again by the evidence auditor.

| Profile | GPUs | Processes | Peer successes | Readback successes |
| --- | --- | --- | --- | --- |
| Two devices | 6, 7 | 2 | 4 | 4 |
| Three devices | 5, 6, 7 | 2 | 6 | 6 |
| Five devices | 3, 4, 5, 6, 7 | 2 | 10 | 10 |
| Seven devices | 1, 2, 3, 4, 5, 6, 7 | 2 | 14 | 14 |
| Reverse seven | 7, 6, 5, 4, 3, 2, 1 | 2 | 14 | 14 |

The native completion counter after cutoff and post-copy source preservation
are unobserved. Transport identity comes from authenticated pending-peer
admission, not a post-cutoff counter reading. Logical success also requires
individual receipts; quiescence or matching destination bytes alone is insufficient.

Explicit owned shutdown and capture-credit disposal pass in every process.
Fresh UID/BDF, activity, memory, counted PID-to-device mapping and host-memory
checks precede every run. GPU 0's foreign work is untouched. No owned executable
process remains; the uploaded binary and exact scratch directory are removed
and absence checked. Selected GPU memory and the whole process roster return to
baseline. Point observations are not exclusive reservations.

The local/uploaded/final witness SHA-256 is
`d2789f83643d64849459c3de77dd33ea47ca98e22d21f4d79d015fba805038bc`.
The runtime test executable SHA-256 is
`82e8ff519c27201db855f9145a807addd4192b092820c823376d41619ec35565`.

## CPU And Source Qualification

- Full runtime library: 2,036 passed, 32 existing hardware ignores, zero failures
  or filtering. Exactly 25 tests were added: six backend, eight Context and
  eleven group-capture tests; no baseline test or ignore was removed.
- All 23 example tests, strict combined Clippy, no-default checks, formatting,
  whitespace checks and all four witness builds pass.
- All 32 source-control commands pass. Nine files change only 18 SHA literals
  and seven source counts; all 76 associated proof files remain unchanged.
  This is source/extraction qualification, not a new solver run or formal proof.
- Prior 1,925-test KFD qualification is reused through unchanged KFD sources,
  exact test roster/executable and the authenticated committed production-peer
  archive. It is not described as a fresh KFD test run.

## Diagnostics And Limits

The first hardware attempt correctly rejected early release of an upload still
retained by a later copy on its stream. The witness now settles every upload
before reverse-order release. The second attempt exposed an existing restriction:
after `ACQUIRE_VM`, device admission remains active until process exit, so another
Context in that process cannot reopen the same physical devices. The v2 CLI uses
explicit `--round 0|1` and one Context per process. No admission-history or
retention guard was weakened. Both rejected campaigns have cleanup receipts.

`raw.tar.gz`, authenticated by `SHA256SUMS`, retains these diagnostics and earlier
CPU/source-check failures separately from final `attempt-03`, source proposal-05,
source workflow attempt-04-after and hardware-03. It includes source reconstruction,
exact command/output receipts, the independent auditor and `qualification.json`.
The auditor checks underlying sources, rosters, commands, binary identities,
metadata-only changes, unchanged proofs, prior KFD archive members, hardware
admission, all ten results and cleanup. Summary flags alone are insufficient.

Next priorities are genuine compute sharding under a separate finite authority,
reusable device/VM ownership, fully prequeued compute-to-native-peer composition,
isolated native fault campaigns, eight-GPU coverage and matched HIP/HSA scaling.
The included `next-compute-sharding.md` is a design, not executed qualification.
