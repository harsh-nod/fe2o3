# Native Inherited Admission And Completion

Date: 2026-09-26. Implementation/test snapshot:
`239efbd959a6ad646962fc283ccf278ec2c1c923`.
This continues the [early-account checkpoint](conditional-early-account-20260926.md).
All [#272](https://github.com/harsh-nod/fe2o3/issues/272) milestones M0-M7
remain open. This is client integration, not protected backend activation,
machine refinement, GPU validation, or 47/47 completion.

## Implemented

Native V2 and V3 clients can now consume fixed inherited FD 195 on their
original borrowed resource account. Shared retention mechanics preserve V1
ordering: consume the canonical slot and retain one private CLOEXEC duplicate.
Native admission prepays 65,544 logical work units and 8,192 scratch bytes before
inspection. Quota failure still closes the reserved input without inspecting it;
failed input reservations remain caller-owned. No decoder fallback or alternate
descriptor/path injection was added.

The shared consuming `prepare_and_acquire` operation connects preparation,
publication, existing native receipt acquisition, and transport completion.
Publication runs only after preparation's account postchecks and a live original
deadline. The publisher prepays its subject and carries required owners forward.
The peer closes and the full carriage is reserved before completion runs.
Each callback uses the original account, preserves inherited storage floors,
and retains opaque charges on error/unwind. A damaged or replaced account is
not repaired or refunded. The existing receipt state machine is unchanged.

These callbacks do not manufacture compiler ownership or trust policy. The
normal backend still admits V1 custody and refuses conditional publication.
The later documentation-only change clarifies that each operation prepays its
own inputs; a newly published subject need not exist before peer admission.

## Validation

Pinned nightly-2026-04-03, locked/offline Cargo, one build job, 12 GiB VM ceiling,
20-minute command bounds, disabled GPU visibility, and fixed source per build.

| Check | Result |
| --- | --- |
| Focused native units, eight test threads | 30 passed; 0 failed/ignored; 5 filtered |
| Client doctests | 5 positive and 17 compile-fail passed |
| Client/issuer/broker/Cargo/backend all-target check | Passed; Cargo target kinds, not GPU targets |
| V2 and V3 inherited admission | Each: 12 subprocess cases passed, 5 failed; both parent matrices failed |
| Existing V1 inherited admission | Failed at socket validation with EPERM |
| Completion success/failure transcripts, both families | 4 failed at fixture receive-timeout setup with EPERM, before exchange |
| Changed-file formatting, whitespace, hygiene | Passed |

The five failed inherited cases per family are valid admission, exact work
budget, pipe, stream, and unconnected socket. All report socket-inspection EPERM.
The two ignored child roles are invoked explicitly by their bounded parent
matrices; they are not skipped qualification cases. Protected issuer/container
harnesses and GPU runs were not executed. This is not an all-tests-green result.

Independent review found two coverage gaps, now addressed in source: finish
error/unwind/floor-loss/account-replacement cases and a bounded service EOF
acknowledgment required inside finish. Exact packet counts detect retries.
Follow-up source review found no concrete defect. Those socket-dependent checks
still require execution in a permitted environment; their source presence is
not a passing runtime result.

Logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921`.

| Log | SHA-256 |
| --- | --- |
| conditional-inherited-completion-units-r2.log | `09850f6bebf114fd2dd6172f6ac6f2420014cf581501794a137c98a2911eaf71` |
| conditional-inherited-completion-docs-r2.log | `c4c579c8beb152b4bbd05b9f838260f661dde0424042450a9dbfaf56caa93698` |
| conditional-inherited-completion-integrated-check-r2.log | `589658011cf901c9aabe8865c1aabfb66a650e6c8f04fdf073a8c1c4e6d375dc` |
| conditional-inherited-admission-tests-r4.log | `ebb7658ce123c38789de152e6f32a3e1896c472ae45428570af36df4b16bd4d0` |
| conditional-inherited-completion-transcripts-r4.log | `32420142b954fb2ab9b902e75c7eb5baf7853750a97fece69dd661e9010f109b` |

## Remaining Integration

1. Pin native family/profile/launch/readiness independently in the parent and
   admit the genuine V3 policy and inherited peer before compiler collection.
   Keep their original-account borrow within the synchronous device phase;
   do not move account identity tokens across ended borrows or replace accounts.
2. Connect this flow to the existing production transaction. Consume genuine
   conditional Prepared ownership after postchecks, retain original proof and
   collector owners through invocation finish/revalidation, V5 publication,
   SubjectV3 acquisition, and transport. No refusal-to-success reinterpretation.
3. Complete parent policy/history intake, locked V5 recovery before mapping,
   exact occurrence preflight/consumption, and concrete machine refinement.
4. Execute protected validation and the full target-matched kernel matrix before
   changing the tutorial manifest/site or claiming 47/47 completion.

All three SSH aliases failed DNS in fresh checks. Normal fetches from both Git
remotes also failed DNS; concurrent remote main work remains unmerged locally.
No remote jobs or scratch directories were created, and no source worktrees or
reports were removed. Synchronization must use normal, non-forced Git updates.
