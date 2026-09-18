# LogicalMux retained teardown: MI300X GPU 1

Disposition: **all five native packetless lifecycle cases passed** for logical
lane counts 2, 4, 8, 14 and 16. These are five admitted metadata configurations
over the same two physical SDMA queues, not five independent stream counts or
a native queue-capacity sweep.

The earlier GPU 2 campaign is preserved separately in
[`dev-logical-mux-sdma-release-native-gpu2-2026-09-18`](../dev-logical-mux-sdma-release-native-gpu2-2026-09-18/README.md).
It refused its first preflight after foreign work arrived and never ran the
example. This GPU 1 campaign used a fresh device-specific payload, controller,
private directory and admission sequence. Its success does not promote or
replace the GPU 2 refusal. No native example was retried within either campaign.

## Checked behavior

Every isolated public example invocation passed the same checks:

- Observed logical-lane count equals the requested admitted value.
- Exactly two distinct non-primary native queue IDs, targeting engines 0 and 1,
  with 4096-byte rings and the admitted in-flight capacity.
- Configured host-account increase of 8192 bytes and two records, independent
  of logical lane count; unchanged device-account state and default SDMA pool.
- Retained selector and preflight admission; 11 resources returned; complete
  configured-account refunds; inert one-shot retry rejection; completed public
  root Drop. In each process the observed primary ID was 0 and native IDs 1/2.
- Exact two-line stdout, no stderr, status 0, unchanged payload and closed
  original process group.

All 15 authoritative strict observations passed: five preflights, five immediate
and five delayed endpoints. Each invocation began within one second of its
preflight completion. Post-observations began in T0+[0,1] and T0+[20,21] seconds
after parent reap and group closure. The raw sysfs/SMI/PID captures are retained
and independently rederived by the pinned observer. There were zero refusals
in this campaign. Sequential observations are not a continuous reservation.

Selected device: GPU 1, UID `0xab83d2ffef0d3cdf`, BDF `0000:26:00.0`, KFD
node 3/GPU ID 23018, NUMA 0, CPUs 0-47. Exact topology identity is retained in
`raw/native/collected/results/topology/stdout.log`. Other GPUs had active work;
no foreign process or directory was modified.

## Provenance and cleanup

- Signed source commit: `9afafa4176e8b0c6ff53f6892841924f3c7c9c27`.
- All 5,558 qualified source inputs matched that commit's Git blobs. The
  unchanged CPU source cohort SHA-256 is
  `b61c8b42cda67dbdeba649907cf93ca23b862b055f77616fad98fcaab19c405f`.
- CPU seal: `40715bbee4e76d373862261b76cbb62f94d23808cd5ecf0c457162c881d153d9`.
- Static musl ELF: `bcb2a337dd679c31726a326ddfa99e2bb72975b6bff71a7270817148cd32e109`.
- Frozen 26-file payload: `3d19951d78837d794af0e5f4fb4bf64d31d70436816b66825e9ed9ceb3febcba`.

The controller collected and hash-verified all 97 remote files, then matched a
fresh remote inventory before removing only
`/home/harsh/fe2o3-logical-mux-sdma-20260918.c0d7c87a`. All 23 original owned
PIDs/groups were absent during inventory, before/after cleanup, and independent
absence. Accessible same-UID exe/cwd/fd/maps references were empty. Unreadable
entries and the limits of this visibility are explicitly retained in receipts.
Independent exact path absence closed successfully. No remote build occurred.

The archive retains 96 collected files, omitting only the large executable.
Its digest is bound by the CPU receipt, export receipt, payload, remote
inventory and collection verification. Raw receipts are copied byte-for-byte.
The outer manifest includes the nested collected CPU `SHA256SUMS`; only the
archive-root seal is excluded.

## Audit

`verify.py` checks exact source/binary/export identities, the five-case roster,
each native and observer command/environment/bound, full transcripts, all raw
admission derivations and timing windows, original PID roster, collection,
cleanup and independent absence. It uses SHA-pinned historical receipt helpers
but a LogicalMux-specific source binding and native command. The archived local
protocol calibration passed five tests and controller wiring passed four.
Ten archive calibration tests reject identity, transcript, command, case,
observer, source-binding, inventory, cleanup and nested-manifest mutations.

Run `python3 -B docs/evidence/dev-logical-mux-sdma-release-native-gpu1-2026-09-18/verify.py`
for the read-only historical audit. It does not invoke SSH, GPU devices, Cargo
or the ELF. Optional `--source-root` and `--binary` validate supplied current
inputs without execution. `--seal` creates the manifest exclusively once.

## Remaining limits

The initial cursor and zero-packet/MMIO markers are source-qualified path
assertions, not independent hardware traces. No public cursor observation
exists and this probe never submits a copy or advances the cursor. Native
success covers public construction and empty retained teardown only; CPU
fixture fault coverage is not native fault injection. Per-owner destroy order
is enforced in source/fixtures, not independently traced here.

This does not establish submitted-work drain, copy correctness, logical-lane
progress or ordering, independent HIP stream semantics, native fault/panic
retention, aggregate physical residency, runtime-facade exposure, multi-device
behavior, formal implementation refinement, or HIP/HSA performance parity.
R126, A1/A2 and issue #182 remain incomplete.
