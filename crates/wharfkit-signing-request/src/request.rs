use antelope::chain::action::{Action, PermissionLevel};
use antelope::chain::checksum::Checksum256;
use antelope::chain::name::Name;
use antelope::chain::time::TimePointSec;
use antelope::chain::transaction::{Transaction, TransactionExtension, TransactionHeader};
use antelope::chain::varint::VarUint32;
use antelope::serializer::{Decoder, Encoder, Packer};
use antelope::StructPacker;
use serde::{Deserialize, Serialize};

use crate::codec::{base64url_decode, base64url_encode, deflate, inflate};
use crate::error::EsrError;
use crate::options::EsrOptions;
use crate::resolved::ResolvedSigningRequest;

pub const PROTOCOL_VERSION: u8 = 3;

#[derive(Clone, PartialEq, Eq, Default)]
pub enum RequestKind {
    Action,
    #[default]
    Actions,
    Transaction {
        header: TransactionHeader,
        context_free_actions: Vec<Action>,
        transaction_extensions: Vec<TransactionExtension>,
    },
    Identity {
        scope: Option<Name>,
        permission: Option<PermissionLevel>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainIdVariant {
    Alias(u8),
    Id(Checksum256),
}

impl Default for ChainIdVariant {
    fn default() -> Self {
        ChainIdVariant::Id(Checksum256::default())
    }
}

impl ChainIdVariant {
    pub fn to_checksum(&self) -> Checksum256 {
        match self {
            ChainIdVariant::Id(c) => *c,
            ChainIdVariant::Alias(a) => alias_to_chain_id(*a).unwrap_or_default(),
        }
    }
}

impl Packer for ChainIdVariant {
    fn size(&self) -> usize {
        match self {
            ChainIdVariant::Alias(_) => 1 + 1,
            ChainIdVariant::Id(_) => 1 + 32,
        }
    }

    fn pack(&self, enc: &mut Encoder) -> usize {
        let pos = enc.get_size();
        match self {
            ChainIdVariant::Alias(a) => {
                VarUint32::new(0).pack(enc);
                a.pack(enc);
            }
            ChainIdVariant::Id(c) => {
                VarUint32::new(1).pack(enc);
                c.pack(enc);
            }
        }
        enc.get_size() - pos
    }

    fn unpack(&mut self, data: &[u8]) -> usize {
        let mut dec = Decoder::new(data);
        let mut tag = VarUint32::default();
        dec.unpack(&mut tag);
        match tag.value() {
            0 => {
                let mut a = 0u8;
                dec.unpack(&mut a);
                *self = ChainIdVariant::Alias(a);
            }
            1 => {
                let mut c = Checksum256::default();
                dec.unpack(&mut c);
                *self = ChainIdVariant::Id(c);
            }
            other => panic!("ChainIdVariant::unpack: unknown variant tag {other}"),
        }
        dec.get_pos()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, StructPacker)]
pub struct KvPair {
    pub key: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, Default, StructPacker)]
struct IdentityV2 {
    permission: Option<PermissionLevel>,
}

#[derive(Debug, Clone, Default, StructPacker)]
struct IdentityV3 {
    scope: Name,
    permission: Option<PermissionLevel>,
}

#[derive(Clone)]
// Manual Debug impl: TransactionHeader/TransactionExtension in RequestKind don't impl Debug upstream.
pub struct SigningRequest {
    pub chain_id: Checksum256,
    pub actions: Vec<Action>,
    pub flags: u8,
    pub callback: Option<String>,
    pub info: Vec<KvPair>,
    pub req_kind: RequestKind,
}

impl std::fmt::Debug for SigningRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match &self.req_kind {
            RequestKind::Action => "Action",
            RequestKind::Actions => "Actions",
            RequestKind::Transaction { .. } => "Transaction",
            RequestKind::Identity { .. } => "Identity",
        };
        f.debug_struct("SigningRequest")
            .field("kind", &kind)
            .field("chain_id", &self.chain_id)
            .field("actions_len", &self.actions.len())
            .field("flags", &self.flags)
            .field("callback", &self.callback)
            .field("info_len", &self.info.len())
            .finish()
    }
}

