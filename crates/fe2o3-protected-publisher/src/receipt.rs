use std::path::Path;

use base64::Engine;
use ed25519_dalek::pkcs8::DecodePrivateKey;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::PublisherError;
use crate::bounds::{MAX_PRIVATE_KEY_BYTES, MAX_RECEIPT_BYTES, RECEIPT_LIFETIME_SECS};
use crate::config::valid_id;
use crate::oidc::PublisherRequest;
use crate::secure_fs::read_owner_only;

pub trait ReceiptSigner: Send + Sync {
    fn key_id(&self) -> &str;
    fn sign(&self, message: &[u8]) -> Result<[u8; 64], PublisherError>;
}

pub struct FileReceiptSigner {
    key_id: String,
    key: SigningKey,
}

impl FileReceiptSigner {
    pub fn load(key_id: String, path: &Path) -> Result<Self, PublisherError> {
        crate::process_security::harden_process_for_secrets()?;
        if !valid_id(&key_id) {
            return Err(PublisherError::Config);
        }
        let bytes = read_owner_only(path, MAX_PRIVATE_KEY_BYTES)?;
        let pem = match String::from_utf8(bytes) {
            Ok(pem) => Zeroizing::new(pem),
            Err(error) => {
                let mut bytes = error.into_bytes();
                bytes.zeroize();
                return Err(PublisherError::Config);
            }
        };
        let key = SigningKey::from_pkcs8_pem(&pem);
        let key = key.map_err(|_| PublisherError::Config)?;
        Ok(Self { key_id, key })
    }
}

impl ReceiptSigner for FileReceiptSigner {
    fn key_id(&self) -> &str {
        &self.key_id
    }

    fn sign(&self, message: &[u8]) -> Result<[u8; 64], PublisherError> {
        Ok(self.key.sign(message).to_bytes())
    }
}

pub struct ReceiptArtifact {
    pub evidence_identity: String,
    pub response: Vec<u8>,
}

pub fn request_identity(raw: &[u8]) -> String {
    domain_hash(b"fe2o3-protected-publisher-request-identity-v1\0", raw)
}

pub fn raw_request_sha256(raw: &[u8]) -> String {
    hex_digest(Sha256::digest(raw).as_slice())
}

pub fn build_artifact(
    request: &PublisherRequest,
    request_identity: &str,
    request_sha256: &str,
    issued_at: i64,
    signature_domain: &str,
    signer: &dyn ReceiptSigner,
) -> Result<ReceiptArtifact, PublisherError> {
    let expires_at = issued_at
        .checked_add(RECEIPT_LIFETIME_SECS)
        .ok_or(PublisherError::Signing)?;
    let challenge = domain_hash(
        b"fe2o3-protected-publisher-challenge-v1\0",
        request_identity.as_bytes(),
    );
    let key_id = signer.key_id();
    let unsigned = format!(
        "publisher_contract_receipt_schema_version\t2\n\
publisher_identity\t{key_id}\n\
publisher_key_role\tpublisher\n\
destination_contract\texternal-protected-portable-archive-v2\n\
logical_destination\t{logical_destination}\n\
archive_sha256\t{archive_sha256}\n\
manifest_path\t{manifest_path}\n\
manifest_sha256\t{manifest_sha256}\n\
source_commit\t{source_commit}\n\
source_tree\t{source_tree}\n\
target\t{target}\n\
hardware_lane\t{hardware_lane}\n\
baseline_status_sha256\t{baseline_status_sha256}\n\
candidate_status_sha256\t{candidate_status_sha256}\n\
default_tip\t{default_tip}\n\
candidate_head\t{candidate_head}\n\
freshness_challenge\t{challenge}\n\
issued_at_unix\t{issued_at}\n\
expires_at_unix\t{expires_at}\n\
signature_schema_version\t1\n\
signature_domain\t{signature_domain}\n\
signature_role\tpublisher\n\
signature_algorithm\ted25519\n\
signing_key_id\t{key_id}\n",
        logical_destination = request.logical_destination,
        archive_sha256 = request.archive_sha256,
        manifest_path = request.manifest_path,
        manifest_sha256 = request.manifest_sha256,
        source_commit = request.source_commit,
        source_tree = request.source_tree,
        target = request.target,
        hardware_lane = request.hardware_lane,
        baseline_status_sha256 = request.baseline_status_sha256,
        candidate_status_sha256 = request.candidate_status_sha256,
        default_tip = request.default_tip,
        candidate_head = request.candidate_head,
    );
    let signature = signer.sign(unsigned.as_bytes())?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(signature);
    let receipt = format!("{unsigned}signature_base64\t{encoded}\n").into_bytes();
    if receipt.len() > MAX_RECEIPT_BYTES {
        return Err(PublisherError::Signing);
    }
    let evidence_identity = domain_hash(
        b"fe2o3-protected-publisher-evidence-identity-v1\0",
        &receipt,
    );
    let receipt_base64 = base64::engine::general_purpose::STANDARD.encode(&receipt);
    let response = format!(
        "{{\"challenge\":\"{challenge}\",\"publisher_receipt_base64\":\"{receipt_base64}\",\"request_sha256\":\"{request_sha256}\",\"schema_version\":1}}\n"
    )
    .into_bytes();
    Ok(ReceiptArtifact {
        evidence_identity,
        response,
    })
}

