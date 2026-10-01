# Catalog regeneration performance — 2026-10-01

The complete catalog regeneration/check workload met its adopted targets on mi350.
This qualifies the catalog row only; M0 and the other open milestones remain open.

| Measurement | Observed | Target |
| --- | ---: | ---: |
| Measured p50 wall | 0.970702 s | — |
| Measured p95 wall | 0.981239 s | ≤30 s |
| Maximum measured wall | 0.986144 s | — |
| Maximum final child RSS, all 35 runs | 140.133 MiB | ≤512 MiB |

The [machine-readable report](evidence/catalog-performance-20261001.json) retains
all 35 samples, output checks, input hashes and exact qualification receipt hashes.
Five initial runs were calibration; all 30 subsequent runs were measured. No
samples failed, were replaced, or were removed as outliers. Nearest-rank p95
is rank 29 of the 30 measured values.

## Workload and clock

Compiler revision: `64265def21114808561452528644163182cd6c87`.
Each fresh process ran the unmodified generator directly:

```sh
python3.12 -I -S -B scripts/amd-isa-catalog-v1.py \
  --archive AMD_GPU_MR_ISA_XML_2026_08_06.zip --check
```

This includes Python startup, archive/manifest/coverage processing, both
architectures, generation and comparison of all 12 catalog outputs, and normal
process exit. Every run returned the exact expected one-line check result, empty
stderr and exit 0. Repository inputs and selected runtime files were unchanged
before and after the campaign.

Wall time starts immediately before process creation and ends when the parent
reaps that child with `wait4`. Polling observation delay is included; post-reap
pipe drain is separately retained and excluded. RSS is the final Linux
`ru_maxrss` value for that child, not a checkpoint or process-family aggregate.
The generator starts no child processes.

## Applicability

This is a fresh-process, OS-cache-uncontrolled campaign. It does not measure
a warmed compiler session, authoring actions, source recipes, or UI interactions.
Isolated/no-site startup excludes machine-local Python startup hooks; selected
standard-library source/cache and native mapped files are pinned, without
claiming every possible dynamic import has been enumerated.

The original 30-second and 512-MiB targets are unchanged. Nineteen campaign
controls passed before execution, including real wait4, timeout/overflow,
quantile, calibration, retention and deadline controls. All 73 campaign artifacts
were retained under the 1-MiB output bound. This report adds no compilation,
source-authentication, verification, resume or GPU authority.