pub struct SigningRequestCreateArgs {
    pub chain_id: Checksum256,
    pub actions: Vec<Action>,
    pub callback: Option<CallbackSpec>,
    pub expiration: Option<u32>,
}

pub struct CallbackSpec {
    pub url: String,
    pub background: bool,
}

impl CallbackSpec {
    fn into_parts(opt: Option<Self>) -> (Option<String>, bool) {
        match opt {
            Some(c) => (Some(c.url), c.background),
            None => (None, false),
        }
    }
}

pub struct ResolveContext {
    pub chain_id: Checksum256,
    pub expire_seconds: u32,
    pub head_block_id: Checksum256,
    pub head_block_time_unix: u64,
}

impl SigningRequest {
    pub fn create(args: SigningRequestCreateArgs, _opts: &EsrOptions) -> Result<Self, EsrError> {
        let (callback_url, background) = CallbackSpec::into_parts(args.callback);
        let mut flags: u8 = 1;
        if background {
            flags |= 1 << 1;
        }
        let req_kind = if args.actions.len() == 1 {
            RequestKind::Action
        } else {
            RequestKind::Actions
        };
        Ok(SigningRequest {
            chain_id: args.chain_id,
            actions: args.actions,
            flags,
            callback: callback_url,
            info: Vec::new(),
            req_kind,
        })
    }

    pub fn create_identity(
        chain_id: Checksum256,
        scope: Option<Name>,
        permission: Option<PermissionLevel>,
        callback: Option<CallbackSpec>,
        _opts: &EsrOptions,
    ) -> Result<Self, EsrError> {
        let (callback_url, background) = CallbackSpec::into_parts(callback);
        let mut flags: u8 = 0;
        if background {
            flags |= 1 << 1;
        }
        let synthetic_action = identity_synthetic_action(&permission);
        Ok(SigningRequest {
            chain_id,
            actions: vec![synthetic_action],
            flags,
            callback: callback_url,
            info: Vec::new(),
            req_kind: RequestKind::Identity { scope, permission },
        })
    }

    pub fn from_uri(uri: &str, _opts: &EsrOptions) -> Result<Self, EsrError> {
        let body = uri
            .strip_prefix("esr://")
            .or_else(|| uri.strip_prefix("esr:"))
            .ok_or_else(|| EsrError::InvalidUri(format!("missing esr scheme: {uri}")))?;

        let bytes = base64url_decode(body)?;
        if bytes.is_empty() {
            return Err(EsrError::InvalidUri("ESR payload empty".into()));
        }
        let header = bytes[0];
        let version = header & 0x7f;
        let compressed = (header & 0x80) != 0;
        if version != 2 && version != 3 {
            return Err(EsrError::InvalidUri(format!(
                "unsupported protocol version: {version}"
            )));
        }
        let payload = if compressed {
            inflate(&bytes[1..])?
        } else {
            bytes[1..].to_vec()
        };
        Self::decode_payload(version, &payload)
    }

