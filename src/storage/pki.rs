use k256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use std::fs;
use std::path::Path;
use std::str::FromStr;

use crate::error::Result;

#[derive(Clone)]
pub struct SignatureVerifier {
    verifying_key: VerifyingKey,
}

impl SignatureVerifier {
    pub fn new<P: AsRef<Path>>(public_key_file: P) -> Result<Self> {
        let verifying_key_pem = fs::read_to_string(public_key_file)?;
        let verifying_key = VerifyingKey::from_str(&verifying_key_pem)?;
        Ok(Self { verifying_key })
    }

    /// Checks if the signature is valid for the given data.
    pub fn is_valid(&self, bytes: &[u8], signature: &str) -> Result<()> {
        let hex = decode_hex(signature)?;
        let signature = Signature::from_der(&hex[..])?;
        Ok(self.verifying_key.verify(bytes, &signature)?)
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>> {
    Ok((0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()?)
}
