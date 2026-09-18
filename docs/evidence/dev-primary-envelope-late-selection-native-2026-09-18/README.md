# Corrected Primary Envelope Native Campaign

**The native campaign was rejected under its unchanged strict observation
policy.** All three native test commands passed, with five successful harness
summaries (1 positive, 2 error parent/child, 2 panic parent/child). The panic
case's immediate post-observation was refused. Its later fixed observation
does not override that refusal, and no retry or policy relaxation is included.

| Case | Native command/transcript | Strict post-observations |
| --- | --- | --- |
| Ordinary typed-dispatch shutdown | Passed, one harness summary and exact output/profiler marker | Immediate and delayed passed |
| Error envelope | Passed, two parent/child summaries and exact retained-root terminal marker | Immediate and delayed passed |
| Panic envelope | Passed, two parent/child summaries, retained-root terminal marker and original panic-payload assertion | Immediate refused; fixed delayed passed |

The sole refusal was `sysfs-before-busy`: the first raw snapshot recorded 1%
GPU busy, followed by sysfs busy values 0%, 0% and SMI busy 0%. The complete PID
capture had no selected-GPU attachment; VRAM was 298,647,552 bytes, below the
512-MiB strict bound. Identity, captures and timing were complete. The observer
and outer campaign exits remain 1 in their original records. This packet does
not identify the cause of the 1% reading, infer an allocation/queue leak or
attribute it to foreign work or the runtime. No HIP-specific acceptance policy
is transferred to this campaign.

## Bound Inputs

- Signed source: `a0db73625c421e26922a1bb8cab4dec49012417c`.
- Musl harness SHA-256:
  `001b9abd5e257098da8a27091ca81d62601805d7ca5b4ae86f81d6e69317ade2`.
- CPU archive: `dev-primary-envelope-late-selection-cpu-2026-09-18`, manifest
  `fe10e9f138029f7374a3e07d7046ca6a7bd23ca3d5c60b8876b5096a082c04d9`.
- Unchanged final 5,553-entry source maps:
  `6eff59a1d19ae0e0a761b8a7013ff8a9a192a1f44f912baf2713fd1d2dda2ca0`.
- Corrected fixture SHA-256:
  `2b5adf15ec70a5c039c5a531efb6609386b126909a711fdf81e2a9808c1d7a4e`.
- Frozen 20-file payload manifest:
  `9f798dc90a24dc950beca265be80060f7588eff49a681dbd19e8fe8d27208201`.

The local exporter verified the signed commit with the recorded expected
principal/key fingerprint, compared every source-map entry against its Git
blob while ignoring only the historical `base` field, rehashed the qualified
ELF, and created checked mode-0700 staging. Raw binding/signature receipts and
six relevant source snapshots are retained. The CPU binary before/after maps
explicitly bind the selected musl logical path and digest to the sealed CPU
archive. Optional current-input checks are separate from historical receipts.

The fixture-only correction observes retained-primary selection at the actual
production release boundary instead of asserting it before shutdown's control
and resident-data cleanup. The [original rejected 8b campaign](../dev-r126-primary-envelope-native-2026-09-18/README.md)
remains unchanged with manifest
`fa18643ffa6ed6c67388fa61dea9358448d016b94e1fd133e6bce1136f755346`.
It failed before injection; this packet does not rewrite that history.

## Native Protocol

Exact test names, native workload, limits and observation policy are retained
in `raw/native/collected/PLAN.md`, `run.py` and `protocol.py`. Compared with the
previous runner/protocol, only commit/binary/CPU/payload identities and remote
path prefixes changed; the exporter also explicitly enforces private staging.
Original diffs and local CPU qualifications are under `raw/prepare` and
`raw/controller-review`. No production runtime code changed in this campaign.