    fn decode_payload(version: u8, payload: &[u8]) -> Result<Self, EsrError> {
        // antelope-rs Packer::unpack panics on truncated input; catch + surface as
        // Serialization error. Removed once `try_unpack` lands upstream.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut dec = Decoder::new(payload);

            let mut chain_id_v = ChainIdVariant::default();
            dec.unpack(&mut chain_id_v);
            let chain_id = chain_id_v.to_checksum();

            let mut req_tag = VarUint32::default();
            dec.unpack(&mut req_tag);

            let (actions, req_kind) = match req_tag.value() {
                0 => {
                    let mut a = Action::default();
                    dec.unpack(&mut a);
                    (vec![a], RequestKind::Action)
                }
                1 => {
                    let mut v: Vec<Action> = Vec::new();
                    dec.unpack(&mut v);
                    (v, RequestKind::Actions)
                }
                2 => {
                    let mut tx = Transaction::default();
                    dec.unpack(&mut tx);
                    (
                        tx.actions.clone(),
                        RequestKind::Transaction {
                            header: tx.header,
                            context_free_actions: tx.context_free_actions,
                            transaction_extensions: tx.extension,
                        },
                    )
                }
                3 => {
                    if version >= 3 {
                        let mut id = IdentityV3::default();
                        dec.unpack(&mut id);
                        let synth = identity_synthetic_action(&id.permission);
                        (
                            vec![synth],
                            RequestKind::Identity {
                                scope: Some(id.scope),
                                permission: id.permission,
                            },
                        )
                    } else {
                        let mut id = IdentityV2::default();
                        dec.unpack(&mut id);
                        let synth = identity_synthetic_action(&id.permission);
                        (
                            vec![synth],
                            RequestKind::Identity {
                                scope: None,
                                permission: id.permission,
                            },
                        )
                    }
                }
                other => {
                    return Err(EsrError::Serialization(format!(
                        "unknown req variant tag: {other}"
                    )));
                }
            };

            let mut flags = 0u8;
            dec.unpack(&mut flags);

            let mut callback = String::new();
            dec.unpack(&mut callback);
            let callback = if callback.is_empty() {
                None
            } else {
                Some(callback)
            };

            let mut info: Vec<KvPair> = Vec::new();
            dec.unpack(&mut info);

            Ok(SigningRequest {
                chain_id,
                actions,
                flags,
                callback,
                info,
                req_kind,
            })
        }));
        match result {
            Ok(r) => r,
            Err(panic) => {
                let msg = if let Some(s) = panic.downcast_ref::<&'static str>() {
                    (*s).to_string()
                } else if let Some(s) = panic.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "decode panic".to_string()
                };
                Err(EsrError::Serialization(format!("ESR decode: {msg}")))
            }
        }
    }

    pub fn encode(&self, compress: bool, slashes: bool, scheme: &str) -> Result<String, EsrError> {
        let version = self.protocol_version();
        let payload = self.encode_payload();
        let (header, body) = if compress {
            let deflated = deflate(&payload);
            if deflated.len() < payload.len() {
                (version | 0x80, deflated)
            } else {
                (version, payload)
            }
        } else {
            (version, payload)
        };
        let mut out = Vec::with_capacity(1 + body.len());
        out.push(header);
        out.extend(body);
        let b64 = base64url_encode(&out);
        Ok(if slashes {
            format!("{scheme}//{b64}")
        } else {
            format!("{scheme}{b64}")
        })
    }

    fn encode_payload(&self) -> Vec<u8> {
        let mut enc = Encoder::new(64);
        ChainIdVariant::Id(self.chain_id).pack(&mut enc);
        self.encode_req_variant(&mut enc);
        self.flags.pack(&mut enc);
        let cb = self.callback.clone().unwrap_or_default();
        cb.pack(&mut enc);
        self.info.pack(&mut enc);
        enc.get_bytes().to_vec()
    }

    fn encode_req_variant(&self, enc: &mut Encoder) {
        match &self.req_kind {
            RequestKind::Action => {
                VarUint32::new(0).pack(enc);
                match self.actions.first() {
                    Some(a) => a.pack(enc),
                    None => Action::default().pack(enc),
                };
            }
            RequestKind::Actions => {
                VarUint32::new(1).pack(enc);
                self.actions.pack(enc);
            }
            RequestKind::Transaction {
                header,
                context_free_actions,
                transaction_extensions,
            } => {
                VarUint32::new(2).pack(enc);
                header.pack(enc);
                context_free_actions.pack(enc);
                self.actions.pack(enc);
                transaction_extensions.pack(enc);
            }
            RequestKind::Identity { scope, permission } => {
                VarUint32::new(3).pack(enc);
                match scope {
                    Some(s) => {
                        let id = IdentityV3 {
                            scope: *s,
                            permission: *permission,
                        };
                        id.pack(enc);
                    }
                    None => {
                        let id = IdentityV2 {
                            permission: *permission,
                        };
                        id.pack(enc);
                    }
                }
            }
        }
    }

    pub fn set_info_bytes(&mut self, key: &str, bytes: Vec<u8>) {
        self.info.retain(|kv| kv.key != key);
        self.info.push(KvPair {
            key: key.to_string(),
            value: bytes,
        });
    }

    pub fn set_info_key<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), EsrError> {
        let bytes = match serde_json::to_value(value)
            .map_err(|e| EsrError::Serialization(format!("set_info_key({key}): {e}")))?
        {
            serde_json::Value::String(s) => s.into_bytes(),
            other => serde_json::to_vec(&other)
                .map_err(|e| EsrError::Serialization(format!("set_info_key({key}): {e}")))?,
        };
        self.info.retain(|kv| kv.key != key);
        self.info.push(KvPair {
            key: key.to_string(),
            value: bytes,
        });
        Ok(())
    }

    pub async fn resolve(
        self,
        ctx: &ResolveContext,
        signer: Option<PermissionLevel>,
    ) -> Result<ResolvedSigningRequest, EsrError> {
        let signer = signer.unwrap_or_default();

        let expiration_unix = ctx
            .head_block_time_unix
            .saturating_add(ctx.expire_seconds as u64);
        let expiration_u32 = expiration_unix.min(u32::MAX as u64) as u32;

        let id = &ctx.head_block_id.data;
        let head_block_num = u32::from_be_bytes([id[0], id[1], id[2], id[3]]);
        let ref_block_num = (head_block_num & 0xFFFF) as u16;
        let ref_block_prefix = u32::from_le_bytes([id[8], id[9], id[10], id[11]]);

        let (header, context_free_actions, extension) = match &self.req_kind {
            RequestKind::Transaction {
                header,
                context_free_actions,
                transaction_extensions,
            } => {
                let mut h = header.clone();
                let is_empty_header =
                    h.expiration.seconds == 0 && h.ref_block_num == 0 && h.ref_block_prefix == 0;
                if is_empty_header {
                    h.expiration = TimePointSec::new(expiration_u32);
                    h.ref_block_num = ref_block_num;
                    h.ref_block_prefix = ref_block_prefix;
                }
                (
                    h,
                    context_free_actions.clone(),
                    transaction_extensions.clone(),
                )
            }
            _ => (
                TransactionHeader {
                    expiration: TimePointSec::new(expiration_u32),
                    ref_block_num,
                    ref_block_prefix,
                    max_net_usage_words: VarUint32::new(0),
                    max_cpu_usage_ms: 0,
                    delay_sec: VarUint32::new(0),
                },
                Vec::new(),
                Vec::new(),
            ),
        };

        let transaction = Transaction {
            header,
            context_free_actions,
            actions: self.actions.clone(),
            extension,
        };

        Ok(ResolvedSigningRequest {
            request: self,
            transaction,
            signer,
            chain_id: ctx.chain_id,
        })
    }

    pub fn is_identity(&self) -> bool {
        matches!(self.req_kind, RequestKind::Identity { .. })
    }

    pub fn broadcast(&self) -> bool {
        (self.flags & 0x01) != 0
    }

    pub fn set_broadcast(&mut self, broadcast: bool) {
        if broadcast {
            self.flags |= 0x01;
        } else {
            self.flags &= !0x01;
        }
    }

    pub fn resolve_explicit_tapos(
        self,
        chain_id: Checksum256,
        signer: PermissionLevel,
        expiration: TimePointSec,
        ref_block_num: u16,
        ref_block_prefix: u32,
    ) -> Result<ResolvedSigningRequest, EsrError> {
        let (context_free_actions, extension) = match &self.req_kind {
            RequestKind::Transaction {
                context_free_actions,
                transaction_extensions,
                ..
            } => (context_free_actions.clone(), transaction_extensions.clone()),
            _ => (Vec::new(), Vec::new()),
        };
        let header = TransactionHeader {
            expiration,
            ref_block_num,
            ref_block_prefix,
            max_net_usage_words: VarUint32::new(0),
            max_cpu_usage_ms: 0,
            delay_sec: VarUint32::new(0),
        };
        let transaction = Transaction {
            header,
            context_free_actions,
            actions: self.resolve_actions(signer),
            extension,
        };
        Ok(ResolvedSigningRequest {
            request: self,
            transaction,
            signer,
            chain_id,
        })
    }

    pub fn resolve_actions(&self, signer: PermissionLevel) -> Vec<Action> {
        let identity_name = Name::new_from_str("identity");

        self.actions
            .iter()
            .map(|action| {
                if action.account.n == 0 && action.name.n == identity_name.n {
                    return identity_synthetic_action(&Some(signer));
                }
                let needs_sub = action.authorization.iter().any(is_placeholder_permission);
                if !needs_sub {
                    return action.clone();
                }
                let new_auth = action
                    .authorization
                    .iter()
                    .map(|pl| substitute_permission(*pl, signer))
                    .collect();
                Action {
                    account: action.account,
                    name: action.name,
                    authorization: new_auth,
                    data: action.data.clone(),
                }
            })
            .collect()
    }

    fn protocol_version(&self) -> u8 {
        match &self.req_kind {
            RequestKind::Identity { scope: Some(_), .. } => 3,
            _ => 2,
        }
    }
}

