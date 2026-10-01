# Retained XGMI Wait-Cadence Measurement

Signed source: `1cfb7580f36090c41cd9f96141c5d6b2952750ab`.
This is one point-admitted, nonexclusive MI300X campaign on GPUs 1 and 2,
not general HIP/HSA parity or a release-performance threshold.

## Result

All 36 trials complete: 24 ordinary and 12 profiled invocations, 1 MiB copies,
depths 1/16/32 and both directions. All 471 native, six transport, fourteen
preparation and four local preparation groups close. Four fresh executables,
source/tool identities, raw receipts and exact owned remote cleanup pass
independent readback. The marked remote directory is verified absent.

The 25 us sleep-ceiling experiment lowers depth-16 batch latency by
29.94-31.51% and depth-32 latency by 21.51-21.66% versus its 1 ms control.
These deep cells have 11.08-13.95% lower latency than HSA and 4.64-9.74%
lower than HIP. Depth 1 remains slower than both. Results use means of two
ordinary invocation p50s, not pooled medians. The 1 ms control is the
separately named experiment API, not the unchanged `kfd-series` timing path.

All 240 instrumented samples have complete diagnostics. Deeper waits use
more scan-thread CPU and more, shorter sleep requests. Those measurements
remain separate from ordinary latency; requested sleep is not measured
avoidable latency. The [measurement table and scope](../../runtime-retained-pair-cadence-v1.md#mi300x-measurement)
give all six ordinary comparison cells and the CPU-cost tradeoff.

No default wait policy changes. Comparator engine identities are unknown,
and no device timeline, scheduler proof or A7 exit is established. The
earlier two-obligation selector proof covers ceiling selection only.

## Files

- `records.tar.gz`: exact 42-file public packet, including comparison data,
  controllers, identities, signature records and their seven-group census.
  SHA-256: `1aa7dc18d20609fdc65b8fe017cecf251dacdc4220532bac0dd717c5971267f8`.
- `manifest.json`: member paths, byte sizes, modes and hashes for that packet.
  The archive retains its original `final.tar.gz` identity in the manifest.
- `signed-source.bundle`: exact signed candidate over public prerequisite
  `6793b910c2aa662df20763fbbd349cec0994677f`, exporting only
  `refs/heads/native36-candidate`.
- `publication-readback.py` and `publication-readback.json`: additional root
  audit of every published member and its original, without new GPU work.

Inside the packet, `raw.tar.gz` contains 89 original preparation/transport
files. Its `remote-commands/pull/stdout` is the byte-identical 1,484-file native
archive, including all four executables. Raw archive modes remain unchanged.
Loose JSON/source copies have explicitly recorded mode normalization; bytes
and original metadata are preserved in `loose-input-index.json`.

## Replay Boundary

This compact packet is not self-contained for original-controller replay.
The full original prepared source archives are named and hash-bound by the
enclosed manifests and summary; they remain retained externally. Controllers
also disclose original absolute paths and measured host/tool prerequisites.
Equivalent source repacking cannot reproduce original gzip/receive-input
hashes. The source bundle requires its public parent, and the included signer
file documents the reviewed key rather than supplying an independent trust
anchor. Earlier rejected availability and proof/CPU records remain unchanged.
