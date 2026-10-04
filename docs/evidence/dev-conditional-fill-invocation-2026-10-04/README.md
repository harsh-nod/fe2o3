# Conditional Fill Multi-Device Invocation

Source: `0e8d6cb3fbdddbf4bd69593ff2fe35b2c969cd67`, based on
`12959df35ad0afe70ebbbf1b8edc9beaedd6f3e5`, on
`codex/r65-runtime-drain-versions`. The source commit's SSH signature was verified.

## Results

- Host library tests: 235 passed, 3 ignored.
- Host doctests: 61 passed, including 45 compile-fail cases.
- Strict Clippy passed for host all targets with warnings denied.
- Host tests checked with `hardware-test-hooks`; HIP discovery was disabled.
- Host formatting and git diff whitespace checks passed.

The new tests use actual macro-generated fill arguments and captured compiler /
Worker inputs. They cover N/G pairs 1/64, 64/64, 65/128 and 1024/1024, original
buffer continuity, exact credit cleanup, selected image/descriptor association,
coverage/profile rejection, shared authority lifetime and completion ordering.
The completion lifetime tests use an inert authority that rejects execution.

The implementation retains the same remote proof owner across independent device
invocations. It uses the existing multi-device async owner and mandatory native
conditional-fill constraint; it does not manufacture an unconditional executable.
The shared completion carrier retains authority through final decoder commitment.

## Limits And Next Gate

This is CPU qualification, not production admission or two-GPU execution. No
full-workspace suite, standalone UI suite, formal proof rerun, MI300X test or
HIP/HSA performance comparison was performed for this checkpoint. Three ignored
tests were not exercised. Feature-check success is not hardware validation.

The next gate is explicit Cargo/supervisor custodian routing and complete private
proof deployment, followed by a fresh selected-rustc compilation through the real
issuer and anchor, original publication, actual manager/controller proof and
production FD195 audit. Existing synthetic startup fixtures cannot satisfy it.
Then qualify real shared-artifact invocations and settled transfers through PUBLIC
XGMI in both directions on two free MI300X devices.

No shared-host files or services were touched. Unrelated local files and build
processes were preserved; owned evidence scratch is removed after publication.

## Artifacts

`evidence.tar.gz` contains the exact source patch, test logs, base/toolchain
identity, reproduction commands, qualification notes and a SHA-256 manifest.
The reproduction commands were executed individually, not through the script.

- Source patch SHA-256:
  `af73858876a49d4a165274610cb1aea7710afacd74477ce42c1eded67ec3f135`
- Archive SHA-256:
  `24be01fb4f615ac173d7d884813c70f43fd690cf45d9069b354587926cb9d8c7`
- Host test binary SHA-256:
  `3ca55044d7dfba10060f875f8980d2f306ec3fa3e3dd28b693a0983fd5dea775`
- Cargo.lock SHA-256:
  `042abac67822ac72c61f424adf84a8be67c694974f57784512405ff3879257a0`

See the [invocation design](../../runtime-conditional-fill-invocation-v1.md) and
[remaining multi-GPU work](../../runtime-multi-gpu-critical-path.md).
