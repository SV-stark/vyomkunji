// Detached CMS / PKCS#7 SignedData assembly.
//
// The token performs the private-key operation, so this module only builds the
// ASN.1 structure. The signed attributes are DER encoded, the `[0] IMPLICIT`
// tag is applied, and the caller signs the digest of those bytes. The resulting
// signature is embedded in a single-signer detached SignedData with the leaf
// certificate and any intermediates supplied by the caller.

use crate::pkcs11::{HashAlgorithm, KeyKind};
use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, EncapsulatedContentInfo, SignedAttributes, SignedData, SignerIdentifier,
    SignerInfo, SignerInfos,
};
use der::asn1::{ObjectIdentifier, OctetString, SetOfVec, UtcTime};
use der::{Any, Decode, Encode, Tag};
use std::time::SystemTime;
use x509_cert::Certificate;
use x509_cert::attr::Attribute;
use x509_cert::spki::AlgorithmIdentifierOwned;

const ID_DATA: &str = "1.2.840.113549.1.7.1";
const ID_SIGNED_DATA: &str = "1.2.840.113549.1.7.2";
const RSA_ENCRYPTION: &str = "1.2.840.113549.1.1.1";
const ID_CONTENT_TYPE: &str = "1.2.840.113549.1.9.3";
const ID_MESSAGE_DIGEST: &str = "1.2.840.113549.1.9.4";
const ID_SIGNING_TIME: &str = "1.2.840.113549.1.9.5";

fn oid(value: &str) -> ObjectIdentifier {
    ObjectIdentifier::new_unwrap(value)
}

/// Build a single-valued CMS attribute. Signed attributes carry exactly one
/// value for content-type and message-digest; the values set is still DER SET
/// encoded as required by RFC 5652.
fn attribute(attribute_type: &str, value: Any) -> Result<Attribute, String> {
    let mut values = SetOfVec::new();
    values.insert(value).map_err(|e| e.to_string())?;
    Ok(Attribute {
        oid: oid(attribute_type),
        values,
    })
}

fn null_params() -> Any {
    Any::null()
}

fn digest_algorithm_identifier(hash: HashAlgorithm) -> AlgorithmIdentifierOwned {
    AlgorithmIdentifierOwned {
        oid: hash.oid(),
        parameters: Some(null_params()),
    }
}

fn signature_algorithm_identifier(hash: HashAlgorithm, kind: KeyKind) -> AlgorithmIdentifierOwned {
    let value = match kind {
        KeyKind::Rsa => RSA_ENCRYPTION,
        KeyKind::Ec => match hash {
            HashAlgorithm::Sha1 => "1.2.840.10045.4.1",
            HashAlgorithm::Sha256 => "1.2.840.10045.4.3.2",
            HashAlgorithm::Sha384 => "1.2.840.10045.4.3.3",
            HashAlgorithm::Sha512 => "1.2.840.10045.4.3.4",
        },
    };
    AlgorithmIdentifierOwned {
        oid: oid(value),
        // ECDSA AlgorithmIdentifiers omit the parameters field entirely.
        parameters: match kind {
            KeyKind::Rsa => Some(null_params()),
            KeyKind::Ec => None,
        },
    }
}

/// Convert a raw PKCS#11 ECDSA signature (`r || s`) into the DER
/// `ECDSA-Sig-Value` structure required by CMS.
fn ecdsa_raw_to_der(raw: &[u8]) -> Result<Vec<u8>, String> {
    if raw.is_empty() || !raw.len().is_multiple_of(2) {
        return Err("invalid ECDSA signature length".to_string());
    }
    let half = raw.len() / 2;

    fn integer(bytes: &[u8]) -> Vec<u8> {
        let mut value = bytes.to_vec();
        while value.len() > 1 && value[0] == 0 {
            value.remove(0);
        }
        if value[0] & 0x80 != 0 {
            value.insert(0, 0);
        }
        let mut out = Vec::with_capacity(value.len() + 2);
        out.push(0x02);
        out.push(value.len() as u8);
        out.extend(value);
        out
    }

    let r = integer(&raw[..half]);
    let s = integer(&raw[half..]);
    let body_len = r.len() + s.len();
    let mut out = Vec::with_capacity(body_len + 4);
    out.push(0x30);
    if body_len < 0x80 {
        out.push(body_len as u8);
    } else {
        out.push(0x81);
        out.push(body_len as u8);
    }
    out.extend(r);
    out.extend(s);
    Ok(out)
}

