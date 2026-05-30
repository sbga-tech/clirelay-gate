use std::str;

use anyhow::{Context, Result, anyhow};
use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit},
};
use sha2::{Digest, Sha256};

use crate::config::EncryptionKey;

#[derive(Clone)]
pub struct Crypto {
    cipher: ChaCha20Poly1305,
}

impl Crypto {
    pub fn new(key: &EncryptionKey) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new_from_slice(key.as_bytes())
                .expect("validated encryption key length"),
        }
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<EncryptedSecret> {
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|err| anyhow!("generate encryption nonce: {err}"))?;
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|_| anyhow!("encrypt API key"))?;
        Ok(EncryptedSecret {
            ciphertext,
            nonce: nonce.to_vec(),
        })
    }

    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<String> {
        if nonce.len() != 12 {
            return Err(anyhow!("invalid API key nonce length"));
        }
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| anyhow!("decrypt API key"))?;
        String::from_utf8(plaintext).context("API key plaintext is not UTF-8")
    }
}

pub struct EncryptedSecret {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
}

pub fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hex::encode(hasher.finalize())
}

const API_KEY_RANDOM_BYTES: usize = 16;
const API_KEY_HEX_CHARS: usize = API_KEY_RANDOM_BYTES * 2;

pub fn generate_api_key(prefix: &str) -> Result<String> {
    let mut bytes = [0u8; API_KEY_RANDOM_BYTES];
    getrandom::fill(&mut bytes).map_err(|err| anyhow!("generate API key random bytes: {err}"))?;

    let mut encoded = [0u8; API_KEY_HEX_CHARS];
    hex::encode_to_slice(bytes, &mut encoded).expect("hex buffer length is exact");

    let mut key = String::with_capacity(prefix.len() + API_KEY_HEX_CHARS);
    key.push_str(prefix);
    key.push_str(str::from_utf8(&encoded).expect("hex output is valid UTF-8"));
    Ok(key)
}
