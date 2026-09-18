# Combined SDMA Native Teardown: GPU 2

All seven admitted combined-SDMA counts passed this separately frozen
packetless public-API campaign on MI300X GPU 2. This is native creation and
successful retained teardown qualification, not a benchmark, fault-injection
campaign, formal implementation refinement or full R126 acceptance.

## Results

The unchanged CPU-qualified public example ran once for each striped count N,
alongside two directional queues, through its isolated parent/child route.
Every command exited zero with empty stderr and exact two-line stdout.

| Striped Count | Total SDMA Owners | Host Delta Bytes | Returned Resources |
| --- | --- | --- | --- |
| 2 | 4 | 16384 | 17 |
| 4 | 6 | 24576 | 23 |
| 6 | 8 | 32768 | 29 |
| 8 | 10 | 40960 | 35 |
| 10 | 12 | 49152 | 41 |
| 12 | 14 | 57344 | 47 |
| 14 | 16 | 65536 | 53 |

Each case checked distinct non-primary IDs across both sets, H2D/D2H engines
1/0, alternating striped placement, 4096-byte rings, 63 in-flight slots,
capacity 2 engines/8 queues per engine/14 maximum striped queues, exact host
accounting and 11+3*N resource disposal. Success followed configured-account
refunds, an inert rejected retry and completed public-root Drop.

There were 21 strict admitted observations: seven preflights and 14 post-run
observations, with no refusals. Every launch followed admission within one
second. Immediate and delayed observer starts met the frozen T0+[0,1] and
T0+[20,21] second windows after the native process group closed. Count 14
reached the admitted capacity of eight ordinary SDMA queues on each engine.

The directional engine marker serializes previously asserted literal values.
The striped cursor is explicitly source-qualified initial state, not an
independent cursor observation. No packets, transfers, MMIO stores or
cursor-advancing publication were submitted.

## Shared Host

The selected device was GPU 2, UID `0xd2e26fef80cf5c33`, BDF `0000:46:00.0`,
KFD node 4/GPU ID 29122, with CPUs 0-47 and NUMA node 0. The topology seal was
`fe3f37d829f89661660b9ab1e80d453237580a84f7676bad67bf0a70158e07e6`.
The user permitted currently free GPUs; none was exclusively reserved.
Admission required zero GPU and memory busy, VRAM below 512 MiB, complete
identity-consistent captures, and no reported selected-device attachment.
These are sequential observations, not continuous monitoring or a reservation.

The [earlier GPU 1 campaign](../dev-combined-sdma-release-native-2026-09-18/README.md)
remains rejected after contention at the fifth delayed observation. Its four
fully closed cases and fifth successful native command were not reclassified.
This new payload began a fresh seven-case campaign with unchanged gates.
The earlier `/tmp` ENOSPC setup and exact absence evidence also remain sealed
in that earlier packet. No shared work or uncertain stale files were removed.

This campaign used the freshly created mode-0700 directory
`/home/harsh/fe2o3-combined-sdma-20260918.2cf1a6cf`, after checking that `/home`
was writable, executable and had sufficient free space. All 120 remote files
were collected and hash-verified before removing only that marked directory.
All 31 recorded owned PIDs/groups and accessible same-user references were
absent; a separate invocation confirmed exact path and PID/group absence after
cleanup. Raw proc visibility limitations are preserved. No all-user or
inaccessible-reference absence claim is made. No remote build was performed.

## Binding And Audit

- Signed containing source commit: `c19dd3adf33402a63bdcc0404effed39c2f33b75`.
- Complete source cohort: 5,557 inputs, SHA-256
  `9f5447bdbffb3277ff3750f2747a5300d7d69b05065bee149c297a5f4abe08cb`.
- [CPU packet](../dev-combined-sdma-example-cpu-2026-09-18/README.md) seal:
  `529398ddbab4825c6847e918f26e1a0cc6b4554963f20fc37b7b2d9cbb155477`.
- Static musl executable SHA-256:
  `08e0ed94feac79cb8126e536a3926994ebda3c4eff43e3edc3276faf11a1586d`.
- New 25-entry payload SHA-256:
  `af0725e9424354c52cbef6a73eff917d54a0c599fb2047a4a559797d84e93abf`.
- Four protocol and four controller tests passed before upload. Their exact
  complete transcripts are audited. Preliminary `raw/qualification` receipts
  are retained for provenance/hash integrity, not used as historical gates.
- Ten final archive calibration tests cover the full successful campaign and
  hostile mutations of transcripts, export binding, command identity/bounds,
  case roster, raw endpoints/windows, controller state and collected inventory,
  protocol-function changes and every controller command's executable, bound
  and input hash. The earlier eight-test `audit/calibration.*` receipt is retained
  as pre-hardening history; `audit/calibration-final.*` is the final gate.

The read-only `python3 -B verify.py` audits this exact sealed archive and also
audits the prior sealed rejected packet using SHA-pinned shared helper code.
The prior packet and its dependency files must be present in the repository.
Immutable qualified payload inputs must be byte-identical to that prior
packet. An AST comparison additionally proves that protocol semantics changed
only in the exact selected-device/topology assignments and that the fixture
changed only its two nonselected-PID constants. The new plan and binding
metadata may differ. All eleven controller commands, bounds and applicable
input hashes are checked, including transfer host/path and payload approval.
This avoids duplicating the earlier source/CPU checks
while retaining their exact provenance and failed campaign disposition.

`--source-root PATH` additionally rehashes all historical source inputs;
`--binary PATH` rehashes a supplied executable. The ELF is omitted from Git,
but its digest and original complete collection inventory are retained.
Signature output records the earlier cryptographic check; historical audits
do not re-run Git signature verification. `--seal` creates the manifest once.
Archived controls are execution provenance and must not be rerun.

## Remaining Scope

This closes the all-count packetless combined native-success gap for the
exact qualified source, not subsequent source changes. Submitted-work teardown,
native failure retention, nonzero native cursors, physical-residency disposal,
full runtime-facade integration, LogicalMux/terminal-creation retained teardown,
formal production correspondence and matched HIP/HSA performance remain open.
R126, A1/A2, #182 and full HIP/HSA parity are not accepted by this packet.
