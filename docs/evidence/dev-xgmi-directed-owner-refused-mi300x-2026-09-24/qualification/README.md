# Directed Owner Witness CPU Qualification

Status: CPU-qualified development candidate. No native execution, formal
refinement, performance, A1/A2 or broader lane acceptance is claimed.

The [witness contract](../../runtime-directed-owner-witness-v1.md) describes the
exact four-copy dependency diamond, event-release ordering, complete-byte
oracle and explicit owner cleanup. New campaign/native/parser siblings retain
the existing pinned host-admission and ownership-controller helpers without
modifying historical source or evidence. Production runtime code is unchanged.

## Results

The final `cpu1` campaign passes on 3,902 unchanged source/build/harness inputs:

- GNU example: six passed, zero failed, ignored or filtered.
- musl example: six passed, zero failed, ignored or filtered.
- musl executable build: passes; compiled invalid CLI inputs reject before
  native initialization.
- Receipt parser: five Python tests, with the compiled CLI test executed.
- Campaign controls/collection: four Python tests.
- Native runner orchestration: five Python tests, including each endpoint and
  workload failure, parser failure plus secondary postflight failure, changed
  host/source/ELF identity and rehashed Boolean control substitution.
- Runtime formatting and all-feature/all-target strict Clippy: pass.
- Rust/Cargo before/after outputs: byte-identical.
- All twelve serial command receipts: status zero, owned process group absent.

The six Rust tests cover CLI validation, exact routes/ranges/byte accounting,
source/payload/guard boundary mutations, deadline behavior, gate disconnection
and primary workload-error preservation when cleanup also fails. The mocked
runner tests use the real archive/source checks, parser and fixed postflight
sequencer, but mock endpoint parsing, marker ownership and command execution.
They are orchestration evidence, not GPU, host-admission or process-cleanup
qualification. No complete runtime suite is rerun by this example-only packet;
the unchanged production parent retains its separately scoped
[async qualification](../dev-directed-async-peer-cpu-2026-09-24/README.md).

## Provenance

Private execution root:
`/home/harsh/.codex-tmp/fe2o3-directed-owner-20260924-mamEIcU0`.
The archive contains the 38 byte-exact `cpu1` records/snapshots plus the runner
and this scope note. Runner SHA-256:
`04e1194171681571a6d0fc36eb0c7b4ec02e6a84dd7602089d66413ccf282b8e`.

The runner uses the existing pinned owner-lifecycle process helper, captures
source membership and bytes before/after, and defines bounded commands and a
private target with two Cargo jobs and the pinned nightly-2026-04-03 toolchain.
Individual `run_owned` receipts record command, times, process group, status
and group absence; environment and timeout policy come from the runner, not
additional receipt fields. No native benchmark or proof command ran.

Raw GNU/musl stdout retains Cargo's trailing blank line byte-exactly. Diff
whitespace checks pass with only those two raw outputs excluded.

An earlier `focused1` development run also passed six example tests before the
test-name clarification and final harness additions; its raw logs remain in
the private execution root and are not substituted for the final campaign.
Independent source and harness reviewers found no blocking issue. A separate
signed-source native campaign and offline evidence replay remain required.
