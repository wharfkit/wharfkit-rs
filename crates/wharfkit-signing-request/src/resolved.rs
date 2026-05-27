use antelope::chain::action::PermissionLevel;
use antelope::chain::checksum::Checksum256;
use antelope::chain::signature::Signature;
use antelope::chain::transaction::Transaction;

use crate::error::EsrError;
use crate::identity::IdentityProof;
use crate::request::SigningRequest;

// `Transaction` does not implement Debug, so this struct can't either.
#[derive(Clone)]
pub struct ResolvedSigningRequest {
    pub request: SigningRequest,
    pub transaction: Transaction,
    pub signer: PermissionLevel,
    pub chain_id: Checksum256,
}

impl ResolvedSigningRequest {
    pub fn signing_digest(&self) -> Checksum256 {
        self.transaction
            .signing_digest_checksum(&self.chain_id.data)
    }

    pub fn signing_data(&self) -> Vec<u8> {
        self.transaction.signing_data(&self.chain_id.data)
    }

    pub fn get_identity_proof(&self, sig: &Signature) -> Result<IdentityProof, EsrError> {
        let data = self.signing_data();
        let recovered_key = sig.recover_message(&data);
        Ok(IdentityProof {
            signer: self.signer,
            signature: sig.clone(),
            recovered_key,
        })
    }

    // Must use the wallet's exact TAPOS (rbn/rid/ex); rebuilding from fresh head
    // info yields different bytes and recovers a wrong key on verify.
    pub fn from_payload(
        payload: &crate::callback::CallbackPayload,
        opts: &crate::options::EsrOptions,
    ) -> Result<Self, EsrError> {
        use antelope::chain::name::Name;
        use antelope::chain::time::TimePointSec;

        let req_uri = payload
            .extra
            .get("req")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EsrError::Internal("callback missing `req` field".into()))?;
        let request = SigningRequest::from_uri(req_uri, opts)?;

        let sa = payload
            .sa
            .as_deref()
            .ok_or_else(|| EsrError::Internal("callback missing `sa` field".into()))?;
        let sp = payload
            .sp
            .as_deref()
            .ok_or_else(|| EsrError::Internal("callback missing `sp` field".into()))?;
        let signer = PermissionLevel::new(Name::new_from_str(sa), Name::new_from_str(sp));

        fn parse_u(value: Option<&serde_json::Value>) -> Option<u64> {
            value.and_then(|v| match v {
                serde_json::Value::String(s) => s.parse().ok(),
                serde_json::Value::Number(n) => n.as_u64(),
                _ => None,
            })
        }
        let rbn = parse_u(payload.extra.get("rbn"))
            .and_then(|x| u16::try_from(x).ok())
            .ok_or_else(|| EsrError::Internal("callback missing/invalid `rbn`".into()))?;
        let rid = parse_u(payload.extra.get("rid"))
            .and_then(|x| u32::try_from(x).ok())
            .ok_or_else(|| EsrError::Internal("callback missing/invalid `rid`".into()))?;
        let expiration = payload
            .extra
            .get("ex")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EsrError::Internal("callback missing `ex` field".into()))
            .and_then(|s| {
                TimePointSec::from_iso_string(s)
                    .map_err(|e| EsrError::Internal(format!("callback `ex` parse: {e}")))
            })?;

        let chain_id = match payload.cid.as_deref() {
            Some(cid) => Checksum256::from_hex(cid)
                .map_err(|e| EsrError::Internal(format!("callback `cid` not valid hex: {e}")))?,
            None => request.chain_id,
        };

        request.resolve_explicit_tapos(chain_id, signer, expiration, rbn, rid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::EsrOptions;
    use crate::request::{ResolveContext, SigningRequestCreateArgs};
    use antelope::chain::name::Name;
    use antelope::chain::private_key::PrivateKey;
    use antelope::chain::time::TimePointSec;
    use antelope::chain::transaction::{Transaction, TransactionHeader};
    use antelope::chain::varint::VarUint32;
    use std::sync::Arc;
    use wharfkit_abicache::ABICache;

    fn opts() -> EsrOptions {
        EsrOptions::new(Arc::new(ABICache::new_offline()))
    }

    #[tokio::test]
    async fn get_identity_proof_recovers_signer_key() {
        // Deterministic WIF for stable test output.
        let priv_key =
            PrivateKey::from_str("5Jtoxgny5tT7NiNFp1MLogviuPJ9NniWjnU4wKzaX4t7pL4kJ8s", false)
                .unwrap();
        let expected_pub = priv_key.to_public();

        let req = SigningRequest::create(
            SigningRequestCreateArgs {
                chain_id: Checksum256::default(),
                actions: vec![],
                callback: None,
                expiration: None,
            },
            &opts(),
        )
        .unwrap();
        let ctx = ResolveContext {
            chain_id: Checksum256::default(),
            expire_seconds: 60,
            head_block_id: Checksum256::default(),
            head_block_time_unix: 1_700_000_000,
        };
        let resolved = req
            .resolve(
                &ctx,
                Some(PermissionLevel::new(
                    Name::new_from_str("alice"),
                    Name::new_from_str("active"),
                )),
            )
            .await
            .unwrap();

        let data = resolved.signing_data();
        let sig = priv_key.sign_message(&data);

        let proof = resolved.get_identity_proof(&sig).unwrap();
        assert_eq!(
            proof.recovered_key.as_string(),
            expected_pub.as_string(),
            "recovered key should equal signer's pubkey",
        );
        assert_eq!(proof.signer.actor.to_string(), "alice");
    }

    #[tokio::test]
    async fn identity_proof_recovers_with_placeholder_substitution() {
        use crate::request::SigningRequest;

        let priv_key =
            PrivateKey::from_str("5Jtoxgny5tT7NiNFp1MLogviuPJ9NniWjnU4wKzaX4t7pL4kJ8s", false)
                .unwrap();
        let expected_pub = priv_key.to_public();
        let signer =
            PermissionLevel::new(Name::new_from_str("alice"), Name::new_from_str("active"));

        let req =
            SigningRequest::create_identity(Checksum256::default(), None, None, None, &opts())
                .unwrap();
        assert_eq!(req.actions[0].authorization[0].actor.n, 1);
        assert_eq!(req.actions[0].authorization[0].permission.n, 2);

        let resolved_actions = req.resolve_actions(signer);
        assert_eq!(resolved_actions[0].authorization[0], signer);
        assert_ne!(resolved_actions[0].data, req.actions[0].data);

        let transaction = Transaction {
            header: TransactionHeader {
                expiration: TimePointSec::new(0),
                ref_block_num: 0,
                ref_block_prefix: 0,
                max_net_usage_words: VarUint32::new(0),
                max_cpu_usage_ms: 0,
                delay_sec: VarUint32::new(0),
            },
            context_free_actions: vec![],
            actions: resolved_actions,
            extension: vec![],
        };
        let resolved = ResolvedSigningRequest {
            request: req,
            transaction,
            signer,
            chain_id: Checksum256::default(),
        };

        let data = resolved.signing_data();
        let sig = priv_key.sign_message(&data);
        let proof = resolved.get_identity_proof(&sig).unwrap();
        assert_eq!(
            proof.recovered_key.as_string(),
            expected_pub.as_string(),
            "placeholder substitution must produce the same bytes Anchor signs against"
        );
    }
}
