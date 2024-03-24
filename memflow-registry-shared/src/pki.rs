use std::fmt::Write;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use k256::ecdsa::{
    signature::{SignerMut, Verifier},
    Signature, SigningKey, VerifyingKey,
};

use crate::{error::Result, Error};

#[derive(Clone)]
pub struct SignatureGenerator {
    signing_key: SigningKey,
}

impl SignatureGenerator {
    pub fn new<P: AsRef<Path>>(priv_key_file: P) -> Result<Self> {
        let signing_key_pem = fs::read_to_string(priv_key_file)?;
        let signing_key = SigningKey::from_str(&signing_key_pem)?;
        Ok(Self { signing_key })
    }

    /// Signs the payload with the given public key.
    pub fn sign(&mut self, bytes: &[u8]) -> Result<String> {
        let signature: Signature = self.signing_key.sign(bytes);
        let hex = encode_hex(signature.to_der().as_ref());
        Ok(hex)
    }
}

#[derive(Clone)]
pub struct SignatureVerifier {
    verifying_key: VerifyingKey,
}

impl SignatureVerifier {
    pub fn new<P: AsRef<Path>>(public_key_file: P) -> Result<Self> {
        let verifying_key_pem = fs::read_to_string(public_key_file)?;
        Self::with_str(&verifying_key_pem)
    }

    pub fn with_str(verifying_key_pem: &str) -> Result<Self> {
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

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(&mut s, "{:02X}", b).unwrap();
    }
    s
}

fn decode_hex(s: &str) -> Result<Vec<u8>> {
    if s.len() < 2 || s.len() % 2 != 0 {
        return Err(Error::Parse(
            "input must have a length that is a multiple 2".to_owned(),
        ));
    }

    let hex = (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_hex() {
        assert!(decode_hex("12345").is_err());
    }
}
