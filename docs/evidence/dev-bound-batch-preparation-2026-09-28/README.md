# Transactional Bound-Batch Preparation

Qualified on 2026-09-29 UTC at signed source
`fdff00d15abc68b73a1cad905985522fbde59578`.
Implementation parent: `b416a79f99230836a8c52ec623c8560efc5d7204`.
This is a host preparation qualification, not an A1/A2 exit or HIP/HSA parity claim.

## Implementation

`bind_fixed_batch` now prepares every packet, slot and dispatch roster, completes
all boxed conversions, and constructs the retained batch before the first owner
write. The infallible commit changes only selected slot phases and the batch-ID
frontier. Previously the dispatch roster allocation/conversion followed slot
mutation. Selection now directly stores slot leases, removing the separate index
Vec and subsequent lease-copy pass. Selection and preparation remain linear;
Vec growth/shrinking means this is not a uniform allocator-call reduction claim.

Actual selection, generation validation, preparation and commit bodies are shared
with Verus. Actual AQL address/alignment checks, geometry getters, packet field
construction and boxed-batch admission are shared too. The equivalent guarded
alignment bit test has an exact power-of-two domain lemma and an independent CPU
comparison with the standard-library policy.

Without a valid-prepared premise, the complete binder proves exact first-error
behavior with unchanged logical owner state, or exact first-N Available leases,
generations, dispatch identities and packet fields with one checked ID advance.
Successful retention satisfies the existing bound-retention validator. Arbitrary
neighboring slots, generations, pins, ledger payload and owner metadata are
framed; no new zero-pin/nonzero-generation admission rule is introduced.

## Results

| Check | Result |
| --- | --- |
| Signed binding campaign | 30 stages, all 22 executable negatives rejected logically |
| Signed AQL campaign | 26 stages, all 18 executable negatives rejected logically |
| Complete new proof | 55 overlapping obligations; ten-file relocated closure |
| Source continuity | 6,242 unchanged hashes across both campaigns and CPU qualification |
| AQL unit/ABI | 47 passed |
| KFD CPU | 1,439 passed; 320 construction tests excluded |
| Runtime CPU | 1,828 passed; 30 hardware tests ignored |
| Doctests | 124 passed |
| Static checks | Strict Clippy, production build, workspace/included formatting passed |
| Affected proof roots | Cancellation 26, event binding 43, issuer 46, batch release 29, rollback 51 |
| Independent audit | Signed blobs, relocation/mutant hashes and selectors checked; all 98 recorded process groups absent |

Eight new KFD groups cover independent full-byte packet/publication-order oracles,
sparse high slots, genuine neighboring event/reader custody, late refusal and
competing error precedence, address overflow/zero/alignment, exhaustion, metadata
framing and first-N selection. Scoped CPU counters observe zero allocations from
the shared body's actual commit boundary through return at N=1,3,64,8192. This is
not an allocation-cost theorem or a latency measurement.

The five affected old roots were rerun as complete positive proofs at this source;
their historical mutation suites were not rerun. Their original trust profiles
are unchanged. The first signed preparation campaign is retained as rejected:
its short-roster mutant produced an additional proof-index recommendation. A
length assertion and bounds guard fixed the proof diagnostic, and the complete
campaign was rerun. The diagnostic classifier was not relaxed.

## Trust And Limits

The new root uses two explicit, hash-pinned standard-library contracts for
Vec-to-box contents and boxed-slice-to-array contents/cardinality, so it does not
use `--no-cheating`. Actual loop lengths establish conversion success. Installed
CPU and proof-toolchain std sources and notices are retained for review. Contracts
do not constrain allocation, addresses, cost, allocator failure/panic or termination.

Native address/code ownership, geometry-constructor reachability, serialization,
native publication/receipt truth, firmware/device memory ordering, complete
terminalization/unwind and lifecycle reachability remain outside this proof.
CPU ABI/capture tests do not turn those boundaries into formal hardware evidence.
No GPU or matched HIP/HSA performance work was run, and no speedup is claimed.

## Evidence And Reproduction

`raw.tar.gz` is 5,187,647 bytes and contains 1,089 manifested files plus the inner
manifest: the complete successful campaigns, stopped campaign,
development logs, CPU/proof regressions, signed source manifests, process records,
independent audit, standard-library review inputs and `COMMANDS.md`. The inner
`SHA256SUMS` checks extracted bytes; the outer manifest checks this README and
archive. A fresh extraction was verified before publication and the owned restore
directory was removed. No MI300X artifacts or processes were created.

Next: refine actual resource-derived template construction and dispatch-epoch
reservation, including Ordinary refusal classification, then compose that handoff
with this completion binder. Stored-field provenance is distinct from native
mapping authentication and publication authority. All broader milestone gates
remain open as recorded in the current runtime roadmap.
