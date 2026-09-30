# Native Retained XGMI Series Campaign

`benchmarks/runtime_gfx942/xgmi_peer_series_native.py` executes the existing pure
series planner's 18 ordered native KFD/HSA/HIP trials. Depths are 1, 16, and 32;
each depth uses KFD, HSA, HIP, HIP, HSA, KFD, with 1 MiB copies, two warmups,
and ten samples. The existing strict result parser and planner are unchanged.

This is native API timing, not fe2o3 runtime-facade timing. KFD uses retained
pair-series admission under the reviewed ordinary-lifetime assumption. Scope
entry and finish are reported separately, outside samples; operational fences
remain inside samples. Comparator engines remain runtime-selected and unknown.
The runner does not claim matching engines, performance acceptance, HIP/HSA
parity, formal machine-code refinement, or exclusive GPU reservation.

The [September 30 accepted campaign](evidence/dev-xgmi-retained-series-2026-09-30/README.md)
completes all 18 trials on GPUs 1 and 2, with collected raw receipts, signed
candidate history, independent replay and exact-root cleanup. KFD is slower in
every measured cell: 9.7-27.1% versus HSA and 6.4-38.8% versus HIP. Those ratios
compare means of two invocation p50s, not pooled medians or confidence bounds.
No performance exit gate is closed by this evidence.

## Invocation

Use a separately reviewed clean checkout on the target host, with the complete
selected source roots, ROCm, and the pinned Rust toolchain available. Independently
verify the approved commit/signature before invoking the runner. The commit below
is an explicit input, not a signature or signer inferred by the runner.

```sh
python3 -I -B benchmarks/runtime_gfx942/xgmi_peer_series_native.py \
  --campaign --commit APPROVED_FULL_COMMIT \
  --output /absolute/fresh/private-output \
  --device PHYSICAL_INDEX_0 UID_16_HEX_0 PCI_BDF_0 \
  --device PHYSICAL_INDEX_1 UID_16_HEX_1 PCI_BDF_1
```

The output parent must exist outside the checkout. The output itself must not
exist. Device arguments are selectors to check, not accepted observations.
Select currently idle devices and coordinate the one active campaign with other
shared-host users. Idle observations are sequential point observations, not
continuous monitoring or reservation.

## Bounded Shared-Host Transport

`xgmi_peer_series_transport.py` provides a separate, explicit prepare/execute
interface for the reviewed MI300X host. From the approved signed checkout:

```sh
python3 -I -B benchmarks/runtime_gfx942/xgmi_peer_series_transport.py prepare \
  --commit APPROVED_FULL_COMMIT --output /absolute/fresh/local-prepared \
  --device PHYSICAL_INDEX_0 UID_16_HEX_0 PCI_BDF_0 \
  --device PHYSICAL_INDEX_1 UID_16_HEX_1 PCI_BDF_1
python3 -I -B /absolute/fresh/local-prepared/source/benchmarks/runtime_gfx942/xgmi_peer_series_transport.py \
  execute --prepared /absolute/fresh/local-prepared
```

Preparation verifies the explicit `harmenon@amd.com` signer and pinned SSH key,
requires advertised shallow/filter support before fetching, checks that the
initial exact one-commit object closure contains no blobs, and materializes only
the selected source blobs. Per-command upload-pack configuration enables filtering
without changing the original repository configuration. The isolated checkout
contains no remote configuration, alternates, replacement refs or grafts. Source
and object rosters, ordinary archive members and compressed/uncompressed sizes
are checked before transport. `.cargo` may be absent only when absent from the
signed tree; unsigned presence or a symlink is rejected.

Execution uses one fresh marked private directory and the existing
`/home/harsh/.fe2o3-cargo-all-targets.lock`, without creating, truncating or deleting
that shared lock. The maximum lock wait is 900 seconds, with inode continuity
checks on success and failure. A separate fresh monitoring subprocess enforces a
5,400-second campaign bound, a 12 GiB private-storage ceiling, 40 GiB free disk and
64 GiB available memory. It has a liveness pipe and an original-parent pidfd;
failure notification cannot target a reused PID. No monitoring thread overlaps
the owned recorder's fork/pre-exec handoff. These checks bound shared-host usage;
they do not reserve GPUs or guarantee instantaneous resource limits.

The original SSH execution must return a known terminal status before collection.
A bounded raw archive is read back, hashed and independently replayed locally,
including all 18 workload command/visibility/raw-receipt joins. Only then may the
exact marked directory be removed, after original native and monitor closure
receipts agree. Lost SSH ownership, missing receipts or archive mismatches retain
the private directory for inspection; no retry, historical PID probe or global
cleanup is performed. The separate `absence` command checks only that owned path.

## Independent Observations

