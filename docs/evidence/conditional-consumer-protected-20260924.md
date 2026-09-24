# Protected Conditional Consumer, 2026-09-24

This run tests frozen commit `dec40d0f41445da86fd9253d8aae97f21e0756e8`,
not the subsequent CPU bounds, byte-memory theorem, GPU guard, or
native publication changes. It grants no artifact or launch authority and
does not qualify a tutorial kernel or complete an issue #272 milestone.

## Results

- The installed protected runtime audit passed.
- All three real public-lease preflights passed: closure audit, positive Verus
  execution, and rejection of a false proof.
- The original auxiliary `reference-proof` fill source reached the actual
  consuming conditional formula execution. Its callback-scoped receipt did not
  escape. The following boundary rejected with `FE2O3-COND-FINALIZER-001`.
- The changed-store source was rejected by a Verus assertion failure, exit 1,
  with two verified obligations and one error. It was not a setup failure.
- The separate post-bind observation reported nine pending checks; it did not
  execute the aggregate proof or obtain finalization authority.

The original case compiled source for gfx942 on MI350 without exposing GPU
devices or performing a hardware launch. This is not default tutorial-manifest
selection, an executable memory-transition proof, or an ISA numerical proof.

## Binding

| Item | SHA-256 or identity |
| --- | --- |
| Frozen source content snapshot, 7543 files | `610597d313cfa6c458c54679e7b3d8d5004378a10aa39dc9278e830fe11ebd71` |
| Installed runner revision r6 | `067c8a51b76e7e9a0346b9b3a648b9b5cbc5c91b30d8df9cfdfd453bf0fdaefd` |
| Approved request | `600a59a36e1c1253ae92ae498bac3637920a52d983de68fd6caee5e8d88a3021` |
| Transferred input payload | `f0bd92147f5fa67b4140677ac812f042a26e54da9a549331ca3ef870ccece270` |
| Result archive | `6db0df7fffe9829e017344bdc51bcc98d23b58b8c1496b7d1a5d83aaa2ab73ce` |
| Inside report | `a461f97914dc8ac6fc20390c7766607ce5e6ce3b0bebc93117db20058b44c416` |
| Generated conditional source | `180d85eb8364aff5bb5086332424f1ee223114beb07f74a5179f9c03f11d9650` |
| Conditional statement | `99234333ee7457d69b56dda4044a9387c26fa6ba6e14a928249cfa8b3f494911` |
| Conditional execution | `d311862f7b98c60de9508ed7f3b592f572d561a211d9bd0f42d21cbb9e195b61` |
| Conditional receipt | `7d08f8d642d9f40bbd85bf8f8597c0a68152e669282633029b0382239d2614c5` |

Raw results are preserved in `conditional-consumer-protected-r1.tar.gz` and
`conditional-consumer-protected-r1.oOcqU3/`. Preparation and artifact provenance
are recorded in the `conditional-consumer-*-r1` guard reports and request.

## Isolation And Cleanup

The supervisor used private mount, PID, IPC, UTS, network and cgroup namespaces,
root-owned immutable input copies, a read-only protected runtime, no GPU nodes,
and a capability-free controller. UID 9661 is the existing SSH account, not a
new or unused host UID. The base is a normalized snapshot of the recorded
preexisting image, not the pinned 99-package qualification base. Consequently
the report explicitly withholds production qualification credit.

The terminal service exited successfully with status 0. Before removal, the
owned cgroup reported `populated 0` and `frozen 0`. The exact run, image snapshot,
bootstrap and incoming directories were removed; a subsequent bounded name
check found none. The exact service/slice unit query returned an empty list.
The never-started export container had already been removed. No shared runtime,
Docker image, volume, or unrelated process was removed or changed.

Owned scopes removed:

- `/var/tmp/fe2o3-native-proof-r1.61e012d19bfe8c61af4e667652606d4e`
- `/var/tmp/fe2o3-image-base-r1.a52c0953a99ec0ffc0545819ddb569c6`
- `/var/tmp/fe2o3-proof-bootstrap.OOZu72gl`
- `/tmp/fe2o3-proof-input.BmNYbAzh`
