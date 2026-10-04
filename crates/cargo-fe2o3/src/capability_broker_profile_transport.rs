// Exact-family profile ownership and authenticated request/response framing.
use super::{
    BrokerRouteV3, BrokeredCapabilities, BuildSession, CAPABILITY_BROKER_ENV, CHALLENGE_BYTES,
    CONFIG_ID_BYTES, CapabilityBindingV3, CapabilityProfileV1,
    CompilerExecutionClientProfileCapabilityV1, File, FundedClientProfileV3, REQUEST_AUTH_BYTES,
    REQUEST_AUTH_DOMAIN, REQUEST_AUTH_DOMAIN_V4, REQUEST_BYTES, REQUEST_MAGIC_V4,
    RESPONSE_AUTH_DOMAIN, RESPONSE_AUTH_DOMAIN_V4, RESPONSE_BYTES, RETAINED_OBJECT_BINDING_BYTES,
    ROUTE_PREFIX, ROUTE_PREFIX_V4, SECRET_BYTES, io, keyed_digest,
};
use crate::authority_release::profile::FundedProfileFileV3;

#[derive(Clone, Copy)]
pub(super) enum BrokerProfileRef<'a> {
    V1(&'a CompilerExecutionClientProfileCapabilityV1),
    V3(&'a FundedClientProfileV3),
}

pub(super) enum RetainedBrokerProfile {
    V1(CompilerExecutionClientProfileCapabilityV1),
    V3(FundedClientProfileV3),
}

pub(super) enum BrokerProfileTransfer {
    V1(File),
    V3(FundedProfileFileV3),
}

impl BrokerProfileTransfer {
    pub(super) fn file(&self) -> &File {
        match self {
            Self::V1(file) => file,
            Self::V3(file) => file.file(),
        }
    }
}

impl RetainedBrokerProfile {
    pub(super) fn try_clone_for_transfer(&self) -> Result<BrokerProfileTransfer, String> {
        match self {
            Self::V1(profile) => profile
                .try_clone_for_transfer()
                .map(BrokerProfileTransfer::V1),
            Self::V3(profile) => profile
                .try_clone_for_transfer()
                .map(BrokerProfileTransfer::V3),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompilerExecutionProfileFamily {
    LegacyV1,
    NativeV3,
}

/// Untrusted dispatch hint only. Each selected reader authenticates its exact
/// family, request, profile identity and descriptor roster independently.
pub(crate) fn broker_route_family_from_environment()
-> Result<CompilerExecutionProfileFamily, String> {
    let route = std::env::var(CAPABILITY_BROKER_ENV)
        .map_err(|_| format!("managed rustc invocation is missing {CAPABILITY_BROKER_ENV}"))?;
    match route.split(':').next() {
        Some(ROUTE_PREFIX) => Ok(CompilerExecutionProfileFamily::LegacyV1),
        Some(ROUTE_PREFIX_V4) => Ok(CompilerExecutionProfileFamily::NativeV3),
        _ => Err("capability broker route has an unknown transport family".to_owned()),
    }
}

impl CapabilityBindingV3 {
    pub(crate) fn new_protected_v4(
        profile: CapabilityProfileV1,
        config_identity: Option<[u8; CONFIG_ID_BYTES]>,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        retained_object_binding_sha256: [u8; RETAINED_OBJECT_BINDING_BYTES],
        native_profile_identity: [u8; 32],
    ) -> Result<Self, String> {
        if native_profile_identity == [0; 32]
            || config_identity.is_none_or(|identity| identity == [0; 32])
        {
            return Err(
                "native profile binding requires exact nonzero profile and config identities"
                    .to_owned(),
            );
        }
        let mut binding = Self::new_protected(
            profile,
            config_identity,
            compiler_closure,
            retained_object_binding_sha256,
        )?;
        binding.native_profile_identity = Some(native_profile_identity);
        Ok(binding)
    }

    pub(crate) fn from_environment_for_client_v4(
        profile: CapabilityProfileV1,
    ) -> Result<Self, String> {
        let encoded_route = std::env::var(CAPABILITY_BROKER_ENV)
            .map_err(|_| format!("managed rustc invocation is missing {CAPABILITY_BROKER_ENV}"))?;
        let route = BrokerRouteV3::parse_v4(&encoded_route)?;
        if route.binding.profile != profile {
            return Err("capability broker route has the wrong profile".into());
        }
        Ok(route.binding)
    }

    pub(super) const fn request_bytes(self) -> usize {
        REQUEST_BYTES
            + if self.native_profile_identity.is_some() {
                33
            } else {
                0
            }
    }
}

impl BrokeredCapabilities {
    /// Populated only after the exact response and all descriptors authenticate.
    pub(crate) fn authenticated_client_profile_v3_identity(&self) -> Option<[u8; 32]> {
        self.authenticated_binding
            .and_then(|binding| binding.native_profile_identity)
    }

    pub(crate) fn take_compiler_execution_profile_v3(
        &mut self,
    ) -> Result<FundedClientProfileV3, String> {
        if self.authenticated_client_profile_v3_identity().is_none()
            || self.compiler_execution_profile.is_some()
        {
            return Err(
                "broker response is not the authenticated native profile family".to_owned(),
            );
        }
        self.compiler_execution_profile_v3
            .take()
            .ok_or_else(|| "authenticated native profile has already been consumed".to_owned())
    }
}

pub(super) fn authenticate_request(
    request: &[u8],
    session: BuildSession,
    binding: CapabilityBindingV3,
    secret: &[u8; SECRET_BYTES],
) -> io::Result<([u8; CHALLENGE_BYTES], [u8; REQUEST_AUTH_BYTES])> {
    if request.len() != binding.request_bytes() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "broker request has wrong family length",
        ));
    }
    let challenge_start = request.len() - REQUEST_AUTH_BYTES - CHALLENGE_BYTES;
    let challenge = request[challenge_start..challenge_start + CHALLENGE_BYTES]
        .try_into()
        .expect("exact request length checked");
    if request != request_bytes(session, binding, challenge, secret) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "capability broker request is not bound to this broker, session, profile, and config",
        ));
    }
    Ok((
        challenge,
        request[request.len() - REQUEST_AUTH_BYTES..]
            .try_into()
            .expect("exact request length checked"),
    ))
}

