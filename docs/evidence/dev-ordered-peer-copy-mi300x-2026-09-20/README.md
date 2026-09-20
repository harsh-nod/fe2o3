# Ordered Peer Copy Native Correctness

Signed implementation source: `a1301779d5536723cbbb5693823ce7f652d91129`.
One bounded native trial passed on MI300X GPUs 1 and 2 on 2026-09-20,
between 22:36:41 and 22:36:52 UTC (client command timestamps).
The shared host was not exclusively reserved. Fresh endpoint admission,
settled postflight, and delayed postflight each admitted both devices:

| GPU | PCI address | Unique ID |
| --- | --- | --- |
| 1 | `0000:26:00.0` | `0xab83d2ffef0d3cdf` |
| 2 | `0000:46:00.0` | `0xd2e26fef80cf5c33` |

The eight cases cover both directions with 1, 65, and 4096 segments using
whole-operation wait, plus four-segment poll/flush continuation. Each case
checks prepublication cancellation, byte-exact source and destination contents,
untouched canaries, and resource release. The 65- and 4096-segment cases also
exercise overlapping-write order.
The continuation cases also reject cancellation after publication. Explicit
context and native backend shutdown passed. These are ordinary-path tests,
not native fault injection or comprehensive hardware verification.

## Build And CPU Replay

The native ELF SHA-256 is
`7897faf9f4657e4204a988a48438ea6bb974d34a472399cdf46253c86803ddb1`.
It is the musl smoke example, with all features, dev optimization level 1,
debug assertions, and overflow checks enabled. `qualify.py` records a
post-trial build with an explicit environment and pinned nightly. The
resulting ELF was byte-identical to the payload hashed on the remote host
before and after execution. This reused the local Cargo cache; it was not
a clean-target or independently reproduced build.

The post-trial replay also records GNU and musl runtime library results:
each passed 1190 tests with 20 ignored hardware-specific tests. The musl
example passed two CPU tests, the focused R74 Rust model passed three,
and warnings-denied all-features Clippy passed. Source-diff checks before
and after replay matched the signed commit for Cargo/toolchain/configuration
and crate paths. These recorded checks are not a hermetic compilation proof.

## Replay And Scope

From the repository root:

```sh
python3 -I -B docs/evidence/dev-ordered-peer-copy-mi300x-2026-09-20/verify.py
python3 -I -B docs/evidence/dev-ordered-peer-copy-mi300x-2026-09-20/test_verify.py
```

The verifier checks the signed source, pinned helper dependency chain, exact
commands, all six raw endpoint transcripts, payload identity, exact success
roster, chronology, postflight delays, cleanup, CPU replay, and file hashes.
The checksum manifest checks consistency; authentication depends on the
signed Git commit containing this complete packet. It does not prove a
compiler-to-machine-code refinement or independently observe DMA effects.

The controller ran only in its private remote directory
`/home/harsh/fe2o3-ordered-peer-smoke-20260920.zET9ZKt2`. It checked for its
remaining native process, removed only its two known regular files, removed
that directory, and separately confirmed path absence. It did not reset
GPUs, stop foreign processes, or clean unrelated files. Endpoint checks are
sequential snapshots, not continuous monitoring or an exclusive reservation.

`controller.py` and `qualify.py` preserve the original one-shot execution
recipes and historical local paths. Do not rerun them in this sealed packet.
No HIP/HSA comparison, speedup, throughput result, or performance acceptance
is claimed here. The formal control-model scope and remaining integration
work are documented in [Ordered Peer Copy V1](../../runtime-ordered-peer-copy-v1.md).
