//! 服务端集成凭据的认证加密；各用途独立派生密钥。
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

/// 以服务端密钥和用途派生 AES-256 密钥，禁止跨用途重放密文。
fn cipher(server_key: &str, purpose: &str) -> Aes256Gcm {
    let key = Sha256::digest(
        [
            b"oxide:integrations:v1:".as_slice(),
            purpose.as_bytes(),
            b":".as_slice(),
            server_key.as_bytes(),
        ]
        .concat(),
    );
    Aes256Gcm::new_from_slice(&key).expect("SHA-256 输出固定为 32 字节")
}

/// 加密任意服务端凭据，每次写入使用随机 nonce。
pub fn seal(value: &[u8], server_key: &str, purpose: &str) -> Result<String> {
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher(server_key, purpose)
        .encrypt(&Nonce::from(nonce), value)
        .map_err(|_| anyhow::anyhow!("加密凭据失败"))?;
    Ok(format!(
        "v1:{}:{}",
        hex::encode(nonce),
        hex::encode(encrypted)
    ))
}

/// 校验并解密密文；失败时不泄漏明文或密钥。
pub fn open(value: &str, server_key: &str, purpose: &str) -> Result<Vec<u8>> {
    let mut parts = value.split(':');
    if parts.next() != Some("v1") {
        bail!("凭据版本无效");
    }
    let nonce = hex::decode(parts.next().context("nonce 缺失")?)?;
    let ciphertext = hex::decode(parts.next().context("密文缺失")?)?;
    if parts.next().is_some() {
        bail!("凭据格式无效");
    }
    let nonce: [u8; 12] = nonce
        .try_into()
        .map_err(|_| anyhow::anyhow!("nonce 长度无效"))?;
    cipher(server_key, purpose)
        .decrypt(&Nonce::from(nonce), ciphertext.as_ref())
        .map_err(|_| anyhow::anyhow!("解密凭据失败"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn purpose_and_server_key_are_required() {
        let encrypted = seal(b"secret", "server-secret", "storage").unwrap();
        assert_eq!(
            open(&encrypted, "server-secret", "storage").unwrap(),
            b"secret"
        );
        assert!(open(&encrypted, "other-secret", "storage").is_err());
        assert!(open(&encrypted, "server-secret", "agent").is_err());
    }
}
