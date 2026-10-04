//! Fixed-storage lifetime census for the explicit pinned-Verus context policy.
//! Kernel events authenticate births and terminals; numeric PIDs alone do not.

use super::*;

type Result<T> = std::result::Result<T, RetainedFunctionalRefinementRuntimeErrorV1>;
const MAX_LIVE_CONTEXTS_V2: usize =
    GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV2.max_live();

#[derive(Clone, Copy, Debug)]
struct Context {
    group: i32,
    executed: bool,
    leader_terminal: bool,
}

#[cfg(test)]
#[path = "functional_refinement_solver_contexts_v2_tests.rs"]
mod tests;

pub(super) struct SolverContextsV2 {
    policy: GeneratedProofProcessPolicyV2,
    auxiliary_required: bool,
    auxiliary: Option<Context>,
    auxiliary_complete: bool,
    solvers: [Option<Context>; MAX_LIVE_CONTEXTS_V2],
    born: usize,
    executed: usize,
    completed: usize,
    failed: bool,
}

impl SolverContextsV2 {
    #[cfg(test)]
    pub(super) fn new(auxiliary_required: bool) -> Self {
        Self::with_policy(
            GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV2,
            auxiliary_required,
        )
    }

    pub(super) fn with_policy(
        policy: GeneratedProofProcessPolicyV2,
        auxiliary_required: bool,
    ) -> Self {
        Self {
            policy,
            auxiliary_required,
            auxiliary: None,
            auxiliary_complete: !auxiliary_required,
            solvers: [None; MAX_LIVE_CONTEXTS_V2],
            born: 0,
            executed: 0,
            completed: 0,
            failed: false,
        }
    }

    fn refuse<T>(&mut self, detail: &'static str) -> Result<T> {
        self.failed = true;
        Err(process_failure(detail))
    }

    pub(super) fn before_birth(&mut self, role: TraceeRole) -> Result<()> {
        if self.failed || role != TraceeRole::Verifier {
            return self.refuse("solver context birth is not from the authenticated verifier");
        }
        if !self.auxiliary_complete {
            if self.auxiliary.is_some() {
                return self.refuse(
                    "auxiliary verifier must reach its authenticated terminal before solver birth",
                );
            }
            return Ok(());
        }
        if self.born >= self.policy.max_total() {
            return self.refuse("pinned solver context total lifetime bound exceeded");
        }
        if self.solvers.iter().flatten().count() >= self.policy.max_live() {
            return self.refuse("pinned solver context live group bound exceeded");
        }
        Ok(())
    }

    pub(super) fn birth(&mut self, role: TraceeRole, group: i32) -> Result<()> {
        self.before_birth(role)?;
        if group <= 0
            || self.auxiliary.is_some_and(|c| c.group == group)
            || self.solvers.iter().flatten().any(|c| c.group == group)
        {
            return self.refuse("duplicate solver context birth identity");
        }
        let context = Context {
            group,
            executed: false,
            leader_terminal: false,
        };
        if !self.auxiliary_complete {
            self.auxiliary = Some(context);
        } else {
            let slot = self
                .solvers
                .iter_mut()
                .find(|slot| slot.is_none())
                .expect("bounded free solver slot");
            *slot = Some(context);
            self.born += 1;
        }
        Ok(())
    }

    pub(super) fn expected_exec(&mut self, group: i32) -> Result<TraceeRole> {
        if self.failed {
            return self.refuse("solver context retained a prior refusal");
        }
        if self
            .auxiliary
            .is_some_and(|c| c.group == group && !c.executed)
        {
            return Ok(TraceeRole::AuxiliaryVerifier);
        }
        if self
            .solvers
            .iter()
            .flatten()
            .any(|c| c.group == group && !c.executed)
        {
            return Ok(TraceeRole::Solver);
        }
        self.refuse("solver context exec lacks its exact pending birth")
    }

    pub(super) fn executed(&mut self, group: i32, role: TraceeRole) -> Result<()> {
        if self.expected_exec(group)? != role {
            return self.refuse("solver context executed a substituted role");
        }
        if role == TraceeRole::AuxiliaryVerifier {
            self.auxiliary
                .as_mut()
                .expect("checked auxiliary birth")
                .executed = true;
        } else {
            self.solvers
                .iter_mut()
                .flatten()
                .find(|c| c.group == group)
                .expect("checked solver birth")
                .executed = true;
            self.executed += 1;
            #[cfg(test)]
            PEAK_TEST_EXECUTED_SOLVER_GROUPS.set(
                PEAK_TEST_EXECUTED_SOLVER_GROUPS
                    .get()
                    .max(self.solvers.iter().flatten().filter(|c| c.executed).count()),
            );
        }
        Ok(())
    }

