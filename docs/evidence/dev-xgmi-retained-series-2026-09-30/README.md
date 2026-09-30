# Retained XGMI Native Comparison

Signed candidate `d5cfd87795f756d3a72129a139cee7a3f6fd0bee` completes all 18
planned KFD/HSA/HIP native trials on MI300X GPUs 1 and 2. Actual builds, physical
and API identity checks, settled/delayed postflights, and closing host/tool/source/
ELF checks pass. Independent local replay joins every workload command, visibility
setting and raw receipt. All 253 native groups and six transport groups close.
The exact marked remote directory is removed after readback, with a separate
absence check; no other user's files or processes are changed.

This is one nonexclusive campaign with sequential idle observations, not a GPU
reservation. Each trial uses 1 MiB copies, two warmups and ten samples per
direction. At each depth, order is KFD, HSA, HIP, HIP, HSA, KFD. Forward means
GPU 1 to GPU 2; reverse means GPU 2 to GPU 1. Both selected devices are gfx942.

## Observed Latency

Values below are the arithmetic mean of the two invocation p50 **batch**
latencies, in microseconds. They are not pooled-sample medians or confidence
intervals. A ratio above one means KFD is slower.

| Depth | Direction | KFD us | HSA us | HIP us | KFD/HSA | KFD/HIP |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | Forward | 40.736 | 33.385 | 38.277 | 1.220 | 1.064 |
| 1 | Reverse | 40.656 | 33.436 | 37.977 | 1.216 | 1.071 |
| 16 | Forward | 674.617 | 530.887 | 485.920 | 1.271 | 1.388 |
| 16 | Reverse | 674.101 | 531.002 | 490.947 | 1.269 | 1.373 |
| 32 | Forward | 1168.195 | 1065.305 | 961.811 | 1.097 | 1.215 |
| 32 | Reverse | 1168.645 | 1065.145 | 982.201 | 1.097 | 1.190 |

KFD is slower in every measured cell: 9.7-27.1% versus HSA and 6.4-38.8% versus
HIP. Its mean directional scope entry is 6.505-6.688 ms and scope finish is
6.320-6.680 us, both excluded from samples. Operational fences remain inside
samples. Fixed query-settling waits are also outside samples and are not idle
admission: every following physical check still requires zero activity.

This measures the native retained-series API, not the runtime Context facade.
Comparator copy engines are runtime-selected and unknown. The replay establishes
input/receipt consistency; it grants no performance threshold acceptance, broad
HIP/HSA parity, engine matching, protected launch authority, or machine-code
refinement. The earlier ordinary-mode timings are not a matched before/after
comparison with this campaign. A7 remains open.

## Reproduction And Evidence

`raw.tar.gz` is the original collected native archive, including benchmark ELFs,
raw command receipts and observations. SHA-256:
`b911dc0247e2673b18745f4a427ad6020c9794497ed1f47043b0613f415e02e9`.
`transport.tar.xz` retains the local transport receipts, owner/source binding,
accepted replay and cleanup/absence evidence. Its pull stdout is the same raw
archive, not a second execution. Both archives are checked against the retained
original bytes on readback.
Transport archive SHA-256:
`e5f007fb70cfceaffc81d7d3b6c5ea01c0e5f044874e2435a8a5e63a1ad1b208`;
all 99 members (74 files) match original bytes and modes.

`candidate.bundle` supplies the exact signed candidate and its five-commit
implementation history. It requires baseline
`74ed2c0623f56548b7f3d031b72b9adcae1e499a` and that baseline's object history;
it is not a complete clone. From a checkout containing that baseline:

```sh
git bundle verify /path/to/candidate.bundle
git fetch /path/to/candidate.bundle HEAD
git -c gpg.ssh.allowedSignersFile=/path/to/trusted-public-signers verify-commit FETCH_HEAD
```

Bundle HEAD must equal the candidate above. Establish signer trust independently:
`harmenon@amd.com`, fingerprint
`SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`. No private key is included.
Bundle SHA-256:
`b189bf12714e902ebf14f1da0cafedad30989d91ae45fcdc09e22e7f4032303a`.
The [public harness instructions](../../runtime-xgmi-retained-series-native-v1.md)
describe fresh execution. Toolchain, ROCm, reviewed host profile and idle devices
are separately required; this packet is not a hermetic rebuild or attestation.

Native replay-file SHA-256:
`0e60def96b8595a1a2e14666bf423774712a6d87cb77fcb1b6df4e4e8d9a1a90`.
Native census SHA-256:
`50b054752ca9ffce9ee12367befe45df2a5f5820ae19cac779daaa8c19d8a7e1`.
Transport-finished SHA-256:
`5624a21f1f7f7b8a58bdda161d381c1a5ea0afe2646c680f6df2c22dbf0df30a`.

Two earlier native attempts remain rejected: a malformed transferred checkout
stopped before compilation, and a later transient 3% GPU-busy observation stopped
before any timed workload. Their separate local histories are retained, including
recovery and exact-root cleanup. Neither is retroactively accepted here.
