# SCALE-3 Matched Measurement Protocol V1

This is `SCALE-3-PROTO`: a precommitted measurement format and consistency
checker. It contains no benchmark producer, captured measurements, performance
result, device timestamp producer, or new runtime execution authority.

## Trust Boundary

`check-scale3-protocol.py` requires an externally pinned SHA-256 of the exact
plan bytes. It checks one separate correctness capture and one timing capture
against that plan. The timing header additionally binds the exact correctness
capture bytes. Duplicate JSON keys, unexpected fields and unknown schema
versions reject.

Hashes bind supplied bytes; they do not authenticate who produced them or prove
that declared artifacts, devices, native counters or measurements were used.
Internally consistent fabricated input can pass this parser. Every successful
result therefore says `consistency_only: true`,
`hardware_authenticity: not-established`, `native_budget_closure: not-established`,
`performance_claim: false` and
`physical_overlap: unmeasured`. Unit-test captures are fabricated in memory
solely to test this boundary and are not benchmark evidence.

A future, separately reviewed signed runner must authenticate source, the plan,
producer binaries/toolchains and artifact closures; establish topology,
placement, idle-device and queue-census premises; guard bounded processes; and
audit process/native-resource/staging cleanup. HIP and HSA are isolated
benchmark oracles, never fallback paths in the KFD production runtime. Existing
R66 correctness captures do not satisfy this new capture schema.

## Plan

The single JSONL record has schema `fe2o3.scale3-matched-plan.v1`. The validator
defines its complete field roster and fixed `SEMANTICS` values.

| Field | Contract |
|---|---|
| `campaign_id`, `source_commit`, `host_boot_id` | Exact campaign/source identity and common host monotonic-clock domain |
| `gpu` | Exact unique ID, PCI BDF, KFD GPU ID, `gfx942:xnack-`, topology hash, NUMA node and sorted CPU roster |
| `producers` | Exactly KFD/HSA/HIP; each binds its host binary, toolchain closure and runtime version |
| `workload` | Common kernel artifact, ABI/effects/oracle hashes, argument-template hash, symbol, grid/workgroup and kernarg size |
| `workload.buffers` | Full requested sizes and initial/expected hashes for every explicitly allocated buffer, including reset/readback buffers and requested padding |
| `kernel_bindings`, `copy` | One compute invocation plus one directional H2D/D2H request; whole-allocation-disjoint descriptors; exact offsets and payload bytes |
| `policy` | 3/6/9/12 rounds, 1..4 warmups and 2..16 measured samples per backend block; fixed cyclic rotation, publication order and phase deadline |
| `resource_limits` | Requested-allocation and process-RSS ceilings; optional exact native-residency/slot ceilings |

Descriptors and declared disjointness are descriptive plan data, not native
storage identities or compiler-owned admission. The independent workload gate
must establish execution admissibility. The argument template binds every
scalar value and represents device-address arguments by buffer/offset
placeholders; it must not hash backend-specific pointer bytes. Grid and workgroup
dimensions are work-item counts, not HIP block counts. Each grid contains whole workgroups;
the initial format deliberately excludes peer, collective, multi-kernel and
general auxiliary-compute workloads.

## Captures

Both captures use `fe2o3.scale3-matched-capture.v1`, distinct capture IDs and
matching plan/campaign/source/host fields. Capture, invocation and sample IDs
cannot be reused across either file. No correctness record accepts timing
fields, and no timing file can be substituted for the correctness file.

The correctness file contains a header, exactly one KFD/HSA/HIP validation in
that order, and a completion record. Each validation repeats exact GPU and
producer identity, binds the canonical workload descriptor digest, reports all
initial and output buffer hashes, observes both logical operations complete,
and requires complete cleanup. Full requested-buffer hashes include all
declared padding, not unexposed native allocation padding.

The timing file is globally ordered: header; each block's setup, warmup/sample
iterations and cleanup; final completion. Round zero runs KFD/HSA/HIP, round one
HSA/HIP/KFD, round two HIP/KFD/HSA, then repeats. Every backend occupies every
position equally. Each block uses a fresh isolated producer process and one
reused Context/module/buffer set with two independent streams, one compute and
one directional copy, with no cross-dependency. Host buffers are pinned,
CPU-coherent and host-visible; pageable staging and managed/peer device-memory
fallback are excluded. Missing, extra, duplicate, reordered or
relabelled iterations reject; the checker never fills gaps or accepts supplied
percentiles or speedups. The fixed failure policy aborts the entire campaign;
retries and sample replacement are outside this profile.

Each iteration records five cost categories: reset, submission, transfer, wait,
validation. Submission and transfer follow the precommitted publication order.
Every category contains monotonic host wall and process-CPU start/end ticks;
adjacent category boundaries coincide. The producer must retain observations
and defer logging so no unclassified interval is hidden between categories.
Process CPU includes runtime worker threads, not just the invoking thread.

The checker recomputes aggregate wall/CPU costs from the raw boundaries. The
measured interval starts at the first enqueue and ends after both logical
operations are observed complete. Reset, setup, validation/readback and cleanup
costs remain separately recorded and are excluded from that aggregate. Setup
does not include process spawn. Transfer is host enqueue cost, **not** device
copy duration. Every timed iteration still requires full-buffer validation
outside the measured interval. Two logical operations do not imply two packets,
two native slots, unfinished GPU work or physical overlap.

The precommitted summary convention is nearest-rank p50/p95/p99 over measured
samples only. Summed measured time is the denominator for operation/payload
throughput; this is neither campaign-wall throughput nor device-copy bandwidth.
No summary or ratio is produced by this checker.

## Resource Observations

Each completed block reports requested-allocation bytes and process peak RSS.
Requested bytes must equal the explicit buffer roster, not padded backing or
allocator/control/executable residency. RSS uses the fresh process high-water
measurement and is not a substitute for GPU residency.

Exact resident backing and simultaneous native-published slot peaks may be
`{ "value": null, "basis": "unavailable" }` only if the plan does not impose
their ceiling. Unknown values are never converted to zero. A finite native
ceiling requires a corresponding exact observation below it. Native slot
observations count actual retained native packet slots across workload-owned
compute/copy queues, not queued or accepted logical requests. These selected
counters are not the complete MEM-5 resource inventory; a successful check
cannot establish full native-budget closure.

## Invocation And Bounds

```sh
python3 benchmarks/runtime_gfx942/check-scale3-protocol.py \
  --plan PLAN.jsonl --plan-sha256 EXTERNALLY_PINNED_SHA256 \
  --correctness CORRECTNESS.jsonl --timing TIMING.jsonl
```

Each file read is bounded to 2 MiB plus one sentinel byte. Before JSON parsing,
the checker enforces at most 1,024 LF-terminated records and 16 KiB per record,
and rejects invalid UTF-8, alternate JSON encodings, and every floating-point
or nonfinite number. The maximum admitted schedule
contains 794 records. Parser success does not authorize running a workload.

Remaining gates are signed-runner implementation and hardware provenance,
SCALE-1 workload qualification, relevant SCALE-2/native-resource qualification,
and separately reviewed compatible device-timeline evidence for any future
physical-overlap claim. No such evidence or result is supplied here.
