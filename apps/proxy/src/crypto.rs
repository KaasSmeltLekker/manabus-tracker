use aes::Aes256;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use cbc::{Decryptor, Encryptor};
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};

type Aes256CbcEnc = Encryptor<Aes256>;
type Aes256CbcDec = Decryptor<Aes256>;

pub struct AesKey {
    key: [u8; 32],
    iv: [u8; 16],
}

impl AesKey {
    pub fn from_secret(secret: &str) -> Result<Self> {
        let bytes = secret.as_bytes();
        if bytes.len() != 32 {
            return Err(anyhow!("key must be exactly 32 ASCII bytes, got {}", bytes.len()));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(bytes);
        let mut iv = [0u8; 16];
        iv.copy_from_slice(&bytes[..16]);
        Ok(Self { key, iv })
    }

    pub fn encrypt_to_b64(&self, plaintext: &str) -> String {
        let ct = self.encrypt_raw(plaintext.as_bytes());
        general_purpose::STANDARD.encode(ct)
    }

    fn encrypt_raw(&self, plaintext: &[u8]) -> Vec<u8> {
        let enc = Aes256CbcEnc::new(&self.key.into(), &self.iv.into());
        enc.encrypt_padded_vec_mut::<aes::cipher::block_padding::Pkcs7>(plaintext)
    }

    pub fn decrypt_from_b64(&self, b64: &str) -> Result<String> {
        let ct = general_purpose::STANDARD.decode(b64)?;
        self.decrypt_raw(&ct)
    }

    fn decrypt_raw(&self, ciphertext: &[u8]) -> Result<String> {
        let dec = Aes256CbcDec::new(&self.key.into(), &self.iv.into());
        let pt = dec
            .decrypt_padded_vec_mut::<aes::cipher::block_padding::Pkcs7>(ciphertext)
            .map_err(|e| anyhow!("decrypt failed: {e}"))?;
        Ok(String::from_utf8(pt)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_matches_captured_traffic() {
        let key = AesKey::from_secret("0123456789abcdef0123456789abcdef").unwrap();
        let ct = key.encrypt_to_b64("pref=3501");
        let pt = key.decrypt_from_b64(&ct).unwrap();
        assert_eq!(pt, "pref=3501");
    }
}
