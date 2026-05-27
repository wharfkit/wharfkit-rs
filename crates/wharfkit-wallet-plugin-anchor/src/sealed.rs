use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use aes::Aes256;
use antelope::chain::checksum::{Checksum256, Checksum512};
use antelope::chain::private_key::PrivateKey;
use antelope::chain::public_key::PublicKey;
use antelope::chain::{Decoder, Encoder, Packer};
use antelope::StructPacker;
use thiserror::Error;

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

#[derive(Debug, Error)]
pub enum SealError {
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("envelope: {0}")]
    Envelope(String),
}

// Field order is load-bearing — Anchor's listener decodes positionally.
#[derive(StructPacker, Default, Debug, Clone)]
pub struct SealedMessage {
    pub from: PublicKey,
    pub nonce: u64,
    pub ciphertext: Vec<u8>,
    pub checksum: u32,
}

// key_material = sha512(u64_le(nonce) || ecdh_secret); aes_key = [..32];
// iv = [32..48]; the full 64 bytes feed the envelope checksum.
fn derive(secret: &Checksum512, nonce: u64) -> ([u8; 32], [u8; 16], [u8; 64]) {
    let mut buf = Vec::with_capacity(8 + 64);
    buf.extend_from_slice(&nonce.to_le_bytes());
    buf.extend_from_slice(&secret.data);
    let hash = Checksum512::hash(buf);
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash.data[..32]);
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&hash.data[32..48]);
    let mut full = [0u8; 64];
    full.copy_from_slice(&hash.data);
    (key, iv, full)
}

fn envelope_checksum(material: &[u8; 64]) -> u32 {
    let h = Checksum256::hash(material.to_vec());
    u32::from_le_bytes([h.data[0], h.data[1], h.data[2], h.data[3]])
}

pub fn seal(
    plaintext: &[u8],
    sender_priv: &PrivateKey,
    receiver_pub: &PublicKey,
    nonce: u64,
) -> Result<SealedMessage, SealError> {
    let secret = sender_priv.shared_secret(receiver_pub);
    let (key, iv, material) = derive(&secret, nonce);
    let ct = Aes256CbcEnc::new(&key.into(), &iv.into()).encrypt_padded_vec_mut::<Pkcs7>(plaintext);
    Ok(SealedMessage {
        from: sender_priv.to_public(),
        nonce,
        ciphertext: ct,
        checksum: envelope_checksum(&material),
    })
}

pub fn pack_sealed_message(msg: &SealedMessage) -> Vec<u8> {
    let mut enc = Encoder::new(msg.size());
    msg.pack(&mut enc);
    enc.get_bytes().to_vec()
}

pub fn unseal(msg: &SealedMessage, receiver_priv: &PrivateKey) -> Result<Vec<u8>, SealError> {
    let secret = receiver_priv.shared_secret(&msg.from);
    let (key, iv, material) = derive(&secret, msg.nonce);
    if envelope_checksum(&material) != msg.checksum {
        return Err(SealError::Envelope(
            "checksum mismatch (wrong key/nonce or tampered envelope)".into(),
        ));
    }
    Aes256CbcDec::new(&key.into(), &iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(&msg.ciphertext)
        .map_err(|e| SealError::Crypto(format!("aes decrypt: {e:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENDER_WIF: &str = "5Jtoxgny5tT7NiNFp1MLogviuPJ9NniWjnU4wKzaX4t7pL4kJ8s";
    const RECEIVER_WIF: &str = "5KQwrPbwdL6PhXujxW37FSSQZ1JiwsST4cqQzDeyXtP79zkvFD3";

    fn keypairs() -> (PrivateKey, PublicKey, PrivateKey, PublicKey) {
        let sender = PrivateKey::from_str(SENDER_WIF, false).unwrap();
        let receiver = PrivateKey::from_str(RECEIVER_WIF, false).unwrap();
        let sender_pub = sender.to_public();
        let receiver_pub = receiver.to_public();
        (sender, sender_pub, receiver, receiver_pub)
    }

    #[test]
    fn seal_unseal_round_trips() {
        let (sp, _spub, rp, rpub) = keypairs();
        let plaintext = b"esr://AgABAA";
        let env = seal(plaintext, &sp, &rpub, 0xCAFE_BABE_DEAD_BEEF).unwrap();
        let pt = unseal(&env, &rp).unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn envelope_carries_sender_public_key() {
        let (sp, spub, _rp, rpub) = keypairs();
        let env = seal(b"hi", &sp, &rpub, 1).unwrap();
        assert_eq!(env.from.as_string(), spub.as_string());
    }

    #[test]
    fn pack_sealed_message_is_abi_binary_not_json() {
        let (sp, _spub, _rp, rpub) = keypairs();
        let env = seal(b"hi", &sp, &rpub, 1).unwrap();
        let bytes = pack_sealed_message(&env);
        assert!(!bytes.is_empty());
        assert_ne!(bytes[0], b'{', "wire format must not be JSON");
        assert_eq!(bytes[0], 0x00, "first byte = K1 type tag of `from`");
        let mut decoded = SealedMessage::default();
        decoded.unpack(&bytes);
        assert_eq!(decoded.from.as_string(), env.from.as_string());
        assert_eq!(decoded.nonce, env.nonce);
        assert_eq!(decoded.ciphertext, env.ciphertext);
        assert_eq!(decoded.checksum, env.checksum);
    }

    #[test]
    fn envelope_checksum_field_is_set() {
        let (sp, _spub, _rp, rpub) = keypairs();
        let env = seal(b"hi", &sp, &rpub, 1).unwrap();
        assert_ne!(
            env.checksum, 0,
            "checksum is non-zero with overwhelming probability"
        );
    }

    #[test]
    fn tampered_checksum_rejected() {
        let (sp, _spub, rp, rpub) = keypairs();
        let mut env = seal(b"hi", &sp, &rpub, 1).unwrap();
        env.checksum = env.checksum.wrapping_add(1);
        let err = unseal(&env, &rp).unwrap_err();
        assert!(matches!(err, SealError::Envelope(_)));
    }

    #[test]
    fn unseal_with_wrong_key_fails() {
        let (sp, _spub, _rp, rpub) = keypairs();
        let env = seal(b"hi", &sp, &rpub, 1).unwrap();
        let err = unseal(&env, &sp).unwrap_err();
        assert!(matches!(err, SealError::Envelope(_) | SealError::Crypto(_)));
    }
}
