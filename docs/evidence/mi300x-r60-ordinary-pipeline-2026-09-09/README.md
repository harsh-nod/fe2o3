# R60 Ordered Pipeline Qualification and Performance

This evidence covers one fixed 64-launch ordinary vecadd batch on one MI300X.
It establishes bounded numerical execution, ordered publication and completion,
explicit cleanup, and workload-matched timings. It does **not** establish full
HIP/HSA parity, simultaneous kernel execution, copy-performance parity, general
device-language support, or a production Rust/native/machine refinement theorem.

## Sources and Contract

All runs used ROCm 7.2.4 on GPU 1 of `sharkmi300x-1`, `gfx942:xnack-`, unique ID
`ab83d2ffef0d3cdf`, PCI `0000:26:00.0`. Measurement used CPUs 0-47 and NUMA node
0; the queue observer used CPU 95. Other users' devices were not modified.

| Set | Signed source commit | Raw records |
| --- | --- | --- |
| Original baseline | `4f25e3043099318131fa61659400fb5902674f5d` | [baseline](raw/baseline/) |
| Data-cache retention | `63822487edb4cf784e272ec5b3dfbb95468d8652` | [cache-retention](raw/cache-retention/) |
| Data retention and progress-aware wait | `8fe4d0a92f68f0cd020ea2dfb49617f310a80680` | [progress-wait](raw/progress-wait/) |
| Data, control and progress-aware wait | `ec5864b94518dab504d7d5a6bb6706acd6b0be25` | [control-retention](raw/control-retention/) |

The [benchmark protocol](../../../benchmarks/runtime_gfx942/R60-PIPELINE.md)
defines the exact HSACO, 1,048,576 `f32` elements, three coherent HostVisible
4 MiB buffers, 64 ordered launches, 10 warmup batches and 30 measured batches.
Every batch resets the output and checks every output byte and both unchanged
inputs. Each set has two guarded qualifiers and three counterbalanced backend
triples: KFD/HSA/HIP, HSA/HIP/KFD and HIP/KFD/HSA. Profiling is disabled during
timing. Independent audits found unchanged benchmark, checker, guard and kernel
sources between these sets, and unchanged build-tool hashes.

Issue timing includes all implicit runtime setup. Explicit allocation APIs,
output reset and validation are outside timing. The original baseline evicts
native data mappings on each reset. Cache retention keeps compatible native
data only after a globally quiescent full HostVisible write; it still detaches
code/kernarg control, rebuilds that control in the next timed launch, and uses
checked native overwrites. Old native content descriptors are not replaced by
new host digests until overwrite/rebind succeeds. Partial writes, mixed memory
rosters and active compute retain conservative behavior.

The wait variant additionally resets fallback wait backoff when a poll settles
at least one logical compute completion reservation. Stalled polls retain the
existing backoff sequence, and progress never renews the absolute deadline.
Native SDMA and persistent wait routing is unchanged. Physical retirement
without logical settlement is not counted as progress.

The final source also retains attached code/kernarg control for a nonempty
all-HostVisible roster with no simultaneous detached owner. A native
phase/generation preflight checks that all epochs are recycled and excludes
persistent attachment. This is not a device-currentness probe. Dirty-data
reconciliation still precedes host-shadow mutation; actual currentness checks
remain in native reads, the next checked overwrite and publication. Descriptors
remain native facts, fresh launch admission is unchanged, and incompatible
recipes still detach/rebind. Release, unload and shutdown retain their existing
cleanup paths.

The KFD public qualified runtime, HIP module API and raw HSA AQL interface are
different API levels. Their issue API counts are respectively 128, 64 and 192;
all execute the same 64 launches. These timings do not isolate the KFD driver
or GPU kernel. Tail time also depends on work completed during issue.

## Results

Ranges below are across three independently computed process-slot percentiles,
in milliseconds. They are not confidence intervals or pooled percentiles.

