# Changelog

All notable user-visible changes to fe2o3 are documented in this file.

The project follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and intends to use [Semantic Versioning](https://semver.org/) after the public
API stabilizes. Developer-preview versions may make breaking changes.

## [Unreleased]

### Added

- Community contribution, support, security, governance, and release policies.
- Source-only developer-preview release automation with checksums, an SPDX
  source SBOM, and GitHub artifact provenance.
- A KFD-first host doctor and a no-GPU quick start that exports an ordinary
  Rust kernel into an authority-free simulation bundle and executes it on the
  deterministic CPU simulator.
- Bounded logical-stream scheduling over two persistent native KFD compute
  lanes, with FIFO dependencies, cancellation, SDMA/XGMI custody, and exact
  completion indexing.
- Exact Runtime Worker V4 negotiation for execution capabilities, flush,
  asynchronous copy, cancellation, and deadline-bounded drain.
- Inspect-only Worker V3 semantic-to-machine refinement receipts and strict
  dispatch-timestamp evidence schemas that fail closed until trusted
  production producers are available.
- Executable and Verus models for the bounded logical scheduler, including
  pinned expected-negative proofs.

### Changed

- Canonical source, issues, and releases are under `harsh-nod/fe2o3`.
  `powderluv/fe2o3` is the code mirror and design-bound repository for the
  currently undeployed protected parity protocol.
- Workspace packages are explicitly excluded from crates.io publication while
  the publishable dependency closure is under review.
- Generated host bindings and the default host dependency closure now use the
  direct-KFD runtime; HIP/HSA lifecycle code is deprecated and available only
  through explicit qualification features.
- Same-shape direct-KFD runtime launches retain validated native storage,
  kernarg, code, and dispatch state across completion generations and defer
  device-written readback until the facade requires it.
- Runtime Worker V1 servers now require an explicit
  `RuntimeWorkerV1ImmediateProgressBackendV1` opt-in. This is a source-breaking
  change for custom V1 server backends and prevents deferred KFD work from
  being exposed through V1's immediate-progress contract.
- Developer-preview release admission now requires the exact release commit
  and tree to be reachable from the mirror's `main` branch before tagging and
  repeats actor, ref, tag, and mirror authorization at each remote write.