On shared host `mi300x`, GPU 4 was selected by UID `0x54f88318ca05093d` and BDF
`0000:85:00.0`, with CPU set 48-95 and NUMA node 1. Fresh strict preflight was
required separately for each serial case, with launch within one second of
preflight completion. There was no exclusive reservation or continuous idle
guarantee. All three fresh preflights passed.

Each native command used timeout 180 seconds with TERM/KILL grace 5 seconds,
core size zero, 16-MiB file-size limit and the pinned NUMA placement. The outer
campaign used timeout 1200 seconds with KILL grace 15 seconds. T0 was recorded
after the test parent was reaped and its owned process group was absent.

| Case | Immediate actual start | Fixed delayed actual start |
| --- | --- | --- |
| Positive | T0+0.034745214 s, admitted | T0+20.031967765 s, admitted |
| Error | T0+0.035254400 s, admitted | T0+20.030989019 s, admitted |
| Panic | T0+0.033000800 s, **refused** | T0+20.031856163 s, admitted |

All nine complete observations are retained: eight admitted and one refused.
There was exactly one immediate and one fixed delayed observation per case,
with no poll-to-pass. Full parent/child transcripts preserve the native test
statuses, exact markers and the positive's full queue/dispatch/allocation
profiler history and three complete vecadd readbacks. Error/panic injections
are runtime-envelope faults after original-root installation; the child
intentionally retains its backend until process exit. They are not actual
KFD ioctl failures or evidence of successful destructive release of a retained
root. This is not full R126 closure, a kernel/driver theorem, HIP/HSA parity or
a performance comparison.

## Collection And Cleanup

All 67 remote files were collected and matched inventory digest
`1662b36ba72310c032e5385392568609b3e5f6ae6a618b2e8317571847ce0642`
before exact-owned removal of
`/tmp/fe2o3-r126-primary-a0db7362-20260918.mmfeMgC8`.
Cleanup and a separate absence command exited zero. All 15 recorded
native/observer/outer PIDs and groups were absent; no accessible same-UID
exe/cwd/fd/maps references remained. Unreadable same-UID `/proc` entries are
retained as explicit limitations; other-user and inaccessible-reference
absence is not claimed. The controller's final rejection status remains 1.

`retention.json` records original paths and byte identities. The 58,484,448-byte
runtime ELF alone is omitted from the committed packet; its original payload
and full collected-inventory identities remain. The portable checker cannot
rehash omitted bytes or independently reconstruct the build. No old archive,
remote shared directory or foreign process was changed during packaging.

## Portable Audit

During unsealed review:

```sh
python3 -B verify.py --allow-unsealed
python3 -B test_verify.py -v
```

After sealing, `python3 -B verify.py` additionally requires exact manifest
membership and byte integrity. The audit imports only the pinned pure parser
and never invokes SSH, Cargo, GPU commands or the ELF. Eight admitted endpoints
are checked through the unchanged frozen strict protocol. The refused endpoint
is independently replayed through the pinned observer decision over the exact
original capture records, with capture/time hooks supplied by exhausted local
iterators. No observation is normalized into an admitted one. Command bounds,
raw clock ordering and enclosing timestamps, fixed windows, native transcript
counts, complete collection and cleanup rosters are checked separately.

An audit pass means the **rejected historical campaign and its narrower test
passes are faithfully retained**, not that qualification succeeded. Optional
`--source-root PATH` checks all 5,553 files against this historical a0db cohort;
`--binary PATH` rehashes a supplied historical ELF. Those checks are separate
from portable integrity and are reported absent/false when not requested. A
future modified source or binary must not inherit this packet's identity.

Closed CPU-only audit receipts under `audit` retain the portable audit, all
16 archive calibration tests, four frozen protocol tests, four controller
wiring tests, format/lint checks and a separate rehash of the historical ELF.
All exited zero and their local process groups were absent. The relocated
portable audit also passed, and its exact temporary copy was removed with
local path absence recorded. These checks do not add native observations or
rehabilitate the rejected campaign.
