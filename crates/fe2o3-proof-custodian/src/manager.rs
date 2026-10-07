//! Independent keyless root manager. Offered custody never follows coordinator teardown.

use crate::{
    PendingRootApplicationProofControllerV1, ProductionApplicationProofCustodianDeploymentV1,
    RootStagedApplicationProofControllerV1, other, require,
};
use fe2o3_broker_authority_service::{ProofManagerCommandV1, RootProofManagerServerV1};
use std::{
    io,
    os::fd::{AsFd, OwnedFd},
    time::Duration,
};

struct Application {
    operation: [u8; 32],
    pending: Option<PendingRootApplicationProofControllerV1>,
    controller: Option<RootStagedApplicationProofControllerV1>,
    offer: Option<OwnedFd>,
    offered: bool,
    activate: bool,
    activated: bool,
    acknowledged: bool,
    quarantined: bool,
    cancelling: bool,
}
impl Application {
    fn step(&mut self, server: &mut RootProofManagerServerV1, connected: bool) -> io::Result<()> {
        if self.quarantined {
            return Ok(());
        }
        if !connected && !self.offered && self.offer.is_none() {
            self.cancelling = true;
        }
        if self.cancelling {
            let done = if let Some(pending) = &mut self.pending {
                pending.poll_cancel()?
            } else if let Some(controller) = &mut self.controller {
                controller.poll_cancel()?
            } else {
                true
            };
            if done {
                self.pending = None;
                self.controller = None;
                self.quarantined = true;
            }
            return Ok(());
        }
        if let Some(pending) = &mut self.pending {
            pending.poll()?;
            if let Some(controller) = pending.take_ready()? {
                self.controller = Some(controller);
                self.pending = None;
            }
            return Ok(());
        }
        let Some(controller) = &mut self.controller else {
            return Ok(());
        };
        if !self.offered {
            if !connected {
                return Ok(());
            }
            if self.offer.is_none() {
                self.offer = Some(controller.offer_ready()?);
            }
            // offer_ready made physical custody sticky before the first possible send.
            if server
                .offer_ready(controller.session(), self.offer.as_ref().unwrap().as_fd())
                .map_err(other)?
            {
                self.offered = true;
                self.offer = None;
            }
            return Ok(());
        }
        if self.activate && !self.activated {
            self.activated = controller.poll_activate()?;
        }
        if self.activated && !self.acknowledged && connected {
            self.acknowledged = server
                .acknowledge_activation(controller.session())
                .map_err(other)?;
        }
        Ok(())
    }
}

/// Runs the independently installed, single-threaded root manager at its fixed socket.
/// No caller-supplied controller, deployment path, argv, or signing key is accepted.
/// On coordinator loss it retains offered owners indefinitely; this is quarantine,
/// not native settlement. Whole-service containment is a deployment responsibility.
pub fn run_fixed_proof_manager_v1() -> io::Result<()> {
    require(
        std::env::args_os().count() == 1,
        "proof manager does not accept arguments",
    )?;
    fe2o3_protected_service_spawn::require_exact_root_identity_v1().map_err(other)?;
    let initial = ProductionApplicationProofCustodianDeploymentV1::open()?;
    let mut server = RootProofManagerServerV1::listen().map_err(other)?;
    require(
        initial.deployment().identity() == server.application_deployment(),
        "manager application deployment differs",
    )?;
    drop(initial);
    let mut applications: Vec<Application> = Vec::with_capacity(16);
    let mut connected = true;
    let mut cursor = 0;
    loop {
        if connected {
            match server.poll() {
                Ok(Some(ProofManagerCommandV1::Registered(registration))) => {
                    let operation = registration.transcript().root_nonce();
                    let launch = (|| {
                        let deployment = ProductionApplicationProofCustodianDeploymentV1::open()?;
                        require(
                            deployment.deployment().identity() == server.application_deployment(),
                            "manager deployment changed",
                        )?;
                        // The ingress has already reserved one of the fixed 16 slots.
                        require(
                            applications.len() < 16,
                            "manager physical capacity exhausted",
                        )?;
                        deployment.begin_application(*registration)
                    })();
                    match launch {
                        Ok(pending) => applications.push(Application {
                            operation,
                            pending: Some(pending),
                            controller: None,
                            offer: None,
                            offered: false,
                            activate: false,
                            activated: false,
                            acknowledged: false,
                            quarantined: false,
                            cancelling: false,
                        }),
                        Err(error) => {
                            eprintln!("proof-manager startup quarantined: {error}");
                            connected = false;
                        }
                    }
                }
                Ok(Some(ProofManagerCommandV1::Activate { operation, session })) => {
                    if let Some(app) = applications.iter_mut().find(|a| a.operation == operation)
                        && app
                            .controller
                            .as_ref()
                            .is_some_and(|controller| controller.session().identity() == session)
                        && app.offered
                        && !app.activate
                    {
                        app.activate = true;
                    } else {
                        connected = false;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("proof-manager coordinator quarantined: {error}");
                    connected = false;
                }
            }
        }
        if !applications.is_empty() {
            let index = cursor % applications.len();
            cursor = (index + 1) % applications.len();
            if let Err(error) = applications[index].step(&mut server, connected) {
                eprintln!("proof-manager application quarantined: {error}");
                let app = &mut applications[index];
                if app.offered || app.offer.is_some() || app.cancelling {
                    app.quarantined = true;
                } else {
                    app.cancelling = true;
                }
                connected = false;
            }
        }
        if !connected {
            server.disconnect();
        }
        // Custody remains owned even when a process or socket has died. No auto-release.
        std::thread::sleep(Duration::from_millis(1));
    }
}