const PLACEHOLDER_ACTOR_N: u64 = 1;
const PLACEHOLDER_PERMISSION_N: u64 = 2;

fn is_placeholder_permission(pl: &PermissionLevel) -> bool {
    pl.actor.n == PLACEHOLDER_ACTOR_N
        || pl.permission.n == PLACEHOLDER_PERMISSION_N
        || pl.permission.n == PLACEHOLDER_ACTOR_N
}

fn substitute_permission(pl: PermissionLevel, signer: PermissionLevel) -> PermissionLevel {
    let actor = if pl.actor.n == PLACEHOLDER_ACTOR_N {
        signer.actor
    } else {
        pl.actor
    };
    let permission =
        if pl.permission.n == PLACEHOLDER_PERMISSION_N || pl.permission.n == PLACEHOLDER_ACTOR_N {
            signer.permission
        } else {
            pl.permission
        };
    PermissionLevel { actor, permission }
}

fn identity_synthetic_action(permission: &Option<PermissionLevel>) -> Action {
    use antelope::chain::name::Name;
    let placeholder = PermissionLevel {
        actor: Name { n: 1 },
        permission: Name { n: 2 },
    };
    let auth = match permission {
        Some(p) => vec![*p],
        None => vec![placeholder],
    };
    let id = IdentityV2 {
        permission: Some(permission.unwrap_or(placeholder)),
    };
    let mut enc = Encoder::new(id.size());
    id.pack(&mut enc);
    let data = enc.get_bytes().to_vec();
    Action {
        account: Name { n: 0 },
        name: Name::new_from_str("identity"),
        authorization: auth,
        data,
    }
}