| Set / backend | Median issue | Median tail | Median total | p95 total |
| --- | ---: | ---: | ---: | ---: |
| Original / KFD | 348.644737-349.855400 | 50.774839-50.807090 | 399.398985-400.628699 | 401.378015-402.478250 |
| Original / HIP | 0.349572-0.372457 | 15.849249-16.308034 | 16.197138-16.664998 | 16.686740-17.193468 |
| Original / HSA | 0.010436-0.010826 | 16.290018-16.342766 | 16.300644-16.353012 | 17.059658-17.301860 |
| Data retained / KFD | 155.104491-156.084847 | 49.776434-50.841496 | 205.578987-206.871700 | 206.723319-208.165043 |
| Data retained / HIP | 0.348792-0.367459 | 15.913450-16.008102 | 16.291165-16.368590 | 16.874657-17.229487 |
| Data retained / HSA | 0.010786-0.011166 | 16.150414-16.422732 | 16.161170-16.433598 | 16.994275-17.200263 |
| Progress-aware wait / KFD | 155.581637-157.114630 | 9.290685-9.468199 | 165.046531-166.588257 | 165.649779-167.209425 |
| Progress-aware wait / HIP | 0.340359-0.383202 | 16.066303-16.312000 | 16.403257-16.666118 | 17.114629-17.367325 |
| Progress-aware wait / HSA | 0.010736-0.011056 | 16.234405-16.567662 | 16.245041-16.578638 | 17.005396-17.315999 |
| Control retained / KFD | 1.735754-1.745267 | 9.060407-9.272734 | 10.807548-11.006905 | 10.823952-11.033615 |
| Control retained / HIP | 0.344244-0.383933 | 15.848108-16.186193 | 16.223859-16.529226 | 16.792800-17.093279 |
| Control retained / HSA | 0.010466-0.010796 | 16.276609-16.616186 | 16.287144-16.626822 | 17.244955-17.451104 |

Data retention improves same-slot KFD median total by 1.93066-1.94517x
(48.20-48.59% lower) and median issue by 2.23369-2.25561x. Tail cost remains
essentially unchanged. The original total gap was 23.97-24.73x HIP and
24.43-24.58x HSA; after data retention it remains 12.6085-12.6425x HIP and
12.5329-12.7206x HSA, **slower**. These sequential source sets were not
randomized against each other and do not establish a universal speedup.

The wait change further improves same-slot KFD median total by
1.23405-1.25288x versus data retention alone, and its tail interval by
5.29278-5.46934x. Relative to the original baseline, final median total improves
2.39811-2.42737x (58.30-58.80% lower). That variant still takes 9.90312-10.11312x HIP's
and 9.95959-10.25471x HSA's median total time. The shorter host tail observation
does not imply faster GPU kernels.

Finally, retaining compatible control improves same-slot KFD median timed total
36.2952-37.0693x versus the original source, and 15.0955-15.2714x versus the
wait variant. Matched HIP/KFD ratios are 1.47397-1.52941 and HSA/KFD ratios are
1.47972-1.53249: the measured KFD interval is now 32.16-34.62% lower than HIP
and 32.42-34.75% lower than HSA. KFD median issue alone still takes
4.521-5.070x HIP's and 160.778-166.756x HSA's time. This is a win for the fixed
workload's **issue-plus-tail interval**, not complete application throughput:
reset, validation, cold setup and teardown are excluded. No isolated GPU-kernel
speed, general parity or universal speedup follows from it.

Both qualifiers in each set passed with 209 profile events, zero drops, all
64 publications before the first observed completion, contiguous completion
order, zero native-binding cost for successors, byte-exact results and cleanup.
This excludes simultaneous-kernel-execution claims. All nine backend logs per
set passed the archived checker, and all three comparisons recomputed exactly.

## Integrity and Cleanup

Independent audit checked every original manifest entry, every archived source
file against Git, the SSH-signed source commit against the trusted signer, all
three binary hashes, and all 73 recorded commands per set. Commands match after
normalizing only private paths, source commits and run IDs. The runner used
private offline/locked release builds and cleared execution environments.

All 11 monitor seals per set passed with target reaping, absent process groups,
no foreign selected-device queues and no terminal selected-device queues.
Maximum observation gaps were 4,995-6,096 us for the baseline and 5,139-6,105 us
for data retention, 5,101-7,181 us for progress-aware wait, and 5,095-6,030 us for
control retention, below the
unchanged 10 ms limit at a requested 2 ms cadence.
All 23 topology records per set agree. The 22 telemetry edges per set reported
zero GPU utilization and consistent sampled clocks; power was 137-139 W and
138-140 W for the first two sets, 138-139 W for the wait variant and 137-138 W
for control retention. Boundary telemetry is not an in-phase clock guarantee.

