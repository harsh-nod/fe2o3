# Contributing to fe2o3

fe2o3 is a developer preview. Interfaces, evidence formats, and hardware
coverage can change between preview releases. Contributions that preserve the
project's fail-closed contracts and improve the documented user path are
welcome.

## Before opening a pull request

1. Search the [canonical issue tracker](https://github.com/harsh-nod/fe2o3/issues).
2. For a behavior change or substantial design, open an issue before writing
   code. Maintainers may ask for a short design note for changes that cross
   compiler, runtime, or evidence boundaries.
3. Keep a pull request focused. Do not mix formatting, generated evidence, or
   unrelated refactors with the intended change.
4. Never include credentials, private traces, proprietary kernels, or device
   identifiers in issues, tests, or fixtures.

The `powderluv/fe2o3` repository is the code mirror and the protected parity
protocol is repository-bound to it by design. That protocol is not deployed:
both repositories are currently user-owned, GitHub merge queues are unavailable
there, and no qualified offline receipt-import route exists. Open source
changes, issues, pull requests, and releases against `harsh-nod/fe2o3` so
community work has one canonical history. Do not change the parity-status
ledger; generic CI rejects such changes until an organization-owned authority
route is deployed and qualified.

## Development environment

Use the toolchain pinned in `rust-toolchain.toml`. Generic checks do not need a
GPU. Hardware tests require an explicitly admitted AMD GPU and must not be run
on shared hardware without authorization.

Run the fork-safe preflight commands before submitting:

```bash
cargo fmt --all -- --check
bash scripts/ci-local.sh standalone-locks
bash scripts/tests/quickstart.sh
scripts/quickstart.sh source-check examples/vecadd/Cargo.toml
```

GitHub runs the same bounded path for pull requests without secrets or GPU
access. For changes with a wider compiler or runtime impact, also run the
repository's generic qualification entry point:

```bash
scripts/ci-local.sh generic-core
```

Use `scripts/ci-local.sh check` or `cargo fe2o3 check` for packages that contain
namespace-free `#[kernel(typed)]` functions. Raw Cargo does not provide the
compiler-owned binding for those packages and is not the supported workspace
check path.

See `docs/testing.md` for test tiers and hardware gates. A pull request should
state exactly what was run and what was not run. Do not describe synthetic,
simulated, compile-only, or GPU-less results as hardware execution.

## Engineering expectations

- Generalize behavior across kernel types. Avoid kernel-specific production
  paths unless the issue explicitly defines a bounded fixture.
- Preserve typed contracts and deterministic diagnostics at trust boundaries.
- Fail closed when identity, authorization, completeness, or device admission
  cannot be established.
- Keep the production runtime on the direct-KFD path. HIP, HSA, ROCgdb, and
  rocprof integration may be optional tooling, but must not silently become
  runtime authority.
- Add focused tests for behavior changes and adversarial tests for parsers,
  artifact publication, process supervision, and authority boundaries.
- Update public documentation when a command, supported target, artifact
  format, or limitation changes.
- Do not commit build outputs, captured user workloads, or large trace files.

## Pull requests

Complete the pull request template. Link an issue, describe user-visible and
contract changes, list validation, and call out security, compatibility,
hardware, and evidence implications. Maintainers may require CODEOWNERS review
or hardware qualification before merge.

All commits must include a [Developer Certificate of Origin](https://developercertificate.org/)
sign-off:

```text
Signed-off-by: Your Name <you@example.com>
```

Add it with `git commit -s`. The sign-off certifies that you have the right to
submit the contribution under the repository's licenses. fe2o3 does not
currently require a separate contributor license agreement. GitHub-generated
Dependabot commits use the bot's `support@github.com` sign-off; preflight admits
that form only after the GitHub API verifies the exact Dependabot author,
`web-flow` committer, and GitHub signature. Merge commits are not exempt.

This requirement applies to commits proposed after this policy takes effect.
Historical repository commits are audited separately and are not treated as
licensed or release-eligible merely because current pull requests pass DCO.
The first developer-preview release remains blocked until the historical
licensing review tracked in issue #267 is complete.

## Licensing

Unless a file says otherwise, contributions are accepted under either the
Apache License 2.0 or the MIT License, at the user's option. See
`LICENSE-APACHE` and `LICENSE-MIT`.

By participating, you agree to follow `CODE_OF_CONDUCT.md`.