const CHAIN_ID_ALIASES: &[(u8, &str)] = &[
    (
        1,
        "aca376f206b8fc25a6ed44dbdc66547c36c6c33e3a119ffbeaef943642f0e906",
    ),
    (
        2,
        "4667b205c6838ef70ff7988f6e8257e8be0e1284a2f59699054a018f743b1d11",
    ),
    (
        3,
        "e70aaab8997e1dfce58fbfac80cbbb8fecec7b99cf982a9444273cbc64c41473",
    ),
    (
        4,
        "5fff1dae8dc8e2fc4d5b23b2c7665c97f9e9d8edf2b6485a86ba311c25639191",
    ),
    (
        5,
        "73647cde120091e0a4b85bced2f3cfdb3041e266cbbe95cee59b73235a1b3b6f",
    ),
    (
        6,
        "d5a3d18fbb3c084e3b1f3fa98c21014b5f3db536cc15d08f9f6479517c6a3d86",
    ),
    (
        7,
        "cfe6486a83bad4962f232d48003b1824ab5665c36778141034d75e57b956e422",
    ),
    (
        8,
        "b042025541e25a472bffde2d62edd457b7e70cee943412b1ea0f044f88591664",
    ),
    (
        9,
        "b912d19a6abd2b1b05611ae5be473355d64d95aeff0c09bedc8c166cd6468fe4",
    ),
    (
        10,
        "1064487b3cd1a897ce03ae5b6a865651747e2e152090f99c1d19d44e01aea5a4",
    ),
    (
        11,
        "384da888112027f0321850a169f737c33e53b388aad48b5adace4bab97f437e0",
    ),
    (
        12,
        "21dcae42c0182200e93f954a074011f9048a7624c6fe81d3c9541a614a88bd1c",
    ),
];

