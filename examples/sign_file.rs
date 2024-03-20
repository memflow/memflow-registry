use k256::{
    ecdsa::{
        signature::{Keypair, Signer, Verifier},
        Signature, SigningKey, VerifyingKey,
    },
    pkcs8::{EncodePrivateKey, LineEnding},
    SecretKey,
};
use std::str::FromStr;

use std::{fmt::Write, num::ParseIntError};

pub fn encode_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(&mut s, "{:02x}", b).unwrap();
    }
    s
}

// https://docs.rs/k256/latest/k256/ecdsa/index.html
pub fn main() {
    // parse arguments
    let file_name = std::env::args().nth(1).expect("no file_name given");
    let private_key_file_name = std::env::args()
        .nth(2)
        .expect("no private_key file_name given");

    // load private key file
    let signing_key_pem = std::fs::read_to_string(private_key_file_name).unwrap();
    let signing_key = SigningKey::from_str(&signing_key_pem).unwrap();

    // TODO: clamp file size
    let message = std::fs::read(&file_name).unwrap();

    let signature: Signature = signing_key.sign(&message[..]);
    println!("signature for `{}`: {:?}", file_name, signature.to_der());

    // verify signature with public key
    let verifying_key_pem = std::fs::read_to_string("ec-secp256k1-pub-key.pem").unwrap();
    //let verifying_key = VerifyingKey::from(&signing_key);
    let verifying_key = VerifyingKey::from_str(&verifying_key_pem).unwrap();
    assert!(verifying_key.verify(&message[..], &signature).is_ok());
}
