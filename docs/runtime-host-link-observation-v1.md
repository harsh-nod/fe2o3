# Qualification Host-Link Observation

This optional diagnostic supports the [two-GPU critical path](runtime-multi-gpu-critical-path.md).
It does not change runtime admission, establish GPU execution, or certify a
hermetic compiler environment. Dynamic-loader resolution and the trusted host
setup tools remain installed-host premises. Do not block the two-GPU smoke on
building a new general-purpose DSO packager.

## Boundary

The genuine application uses pinned Rust, its bundled LLD, and the installed GCC
driver. It does not use a BFD-only link route. Qualification copies that GCC
driver before binding a static observer over its canonical installed path in
the final private mount namespace. The ordinary Cargo source, configuration,
authority pins and static application checks are unchanged.

For each observed link the proxy:

1. Requires the selected `cc`, Rust `gcc-ld` directory, and `-fuse-ld=lld` route.
   It rejects response/specs, alternate output, map, and reproduce controls.
2. Runs the original command with its arguments, working directory and environment
   unchanged. It records the argv, working directory, PATH and loader search path.
3. Saves the original ELF, then repeats the link with only LLD's `--reproduce`
   option added. The original and replay ELF bytes must match.
4. Bounds both invocations and kills the child process group on timeout or poll
   failure. Cleanup polling is bounded; the harness's existing owned cgroup is
   the final cleanup boundary. Normal exit codes are preserved; signals become
   shell-style `128 + signal` failure codes.

Root postflight runs after a successful genuine campaign, in the same namespace
as the GCC replacement. It remeasures the installed input roster and checks all
captured archives without extracting or executing their contents. External CRTs,
archives, scripts and Rust target libraries must match their pre-link measurements.
Generated objects and version scripts are recognized individually from the
original invocation, not by an unrestricted temporary-directory exemption.

The final application must have the expected static CRT order, required archive
inputs and x86-64 ET_EXEC header. The genuine application runner independently
retains its complete `sealed_static_application_identity_v1` policy. The observer
does not replace that policy or produce runtime authority.

The archive describes an **output-equivalent replay**, not every transient input
occurrence of the first link. A freshly printed premise digest is a measurement,
not independent prior approval. Optional comparison against a separately retained
digest establishes exact equality of the measured roster, not a formal loader
resolution proof.

## Invocation

Build the observer with an installed Rust musl target:

```sh
rustc --edition=2024 --target x86_64-unknown-linux-musl -Copt-level=2 -Dwarnings \
  scripts/qualification-host-link-proxy.rs -o "$owned_directory/cc-proxy"
```

Add these inputs to the existing complete reviewed qualification environment:

```sh
FE2O3_GENUINE_HOST_LINK_PROXY=/absolute/path/to/cc-proxy
FE2O3_GENUINE_HOST_LINK_PROXY_SHA256=<independently-retained-sha256>
# Optional exact match against a previously qualified installed-host profile:
FE2O3_GENUINE_HOST_LINK_SHA256=<independently-retained-premise-sha256>
```

Both `genuine` and `genuine-two-gpu` accept the option; `resources` rejects it.
Omitting all three variables preserves the ordinary campaign. Do not set the
optional digest to an empty string. The copied proxy survives hidden-home
projection and is checked again before execution. Captures occupy a private
8 GiB tmpfs and disappear with the namespace; they are never installed globally.

Success emits `FE2O3_HOST_LINK_OBSERVATION_V1` only after the unchanged genuine
test and complete postflight pass. Failed application, incomplete capture,
unexpected input, changed ELF, bounds or postflight failure all fail qualification.
No application failure is converted into observation success. This lane adds
build-time diagnostic overhead and must not be used as a performance baseline.

## Focused Checks

```sh
python3 -E -s -B scripts/tests/qualification_host_link_test.py
python3 -E -s -B scripts/tests/qualification_host_link_shell_test.py
rustc --edition=2024 --target x86_64-unknown-linux-musl --test \
  scripts/qualification-host-link-proxy.rs -o "$owned_directory/proxy-tests"
"$owned_directory/proxy-tests"
```

The full genuine campaign is additionally required to qualify actual link roster,
namespace integration and admission. CPU fixtures and synthetic character nodes
do not qualify the two-GPU execution branch or HIP/HSA parity.

The optional live-proxy test requires an already prepared private namespace with
the pinned proxy at `FE2O3_TEST_HOST_LINK_PROXY`; otherwise that one test skips.
The [qualification evidence](evidence/dev-qualification-host-link-2026-10-04/README.md)
retains the complete namespace/probe invocation and successful genuine campaign.