fn alias_to_chain_id(alias: u8) -> Option<Checksum256> {
    CHAIN_ID_ALIASES
        .iter()
        .find(|(a, _)| *a == alias)
        .and_then(|(_, hex)| Checksum256::from_hex(hex).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encode_decode_roundtrip_no_actions() {
        let req = SigningRequest::create(
            SigningRequestCreateArgs {
                chain_id: Checksum256::default(),
                actions: vec![],
                callback: None,
                expiration: None,
            },
            &EsrOptions::offline(),
        )
        .unwrap();
        let uri = req.encode(true, false, "esr:").unwrap();
        assert!(uri.starts_with("esr:"));
        assert!(!uri.starts_with("esr://"));
        let parsed = SigningRequest::from_uri(&uri, &EsrOptions::offline()).unwrap();
        assert_eq!(parsed.actions.len(), 0);
    }

    #[test]
    fn encode_decode_roundtrip_uncompressed_with_slashes() {
        let req = SigningRequest::create(
            SigningRequestCreateArgs {
                chain_id: Checksum256::default(),
                actions: vec![],
                callback: Some(CallbackSpec {
                    url: "https://cb.anchor.link/uuid".into(),
                    background: false,
                }),
                expiration: None,
            },
            &EsrOptions::offline(),
        )
        .unwrap();
        let uri = req.encode(false, true, "esr:").unwrap();
        assert!(uri.starts_with("esr://"));
        let parsed = SigningRequest::from_uri(&uri, &EsrOptions::offline()).unwrap();
        assert_eq!(
            parsed.callback.as_deref(),
            Some("https://cb.anchor.link/uuid")
        );
    }

    #[test]
    fn background_flag_sets_bit_1() {
        let req = SigningRequest::create(
            SigningRequestCreateArgs {
                chain_id: Checksum256::default(),
                actions: vec![],
                callback: Some(CallbackSpec {
                    url: "https://cb.anchor.link/uuid".into(),
                    background: true,
                }),
                expiration: None,
            },
            &EsrOptions::offline(),
        )
        .unwrap();
        assert_eq!(req.flags & 0x02, 0x02);
    }

    #[test]
    fn invalid_uri_errors() {
        let err = SigningRequest::from_uri("not-an-esr-uri", &EsrOptions::offline()).unwrap_err();
        assert!(matches!(err, EsrError::InvalidUri(_)));
    }

    #[test]
    fn set_info_key_replaces_existing() {
        let mut req = SigningRequest::create(
            SigningRequestCreateArgs {
                chain_id: Checksum256::default(),
                actions: vec![],
                callback: None,
                expiration: None,
            },
            &EsrOptions::offline(),
        )
        .unwrap();
        req.set_info_key("comment", "first").unwrap();
        req.set_info_key("comment", "second").unwrap();
        assert_eq!(req.info.len(), 1);
        assert_eq!(req.info[0].key, "comment");
        assert_eq!(req.info[0].value, b"second");
    }

    #[tokio::test]
    async fn resolve_produces_signing_digest() {
        let req = SigningRequest::create(
            SigningRequestCreateArgs {
                chain_id: Checksum256::default(),
                actions: vec![],
                callback: None,
                expiration: None,
            },
            &EsrOptions::offline(),
        )
        .unwrap();
        let ctx = ResolveContext {
            chain_id: Checksum256::default(),
            expire_seconds: 60,
            head_block_id: Checksum256::default(),
            head_block_time_unix: 1_700_000_000,
        };
        let resolved = req.resolve(&ctx, None).await.unwrap();
        let digest = resolved.signing_digest();
        assert_eq!(digest.data.len(), 32);
    }

    #[tokio::test]
    async fn resolve_applies_head_block_id_to_ref_fields() {
        let req = SigningRequest::create(
            SigningRequestCreateArgs {
                chain_id: Checksum256::default(),
                actions: vec![],
                callback: None,
                expiration: None,
            },
            &EsrOptions::offline(),
        )
        .unwrap();
        let mut head_id = [0u8; 32];
        head_id[0..4].copy_from_slice(&0x01020304u32.to_be_bytes());
        head_id[8..12].copy_from_slice(&0xdeadbeefu32.to_le_bytes());
        let ctx = ResolveContext {
            chain_id: Checksum256::default(),
            expire_seconds: 60,
            head_block_id: Checksum256::from_bytes(&head_id).unwrap(),
            head_block_time_unix: 1_700_000_000,
        };
        let resolved = req.resolve(&ctx, None).await.unwrap();
        assert_eq!(resolved.transaction.header.ref_block_num, 0x0304);
        assert_eq!(resolved.transaction.header.ref_block_prefix, 0xdeadbeef);
    }

    #[test]
    fn ts_identity_uri_decodes() {
        let ts_uri =
            "esr://g2NgZGYAgoaXEr0MDIoZJSUFxVb6-slJeol5yRn5RXo5mXnZ-jn56Zl5uqWlmSmM7Mn5ubmpeSUi5RmJRWnZmSW66fkp-SUKYCUA";
        let req = SigningRequest::from_uri(ts_uri, &EsrOptions::offline())
            .expect("decode TS identity URI");
        assert!(req.is_identity(), "decoded variant should be Identity");
        assert_eq!(
            req.callback.as_deref(),
            Some("https://cb.anchor.link/login-uuid")
        );
        let comment = req
            .info
            .iter()
            .find(|kv| kv.key == "comment")
            .expect("comment key present");
        assert_eq!(comment.value, b"wharfkit-godot login");
        assert_eq!(req.actions.len(), 1);
        assert_eq!(req.actions[0].name.to_string(), "identity");
        assert_eq!(req.actions[0].account.n, 0);
    }

    #[test]
    fn ts_transact_single_uri_decodes() {
        let ts_uri = "esr://gmMsfmIRpc7x7DpLh8nvg-zz9VdvrLYRihbJ-mIxXW5CYY4vA8OyJhPmVwahDAwM4bo2Z88yMoBAa4wJiFrx1sjIGlmAgYHPVkAdwmJx9Q8G0VIZqTk5-QppRfm5CsU5mcmpukYKaZkVJaVFqYwqGSUlBcVW-vrJSXqJeckZ-UV6OZl52folRYl5xYnJJbqlpZkpDAA";
        let req = SigningRequest::from_uri(ts_uri, &EsrOptions::offline())
            .expect("decode TS single-action URI");
        assert_eq!(req.actions.len(), 1);
        assert_eq!(req.actions[0].account.to_string(), "eosio.token");
        assert_eq!(req.actions[0].name.to_string(), "transfer");
        let expected_data = hex::decode(
            "0000000000855c340000000000000e3d102700000000000004454f53000000001a68656c6c6f2066726f6d20736c6963652d322066697874757265"
        ).unwrap();
        assert_eq!(req.actions[0].data, expected_data);
    }

    #[test]
    fn ts_transact_multi_uri_decodes() {
        let ts_uri = "esr://gmMsfmIRpc7x7DpLh8nvg-zz9VdvrLYRihbJ-mIxXW5CYY4vIxPDsiYT5lcGoQwMDOG6NmfPMjKAQGuMCYha8dbISA1ZgIGBz1ZAHcJicfUPBtGsaZlFxSUEzVFHFmjwWO-o4IdqDltxanJ-XgqjYkZJSUGxlb5-cpJeYl5yRn6RXk5mXrZ-bmlOSaZuaWlmCgMA";
        let req = SigningRequest::from_uri(ts_uri, &EsrOptions::offline())
            .expect("decode TS multi-action URI");
        assert_eq!(req.actions.len(), 2);
    }

    #[test]
    fn ts_callback_template_uri_decodes() {
        let ts_uri = "esr://gmMsfmIRpc7x7DpLh8nvg-zz9VdvrLYRihbJ-mIxXW5CYY4vA8OyJhPmVwahDAwM4bo2Z88yMoBAa4wJiFrx1sjIGlmAgYHPVkAdwmJx9Q8G0VIZqTk5-QppRfm5CsU5mcmpukYKaZkVJaVFqYxGGSUlBcVW-vrJSXqJeckZ-UV6OZl52frV1cWZ6bW19iUVttXVJRW1tWpJeUBWUl5tLQMA";
        let req = SigningRequest::from_uri(ts_uri, &EsrOptions::offline())
            .expect("decode TS callback-template URI");
        assert_eq!(
            req.callback.as_deref(),
            Some("https://cb.anchor.link/{{sig}}?tx={{tx}}&bn={{bn}}")
        );
    }
}