/// Build a detached CMS SignedData. `sign_digest` is invoked once with the
/// digest of the DER-encoded signed attributes and must return the token
/// signature over that digest (DigestInfo-wrapped for RSA, raw for ECDSA).
pub fn build_detached_cms<F>(
    leaf_der: &[u8],
    chain: &[Vec<u8>],
    content_digest: &[u8],
    hash: HashAlgorithm,
    kind: KeyKind,
    sign_digest: F,
) -> Result<Vec<u8>, String>
where
    F: FnOnce(&[u8]) -> Result<Vec<u8>, String>,
{
    if content_digest.len() != hash.digest_len() {
        return Err(format!(
            "content digest length {} does not match {}",
            content_digest.len(),
            hash.name()
        ));
    }

    let leaf = Certificate::from_der(leaf_der)
        .map_err(|e| format!("could not parse signing certificate: {}", e))?;

    let mut attrs: SignedAttributes = SetOfVec::new();
    attrs
        .insert(attribute(
            ID_CONTENT_TYPE,
            Any::encode_from(&oid(ID_DATA)).map_err(|e| e.to_string())?,
        )?)
        .map_err(|e| e.to_string())?;
    attrs
        .insert(attribute(
            ID_MESSAGE_DIGEST,
            Any::new(Tag::OctetString, content_digest.to_vec()).map_err(|e| e.to_string())?,
        )?)
        .map_err(|e| e.to_string())?;
    // Signing time is optional, but many Indian portal verifiers display it.
    // A failure to read the clock must not block signing, so omit it on error.
    if let Ok(now) = UtcTime::from_system_time(SystemTime::now())
        && let Ok(value) = Any::encode_from(&now)
        && let Ok(attr) = attribute(ID_SIGNING_TIME, value)
    {
        let _ = attrs.insert(attr);
    }

    // Signed attributes are signed as an explicit [0] IMPLICIT SET OF, i.e. the
    // same DER as the SET OF with the outer tag changed from 0x31 to 0xA0.
    let mut signed_attrs_der = attrs.to_der().map_err(|e| e.to_string())?;
    if signed_attrs_der.first() != Some(&0x31) {
        return Err("unexpected signed attributes encoding".to_string());
    }
    signed_attrs_der[0] = 0xA0;

    let signed_attrs_digest = hash.digest(&signed_attrs_der);
    let mut signature = sign_digest(&signed_attrs_digest)?;
    if kind == KeyKind::Ec {
        signature = ecdsa_raw_to_der(&signature)?;
    }

    let digest_alg = digest_algorithm_identifier(hash);
    let signer_info = SignerInfo {
        version: CmsVersion::V1,
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: leaf.tbs_certificate.issuer.clone(),
            serial_number: leaf.tbs_certificate.serial_number.clone(),
        }),
        digest_alg: digest_alg.clone(),
        signed_attrs: Some(attrs),
        signature_algorithm: signature_algorithm_identifier(hash, kind),
        signature: OctetString::new(signature).map_err(|e| e.to_string())?,
        unsigned_attrs: None,
    };

    let mut certs: Vec<CertificateChoices> = vec![CertificateChoices::Certificate(leaf)];
    for der_bytes in chain {
        if let Ok(cert) = Certificate::from_der(der_bytes) {
            certs.push(CertificateChoices::Certificate(cert));
        }
    }
    let certificates = CertificateSet::try_from(certs).map_err(|e| e.to_string())?;

    let mut digest_algorithms = SetOfVec::new();
    digest_algorithms
        .insert(digest_alg)
        .map_err(|e| e.to_string())?;
    let signer_infos = SignerInfos::try_from(vec![signer_info]).map_err(|e| e.to_string())?;

    let signed_data = SignedData {
        version: CmsVersion::V1,
        digest_algorithms,
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: oid(ID_DATA),
            econtent: None,
        },
        certificates: Some(certificates),
        crls: None,
        signer_infos,
    };

    let content = Any::encode_from(&signed_data).map_err(|e| e.to_string())?;
    let content_info = ContentInfo {
        content_type: oid(ID_SIGNED_DATA),
        content,
    };
    content_info.to_der().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::ecdsa_raw_to_der;

    #[test]
    fn ecdsa_signature_is_converted_to_der() {
        let raw = [0x01u8; 64];
        let der = ecdsa_raw_to_der(&raw).unwrap();
        assert_eq!(der[0], 0x30);
        assert!(der.len() > 64);
    }

    #[test]
    fn invalid_ecdsa_length_is_rejected() {
        assert!(ecdsa_raw_to_der(&[0u8; 63]).is_err());
        assert!(ecdsa_raw_to_der(&[]).is_err());
    }
}
