# Fresh Strict Hot-Batch Campaign

The fresh campaign passes strict collection and record replay. The controlling
SSH session, all eighteen trials, all 134 remote commands, all 108 endpoint
observations and all nineteen local commands pass. The prior campaign's
SSH/controller rejection remains unchanged in its
[separate packet](../dev-xgmi-hot-batch-mi300x-2026-09-24/README.md).

The fresh attempt executes the exact signed five-file protocol at
`f8f7c46f584ce7dfa72e4e106d64b0f71d3f8e5e`, using benchmark source
`8dc128357ecd55e1ba4f2866eb075899481f9aa0`. It preserves the complete workload,
order, admission, postflight, collection and cleanup gates. The private local
run is `fe2o3-hot-batch-20260924-feYb12D3/native2`; `raw/native1` denotes the
first attempt retained in this new packet, not the prior rejected attempt.

`packet.py` authenticates and reuses the packaging/checking helpers published
at `c640f434c407384c1f9f6668c5b82c493b70a704`. Only their fixed evidence and local
build paths are rebound. The original signed protocol path and commit remain
unchanged. Exact protocol/qualification copies are retained here so that the
checker still verifies the original qualification and frozen protocol bytes.
The wrapper exposes no recovery option and invokes the original strict gate.
It also checks every local/remote receipt against its declared elapsed bound
plus the recorder's fifteen-second stop allowance. Successful replay explicitly
reports `campaign_accepted: true` and `performance_acceptance: false`: this is
workload characterization, not an A7 threshold pass.

## Matched Characterization

Each slot transfers one MiB. Each process measures thirty batches after ten
warmups and one untimed prime, in both directions. The table gives the minimum
and maximum of the four independent process/direction p50 summaries per cell;
samples and percentiles are not pooled. Values are **whole-batch microseconds**,
not individual-copy latency percentiles.

| Depth | KFD p50 us | HSA p50 us | HIP p50 us |
| --- | --- | --- | --- |
| 1 | 14,350.009-14,378.397 | 30.165-30.796 | 38.557-39.149 |
| 16 | 15,007.733-15,068.049 | 526.867-538.765 | 512.946-522.049 |
| 32 | 15,427.243-15,539.843 | 1,064.911-1,086.944 | 1,014.666-1,040.654 |

KFD effective useful-byte throughput is approximately 0.073 GB/s at depth one
and 2.159-2.175 GB/s at depth 32. At depth 32, HSA reports 30.870-31.509 GB/s
and HIP reports 32.244-33.069 GB/s. Larger batches amortize fixed KFD work but
do not establish copy-performance parity. The unchanged source was measured;
these results do not qualify the subsequent link-directory optimization.

KFD measures facade enqueue through aggregate close; HIP/HSA measure native
enqueue through observed completion. KFD uses ordered single SDMA; comparator
engine selection/concurrency is unknown. These are not isolated engine-bandwidth
measurements. Every slot's final source/destination payload and guards passed,
but final canaries do not witness every intermediate repeated transfer.

## Validation And Cleanup

The packet retains 488 campaign files plus the local cleanup receipt, including
all three executed ELF byte streams. Exact remote inventories match collection.
The marker-owned directory on MI300X was removed and independent path/process
absence passed. Only the exact local source, target and build archive were
removed, reclaiming 424,538,112 allocated bytes. No foreign process or file was
removed, and no GPU was reset.

`audit1` passes strict replay, 45 packet/wrapper tests and nine collection/cleanup
tests. These include continued rejection of the original campaign, signature
and source-association refusal, rehashed endpoint/result/receipt corruption,
native transport failure, incomplete phase transcripts and deadline overruns.
`audit2` repeats the same checks with this final report in its input bracket.
The helper source and both inherited test scripts are authenticated against
their signed commit before use and included in the audit bracket.

Authenticate the enclosing signed Git commit, then replay in the original
checkout with its pinned signer file and cleanup paths:

```sh
python3 -I -B docs/evidence/dev-xgmi-hot-batch-strict-mi300x-2026-09-24/packet.py verify
```

Shared-host observations are sequential point checks, not continuous monitoring
or an exclusive reservation. No general parity, engine concurrency, native fault
recovery, formal correspondence, aggregate memory bound or A1/A2/A7 acceptance
is implied. The next performance comparison must freshly qualify any candidate
source against a matched baseline without weakening whole-host currentness.