The task-owned enclosing remote stages were removed after archiving. Separate
SSH path-absence checks succeeded; selected GPU 1 reported zero utilization and
allocated VRAM after each accepted set. The enclosing archive predates that
last deletion, so the runner seals alone are not evidence of enclosing-stage
removal. The cache-retention outer directory also retains post-run telemetry;
both the wait and control sets retain an explicit stage-absence PASS and idle
telemetry.

The compact raw sets retain the original `sha256.json` unchanged, all command
captures (including empty captures), source-file hashes, provenance, signed
commit, trusted signer, profiles, measurements, comparisons and outer logs.
Four artifacts named by each original manifest are intentionally omitted from
Git: `source.tar` and `binaries/{kfd,hip,hsa}`. Their hashes remain in the
original manifest and provenance. Complete archives are retained locally, not
published as downloadable release assets; the compact sets are not complete
binary/source archive reproductions. [raw/retained-files.sha256](raw/retained-files.sha256) separately seals
the files actually retained here and does not replace the original manifest.

| Set | Source archive SHA256 | Complete local evidence archive SHA256 |
| --- | --- | --- |
| Original | `440d372862ad5caa0179a2df30c839a6541d371b9664de4bd387e2f19f6d32d7` | `1e5b46d75df557cf43ff235b0a222e0ab6a3f779268a4c102f716dde068eec95` |
| Data retained | `630699e2ab28726a2659e2137905a45943e2310c71827c7b9ad1d79d0aed0f56` | `21a1adb050246a767ba360dc7123351293f5a6d8956ae33a71392800d9653591` |
| Progress-aware wait | `e06ba71dc0a2921309b296c90f0f3278e8d1b0813f188739633e4a33bf671fe5` | `1e3fd5c94de77e06034c53fa66cf2e869e1bcc338e951a67695a961545b83de4` |
| Control retained | `d2e922ba905ab1bee52b721e760e5b1251cd931ae1d06ec7a436c56c3c6b98d4` | `159a6b49569f3717e8fcb771431a806cd5546bb2abcf1aa73d580e0c2d6d7836` |

## Rejected Attempts

No timings from rejected sets are included above. The original baseline was
accepted only after the following failures were diagnosed or rerun. Their
[outer failure records](raw/rejected/) remain separate from accepted evidence.

| Attempt source | Failure | Resolution |
| --- | --- | --- |
| `748be096` | Ordinary HostVisible R/R/W incorrectly entered persistent N3 admission; abort-on-drop obscured the diagnostic | Restrict persistent admission and print errors before terminal teardown |
| `80f5f772` | Auxiliary queue teardown rejected a valid detached/discharged dispatch state | Admit the discharged state with explicit cleanup and regression tests |
| `df17e048` | False dropped-profiler event for a bootstrap queue without a logical lane | Suppress only that nonexistent logical-lane event; real recorder regression |
| `4abde484` | HSA kernarg API alignment differs from the ELF's natural argument alignment | Check the required 16-byte API/pool/pointer alignment separately from the 48-byte ABI layout |
| `4f25e304` | HIP phase monitor exceeded the 10 ms census-gap limit | Discard the whole set and rerun unchanged protocol; no threshold relaxation |

All failed task-owned stages were removed without terminating other users'
work. An output hash or a partial backend run before cleanup/guard acceptance
was never treated as an accepted benchmark.

## Remaining Work

See [host and model validation](HOST-VALIDATION.md) for passing affected gates,
the interrupted canonical attempt, corrected debug-fixture check and the
remaining repository-wide gates. A full canonical pass is not established.

General runtime parity and production machine refinement remain open. Native
failure injection for retained-data overwrite/rebind custody and release after
reset without relaunch, including eventual code/kernarg release failures and
auxiliary-lane cleanup after rejected relaunch, also remain coverage gaps; the accepted 40-batch loops
exercise successful repeated reset, reuse, completion, readback and teardown,
not those failure paths. Further optimizations require their own exact-source
guarded evidence and must preserve currentness, custody and deadline behavior.
