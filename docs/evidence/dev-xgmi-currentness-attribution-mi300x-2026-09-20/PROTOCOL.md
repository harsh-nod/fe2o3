# Full-Currentness Host Attribution On MI300X

This bounded campaign uses the CPU-qualified runtime currentness capture.
The candidate pair is physical GPUs 1 and 2, selected after a read-only host
inventory showed zero GPU utilization, low VRAM use, and no process attachment
on GPUs 1 through 7 while GPU 0 had other active work. This observation is
not qualification, a reservation, or permission to disturb that work.

The actual campaign requires fresh authenticated endpoint admission before
every trial, followed by settled and delayed postflight checks. Admission
checks exact PCI/UID identity, three complete chronological sysfs snapshots,
zero GPU and memory-engine utilization, VRAM below 512 MiB, and no selected
device process attachments. Other GPUs may be active. If either candidate
fails admission, stop and preserve the attempt; do not reset devices, kill
foreign processes, or silently substitute another pair within the campaign.

One signed, CPU-qualified source tree builds one diagnostic-enabled KFD ELF.
Four trials use that same ELF in off/on/on/off order: 1 MiB, depth one,
persistent mappings, one prime batch, 10 warmups and 30 samples per direction.
The on mode uses `--aggregate-peer-batch-hot-currentness-diagnose` and emits
82 rows, including primes and warmups. The strict parser requires all 41
fields, ordinal/submission/direction identity, complete canonical u64 timing,
checked hierarchical sums, and topology/pair/aggregate containment. Nested
durations are already included in their parents and are not added again.

This is host attribution, not device/copy-engine timing or a HIP/HSA speedup
comparison. Shared-host off/on observations do not establish causal
instrumentation overhead. Timing data carries no execution authority.
Native execution, if accepted, does not establish formal refinement or full
runtime parity.

The controller preserves the existing ownership marker, bounded process
groups, private build/temp paths, byte-exact collection, cleanup-after-collect,
and explicit local/remote absence checks. Failed collection retains recovery
state instead of deleting uncollected evidence. Previous sealed packets are
not modified. Run `campaign.py` only from its signed, mirrored clean tree;
then `verify.py --seal` and `verify.py` reconstruct every receipt and result.
