use super::*;

#[test]
fn atomic_and_collective_contracts_gate_ordinary_typed_submissions() {
    let (mut closed, stream, allocation, kernel) = context_with_launch_prerequisites();
    let arguments = AddArguments {
        allocation,
        scalar: 1,
    };
    let atomic_contract = RuntimeAtomicLaunchContractV1 {
        operation: RuntimeAtomicOperationV1::Add,
        scope: RuntimeMemoryScopeV1::Workgroup,
        order: RuntimeMemoryOrderV1::Relaxed,
        failure_order: None,
        weak: false,
        geometry: geometry(),
    };
    assert!(matches!(
        closed.launch_atomic(stream, &kernel, &arguments, atomic_contract, &[]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Unsupported
        ))
    ));
    assert_eq!(closed.backend().submit_count, 0);

    let mut context = RuntimeContextV1::open(MockBackend {
        execution_capabilities: RuntimeExecutionCapabilitiesV1 {
            atomics: true,
            collectives: true,
            ..RuntimeExecutionCapabilitiesV1::default()
        },
        ..MockBackend::default()
    })
    .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<AddArguments>(module, "atomic_collective")
        .unwrap();
    let arguments = AddArguments {
        allocation,
        scalar: 1,
    };

    let mut atomic = context
        .launch_atomic(stream, &kernel, &arguments, atomic_contract, &[])
        .unwrap();
    assert_eq!(context.backend().submit_count, 1);
    assert_eq!(context.backend().last_launch_geometry, Some(geometry()));
    assert_eq!(
        context.backend().last_atomic_contract,
        Some(atomic_contract)
    );
    context.wait(&mut atomic, Duration::from_secs(1)).unwrap();
    context.release_submission(atomic).unwrap();

    let partial_atomic_geometry = RuntimeLaunchGeometryV1 {
        grid: [65, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    };
    let mut partial_atomic = context
        .launch_atomic(
            stream,
            &kernel,
            &arguments,
            RuntimeAtomicLaunchContractV1 {
                geometry: partial_atomic_geometry,
                ..atomic_contract
            },
            &[],
        )
        .unwrap();
    assert_eq!(context.backend().submit_count, 2);
    assert_eq!(
        context.backend().last_launch_geometry,
        Some(partial_atomic_geometry)
    );
    context
        .wait(&mut partial_atomic, Duration::from_secs(1))
        .unwrap();
    context.release_submission(partial_atomic).unwrap();

    let collective_contract = RuntimeCollectiveLaunchContractV1 {
        operation: RuntimeCollectiveOperationV1::ReduceSum,
        scope: RuntimeMemoryScopeV1::Workgroup,
        order: RuntimeMemoryOrderV1::AcquireRelease,
        participants: 64,
        geometry: geometry(),
    };
    let mut collective = context
        .launch_collective(stream, &kernel, &arguments, collective_contract, &[])
        .unwrap();
    assert_eq!(context.backend().submit_count, 3);
    assert_eq!(
        context.backend().last_collective_contract,
        Some(collective_contract)
    );
    context
        .wait(&mut collective, Duration::from_secs(1))
        .unwrap();
    context.release_submission(collective).unwrap();

    assert!(matches!(
        context.launch_atomic(
            stream,
            &kernel,
            &arguments,
            RuntimeAtomicLaunchContractV1 {
                operation: RuntimeAtomicOperationV1::Exchange,
                ..atomic_contract
            },
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidAtomicContract
        ))
    ));
    assert!(matches!(
        context.launch_collective(
            stream,
            &kernel,
            &arguments,
            RuntimeCollectiveLaunchContractV1 {
                participants: 63,
                ..collective_contract
            },
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidCollectiveContract
        ))
    ));
    assert!(matches!(
        context.launch_collective(
            stream,
            &kernel,
            &arguments,
            RuntimeCollectiveLaunchContractV1 {
                participants: 64,
                geometry: RuntimeLaunchGeometryV1 {
                    grid: [65, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                ..collective_contract
            },
            &[],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidCollectiveContract
        ))
    ));
    assert_eq!(context.backend().submit_count, 3);
}

#[test]
fn compare_exchange_contract_requires_a_legal_failure_order_and_explicit_weakness() {
    use RuntimeMemoryOrderV1::{Acquire, AcquireRelease, Relaxed, Release, SequentiallyConsistent};

    let orders = [
        Relaxed,
        Acquire,
        Release,
        AcquireRelease,
        SequentiallyConsistent,
    ];
    let legal_pairs = [
        (Relaxed, Relaxed),
        (Acquire, Relaxed),
        (Acquire, Acquire),
        (Release, Relaxed),
        (AcquireRelease, Relaxed),
        (AcquireRelease, Acquire),
        (SequentiallyConsistent, Relaxed),
        (SequentiallyConsistent, Acquire),
        (SequentiallyConsistent, SequentiallyConsistent),
    ];
    for success in orders {
        for failure in orders {
            assert_eq!(
                valid_compare_exchange_order_pair(success, failure),
                legal_pairs.contains(&(success, failure)),
                "unexpected compare-exchange order pair: {success:?}/{failure:?}",
            );
        }
    }

    let contract = RuntimeAtomicLaunchContractV1 {
        operation: RuntimeAtomicOperationV1::CompareExchange,
        scope: RuntimeMemoryScopeV1::Device,
        order: AcquireRelease,
        failure_order: Some(Acquire),
        weak: true,
        geometry: geometry(),
    };
    assert!(atomic_contract_is_legal(contract));
    assert!(!atomic_contract_is_legal(RuntimeAtomicLaunchContractV1 {
        failure_order: Some(Release),
        ..contract
    }));
    assert!(!atomic_contract_is_legal(RuntimeAtomicLaunchContractV1 {
        failure_order: None,
        ..contract
    }));
    assert!(!atomic_contract_is_legal(RuntimeAtomicLaunchContractV1 {
        operation: RuntimeAtomicOperationV1::Add,
        failure_order: Some(Relaxed),
        ..contract
    }));
    assert!(!atomic_contract_is_legal(RuntimeAtomicLaunchContractV1 {
        operation: RuntimeAtomicOperationV1::Add,
        failure_order: None,
        ..contract
    }));
}
