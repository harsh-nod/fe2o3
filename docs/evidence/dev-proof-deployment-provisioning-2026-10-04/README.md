# Proof Deployment Provisioning Qualification

Source: `2bf96125582b9036b45b4f529a79530ae5b1fcd1`, based on
`09b15e9194c378f4df39ba0e23fc9754089b721a`, on
`codex/r65-runtime-drain-versions`. The source commit's SSH signature was verified.

## Results

- 25 custodian unit tests and seven doctests pass.
- Strict all-target Clippy with `--no-deps`, formatting and whitespace checks pass.
- The new static builder passes entrypoint, loader-independence, undefined-symbol,
  stack, invalid-entry and production ELF checks for manager, controller and provisioner.
- The isolated real-root campaign runs the actual static install CLI and passes
  independent-pin, credential-conflict, image/metadata, raw-SHA-versus-domain-hash
  rejection, atomic pair publication, production application-deployment opening,
  unchanged-object reinstallation and partial-existing rejection controls.
- The existing compiler-only V1 bundle shell contract passes; its inventory and
  semantics were not changed.

Seven default unit entries are ignored. The new static-image and root-install
tests were explicitly activated separately; this is not every ignored campaign.
The final root campaign completed in 5.99 seconds and its namespace exited.
Process inspection found no remaining campaign process. MI300X was not used.

## Limits And Next Work

The root campaign measures actual cached image bytes but uses inert analyzer
closure/toolchain/Verus facts and a test-key compiler client profile. It does not
qualify a successful non-root inspection, actual controller resource admission,
manager startup, genuine compiler receipt/current-record audit, completed proof,
GPU execution, performance or a new formal theorem.

Manager, controller and provisioner were freshly built. The coordinator and Worker
were existing cache inputs, not rebuilt here; all exact hashes are archived.
Complete the immutable final-path analyzer/Verus package, positively qualify
inspection and service-profile resource admission, then execute the ordinary
two-GPU pipeline. See [provisioning](../../runtime-proof-deployment-provisioning-v1.md)
and [critical path](../../runtime-multi-gpu-critical-path.md).

## Artifacts

`evidence.tar.gz` includes final and initial failure logs, exact source patch,
toolchain and image identities, ELF reports, signer verification, review/qualification
notes, reproduction commands and a verified `SHA256SUMS` manifest. It contains no
executables. Commands were executed individually, not as the reproduction script.

Initial failures include development test issues, a linker SIGBUS while disk space
was exhausted, an integration-test build overwriting checked static images, and a
cross-mount hardlink fixture. Their fixes and successful reruns are distinguished
in the archive. Only obsolete generated cache/test files in this owned worktree
were removed for space; unrelated files were preserved. Owned evidence scratch is
removed after publication.

Source patch SHA-256:
`ece5fd601dbd005efa90c2d2bf4fd419aac37c3610a4f5b2df6eb4c002b8af8c`

Archive SHA-256:
`d6d2079191f2f060b824f452f0774cd777e8a8b809bafe17b532169952dd7136`
