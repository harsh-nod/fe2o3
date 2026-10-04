use super::*;

#[test]
fn descriptor_failures_preserve_fixed_labels_and_errno() {
    for (failure, expected) in [
        (
            checks::Failure::InvalidDescriptor {
                role: "pipe",
                reason: "flags",
            },
            "invalid pipe descriptor: flags",
        ),
        (
            checks::Failure::DescriptorChanged("manifest"),
            "retained manifest changed",
        ),
        (
            checks::Failure::DescriptorAlias("source alias"),
            "source alias",
        ),
    ] {
        let error = Error::from(failure);
        assert_eq!(error.to_string(), expected);
        assert!(StdError::source(&error).is_none());
    }
    let error = Error::from(checks::Failure::Io {
        operation: "inspect test descriptor",
        source: rustix::io::Errno::BADF,
    });
    assert!(matches!(
        error,
        Error::Io {
            operation: "inspect test descriptor",
            errno: rustix::io::Errno::BADF,
        }
    ));
    assert_eq!(
        StdError::source(&error)
            .unwrap()
            .downcast_ref::<rustix::io::Errno>(),
        Some(&rustix::io::Errno::BADF),
    );
}

#[test]
fn family_errors_preserve_native_source_types_and_resource_chains() {
    let errors = [
        Error::from(Resource::Accounting),
        Error::from(SupervisorError::Resource(Resource::Accounting)),
        Error::from(HandoffError::Resource(Resource::Accounting)),
        Error::from(CapabilityError::Resource(Resource::Accounting)),
        Error::from(ManifestError::Resource(Resource::Accounting)),
        Error::from(ParentError::from(Resource::Accounting)),
    ];
    for error in errors {
        assert_eq!(
            crate::launch_v2::test_support::resource(&error),
            Resource::Accounting
        );
    }
    let error = Error::from(SupervisorError::RootChanged);
    assert!(StdError::source(&error).unwrap().is::<SupervisorError>());
    let error = Error::from(HandoffError::DescriptorChanged);
    assert!(StdError::source(&error).unwrap().is::<HandoffError>());
    let error = Error::from(StaticError::InvalidMagic);
    assert_eq!(
        StdError::source(&error)
            .unwrap()
            .downcast_ref::<StaticError>(),
        Some(&StaticError::InvalidMagic),
    );
    for error in [
        Error::InvalidParentIdentity,
        Error::ParentChanged,
        Error::LaunchManifestMismatch,
    ] {
        assert!(StdError::source(&error).is_none());
        assert!(!error.to_string().is_empty());
    }
}
