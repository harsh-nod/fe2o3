//! Global generated receipts preserve the original child and never become peers.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::kfd_backend) struct MultiGeneratedSubmissionV1 {
    global: u64,
    shell: MultiGeneratedShellPlanV1,
    // None roots the entering attempt before the child can retain a receipt.
    local: Option<u64>,
}

impl MultiGeneratedSubmissionV1 {
    pub(super) fn shell_key(&self) -> u64 {
        self.shell.global.key
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    fn generated_submission_matches_v1(&self, id: u64, route: MultiGeneratedSubmissionV1) -> bool {
        let Some(local) = route.local else {
            return false;
        };
        let Some(child) = route.shell.scope.child else {
            return false;
        };
        id != 0
            && route.global == id
            && self.generated_shells.get(&route.shell.global.key) == Some(&route.shell)
            && self.validate_generated_shell_records_v1(&route.shell.global)
            && !self.submissions.contains_key(&id)
            && !self
                .submissions
                .values()
                .any(|submission| match submission {
                    RoutedSubmissionV1::Native { route, .. } => {
                        *route == RoutedHandleV1 { child, local }
                    }
                    RoutedSubmissionV1::DeferredCompute(root) => {
                        root.route == Some(RoutedHandleV1 { child, local })
                    }
                    RoutedSubmissionV1::CooperativeCopy(_) => false,
                })
            && self
                .generated_submissions
                .values()
                .filter(|other| {
                    other.shell.scope.child == Some(child) && other.local == Some(local)
                })
                .count()
                == 1
            && self.children[child].generated_submission_owner_matches_v1(local, &route.shell.local)
    }

    fn checked_generated_submission_v1(
        &mut self,
        id: u64,
        plan: Option<&GeneratedShellPlanV1>,
    ) -> Result<MultiGeneratedSubmissionV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let route = self
            .generated_submissions
            .get(&id)
            .copied()
            .filter(|route| plan.is_none_or(|plan| *plan == route.shell.global))
            .ok_or_else(|| {
                KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown generated submission or plan",
                )
            })?;
        if !self.generated_submission_matches_v1(id, route) {
            return Err(self.generated_submission_corruption_v1(route.shell.scope));
        }
        Ok(route)
    }

    fn generated_submission_corruption_v1(
        &mut self,
        scope: GeneratedAdoptionScopeV1,
    ) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        self.quarantine_generated_scope_v1(scope);
        RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Terminal,
            "generated submission routing lost original custody",
        ))
    }

    fn with_generated_issue_child_v1<R>(
        &mut self,
        scope: GeneratedAdoptionScopeV1,
        call: impl FnOnce(
            &mut KfdRuntimeBackendV1,
        ) -> Result<R, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<R, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.with_preparation_child_v1(scope.device, call)
        })) {
            Ok(result) => {
                if matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))) {
                    self.quarantine_generated_scope_v1(scope);
                }
                result
            }
            Err(payload) => self.resume_generated_scope_panic_v1(scope, payload),
        }
    }

    pub(crate) fn prepare_generated_issue_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.prepare_generated_issue_with_v1(plan, |child, local| {
            child.prepare_generated_issue_v1(local, roster)
        })
    }

    fn prepare_generated_issue_with_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        prepare: impl FnOnce(
            &mut KfdRuntimeBackendV1,
            &GeneratedShellPlanV1,
        ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let shell = self.checked_generated_route_v1(plan)?;
        self.require_submission_capacity_v1()?;
        if self
            .generated_submissions
            .values()
            .any(|route| route.shell.global.key == plan.key)
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "generated issue already retained",
            ));
        }
        let id = self.next_handle;
        let next = id.checked_add(1).filter(|_| id != 0).ok_or_else(|| {
            KfdRuntimeBackendV1::capacity("generated submission identities exhausted")
        })?;
        if !self.generated_global_handle_available_v1(id) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated submission identity collision",
            ));
        }
        self.generated_submissions
            .try_reserve(1)
            .map_err(|_| KfdRuntimeBackendV1::capacity("generated submission route capacity"))?;
        self.next_handle = next;
        assert!(
            self.generated_submissions
                .insert(
                    id,
                    MultiGeneratedSubmissionV1 {
                        global: id,
                        shell,
                        local: None
                    }
                )
                .is_none()
        );
        let result =
            self.with_generated_issue_child_v1(shell.scope, |child| prepare(child, &shell.local));
        match result {
            Ok(local) => {
                // Retain even a malformed returned handle before protocol checks.
                self.generated_submissions
                    .get_mut(&id)
                    .expect("rooted issue")
                    .local = Some(local);
                self.checked_generated_submission_v1(id, Some(plan))?;
                Ok(id)
            }
            Err(RuntimeBackendFailureV1::Rejected(error)) => {
                // A rejection can discard only a still-empty entering route.
                if !self.validate_generated_shell_records_v1(plan)
                    || self.generated_shells.get(&plan.key) != Some(&shell)
                    || !self.children[shell.scope.child.expect("validated child")]
                        .generated_shell_has_no_submission_v1(&shell.local)
                {
                    return Err(self.generated_submission_corruption_v1(shell.scope));
                }
                self.generated_submissions.remove(&id);
                Err(RuntimeBackendFailureV1::Rejected(error))
            }
            Err(_) => Err(self.generated_submission_corruption_v1(shell.scope)),
        }
    }

    pub(crate) fn advance_generated_issue_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        id: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let route = self.checked_generated_submission_v1(id, Some(plan))?;
        self.with_generated_issue_child_v1(route.shell.scope, |child| {
            child.advance_generated_issue_v1(
                &route.shell.local,
                route.local.expect("validated receipt"),
            )
        })
    }

    pub(crate) fn advance_generated_issue_preserving_rejection_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        id: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let route = self.checked_generated_submission_v1(id, Some(plan))?;
        let Some(local) = route.local else {
            return Err(self.generated_submission_corruption_v1(route.shell.scope));
        };
        self.with_generated_issue_child_v1(route.shell.scope, |child| {
            child.advance_generated_issue_preserving_rejection_v1(&route.shell.local, local)
        })
    }

    pub(crate) fn generated_rejected_publication_v1(
        &self,
        plan: &GeneratedShellPlanV1,
        id: u64,
    ) -> bool {
        !self.terminal
            && self.generated_submissions.get(&id).is_some_and(|route| {
                route.shell.global == *plan
                    && self.generated_submission_matches_v1(id, *route)
                    && route
                        .shell
                        .scope
                        .child
                        .zip(route.local)
                        .is_some_and(|(child, local)| {
                            self.children.get(child).is_some_and(|owner| {
                                owner.generated_rejected_publication_v1(&route.shell.local, local)
                            })
                        })
            })
    }

    pub(crate) fn retire_generated_rejected_data_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        id: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let route = self.checked_generated_submission_v1(id, Some(plan))?;
        let Some(local) = route.local else {
            return Err(self.generated_submission_corruption_v1(route.shell.scope));
        };
        self.with_generated_issue_child_v1(route.shell.scope, |child| {
            child.retire_generated_rejected_data_v1(&route.shell.local, local)
        })
    }

    pub(crate) fn progress_generated_submission_v1(
        &mut self,
        id: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let route = self.checked_generated_submission_v1(id, None)?;
        self.with_generated_issue_child_v1(route.shell.scope, |child| {
            child.progress_generated_submission_v1(route.local.expect("validated receipt"))
        })
    }

    pub(crate) fn generated_submission_can_retire_v1(&self, id: u64) -> bool {
        !self.terminal
            && self.generated_submissions.get(&id).is_some_and(|route| {
                self.generated_submission_matches_v1(id, *route)
                    && self.children[route.shell.scope.child.expect("validated child")]
                        .generated_submission_can_retire_v1(route.local.expect("validated receipt"))
            })
    }

    pub(crate) fn generated_data_unpublished_v1(
        &self,
        plan: &GeneratedShellPlanV1,
        submission: Option<u64>,
    ) -> bool {
        if self.require_live().is_err() || !self.validate_generated_shell_records_v1(plan) {
            return false;
        }
        let shell = self.generated_shells[&plan.key];
        let local = match submission {
            None => {
                if self.generated_submissions.values().any(|route| {
                    route.shell.global.key == plan.key
                        || (route.shell.scope.child == shell.scope.child
                            && route.shell.local.key == shell.local.key)
                }) {
                    return false;
                }
                None
            }
            Some(id) => {
                let Some(route) = self.generated_submissions.get(&id) else {
                    return false;
                };
                if route.shell != shell || !self.generated_submission_matches_v1(id, *route) {
                    return false;
                }
                route.local
            }
        };
        self.children[shell.scope.child.expect("validated child")]
            .generated_data_unpublished_v1(&shell.local, local)
    }

    pub(crate) fn read_generated_submission_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        id: u64,
        roster: &GeneratedHostRosterV1,
        destinations: &mut [(crate::Gfx942RuntimeBufferAccessV1, Vec<u8>)],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let route = self.checked_generated_submission_v1(id, Some(plan))?;
        self.with_generated_issue_child_v1(route.shell.scope, |child| {
            child.read_generated_submission_v1(
                &route.shell.local,
                route.local.expect("validated receipt"),
                roster,
                destinations,
            )
        })
    }

    pub(in crate::kfd_backend) fn poll_generated_submission_v1(
        &mut self,
        id: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.progress_generated_submission_v1(id)? {
            Err(KfdRuntimeBackendV1::quiescent_error(
                KfdRuntimeBackendErrorKindV1::Native,
                "generated dispatch physically complete without delivered result",
            ))
        } else {
            Ok(BackendPollV1::Pending)
        }
    }

    pub(in crate::kfd_backend) fn release_generated_submission_v1(
        &mut self,
        id: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        // Owner matching deliberately survives DATA retirement, unlike issue admission.
        let route = self.checked_generated_submission_v1(id, None)?;
        self.with_generated_issue_child_v1(route.shell.scope, |child| {
            child.release_generated_submission_v1(route.local.expect("validated receipt"))
        })?;
        self.generated_submissions.remove(&id);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