fn domain_hash(domain: &[u8], value: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(value);
    hex_digest(hash.finalize().as_slice())
}

fn hex_digest(value: &[u8]) -> String {
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
pub(crate) struct TestSigner {
    key_id: String,
    key: SigningKey,
    fail: bool,
}

#[cfg(test)]
impl TestSigner {
    pub(crate) fn new(key_id: &str) -> Self {
        Self {
            key_id: key_id.into(),
            key: SigningKey::from_bytes(&[7; 32]),
            fail: false,
        }
    }

    pub(crate) fn failing(key_id: &str) -> Self {
        Self {
            key_id: key_id.into(),
            key: SigningKey::from_bytes(&[8; 32]),
            fail: true,
        }
    }

    pub(crate) fn public_key_pem(&self) -> String {
        use ed25519_dalek::pkcs8::EncodePublicKey;
        use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
        self.key
            .verifying_key()
            .to_public_key_pem(LineEnding::LF)
            .unwrap()
    }
}

#[cfg(test)]
impl ReceiptSigner for TestSigner {
    fn key_id(&self) -> &str {
        &self.key_id
    }
    fn sign(&self, message: &[u8]) -> Result<[u8; 64], PublisherError> {
        if self.fail {
            Err(PublisherError::Signing)
        } else {
            Ok(self.key.sign(message).to_bytes())
        }
    }
}

#[cfg(test)]
mod file_signer_tests {
    use super::*;
    use ed25519_dalek::pkcs8::EncodePrivateKey;
    use ed25519_dalek::pkcs8::spki::der::pem::LineEnding;
    use ed25519_dalek::{Signature, Verifier};
    use std::fs::hard_link;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use crate::test_support::secure_tempdir;

    #[test]
    fn owner_only_single_link_pkcs8_key_signs() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.pem");
        let key = SigningKey::from_bytes(&[19; 32]);
        let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
        std::fs::write(&path, pem.as_bytes()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let signer = FileReceiptSigner::load("publisher-v1".into(), &path).unwrap();
        let message = b"bounded publisher receipt";
        let signature = Signature::from_bytes(&signer.sign(message).unwrap());
        key.verifying_key().verify(message, &signature).unwrap();
    }

    #[test]
    fn permissive_symlink_and_hardlink_keys_reject() {
        let temp = secure_tempdir();
        let path = temp.path().join("publisher.pem");
        let key = SigningKey::from_bytes(&[23; 32]);
        let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
        std::fs::write(&path, pem.as_bytes()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(FileReceiptSigner::load("publisher-v1".into(), &path).is_err());

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let link = temp.path().join("publisher-link.pem");
        symlink(&path, &link).unwrap();
        assert!(FileReceiptSigner::load("publisher-v1".into(), &link).is_err());
        let hard = temp.path().join("publisher-hard.pem");
        hard_link(&path, &hard).unwrap();
        assert!(FileReceiptSigner::load("publisher-v1".into(), &hard).is_err());
    }
}
