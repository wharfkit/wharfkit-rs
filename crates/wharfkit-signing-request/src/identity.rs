use antelope::chain::action::PermissionLevel;
use antelope::chain::checksum::Checksum256;
use antelope::chain::key_type::KeyType;
use antelope::chain::name::Name;
use antelope::chain::private_key::PrivateKey;
use antelope::chain::public_key::PublicKey;
use antelope::chain::signature::Signature;
use antelope::chain::{Decoder, Encoder, Packer};
use antelope::StructPacker;

use crate::error::EsrError;
use crate::options::EsrOptions;
use crate::request::{CallbackSpec, SigningRequest};

#[derive(StructPacker, Default, Clone)]
pub struct LinkInfo {
    pub expiration: antelope::chain::time::TimePointSec,
}

#[derive(StructPacker, Default, Debug, Clone)]
pub struct BuoySession {
    pub session_name: Name,
    pub request_key: PublicKey,
    pub user_agent: String,
}

pub fn sanitize_app_name(raw: &str) -> String {
    let mut out = String::with_capacity(12);
    for c in raw.chars() {
        let lc = c.to_ascii_lowercase();
        if matches!(lc, 'a'..='z' | '1'..='5' | '.') {
            out.push(lc);
            if out.len() >= 12 {
                break;
            }
        }
    }
    if out.is_empty() {
        out.push_str("wharfkit");
    }
    out
}

pub struct IdentityRequest {
    pub request: SigningRequest,
    pub same_device_request: SigningRequest,
    pub callback: String,
    pub request_key: PublicKey,
    pub private_key: PrivateKey,
}

pub struct IdentityRequestArgs {
    pub chain_id: Checksum256,
    pub buoy_url: String,
    pub uuid: uuid::Uuid,
    pub app_name: String,
    pub user_agent: String,
}

impl IdentityRequest {
    pub fn create(args: IdentityRequestArgs, opts: &EsrOptions) -> Result<Self, EsrError> {
        let private_key = PrivateKey::random(KeyType::K1)
            .map_err(|e| EsrError::Internal(format!("PrivateKey::random: {e}")))?;
        let request_key = private_key.to_public();
        Self::create_with_keypair(args, opts, private_key, request_key)
    }

    pub fn create_with_keypair(
        args: IdentityRequestArgs,
        opts: &EsrOptions,
        private_key: PrivateKey,
        request_key: PublicKey,
    ) -> Result<Self, EsrError> {
        let callback_url = format!("{}/{}", args.buoy_url.trim_end_matches('/'), args.uuid);

        let mut request = SigningRequest::create_identity(
            args.chain_id,
            None,
            None,
            Some(CallbackSpec {
                url: callback_url.clone(),
                background: true,
            }),
            opts,
        )?;

        let session = BuoySession {
            session_name: Name::new_from_str(&sanitize_app_name(&args.app_name)),
            request_key: request_key.clone(),
            user_agent: args.user_agent.clone(),
        };
        let mut enc = Encoder::new(session.size());
        session.pack(&mut enc);
        request.set_info_bytes("link", enc.get_bytes().to_vec());

        let same_device_request = request.clone();

        Ok(IdentityRequest {
            request,
            same_device_request,
            callback: callback_url,
            request_key,
            private_key,
        })
    }
}

#[derive(Debug, Clone)]
pub struct IdentityProof {
    pub signer: PermissionLevel,
    pub signature: Signature,
    pub recovered_key: PublicKey,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_with_keypair_emits_anchor_compatible_uri() {
        let priv_key =
            PrivateKey::from_str("5Jtoxgny5tT7NiNFp1MLogviuPJ9NniWjnU4wKzaX4t7pL4kJ8s", false)
                .unwrap();
        let pub_key = priv_key.to_public();
        let uuid = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000000").unwrap();
        let eos_id = antelope::chain::checksum::Checksum256::from_hex(
            "aca376f206b8fc25a6ed44dbdc66547c36c6c33e3a119ffbeaef943642f0e906",
        )
        .unwrap();

        let req = IdentityRequest::create_with_keypair(
            IdentityRequestArgs {
                chain_id: eos_id,
                buoy_url: "https://cb.anchor.link".into(),
                uuid,
                app_name: "wharfkitgodot".into(),
                user_agent: "@wharfkit-rs test".into(),
            },
            &EsrOptions::offline(),
            priv_key,
            pub_key.clone(),
        )
        .unwrap();

        let uri = req.request.encode(true, true, "esr:").unwrap();
        eprintln!("SAMPLE IDENTITY URI: {uri}");
        assert!(uri.starts_with("esr://"));

        let parsed =
            crate::request::SigningRequest::from_uri(&uri, &EsrOptions::offline()).unwrap();
        assert!(parsed.is_identity());
        assert_eq!(
            parsed.callback.as_deref(),
            Some("https://cb.anchor.link/00000000-0000-4000-8000-000000000000")
        );
        let link = parsed
            .info
            .iter()
            .find(|kv| kv.key == "link")
            .expect("link info entry");
        let mut session = BuoySession::default();
        session.unpack(&link.value);
        assert_eq!(session.session_name.to_string(), "wharfkitgodo");
        assert_eq!(session.request_key.as_string(), pub_key.as_string());
        assert_eq!(session.user_agent, "@wharfkit-rs test");
    }

    #[test]
    fn create_with_keypair_emits_buoy_callback_url() {
        let priv_key =
            PrivateKey::from_str("5Jtoxgny5tT7NiNFp1MLogviuPJ9NniWjnU4wKzaX4t7pL4kJ8s", false)
                .unwrap();
        let pub_key = priv_key.to_public();

        let uuid = uuid::Uuid::new_v4();
        let req = IdentityRequest::create_with_keypair(
            IdentityRequestArgs {
                chain_id: Checksum256::default(),
                buoy_url: "https://cb.anchor.link".into(),
                uuid,
                app_name: "wharfkit".into(),
                user_agent: String::new(),
            },
            &EsrOptions::offline(),
            priv_key,
            pub_key.clone(),
        )
        .unwrap();

        assert_eq!(
            req.callback,
            format!("https://cb.anchor.link/{uuid}"),
            "callback URL = relay + UUID"
        );
        assert_eq!(
            req.request_key.as_string(),
            pub_key.as_string(),
            "request_key returned verbatim"
        );
        assert!(req.request.info.iter().any(|kv| kv.key == "link"));
    }

    #[test]
    fn sanitize_app_name_handles_common_inputs() {
        assert_eq!(sanitize_app_name("WharfKit Slice 3 Sample"), "wharfkitslic");
        assert_eq!(sanitize_app_name("wharfkitgodot"), "wharfkitgodo");
        assert_eq!(sanitize_app_name("My App!"), "myapp");
        assert_eq!(sanitize_app_name(""), "wharfkit");
        assert_eq!(sanitize_app_name("!!!"), "wharfkit");
    }
}
