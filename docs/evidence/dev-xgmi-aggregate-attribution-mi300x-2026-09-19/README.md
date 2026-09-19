# Aggregate XGMI Host Attribution

Status: protocol prepared; no native result is claimed until collection and
replay succeed. This packet follows the matched persistent-hot comparison in
`dev-xgmi-peer-hot-mi300x-2026-09-19`, which measured roughly 14.3 ms KFD versus
30 us HSA and 37 us HIP for a one-MiB peer copy.

## Protocol

- One signed source tree, qualified on GNU and musl, published to both remotes.
- One KFD ELF built with `hardware-diagnostic`; identical executable and inputs
  for diagnostic off, on, on, off. One MiB, depth one, ten warmups and thirty
  measured samples in both ordered directions.
- Adapt the prior controller and complete collection verifier, and reuse its
  authenticated lifecycle tests and endpoint observer. Historical packets remain
  unchanged. Source, ELF, payload, toolchain, command, and process receipts remain
  bound to the signed source. The new runner and parser validate the new mode.
- Shared MI300X: no exclusive reservation. Every trial requires fresh identity,
  zero GPU/memory activity, bounded VRAM, and no selected-device processes.
  Postflight checks run after two seconds and again after twenty seconds. Never
  reset GPUs, kill foreign work, or delete outside the marked owned directory.
- Complete byte-exact collection precedes cleanup. Verify owned path/process
  absence and remove the owned local transport directory. An incomplete or failed
  campaign is preserved and cannot certify native success.

## Interpretation

Enabled trials must contain exactly 82 successful call observations: two prime,
twenty warmup, and sixty measured calls. Ordinals 0..81 map to backend submission
IDs 7..88, alternating the two selected device identities. Each row contains all
seven nonoverlapping host phases and a checked total at least their sum. The
diagnostic capture is preallocated and extracted only after successful runtime
and native teardown. Rejected shapes, pending work, retries, errors, incomplete
timing, or terminal cleanup invalidate the entire capture without changing the
workload result.

The phase total covers backend aggregate progress, not the entire facade timer.
It excludes facade enqueue/settlement/release outside that call. Phase timings
are not device execution times, authenticated profiler events, or authority to
complete or release resources. Comparisons of diagnostic-on and off runs are
descriptive perturbation checks, not statistical speedup claims. Repeated payload
and final canary checks do not independently detect every skipped post-prime copy.

This packet makes no performance-acceptance, full HIP/HSA parity, exclusive-host,
or formal machine-code-refinement claim. Existing currentness and fail-closed
custody checks remain enabled. CPU equivalence tests cover submit/wait/close
outcomes, original deadlines, custody pointer order, and panic identity. Native
dual-recorder enable rejection and mixed ordinary/aggregate invalidation do not
yet have dedicated executable coverage.

## Commands

Run `python3 -I -B campaign.py` only from a clean, signed and dual-published source
tree with a completed live-matching CPU packet. Then run
`python3 -I -B verify.py --seal` followed by `python3 -I -B verify.py`. The verifier
authenticates the signed tooling and full CPU archive before importing their
code, rederives observations from raw output, checks exact artifact closure, and
checks the immutable SHA-256 manifest.
