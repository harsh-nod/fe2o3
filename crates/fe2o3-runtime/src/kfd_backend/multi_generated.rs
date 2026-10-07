//! Private generated namespaces retain both global and original child plans.

use super::*;
use crate::generated_source::{GeneratedHostRosterV1, RuntimeGfx942GeneratedSourceMutV1};
use generated_shells::{GeneratedShellCommitPlanV1, GeneratedShellPlanV1};

mod issue;
pub(super) use issue::MultiGeneratedSubmissionV1;
mod cold_device;
pub(super) use cold_device::DeviceCustodyV1;
pub(crate) use cold_device::GeneratedColdDeviceFailureV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedAdoptionScopeV1 {
    child: Option<usize>,
    device: u64,
    stream: u64,
    local_stream: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MultiGeneratedShellPlanV1 {
    scope: GeneratedAdoptionScopeV1,
    global: GeneratedShellPlanV1,
    local: GeneratedShellPlanV1,
}

impl MultiGeneratedShellPlanV1 {
    pub(crate) fn plan(&self) -> &GeneratedShellPlanV1 {
        &self.global
    }
}

pub(crate) struct MultiGeneratedShellCommitV1 {
    route: MultiGeneratedShellPlanV1,
    child: GeneratedShellCommitPlanV1,
}

impl MultiGeneratedShellCommitV1 {
    pub(crate) fn plan(&self) -> &GeneratedShellPlanV1 {
        &self.route.global
    }
}

fn global_plan(local: GeneratedShellPlanV1, key: u64, stream: u64) -> Option<GeneratedShellPlanV1> {
    if key == 0 || local.count == 0 || local.count > GFX942_MAX_FIXED_DISPATCH_DATA_V1 {
        return None;
    }
    let mut global = local;
    global.key = key;
    global.next_handle = key.checked_add(1 + local.count as u64)?;
    global.binding.backend_stream = stream;
    for (ordinal, member) in global.members[..global.count].iter_mut().enumerate() {
        let member = member.as_mut()?;
        member.backend = key + 1 + ordinal as u64;
        member.description.adoption = key;
    }
    Some(global)
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn generated_adoption_scope_v1(
        &self,
        device: u64,
        stream: u64,
    ) -> Result<GeneratedAdoptionScopeV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        self.require_device(device)?;
        if self.streams.get(&stream) != Some(&device) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown generated stream",
            ));
        }
        Ok(GeneratedAdoptionScopeV1 {
            child: None,
            device,
            stream,
            local_stream: stream,
        })
    }

    pub(crate) fn generated_lane_ready_for_stream_v1(
        &self,
        device: u64,
        stream: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.generated_adoption_scope_v1(device, stream)?;
        self.generated_lane_ready_v1()
    }

    pub(crate) fn quarantine_generated_scope_v1(&mut self, _: GeneratedAdoptionScopeV1) {
        self.quarantine_generated_adoption_v1();
    }

    pub(crate) fn resume_generated_scope_panic_v1(
        &mut self,
        _: GeneratedAdoptionScopeV1,
        payload: Box<dyn std::any::Any + Send>,
    ) -> ! {
        self.resume_generated_adoption_panic_v1(payload)
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(crate) fn generated_adoption_scope_v1(
        &self,
        device: u64,
        stream: u64,
    ) -> Result<GeneratedAdoptionScopeV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let child = self.child_for_device(device)?;
        let route = Self::route(
            &self.streams,
            stream,
            "unknown generated multi-device stream",
        )?;
        if route.child != child {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "generated stream belongs to another child",
            ));
        }
        let backend = self
            .children
            .get(child)
            .filter(|backend| backend.description.backend_device == device)
            .filter(|_| self.compute_xgmi_children.len() == self.children.len())
            .ok_or_else(|| {
                RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "generated child routing is inconsistent",
                ))
            })?;
        backend.generated_adoption_scope_v1(device, route.local)?;
        Ok(GeneratedAdoptionScopeV1 {
            child: Some(child),
            device,
            stream,
            local_stream: route.local,
        })
    }

    pub(crate) fn generated_lane_ready_for_stream_v1(
        &self,
        device: u64,
        stream: u64,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let scope = self.generated_adoption_scope_v1(device, stream)?;
        let child = scope.child.expect("multi-device scope");
        if self.compute_xgmi_child_occupied_v1(child) {
            return Ok(false);
        }
        self.children[child].generated_lane_ready_v1()
    }

    pub(crate) fn quarantine_generated_scope_v1(&mut self, scope: GeneratedAdoptionScopeV1) {
        self.terminal = true;
        // Attribute failure to the pre-effect owner, not a newly resolved route.
        if let Some(child) = scope.child.and_then(|index| self.children.get_mut(index)) {
            child.quarantine_generated_adoption_v1();
        } else {
            for child in &mut self.children {
                child.quarantine_generated_adoption_v1();
            }
        }
    }

    pub(crate) fn resume_generated_scope_panic_v1(
        &mut self,
        scope: GeneratedAdoptionScopeV1,
        payload: Box<dyn std::any::Any + Send>,
    ) -> ! {
        self.terminal = true;
        sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
            self.quarantine_generated_scope_v1(scope)
        })
    }

    fn generated_global_handle_available_v1(&self, id: u64) -> bool {
        !self.streams.contains_key(&id)
            && !self.allocations.contains_key(&id)
            && !self.modules.contains_key(&id)
            && !self.kernels.contains_key(&id)
            && !self.submissions.contains_key(&id)
            && !self.events.contains_key(&id)
            && !self.generated_allocations.contains_key(&id)
            && !self.generated_shells.contains_key(&id)
            && !self.generated_submissions.contains_key(&id)
    }

    pub(crate) fn prepare_generated_shells_v1(
        &mut self,
        binding: GeneratedShellBindingV1,
        roster: &GeneratedHostRosterV1,
        logical: &[crate::RuntimeAllocationIdV1],
    ) -> Result<MultiGeneratedShellPlanV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let scope =
            self.generated_adoption_scope_v1(binding.backend_device, binding.backend_stream);
        let scope = self.latch(scope)?;
        if self.next_handle == 0
            || roster.count == 0
            || roster.count > GFX942_MAX_FIXED_DISPATCH_DATA_V1
            || self.generated_shells.len() >= MAX_RUNTIME_STREAMS_V1
            || self
                .allocations
                .len()
                .checked_add(self.generated_allocations.len())
                .and_then(|count| count.checked_add(roster.count))
                .is_none_or(|count| count > crate::MAX_RUNTIME_ALLOCATIONS_V1)
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "multi-device generated shell capacity",
            ));
        }
        let end = self
            .next_handle
            .checked_add(1 + roster.count as u64)
            .ok_or_else(|| {
                KfdRuntimeBackendV1::capacity("multi-device generated handle exhaustion")
            })?;
        if !(self.next_handle..end).all(|id| self.generated_global_handle_available_v1(id)) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated global handle collision",
            ));
        }
        self.generated_allocations
            .try_reserve(roster.count)
            .map_err(|_| KfdRuntimeBackendV1::capacity("generated route allocation"))?;
        self.generated_shells
            .try_reserve(1)
            .map_err(|_| KfdRuntimeBackendV1::capacity("generated owner route allocation"))?;
        let local_binding = GeneratedShellBindingV1 {
            backend_stream: scope.local_stream,
            ..binding
        };
        let local = self.with_preparation_child_v1(scope.device, |child| {
            child.prepare_generated_shells_v1(local_binding, roster, logical)
        })?;
        let global =
            global_plan(local, self.next_handle, scope.stream).expect("preflighted global range");
        Ok(MultiGeneratedShellPlanV1 {
            scope,
            global,
            local,
        })
    }

    pub(crate) fn bind_generated_shell_requests_v1(
        &self,
        route: MultiGeneratedShellPlanV1,
        witnesses: [Option<crate::RuntimeAllocationRequestWitnessV1<'_>>;
            GFX942_MAX_FIXED_DISPATCH_DATA_V1],
    ) -> Result<MultiGeneratedShellCommitV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        let scope = self.generated_adoption_scope_v1(route.scope.device, route.scope.stream)?;
        if scope != route.scope
            || self.next_handle != route.global.key
            || global_plan(route.local, route.global.key, scope.stream) != Some(route.global)
            || !(route.global.key..route.global.next_handle)
                .all(|id| self.generated_global_handle_available_v1(id))
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated pending route changed",
            ));
        }
        self.require_compute_xgmi_child_available_v1(scope.child.expect("selected child"))?;
        let child = self.children[scope.child.expect("selected child")]
            .bind_generated_shell_requests_v1(route.local, witnesses)?;
        Ok(MultiGeneratedShellCommitV1 { route, child })
    }

    pub(crate) fn commit_generated_shells_v1<E>(
        &mut self,
        authenticated: MultiGeneratedShellCommitV1,
        source: &mut RuntimeGfx942GeneratedSourceMutV1<'_, E>,
        roster: &GeneratedHostRosterV1,
    ) {
        self.commit_generated_route_v1(authenticated, |backend, child| {
            backend.commit_generated_shells_v1(child, source, roster);
        });
    }

    pub(super) fn commit_generated_route_v1(
        &mut self,
        authenticated: MultiGeneratedShellCommitV1,
        commit: impl FnOnce(&mut KfdRuntimeBackendV1, GeneratedShellCommitPlanV1),
    ) {
        let MultiGeneratedShellCommitV1 { route, child } = authenticated;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_eq!(
                self.generated_adoption_scope_v1(route.scope.device, route.scope.stream)
                    .unwrap(),
                route.scope
            );
            assert_eq!(self.next_handle, route.global.key);
            assert_eq!(child.plan(), &route.local);
            assert_eq!(
                global_plan(route.local, route.global.key, route.scope.stream),
                Some(route.global)
            );
            assert!(
                (route.global.key..route.global.next_handle)
                    .all(|id| self.generated_global_handle_available_v1(id))
            );
            let index = route.scope.child.expect("selected child");
            self.require_compute_xgmi_child_available_v1(index).unwrap();
            self.next_handle = route.global.next_handle;
            assert!(
                self.generated_shells
                    .insert(route.global.key, route)
                    .is_none()
            );
            for ordinal in 0..route.global.count {
                let global = route.global.members[ordinal].expect("complete global roster");
                let local = route.local.members[ordinal].expect("complete local roster");
                assert!(
                    self.generated_allocations
                        .insert(
                            global.backend,
                            RoutedHandleV1 {
                                child: index,
                                local: local.backend
                            }
                        )
                        .is_none()
                );
            }
            // Root both namespaces before the child consumes the original control.
            commit(&mut self.children[index], child);
        }));
        if let Err(payload) = result {
            self.resume_generated_scope_panic_v1(route.scope, payload);
        }
    }

    pub(crate) fn generated_shell_plan_v1(
        &self,
        key: u64,
        generation: u64,
        stream: crate::RuntimeStreamIdV1,
        hold: u64,
    ) -> Option<GeneratedShellPlanV1> {
        let route = self.generated_shells.get(&key)?;
        (route.global.binding.context_generation == generation
            && route.global.binding.stream == stream
            && route.global.binding.hold == hold)
            .then_some(route.global)
    }

    pub(crate) fn validate_generated_shell_records_v1(&self, plan: &GeneratedShellPlanV1) -> bool {
        let Some(route) = self.generated_shells.get(&plan.key) else {
            return false;
        };
        let Some(index) = route.scope.child else {
            return false;
        };
        let Some(child) = self.children.get(index) else {
            return false;
        };
        route.global == *plan
            && global_plan(route.local, plan.key, route.scope.stream) == Some(*plan)
            && self
                .generated_adoption_scope_v1(route.scope.device, route.scope.stream)
                .ok()
                == Some(route.scope)
            && child.validate_generated_shell_records_v1(&route.local)
            && !self.allocations.values().any(|member| {
                member.child == index
                    && member.local > route.local.key
                    && member.local < route.local.next_handle
            })
            && self
                .generated_allocations
                .values()
                .filter(|member| {
                    member.child == index
                        && member.local > route.local.key
                        && member.local < route.local.next_handle
                })
                .count()
                == plan.count
            && plan.members[..plan.count]
                .iter()
                .zip(&route.local.members[..plan.count])
                .all(|(global, local)| {
                    let (Some(global), Some(local)) = (global, local) else {
                        return false;
                    };
                    !self.allocations.contains_key(&global.backend)
                        && self.generated_allocations.get(&global.backend)
                            == Some(&RoutedHandleV1 {
                                child: index,
                                local: local.backend,
                            })
                })
    }

    pub(crate) fn validate_generated_shell_disposal_v1(&self, plan: &GeneratedShellPlanV1) -> bool {
        self.validate_generated_shell_records_v1(plan)
            && !self
                .generated_submissions
                .values()
                .any(|submission| submission.shell_key() == plan.key)
            && self.generated_shells.get(&plan.key).is_some_and(|route| {
                self.children[route.scope.child.expect("validated child")]
                    .validate_generated_shell_disposal_v1(&route.local)
            })
    }

    pub(crate) fn dispose_generated_shells_v1(&mut self, plan: &GeneratedShellPlanV1) {
        let route = *self
            .generated_shells
            .get(&plan.key)
            .expect("rooted generated route");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert!(self.validate_generated_shell_disposal_v1(plan));
            self.children[route.scope.child.expect("validated child")]
                .dispose_generated_shells_v1(&route.local);
            for member in plan.members[..plan.count].iter().flatten() {
                assert!(self.generated_allocations.remove(&member.backend).is_some());
            }
            assert!(self.generated_shells.remove(&plan.key).is_some());
        }));
        if let Err(payload) = result {
            self.resume_generated_scope_panic_v1(route.scope, payload);
        }
    }

    pub(crate) fn generated_empty_prefix_v1(&self, stream: u64) -> bool {
        self.streams.get(&stream).is_some_and(|route| {
            !self
                .generated_shells
                .values()
                .any(|shell| shell.scope.stream == stream)
                && self
                    .children
                    .get(route.child)
                    .is_some_and(|child| child.generated_empty_prefix_v1(route.local))
        })
    }

    fn checked_generated_route_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
    ) -> Result<MultiGeneratedShellPlanV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let route = self
            .generated_shells
            .get(&plan.key)
            .filter(|route| route.global == *plan)
            .copied()
            .ok_or_else(|| {
                KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "unknown generated global plan",
                )
            })?;
        if !self.validate_generated_shell_records_v1(plan) {
            self.quarantine_generated_scope_v1(route.scope);
            return Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "retained generated route changed",
                ),
            ));
        }
        Ok(route)
    }

    pub(crate) fn adopt_generated_data_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
        program: ValidatedKernelEnvelope<'_>,
        buffers: &[crate::Gfx942KfdDispatchBufferV1],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let route = self.checked_generated_route_v1(plan)?;
        self.with_preparation_child_v1(route.scope.device, |child| {
            child.adopt_generated_data_v1(&route.local, roster, program, buffers)
        })
    }

    pub(crate) fn retire_generated_data_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let route = self.checked_generated_route_v1(plan)?;
        self.with_preparation_child_v1(route.scope.device, |child| {
            child.retire_generated_data_v1(&route.local)
        })
    }
}

#[cfg(test)]
mod tests;