pub(super) fn request_bytes(
    session: BuildSession,
    binding: CapabilityBindingV3,
    challenge: [u8; CHALLENGE_BYTES],
    secret: &[u8; SECRET_BYTES],
) -> Vec<u8> {
    let native = binding.native_profile_identity.is_some();
    let mut request = Vec::with_capacity(binding.request_bytes());
    request.extend_from_slice(if native {
        REQUEST_MAGIC_V4
    } else {
        binding.profile.request_magic()
    });
    request.extend_from_slice(session.as_bytes());
    match binding.config_identity {
        Some(identity) => {
            request.push(1);
            request.extend_from_slice(&identity);
        }
        None => {
            request.push(0);
            request.extend_from_slice(&[0; CONFIG_ID_BYTES]);
        }
    }
    request.push(u8::from(binding.protected_compiler_closure_v2));
    request.extend_from_slice(&binding.compiler_closure_sha256);
    request.extend_from_slice(&binding.rustc_executable_sha256);
    request.extend_from_slice(&binding.retained_object_binding_sha256);
    if let Some(identity) = binding.native_profile_identity {
        request.push(3);
        request.extend_from_slice(&identity);
    }
    request.extend_from_slice(&challenge);
    let authentication = keyed_digest(
        if native {
            REQUEST_AUTH_DOMAIN_V4
        } else {
            REQUEST_AUTH_DOMAIN
        },
        secret,
        &[&request],
    );
    request.extend_from_slice(&authentication);
    debug_assert_eq!(request.len(), binding.request_bytes());
    request
}

#[cfg(test)]
pub(super) fn response_bytes(
    secret: &[u8; SECRET_BYTES],
    challenge: [u8; CHALLENGE_BYTES],
    request_auth: [u8; REQUEST_AUTH_BYTES],
) -> [u8; RESPONSE_BYTES] {
    response_bytes_with_domain(RESPONSE_AUTH_DOMAIN, secret, challenge, request_auth)
}

pub(super) fn response_bytes_for(
    binding: CapabilityBindingV3,
    secret: &[u8; SECRET_BYTES],
    challenge: [u8; CHALLENGE_BYTES],
    request_auth: [u8; REQUEST_AUTH_BYTES],
) -> [u8; RESPONSE_BYTES] {
    response_bytes_with_domain(
        if binding.native_profile_identity.is_some() {
            RESPONSE_AUTH_DOMAIN_V4
        } else {
            RESPONSE_AUTH_DOMAIN
        },
        secret,
        challenge,
        request_auth,
    )
}

fn response_bytes_with_domain(
    domain: &[u8],
    secret: &[u8; SECRET_BYTES],
    challenge: [u8; CHALLENGE_BYTES],
    request_auth: [u8; REQUEST_AUTH_BYTES],
) -> [u8; RESPONSE_BYTES] {
    let authentication = keyed_digest(domain, secret, &[&challenge, &request_auth]);
    let mut response = [0_u8; RESPONSE_BYTES];
    response[0] = 1;
    response[1..].copy_from_slice(&authentication);
    response
}