Before workloads, the runner verifies selected build/helper source bytes against
the explicit commit, rejects extra or missing selected files, measures tool
identities, and builds native benchmark ELFs into private storage. It measures
the actual Rust toolchain binaries as well as Rustup/tool invocations. The
reviewed host-identity helper records ELF/library resolution, ROCm SMI provenance,
kernel identity, loaded GNU build ID, installed compressed/decompressed module
hashes, and matching installed module build ID. A separate query additionally
records the installed `amdgpu-dkms` package, its observed source-tree name, and
all 22 reviewed source-file hashes and canonical paths. Symlinked package roots,
ancestor directories, and source files are refused. Every value is observed before comparison
with the planner's reviewed profile. A version string alone is insufficient.

The host collector reuses the repository's pinned `r26-system-identity.py`;
ownership and idle admission reuse the pinned settled/hot helpers. No historical
receipt or external scratch directory is an input. The complete benchmark/helper
source closure is checked before and after execution. Host tools, Python and
system libraries, compiler internals/dependencies, the kernel, sysfs/procfs, and
the local filesystem remain trusted. This is measured local continuity, not
remote attestation or a proof connecting reviewed source to the loaded module.

Each benchmark has a new `--inspect-peer-pair` early-return path. HIP observes
device count, UUID, architecture and PCI address using HIP APIs. HSA observes
the agent roster, UUID, architecture, XNACK mode, PCI domain and BDF using HSA
APIs. KFD observes admitted device identities, currentness, topology incarnation,
and both directional engine IDs without creating memory sessions or copy queues.
These paths perform no benchmark allocations, copies, peer enabling or timed
workloads. Native runtime initialization itself is not claimed to be free of
driver-internal resource activity.

The parser joins those API observations with the pinned raw physical idle/SMI
observer. It never infers device identities from visibility-filter strings.
Visibility and other planner-listed execution variables are cleared before
backend-specific filters are installed. Both the first admission and each
trial's query must agree on physical UIDs/BDFs, KFD GPU IDs, routes, boot ID,
topology generation, and filtered device ordering. Each workload is preceded
by another physical idle observation after its query processes have exited.

Every complete, validated KFD/HSA/HIP query set is followed by one fixed two-second
settling interval before the next physical observation. This applies to opening,
per-trial and closing queries. The runner records the closed query-stage roster,
requested interval and monotonic start/finish times separately from timed
workloads. The wait is preconditioning, not an idle observation or reservation;
it never polls or retries until idle. Every subsequent physical observation still
requires exactly zero GPU/memory activity and the unchanged PID, memory and
identity checks. A nonzero observation rejects the attempt.

An earlier native attempt at signed commit
`29599a237901cb27ee90cd34d7f7a533dd90d424` compiled all three benchmarks and
passed opening host and API identity checks, but the first per-trial physical
observation saw GPU 1 at 3% GPU activity and rejected the run. Later reads were
zero and no selected-device PID was reported. These observations establish
neither external activity nor API initialization as the cause. No timed trial
ran, no performance result was accepted, and the exact private remote directory
was removed only after raw archive readback and fresh closure checks. The failed
attempt is retained as a rejection, not retroactively accepted by the settling
change. Observations remain nonexclusive sequential point checks.

## Closure And Cleanup

Each invocation uses the synchronous repository-owned recorder and its own new
process group. Raw stdout, stderr and receipt hashes are pinned as that invocation
returns. A malformed successful producer stops subsequent workloads. Both settled
and delayed postflights still run on failure, followed by independent closing
admission, host, tool, source, ELF, raw-artifact and namespace checks.
If failure occurs before native binaries exist, closing API admission is
explicitly skipped and rejected, not reported as successful observation.

The final census is restricted to the current recorder's exact attempted stage
roster. It checks fresh closure receipts and pinned raw bytes; it never probes or
signals historical receipt PIDs. Only the current synchronous recorder may stop
its own live child group. No global `/proc` scan or shared-directory cleanup is
performed. Scratch is removed only after all fresh groups have closure evidence;
an incomplete receipt preserves scratch for inspection. Signal handlers are
restored even if a closing evidence write fails.

The output retains benchmark ELFs and all receipts. Preserve or collect these
after the original runner handle reaches terminal status, then remove only this
exact private output directory. Success requires terminal exit zero, a complete
18-trial replay and `finished.json` with no failures. No partial receipt is an
accepted campaign.

## Light Controls

```sh
python3 -I -B benchmarks/runtime_gfx942/test_xgmi_peer_series_native.py
python3 -I -B benchmarks/runtime_gfx942/test_xgmi_peer_series_transport.py
```

These controls use synthetic data and a fake recorder. They do not compile or
invoke native code, contact a remote machine, or exercise a GPU. Native query
paths have source-level early-return/call-roster controls; compilation and live
query/benchmark qualification are separate required gates.
