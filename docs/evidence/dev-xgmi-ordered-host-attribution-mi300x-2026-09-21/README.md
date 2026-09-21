# Ordered XGMI Host Attribution

Development host-time attribution, not HIP/HSA parity, physical link bandwidth,
causal instrumentation overhead or formal executable refinement.

## Scope

Signed source `a7b8602dce0c5d38ae1425f6544392f03f55341b` ran on MI300X
GPUs 5 and 6, identified by PCI BDFs `0000:a6:00.0` / `0000:c6:00.0` and
unique IDs `b7baafd0fb173d8e` / `10a254ce4987e716`. The same feature-enabled
musl release ELF ran off/on/on/off diagnostic trials. Its SHA-256 was
`e4d077e494d4c2d8852fd27c488337b8fbce445606609f40da76f23fa93ae4f6` before
and after execution. Each direction used 65 descriptors totaling 65,536 useful
bytes, one prime, two warmups and ten samples. Byte/canary validation and explicit
teardown passed. This campaign exercises caller-driven Context ordered copies,
not the newly added owner-engine wrappers on hardware.

All 24 fresh before/settled/delayed endpoint observations passed. Settled checks
started at least two seconds after each trial; delayed checks started at least
20 seconds after the settled pair. These observations are not a reservation.
The private remote directory was collected, removed, and independently checked
for path/process absence. Other host work was not reset, killed or removed.

## Results

The two instrumented trials contain 52 diagnostic records, of which 40 are timed
samples. Prime/warmup records are excluded below. Mean backend host time was
**25.677782 ms**. Fractions are sums of phase time divided by summed backend time,
not averaged per-record percentages; nested phases must not be added again.
About 0.05% of backend time falls outside the seven phase spans below.

| Backend phase | Mean ms | Fraction |
| --- | ---: | ---: |
| Admission | 0.001459 | 0.0057% |
| Preparation | 0.000452 | 0.0018% |
| Opening currentness | 7.426756 | 28.9229% |
| 65 submissions | 5.368402 | 20.9068% |
| 65 waits | 5.327310 | 20.7468% |
| Closing currentness | 7.534553 | 29.3427% |
| Settlement | 0.006408 | 0.0250% |

Opening plus closing currentness accounts for 58.27%; submission plus wait
accounts for 41.65%. The two full topology discoveries nested inside currentness
average 14.386454 ms. Submission/wait phases include host currentness and queue
work; they do not isolate DMA engine execution time.

Facade p50 list latencies in milliseconds, directions 5-to-6 / 6-to-5:

| Trial | Direction 0 | Direction 1 |
| --- | ---: | ---: |
| 1-off | 25.178970 | 25.083122 |
| 2-on | 25.747175 | 25.702698 |
| 3-on | 25.512237 | 25.602973 |
| 4-off | 25.216566 | 25.077792 |

These four trials contain 80 ordinary timed list samples. Different endpoints,
source revisions and the absence of HIP/HSA trials prevent attributing differences
from older packets to the directory-allocation change or claiming matched parity.

## Replay

`python3 -I -B docs/evidence/dev-xgmi-ordered-host-attribution-mi300x-2026-09-21/verify.py`
authenticates the signed source and all 5,633 source-file digests, checks pinned
helper bytes before importing them, reconstructs exact local/remote commands and
environments, reparses raw workloads and endpoints, validates chronology/delays,
and requires successful cleanup and the archive seal. It does not contact GPUs
or execute any recorded benchmark/SSH command. The executable itself is identified
by recorded hashes, not included in this packet. Authenticity of the historical
receipts relies on the signed archive commit, not remote hardware attestation.

The archived source-bound CPU qualification passed 1,211 runtime tests on each of
GNU and musl, with 20 hardware-specific tests ignored per target, plus strict
Clippy, no-default-features, rustfmt and before/after source checks. Campaign
qualification additionally passed five Rust example tests and all five Python
suites. None of these results proves Rust/native or machine-code refinement.

`python3 -I -B docs/evidence/dev-xgmi-ordered-host-attribution-mi300x-2026-09-21/test_verify.py`
checks positive replay and rejection of altered source/build/mode/command,
timing/identity/roster, admission, cleanup and qualification evidence. Replays
require the signed checkpoint's helper bytes; changed helpers fail closed.
