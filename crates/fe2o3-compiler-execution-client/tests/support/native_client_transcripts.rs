// Same socket transcripts for both nominal native client families.
macro_rules! native_client_transcripts {
    ($version:expr, $currentness_challenge:ident, $new_verification:ident, $issue_current:ident) => {
        const WORK: usize = 100_000_000;
        const STORAGE: usize = 4 * 1024 * 1024;
        const TIMEOUT: Duration = Duration::from_secs(10);
        // Prepay fixed fixture wire arrays, keys and socket/anchor staging. Production
        // output reservations are added separately, including every packet decode.
        const FIXTURE_STORAGE: usize = 64 * 1024;

        fn keep<T>((value, storage): (T, Storage), b: &mut Budget<'_>) -> T {
            b.reserve_storage(storage.additional_storage()).unwrap();
            value
        }
        fn policy(b: &mut Budget<'_>) -> Policy {
            keep(Policy::decode(&fixture::policy_wire($version), b).unwrap(), b)
        }
        fn subject(b: &mut Budget<'_>) -> Subject {
            let (subject, storage) = Subject::decode(&fixture::subject_wire($version), b).unwrap();
            b.reserve_storage(storage.retained_storage()).unwrap();
            subject
        }
        fn carriage(b: &mut Budget<'_>) -> Carriage {
            let policy = policy(b);
            let request = keep(Request::decode(&fixture::request_wire($version), b).unwrap(), b);
            let receipt = keep(
                Receipt::issue(&policy, &request, &SigningKey::from_bytes(&[0x51; 32]), b).unwrap(),
                b,
            );
            let publication = keep(
                Publication::new([0x81; 32], [0x82; 32], receipt, b).unwrap(),
                b,
            );
            let ack = keep(Ack::new(&publication, [0x83; 32], b).unwrap(), b);
            keep(
                Carriage::new(policy, request, publication, ack, b).unwrap(),
                b,
            )
        }
        fn pair() -> (OwnedFd, OwnedFd) {
            let pair = net::socketpair(
                AddressFamily::UNIX,
                SocketType::SEQPACKET,
                SocketFlags::CLOEXEC,
                None,
            )
            .expect("create Unix seqpacket fixture");
            for fd in [&pair.0, &pair.1] {
                net::sockopt::set_socket_timeout(fd, net::sockopt::Timeout::Recv, Some(TIMEOUT))
                    .expect("set fixture receive timeout");
                net::sockopt::set_socket_timeout(fd, net::sockopt::Timeout::Send, Some(TIMEOUT))
                    .expect("set fixture send timeout");
            }
            pair
        }
        fn receive(peer: &OwnedFd, b: &mut Budget<'_>) -> Query {
            let mut bytes = [0; MAX_REQUEST];
            let (_, n) = net::recv(peer, &mut bytes[..], RecvFlags::TRUNC).unwrap();
            assert!(n > 0 && n <= bytes.len());
            keep(Query::decode(&bytes[..n], b).unwrap(), b)
        }
        fn send(peer: &OwnedFd, bytes: &[u8]) {
            assert_eq!(
                net::send(peer, bytes, SendFlags::NOSIGNAL).unwrap(),
                bytes.len()
            );
        }
        fn respond(peer: &OwnedFd, query: &Query, policy: &Policy, reply: Reply<'_>, b: &mut Budget<'_>) {
            let response = keep(
                Response::new(query.identity(), policy, reply, b).unwrap(),
                b,
            );
            send(peer, response.canonical_bytes());
        }
        fn assert_closed(peer: &OwnedFd) {
            let mut bytes = [0; MAX_REQUEST];
            assert_eq!(
                net::recv(peer, &mut bytes[..], RecvFlags::empty())
                    .unwrap()
                    .1,
                0
            );
        }

        #[test]
        fn native_cancel_requires_exact_acknowledgment_and_preserves_original_account() {
            for wrong_kind in [false, true] {
                let (peer, server) = pair();
                let worker = thread::spawn(move || {
                    let mut work = Work::new(WORK);
                    let mut b = Budget::new(&mut work, STORAGE);
                    b.reserve_storage(FIXTURE_STORAGE).unwrap();
                    let p = policy(&mut b);
                    let query = receive(&server, &mut b);
                    assert_eq!(query.kind(), QueryKind::Cancel);
                    let reply = if wrong_kind {
                        Reply::Ready {
                            sequence: 1,
                            prior_rollback_anchor: [0; 32],
                        }
                    } else {
                        Reply::Cancelled {
                            sequence: 1,
                            prior_rollback_anchor: [0; 32],
                        }
                    };
                    respond(&server, &query, &p, reply, &mut b);
                    assert_closed(&server);
                });
                let mut work = Work::new(WORK);
                let mut b = Budget::new(&mut work, STORAGE);
                b.reserve_storage(FIXTURE_STORAGE).unwrap();
                let p = policy(&mut b);
                let floor = b.storage();
                let original = b.work_ledger_identity_v1();
                b.reserve_storage(Client::PEER_STORAGE).unwrap();
                let client = Client::admit(peer, TIMEOUT, &mut b).unwrap();
                let (client, ()) = client.prepare::<_, Error>(|budget| {
                    assert!(budget.work_ledger_identity_v1() == original);
                    budget.charge_work(19)?;
                    Ok(())
                }).unwrap();
                assert_eq!(client.cancel(&p).is_err(), wrong_kind);
                assert_eq!(b.storage(), floor);
                assert!(b.work_ledger_identity_v1() == original);
                worker.join().unwrap();
            }
        }

        #[derive(Clone, Copy)]
        enum Stage {
            Ready,
            Prepared,
            Issued,
            Published,
        }

        #[test]
        fn native_cancel_after_preparation_keeps_expired_deadline() {
            let (peer, server) = pair();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(FIXTURE_STORAGE).unwrap();
            let policy = policy(&mut budget);
            let floor = budget.storage();
            let original = budget.work_ledger_identity_v1();
            budget.reserve_storage(Client::PEER_STORAGE).unwrap();
            let client = Client::admit(peer, Duration::from_millis(1), &mut budget).unwrap();
            let (client, ()) = client.prepare::<_, Error>(|b| {
                b.charge_work(19)?;
                thread::sleep(Duration::from_millis(5));
                Ok(())
            }).unwrap();
            assert!(matches!(client.cancel(&policy), Err(Error::Transport(
                fe2o3_compiler_execution_client::CompilerExecutionClientErrorV1::Timeout
            ))));
            assert_closed(&server);
            assert!(budget.work_ledger_identity_v1() == original);
            assert_eq!(budget.storage(), floor);
        }
        fn spawn_lifecycle(peer: OwnedFd, stage: Stage) -> thread::JoinHandle<usize> {
            spawn_lifecycle_notifying(peer, stage, |_| {})
        }
        fn spawn_lifecycle_notifying(
            peer: OwnedFd,
            stage: Stage,
            closed: impl FnOnce(usize) + Send + 'static,
        ) -> thread::JoinHandle<usize> {
            thread::spawn(move || {
                let mut work = Work::new(WORK);
                let mut b = Budget::new(&mut work, STORAGE);
                b.reserve_storage(FIXTURE_STORAGE).unwrap();
                let c = carriage(&mut b);
                let mut packets = 0;
                for _ in 0..5 {
                    let query = receive(&peer, &mut b);
                    packets += 1;
                    let reply = match query.kind() {
                        QueryKind::Recover => {
                            let s = keep(query.decode_subject(&mut b).unwrap(), &mut b);
                            assert_eq!(s.canonical_bytes(), c.request().subject().canonical_bytes());
                            if matches!(stage, Stage::Published) {
                                Reply::Recovered(&c)
                            } else {
                                Reply::ReceiptAbsent {
                                    sequence: 1,
                                    prior_rollback_anchor: [0; 32],
                                }
                            }
                        }
                        QueryKind::Inspect => match stage {
                            Stage::Ready => Reply::Ready {
                                sequence: 1,
                                prior_rollback_anchor: [0; 32],
                            },
                            Stage::Prepared => Reply::Prepared(c.request().challenge()),
                            Stage::Issued => Reply::Issued(c.publication()),
                            Stage::Published => panic!("recovery must terminate"),
                        },
                        QueryKind::Prepare => Reply::Prepared(c.request().challenge()),
                        QueryKind::Issue => {
                            let r = keep(query.decode_request(&mut b).unwrap(), &mut b);
                            assert_eq!(r.canonical_bytes(), c.request().canonical_bytes());
                            Reply::Issued(c.publication())
                        }
                        QueryKind::Publish => {
                            let r = keep(query.decode_request(&mut b).unwrap(), &mut b);
                            let p = keep(query.decode_publication(&mut b).unwrap(), &mut b);
                            assert_eq!(r.canonical_bytes(), c.request().canonical_bytes());
                            assert_eq!(p.canonical_bytes(), c.publication().canonical_bytes());
                            Reply::Published {
                                acknowledgment: c.acknowledgment(),
                                disposition: Disposition::Advanced,
                            }
                        }
                        _ => panic!("unexpected transcript command"),
                    };
                    let terminal = matches!(reply, Reply::Recovered(_) | Reply::Published { .. });
                    respond(&peer, &query, c.policy(), reply, &mut b);
                    if terminal {
                        assert_closed(&peer);
                        closed(packets);
                        return packets;
                    }
                }
                panic!("native lifecycle exceeded five exchanges");
            })
        }

        #[test]
        fn native_acquisition_and_all_recovery_suffixes_use_the_public_client() {
            for (stage, count) in [
                (Stage::Ready, 5),
                (Stage::Prepared, 4),
                (Stage::Issued, 3),
                (Stage::Published, 1),
            ] {
                let mut work = Work::new(WORK);
                let mut b = Budget::new(&mut work, STORAGE);
                b.reserve_storage(FIXTURE_STORAGE).unwrap();
                let p = policy(&mut b);
                let s = subject(&mut b);
                let expected = carriage(&mut b);
                let baseline = b.storage();
                let prefix = b.work();
                b.reserve_storage(Client::PEER_STORAGE).unwrap();
                let (peer, service) = pair();
                let server = spawn_lifecycle(service, stage);
                let result = Client::admit(peer, TIMEOUT, &mut b).unwrap().acquire(&p, s);
                let seen = server.join().unwrap();
                let (actual, charge) = result.unwrap();
                assert_eq!(seen, count);
                assert_eq!(actual.canonical_bytes(), expected.canonical_bytes());
                assert_eq!(charge.additional_storage(), actual.retained_storage());
                assert_eq!(b.storage(), baseline);
                assert!(b.work() > prefix);
                b.reserve_storage(charge.additional_storage()).unwrap();
                assert!(!actual.grants_compiler_authority());
                assert!(!actual.grants_launch_authority());
            }
        }

        #[test]
        fn native_recovery_only_absence_requires_cancel_at_the_exact_position() {
            for fault in ["none", "sequence", "anchor", "kind"] {
                let mut work = Work::new(WORK);
                let mut b = Budget::new(&mut work, STORAGE);
                b.reserve_storage(FIXTURE_STORAGE).unwrap();
                let p = policy(&mut b);
                let s = subject(&mut b);
                let floor = b.storage();
                let ledger = b.work_ledger_identity_v1();
                b.reserve_storage(Client::PEER_STORAGE).unwrap();
                let (peer, server) = pair();
                let client = Client::admit(peer, TIMEOUT, &mut b).unwrap();
                let worker = thread::spawn(move || {
                    let mut work = Work::new(WORK);
                    let mut b = Budget::new(&mut work, STORAGE);
                    b.reserve_storage(FIXTURE_STORAGE).unwrap();
                    let p = policy(&mut b);
                    let recover = receive(&server, &mut b);
                    assert_eq!(recover.kind(), QueryKind::Recover);
                    respond(&server, &recover, &p, Reply::ReceiptAbsent {
                        sequence: 3, prior_rollback_anchor: [0x75; 32],
                    }, &mut b);
                    let cancel = receive(&server, &mut b);
                    assert_eq!(cancel.kind(), QueryKind::Cancel);
                    let reply = if fault == "kind" {
                        Reply::Ready { sequence: 3, prior_rollback_anchor: [0x75; 32] }
                    } else {
                        Reply::Cancelled {
                            sequence: if fault == "sequence" { 4 } else { 3 },
                            prior_rollback_anchor: if fault == "anchor" { [0x76; 32] } else { [0x75; 32] },
                        }
                    };
                    respond(&server, &cancel, &p, reply, &mut b);
                    assert_closed(&server);
                });
                let result = client.recover_only(&p, s);
                worker.join().unwrap();
                assert_eq!(b.storage(), floor);
                assert!(b.work_ledger_identity_v1() == ledger);
                if fault == "none" {
                    let (recovered, charge) = result.unwrap();
                    b.reserve_storage(charge.additional_storage()).unwrap();
                    assert!(matches!(recovered, Recovery::Absent { sequence: 3, rollback_anchor } if rollback_anchor == [0x75; 32]));
                } else {
                    assert!(matches!(result, Err(Error::Mismatch(_))));
                }
            }
        }

        #[test]
        fn native_recovery_only_returns_only_the_exact_existing_carriage() {
            for changed in [false, true] {
                let mut work = Work::new(WORK);
                let mut b = Budget::new(&mut work, STORAGE);
                b.reserve_storage(FIXTURE_STORAGE).unwrap();
                let p = policy(&mut b);
                let expected = carriage(&mut b);
                let mut wire = fixture::subject_wire($version);
                if changed {
                    wire[120] ^= 1;
                    fixture::seal(&mut wire, "INERT-COMPILER-EXECUTION-SUBJECT", $version);
                }
                let (s, charge) = Subject::decode(&wire, &mut b).unwrap();
                b.reserve_storage(charge.retained_storage()).unwrap();
                let floor = b.storage();
                let ledger = b.work_ledger_identity_v1();
                b.reserve_storage(Client::PEER_STORAGE).unwrap();
                let (peer, server) = pair();
                let client = Client::admit(peer, TIMEOUT, &mut b).unwrap();
                let worker = thread::spawn(move || {
                    let mut work = Work::new(WORK);
                    let mut b = Budget::new(&mut work, STORAGE);
                    b.reserve_storage(FIXTURE_STORAGE).unwrap();
                    let c = carriage(&mut b);
                    let request = receive(&server, &mut b);
                    assert_eq!(request.kind(), QueryKind::Recover);
                    respond(&server, &request, c.policy(), Reply::Recovered(&c), &mut b);
                    assert_closed(&server);
                });
                let result = client.recover_only(&p, s);
                worker.join().unwrap();
                assert_eq!(b.storage(), floor);
                assert!(b.work_ledger_identity_v1() == ledger);
                if changed {
                    assert!(matches!(result, Err(Error::Mismatch("recovered native carriage differs from expected source"))));
                } else {
                    let (recovered, charge) = result.unwrap();
                    b.reserve_storage(charge.additional_storage()).unwrap();
                    let Recovery::Recovered(c) = recovered else { panic!("expected recovery") };
                    assert_eq!(c.canonical_bytes(), expected.canonical_bytes());
                    assert!(charge.additional_storage() >= c.retained_storage());
                }
            }
        }

        #[test]
        fn native_rejects_request_substitution_and_v1_without_retry() {
            for mixed_version in [false, true] {
                let mut work = Work::new(WORK);
                let mut b = Budget::new(&mut work, STORAGE);
                b.reserve_storage(FIXTURE_STORAGE).unwrap();
                let p = policy(&mut b);
                let s = subject(&mut b);
                let baseline = b.storage();
                b.reserve_storage(Client::PEER_STORAGE).unwrap();
                let (peer, service) = pair();
                let server = thread::spawn(move || {
                    let mut work = Work::new(WORK);
                    let mut b = Budget::new(&mut work, STORAGE);
                    b.reserve_storage(FIXTURE_STORAGE).unwrap();
                    let p = policy(&mut b);
                    let original = receive(&service, &mut b);
                    let other = keep(
                        Query::new(&p, QueryPayload::Inspect, &mut b).unwrap(),
                        &mut b,
                    );
                    assert_ne!(original.identity(), other.identity());
                    let response = keep(
                        Response::new(
                            other.identity(),
                            &p,
                            Reply::ReceiptAbsent {
                                sequence: 1,
                                prior_rollback_anchor: [0; 32],
                            },
                            &mut b,
                        )
                        .unwrap(),
                        &mut b,
                    );
                    if mixed_version {
                        use fe2o3_compiler_execution_protocol::{
                            CompilerExecutionIssuerPolicyV1 as LegacyPolicy,
                            CompilerExecutionServiceRequestV1 as LegacyRequest,
                            CompilerExecutionServiceResponseV1 as LegacyResponse,
                        };
                        let legacy = LegacyPolicy::decode(&fixture::policy_wire(1)).unwrap();
                        let query = LegacyRequest::inspect(&legacy);
                        let response =
                            LegacyResponse::receipt_absent(query.identity(), &legacy, 1, [0; 32]).unwrap();
                        send(&service, response.canonical_bytes());
                    } else {
                        send(&service, response.canonical_bytes());
                    }
                    assert_closed(&service);
                });
                let result = Client::admit(peer, TIMEOUT, &mut b).unwrap().acquire(&p, s);
                server.join().unwrap();
                if mixed_version {
                    assert!(matches!(result, Err(Error::Protocol(_))));
                } else {
                    assert!(matches!(result, Err(Error::Mismatch(_))));
                }
                assert_eq!(b.storage(), baseline);
            }
        }

        #[test]
        fn native_completion_retains_one_account_through_all_recovery_suffixes() {
            for (stage, packets) in [(Stage::Ready, 5), (Stage::Prepared, 4),
                (Stage::Issued, 3), (Stage::Published, 1)] {
                completion_case(stage, packets, FinishFault::None);
            }
        }

        #[test]
        fn native_completion_finish_failures_preserve_charges_and_close_without_retry() {
            for fault in [FinishFault::Error, FinishFault::Unwind,
                FinishFault::Floor, FinishFault::Foreign] {
                completion_case(Stage::Published, 1, fault);
            }
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum FinishFault { None, Error, Unwind, Floor, Foreign }

        fn completion_case(stage: Stage, packets: usize, fault: FinishFault) {
            // These are signed fixture transcripts and drop-order markers, not
            // compiler prepared ownership, protected publication or GPU evidence.
            struct Retained(std::rc::Rc<std::cell::Cell<bool>>);
            impl Drop for Retained {
                fn drop(&mut self) { self.0.set(true); }
            }
            let mut work = Work::new(WORK);
            let mut other_work = Work::new(WORK);
            let mut other = Budget::new(&mut other_work, STORAGE);
            other.reserve_storage(STORAGE / 2).unwrap();
            let mut displaced = None;
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(FIXTURE_STORAGE).unwrap();
            let policy = policy(&mut budget);
            let expected = carriage(&mut budget);
            let floor = budget.storage();
            let output_floor = floor + Client::SUBJECT_STORAGE + expected.retained_storage();
            let account = budget.work_ledger_identity_v1();
            let phase = std::cell::Cell::new(0);
            let finish_work = std::cell::Cell::new(0);
            let dropped = std::rc::Rc::new(std::cell::Cell::new(false));
            let retained = Retained(dropped.clone());
            budget.reserve_storage(Client::PEER_STORAGE).unwrap();
            let (peer, service) = pair();
            let (closed_tx, closed_rx) = std::sync::mpsc::sync_channel(1);
            let server = spawn_lifecycle_notifying(service, stage, move |count| {
                closed_tx.send(count).unwrap();
            });
            let client = Client::admit(peer, TIMEOUT, &mut budget).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| client
                .prepare_and_acquire::<_, _, _, Error>(
                    &policy,
                    |b| {
                        assert_eq!(phase.replace(1), 0);
                        assert!(b.work_ledger_identity_v1() == account);
                        b.charge_work(19)?;
                        Ok(retained)
                    },
                    |retained, b| {
                        assert_eq!(phase.replace(2), 1);
                        assert!(!dropped.get());
                        assert!(b.work_ledger_identity_v1() == account);
                        Ok((subject(b), retained))
                    },
                    |actual, retained, b| {
                        assert_eq!(phase.replace(3), 2);
                        assert!(!dropped.get());
                        assert!(b.work_ledger_identity_v1() == account);
                        assert_eq!(b.storage(), output_floor + size_of::<Client<'_, '_>>());
                        assert_eq!(actual.canonical_bytes(), expected.canonical_bytes());
                        // A final client drop is too late: EOF must be observed
                        // by the service before this callback can proceed.
                        assert_eq!(closed_rx.recv_timeout(TIMEOUT).unwrap(), packets);
                        finish_work.set(b.work());
                        b.charge_work(7)?;
                        b.reserve_storage(11)?;
                        let _retained = retained;
                        match fault {
                            FinishFault::None => Ok(actual),
                            FinishFault::Error => Err(Error::Mismatch("finish refused")),
                            FinishFault::Unwind => panic!("finish unwind"),
                            FinishFault::Floor => {
                                b.release_storage(12)?;
                                Ok(actual)
                            }
                            FinishFault::Foreign => {
                                displaced = Some(std::mem::replace(b, other));
                                Ok(actual)
                            }
                        }
                    },
                )));
            assert_eq!(server.join().unwrap(), packets);
            match fault {
                FinishFault::None => {
                    let actual = result.unwrap().unwrap();
                    assert!(!actual.grants_compiler_authority());
                    assert!(!actual.grants_launch_authority());
                }
                FinishFault::Error => assert!(matches!(result, Ok(Err(Error::Mismatch("finish refused"))))),
                FinishFault::Unwind => assert!(result.is_err()),
                FinishFault::Floor | FinishFault::Foreign => {
                    assert!(matches!(result, Ok(Err(Error::Resource(Resource::Accounting)))));
                }
            }
            assert_eq!(phase.get(), 3);
            assert!(dropped.get());
            if fault == FinishFault::Foreign {
                assert_eq!(budget.work(), 0);
                assert_eq!(budget.storage(), STORAGE / 2);
                let original = displaced.unwrap();
                assert!(original.work_ledger_identity_v1() == account);
                assert_eq!(original.work(), finish_work.get() + 7);
                assert_eq!(original.storage(), output_floor + size_of::<Client<'_, '_>>() + 11);
            } else {
                assert!(budget.work_ledger_identity_v1() == account);
                assert_eq!(budget.work(), finish_work.get() + 7);
                let retained = if fault == FinishFault::Floor { size_of::<Client<'_, '_>>() - 1 } else { 11 };
                assert_eq!(budget.storage(), output_floor + retained);
            }
        }

        fn signed_anchor(challenge: &AnchorChallengeV1, key: &SigningKey) -> AnchorTransitionReceiptV1 {
            let unsigned =
                UnsignedAnchorObservationV1::from_challenge(challenge, AnchorPositionV1::Proposed);
            let signature = key.sign(&unsigned.signing_bytes()).to_bytes();
            let signed = unsigned.attach_signature(signature);
            AnchorTransitionReceiptV1::new(
                challenge.clone(),
                &signed,
                &PinnedAnchorKeyV1::from_bytes(key.verifying_key().to_bytes()).unwrap(),
            )
            .unwrap()
        }
        fn current(c: &Carriage, nonce: [u8; 32], b: &mut Budget<'_>) -> Current {
            let p = keep(Policy::decode(c.policy().canonical_bytes(), b).unwrap(), b);
            let r = keep(
                Request::decode(c.request().canonical_bytes(), b).unwrap(),
                b,
            );
            let publication = keep(
                Publication::decode(c.publication().canonical_bytes(), b).unwrap(),
                b,
            );
            let transaction = keep(Transaction::new(p, r, publication, b).unwrap(), b);
            let signer = SigningKey::from_bytes(&[0x52; 32]);
            let key = PinnedAnchorKeyV1::from_bytes(signer.verifying_key().to_bytes()).unwrap();
            let pending = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
                .prepare(transaction.external_anchor_digest(b).unwrap(), &key)
                .unwrap()
                .begin_advance(CallerNonceV1::from_bytes([0x91; 32]), &key)
                .unwrap();
            let commit = signed_anchor(pending.challenge(), &signer);
            let challenge = keep(
                Verification::$currentness_challenge(c, &commit, nonce, b).unwrap(),
                b,
            );
            let observation = signed_anchor(&challenge, &signer);
            let verification = keep(
                Verification::$new_verification(c, commit, observation, nonce, [0x94; 32], [0x95; 32], b).unwrap(),
                b,
            );
            keep(
                Current::$issue_current(
                    c.policy(),
                    c,
                    verification,
                    nonce,
                    &SigningKey::from_bytes(&[0x51; 32]),
                    b,
                )
                .unwrap(),
                b,
            )
        }

        #[test]
        fn native_currentness_authenticates_fresh_challenges_and_rejects_replay() {
            let mut previous = None;
            for replay in [false, false, true] {
                let mut work = Work::new(WORK);
                let mut b = Budget::new(&mut work, STORAGE);
                b.reserve_storage(FIXTURE_STORAGE).unwrap();
                let p = policy(&mut b);
                let c = carriage(&mut b);
                let baseline = b.storage();
                b.reserve_storage(Client::PEER_STORAGE).unwrap();
                let (peer, service) = pair();
                let server = thread::spawn(move || {
                    let mut work = Work::new(WORK);
                    let mut b = Budget::new(&mut work, STORAGE);
                    b.reserve_storage(FIXTURE_STORAGE).unwrap();
                    let query = receive(&service, &mut b);
                    assert_eq!(query.kind(), QueryKind::VerifyCurrent);
                    let c = keep(query.decode_carriage(&mut b).unwrap(), &mut b);
                    let nonce = query.verification_challenge().unwrap();
                    assert_ne!(nonce, [0; 32]);
                    let attestation = current(&c, if replay { previous.unwrap() } else { nonce }, &mut b);
                    respond(
                        &service,
                        &query,
                        c.policy(),
                        Reply::VerifiedCurrent(&attestation),
                        &mut b,
                    );
                    assert_closed(&service);
                    nonce
                });
                let result = Client::admit(peer, TIMEOUT, &mut b)
                    .unwrap()
                    .verify_current_only(&p, &c);
                let nonce = server.join().unwrap();
                assert_ne!(Some(nonce), previous);
                if replay {
                    assert!(matches!(result, Err(Error::Journal(_))));
                } else {
                    let (verified, charge) = result.unwrap();
                    assert_eq!(verified.attestation().challenge(), nonce);
                    assert!(!verified.grants_authority());
                    assert_eq!(
                        charge.additional_storage(),
                        size_of::<(
                            fe2o3_compiler_execution_protocol::VerifiedCompilerExecutionCurrentRecordV3,
                            Storage
                        )>()
                    );
                }
                assert_eq!(b.storage(), baseline);
                previous = Some(nonce);
            }
        }

        #[test]
        fn native_admission_resource_refusal_precedes_io_and_preserves_cumulative_work() {
            for (limit, storage) in [(7, STORAGE), (WORK, Client::PEER_STORAGE)] {
                let mut work = Work::new(limit);
                let mut b = Budget::new(&mut work, storage);
                b.reserve_storage(Client::PEER_STORAGE).unwrap();
                let (peer, service) = pair();
                let result = Client::admit(peer, TIMEOUT, &mut b);
                assert!(matches!(result, Err(Error::Resource(_))));
                drop(result);
                assert_closed(&service);
                assert_eq!(b.storage(), Client::PEER_STORAGE);
                assert_eq!(b.work(), if limit == 7 { 0 } else { 64 * 1024 });
            }
        }

        #[test]
        fn native_acquisition_quota_exhaustion_closes_peer_without_a_packet() {
            let mut work = Work::new(WORK);
            let mut b = Budget::new(&mut work, STORAGE);
            b.reserve_storage(FIXTURE_STORAGE).unwrap();
            let p = policy(&mut b);
            let s = subject(&mut b);
            let baseline = b.storage();
            // Leave exactly the admission quota, then fail the first session charge.
            b.charge_work(WORK - b.work() - 64 * 1024).unwrap();
            b.reserve_storage(Client::PEER_STORAGE).unwrap();
            let (peer, service) = pair();
            let result = Client::admit(peer, TIMEOUT, &mut b).unwrap().acquire(&p, s);
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(b.work(), WORK);
            assert_eq!(b.storage(), baseline);
            assert_closed(&service);
        }
    };
}
pub(crate) use native_client_transcripts;