    fn terminal(
        &mut self,
        role: TraceeRole,
        group: i32,
        leader: bool,
        status: i32,
        group_remains: bool,
    ) -> Result<()> {
        if self.failed {
            return self.refuse("solver context retained a prior terminal refusal");
        }
        let slot = match role {
            TraceeRole::AuxiliaryVerifier if self.auxiliary.is_some_and(|c| c.group == group) => {
                &mut self.auxiliary
            }
            TraceeRole::Solver => match self
                .solvers
                .iter_mut()
                .find(|c| c.is_some_and(|c| c.group == group))
            {
                Some(slot) => slot,
                None => return self.refuse("solver terminal lacks an owned context group"),
            },
            _ => return self.refuse("terminal is not an authenticated context role"),
        };
        let context = slot.as_mut().expect("selected context");
        if !context.executed || (leader && context.leader_terminal) {
            return self
                .refuse("solver context terminated before exec or repeated a leader terminal");
        }
        if leader {
            if terminal_status(status) != (Some(0), None) {
                return self
                    .refuse("authenticated solver context leader did not exit successfully");
            }
            context.leader_terminal = true;
        }
        if !group_remains {
            if !context.leader_terminal {
                return self.refuse("solver context group retired without its leader terminal");
            }
            *slot = None;
            if role == TraceeRole::AuxiliaryVerifier {
                self.auxiliary_complete = true;
            } else {
                self.completed += 1;
            }
        }
        Ok(())
    }

    pub(super) fn retire_terminal(
        &mut self,
        tree: &mut Tracees,
        pid: i32,
        status: i32,
    ) -> Result<()> {
        let Some(task) = tree.get(&pid).copied() else {
            return self.refuse("context terminal names an unknown task");
        };
        if !task.exit_boundary.is_some_and(|exit| exit.matches(status)) {
            return self.refuse("context task skipped its authenticated exit checkpoint");
        }
        let task = match tree.remove_terminal(&pid) {
            Ok(task) => task,
            Err(error) => {
                self.failed = true;
                return Err(error);
            }
        };
        let remains = tree
            .values()
            .any(|other| other.thread_group == task.thread_group);
        self.terminal(task.role, task.thread_group, task.leader, status, remains)
    }

    pub(super) fn retire_queued(&mut self, tree: &mut Tracees) -> Result<()> {
        for pid in tree.pids() {
            let task = tree[&pid];
            if matches!(
                task.role,
                TraceeRole::Solver | TraceeRole::AuxiliaryVerifier
            ) && let Some(status) = task.queued_status.filter(|s| !stopped(*s))
            {
                tree.get_mut(&pid)
                    .expect("retained queued terminal")
                    .queued_status = None;
                self.retire_terminal(tree, pid, status)?;
            }
        }
        Ok(())
    }

    pub(super) fn finish(&mut self) -> Result<()> {
        if self.failed
            || !self.auxiliary_complete
            || self.auxiliary.is_some()
            || self.solvers.iter().any(Option::is_some)
            || self.born == 0
            || self.born != self.executed
            || self.executed != self.completed
            || (!self.auxiliary_required && !self.auxiliary_complete)
        {
            let detail = format!(
                "solver context census lacks complete successful authenticated terminals: prior_failed={} auxiliary_required={} auxiliary_complete={} auxiliary_pending={} born={} executed={} completed={} live={}",
                self.failed,
                self.auxiliary_required,
                self.auxiliary_complete,
                self.auxiliary.is_some(),
                self.born,
                self.executed,
                self.completed,
                self.solvers.iter().flatten().count(),
            );
            self.failed = true;
            return Err(process_failure(detail));
        }
        #[cfg(test)]
        FINISHED_TEST_SOLVER_CONTEXTS.set(Some((
            self.born,
            self.executed,
            self.completed,
            PEAK_TEST_EXECUTED_SOLVER_GROUPS.get(),
        )));
        Ok(())
    }
}
