//! Nostr publishing identity (D11): the npub signs, the app never logs in.
//! Key creation is local; the nsec is only ever stored NIP-49-encrypted.

use bech32::{Bech32, Hrp};
use k256::schnorr::signature::hazmat::PrehashSigner;
use k256::schnorr::{Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub npub: String,
    pub pubkey_hex: String,
}

/// Bech32-encode raw bytes with the given hrp (npub / nsec).
fn encode(hrp: &str, bytes: &[u8]) -> Result<String> {
    let hrp = Hrp::parse(hrp).map_err(|e| crate::CoreError::Msg(e.to_string()))?;
    bech32::encode::<Bech32>(hrp, bytes).map_err(|e| crate::CoreError::Msg(e.to_string()))
}

fn err<T>(s: impl Into<String>) -> Result<T> {
    crate::msg(s)
}

/// A locally generated keypair. `nsec` is returned once, for the backup
/// flow; callers must pass it to `encrypt_nsec` and never store it raw.
pub struct Keypair {
    pub identity: Identity,
    pub nsec: String,
    secret: [u8; 32],
}

pub fn generate_keypair() -> Result<Keypair> {
    let mut secret = [0u8; 32];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut secret);
    keypair_from_secret(secret)
}

pub fn keypair_from_secret(secret: [u8; 32]) -> Result<Keypair> {
    let sk = SigningKey::from_bytes(k256::FieldBytes::from_slice(&secret))
        .map_err(|e| crate::CoreError::Msg(e.to_string()))?;
    let vk: VerifyingKey = *sk.verifying_key();
    let pubkey = vk.to_bytes();
    Ok(Keypair {
        identity: Identity {
            npub: encode("npub", &pubkey)?,
            pubkey_hex: hex::encode(pubkey),
        },
        nsec: encode("nsec", &secret)?,
        secret,
    })
}

/// NIP-49-style encrypted secret: scrypt -> ChaCha20-Poly1305.
/// Stored blob is base64-free hex of salt(16) || nonce(12) || ciphertext.
pub fn encrypt_nsec(secret_hex_or_nsec: &str, password: &str) -> Result<String> {
    use chacha20poly1305::{aead::{Aead, KeyInit}, XChaCha20Poly1305, XNonce};
    let secret = decode_secret(secret_hex_or_nsec)?;
    let mut salt = [0u8; 16];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut salt);
    let params = scrypt::Params::new(14, 8, 1, 32).map_err(|e| crate::CoreError::Msg(e.to_string()))?;
    let mut key = [0u8; 32];
    scrypt::scrypt(password.as_bytes(), &salt, &params, &mut key)
        .map_err(|e| crate::CoreError::Msg(e.to_string()))?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let mut nonce = [0u8; 24];
    rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut nonce);
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), secret.as_ref())
        .map_err(|_| crate::CoreError::Msg("encryption failed".into()))?;
    let mut blob = Vec::new();
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ct);
    Ok(hex::encode(blob))
}

pub fn decrypt_nsec(blob_hex: &str, password: &str) -> Result<Keypair> {
    use chacha20poly1305::{aead::{Aead, KeyInit}, XChaCha20Poly1305, XNonce};
    let blob = hex::decode(blob_hex.trim())
        .map_err(|_| crate::CoreError::Msg("stored identity is corrupt".into()))?;
    if blob.len() < 16 + 24 + 16 {
        return err("stored identity is corrupt");
    }
    let (salt, rest) = blob.split_at(16);
    let (nonce, ct) = rest.split_at(24);
    let params = scrypt::Params::new(14, 8, 1, 32).map_err(|e| crate::CoreError::Msg(e.to_string()))?;
    let mut key = [0u8; 32];
    scrypt::scrypt(password.as_bytes(), salt, &params, &mut key)
        .map_err(|e| crate::CoreError::Msg(e.to_string()))?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let pt = cipher
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| crate::CoreError::Msg("wrong password for this identity".into()))?;
    if pt.len() != 32 {
        return err("stored identity is corrupt");
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&pt);
    keypair_from_secret(secret)
}

fn decode_secret(s: &str) -> Result<[u8; 32]> {
    let s = s.trim();
    // Accept raw hex, otherwise treat as bech32 nsec.
    if let Ok(bytes) = hex::decode(s) {
        if bytes.len() == 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(&bytes);
            return Ok(out);
        }
    }
    let (_hrp, data) = bech32::decode(s).map_err(|e| crate::CoreError::Msg(e.to_string()))?;
    if data.len() != 32 {
        return err("invalid nsec length");
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&data);
    Ok(out)
}

/// A signed Nostr event, ready to hand to a relay (relay I/O is on hold).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub pubkey: String,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

/// Build and BIP340-sign an event. kind 30023 = NIP-23 long-form narrative.
pub fn sign_event(kp: &Keypair, kind: u32, tags: Vec<Vec<String>>, content: &str, created_at: u64) -> Result<Event> {
    let pubkey_hex = kp.identity.pubkey_hex.clone();
    let ser = serde_json::json!([0, pubkey_hex, created_at, kind, tags, content]).to_string();
    let id = hex::encode(Sha256::digest(ser.as_bytes()));
    let sk = SigningKey::from_bytes(k256::FieldBytes::from_slice(&kp.secret))
        .map_err(|e| crate::CoreError::Msg(e.to_string()))?;
    let sig: Signature = sk
        .sign_prehash(&Sha256::digest(ser.as_bytes()))
        .map_err(|e| crate::CoreError::Msg(e.to_string()))?;
    Ok(Event {
        id,
        pubkey: pubkey_hex,
        created_at,
        kind,
        tags,
        content: content.to_string(),
        sig: hex::encode(sig.to_bytes()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_and_encrypt() {
        let kp = keypair_from_secret([7u8; 32]).unwrap();
        assert!(kp.identity.npub.starts_with("npub1"));
        assert!(kp.nsec.starts_with("nsec1"));
        let blob = encrypt_nsec(&kp.nsec, "correct horse").unwrap();
        assert!(!blob.contains(&kp.nsec));
    }

    #[test]
    fn sign_nip23() {
        let kp = keypair_from_secret([9u8; 32]).unwrap();
        let ev = sign_event(&kp, 30023, vec![vec!["d".into(), "1".into()]], "# Hello", 1_700_000_000).unwrap();
        assert_eq!(ev.kind, 30023);
        assert_eq!(ev.sig.len(), 128);
    }
}
