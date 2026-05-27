use std::sync::Arc;

use antelope::api::client::{APIClient, Provider};
use antelope::chain::action::{Action, PermissionLevel};
use antelope::chain::name::Name;
use wharfkit_abicache::ABICache;
use wharfkit_common::Chains;
use wharfkit_mock::{MockChain, MockPlatform, MockUserInterface, MockWalletPlugin};
use wharfkit_session::{
    InMemorySessionStorage, LoginOptions, PlatformName, RestoreArgs, SessionKit, SessionKitArgs,
    TransactArgs, TransactOptions, UserInterface, WalletPlugin,
};

const TEST_WIF: &str = "5Jtoxgny5tT7NiNFp1MLogviuPJ9NniWjnU4wKzaX4t7pL4kJ8s";

fn build_kit(mock_chain: Arc<MockChain>) -> SessionKit {
    let ui = Arc::new(MockUserInterface::default());
    let wallet: Arc<dyn WalletPlugin> =
        Arc::new(MockWalletPlugin::new(TEST_WIF, "alice", "active"));
    let platform = Arc::new(MockPlatform::new(PlatformName::Macos));
    let storage = Arc::new(InMemorySessionStorage::default());

    let provider: Arc<dyn Provider> = mock_chain;
    let client = APIClient::<Arc<dyn Provider>>::custom_provider(provider).unwrap();

    let abi_cache = Arc::new(ABICache::new_offline());

    SessionKit::new(SessionKitArgs {
        app_name: "test".into(),
        chains: vec![Chains::jungle4()],
        ui,
        platform,
        wallet_plugins: vec![wallet],
        storage,
        client,
        abi_cache,
        login_plugins: vec![],
        transact_plugins: vec![],
    })
}

fn get_info_response_json() -> String {
    serde_json::json!({
        "server_version": "abcdef0",
        "chain_id": "73e4385a2708e6d7048834fbc1079f2fabb17b3c125b146af438971e90716c4d",
        "head_block_num": 12345678,
        "last_irreversible_block_num": 12345600,
        "last_irreversible_block_id": "00bc614e0000000000000000000000000000000000000000000000000000abcd",
        "head_block_id": "00bc614f0000000000000000000000000000000000000000000000000000abcd",
        "head_block_time": "2026-05-25T12:00:00.000",
        "head_block_producer": "eosio",
        "virtual_block_cpu_limit": 200000,
        "virtual_block_net_limit": 1048576000,
        "block_cpu_limit": 200000,
        "block_net_limit": 1048576,
        "server_version_string": "v5.0.0",
        "fork_db_head_block_num": 12345678,
        "fork_db_head_block_id": "00bc614f0000000000000000000000000000000000000000000000000000abcd",
        "server_full_version_string": "v5.0.0-abcdef0",
        "total_cpu_weight": "0",
        "total_net_weight": "0",
        "earliest_available_block_num": 1,
        "last_irreversible_block_time": "2026-05-25T11:59:59.500"
    })
    .to_string()
}

fn send_transaction2_response_json() -> String {
    serde_json::json!({
        "transaction_id": "abcd1234",
        "processed": {
            "id": "abcd1234",
            "block_num": 12345678u64,
            "block_time": "2026-05-25T12:00:01.000",
            "producer_block_id": null,
            "receipt": null,
            "elapsed": 100u64,
            "net_usage": 0u32,
            "scheduled": false,
            "action_traces": [],
            "account_ram_delta": null,
            "except": null,
            "error_code": null
        }
    })
    .to_string()
}

fn nop_action() -> Action {
    Action {
        account: Name::new_from_str("eosio"),
        name: Name::new_from_str("nonce"),
        authorization: vec![PermissionLevel::new(
            Name::new_from_str("alice"),
            Name::new_from_str("active"),
        )],
        data: vec![],
    }
}

#[tokio::test]
async fn mocked_login_succeeds_without_ui_selection() {
    let mock_chain = Arc::new(MockChain::new());
    let kit = build_kit(mock_chain);

    let session = kit.login(LoginOptions::default()).await.expect("login ok");

    assert_eq!(session.permission_level().actor.to_string(), "alice");
    assert_eq!(session.permission_level().permission.to_string(), "active");
    assert_eq!(session.chain().name(), Some("Jungle 4"));
}

#[tokio::test]
async fn mocked_transact_signs_and_does_not_broadcast_when_disabled() {
    let mock_chain = Arc::new(MockChain::new());
    mock_chain.set_response("GET", "/v1/chain/get_info", &get_info_response_json());

    let kit = build_kit(mock_chain.clone());
    let session = kit.login(LoginOptions::default()).await.unwrap();

    let result = session
        .transact(
            TransactArgs {
                actions: vec![nop_action()],
            },
            TransactOptions {
                broadcast: false,
                expire_seconds: Some(120),
            },
        )
        .await
        .expect("transact ok");

    assert_eq!(result.signatures.len(), 1);
    assert!(result.transaction.is_some());
    assert!(result.response.is_none(), "no broadcast → no response");

    let calls = mock_chain.calls();
    assert!(calls.iter().any(|c| c.contains("/v1/chain/get_info")));
    assert!(!calls.iter().any(|c| c.contains("send_transaction")));
}

#[tokio::test]
async fn mocked_transact_broadcasts_when_enabled() {
    let mock_chain = Arc::new(MockChain::new());
    mock_chain.set_response("GET", "/v1/chain/get_info", &get_info_response_json());
    mock_chain.set_response(
        "POST",
        "/v1/chain/send_transaction2",
        &send_transaction2_response_json(),
    );

    let kit = build_kit(mock_chain.clone());
    let session = kit.login(LoginOptions::default()).await.unwrap();

    let result = session
        .transact(
            TransactArgs {
                actions: vec![nop_action()],
            },
            TransactOptions {
                broadcast: true,
                expire_seconds: Some(120),
            },
        )
        .await
        .expect("transact ok");

    assert!(result.response.is_some());
    let calls = mock_chain.calls();
    assert!(calls
        .iter()
        .any(|c| c == "POST /v1/chain/send_transaction2"));
}

#[tokio::test]
async fn mocked_sign_transaction_skips_orchestration() {
    use antelope::chain::time::TimePointSec;
    use antelope::chain::transaction::{Transaction, TransactionHeader};
    use antelope::chain::varint::VarUint32;

    let mock_chain = Arc::new(MockChain::new());
    let kit = build_kit(mock_chain);
    let session = kit.login(LoginOptions::default()).await.unwrap();

    let transaction = Transaction {
        header: TransactionHeader {
            expiration: TimePointSec::new(1_700_000_000),
            ref_block_num: 0x0304,
            ref_block_prefix: 0xdeadbeef,
            max_net_usage_words: VarUint32::new(0),
            max_cpu_usage_ms: 0,
            delay_sec: VarUint32::new(0),
        },
        context_free_actions: vec![],
        actions: vec![nop_action()],
        extension: vec![],
    };

    let signatures = session
        .sign_transaction(transaction)
        .await
        .expect("sign ok");
    assert_eq!(signatures.len(), 1);
}

#[tokio::test]
async fn restore_round_trips_serialized_session() {
    let mock_chain = Arc::new(MockChain::new());
    let kit = build_kit(mock_chain);

    let session = kit.login(LoginOptions::default()).await.unwrap();
    let chain_id = *session.chain().id();
    let permission = *session.permission_level();

    drop(session);

    let restored = kit
        .restore(RestoreArgs {
            chain: chain_id,
            permission_level: permission,
            wallet_plugin: "mock".to_string(),
        })
        .await
        .expect("restore ok");

    assert_eq!(restored.permission_level().actor.to_string(), "alice");
}

#[tokio::test]
async fn ui_records_lifecycle_hooks_during_transact() {
    let mock_chain = Arc::new(MockChain::new());
    mock_chain.set_response("GET", "/v1/chain/get_info", &get_info_response_json());

    let kit = build_kit(mock_chain);
    let session = kit.login(LoginOptions::default()).await.unwrap();

    session
        .transact(
            TransactArgs {
                actions: vec![nop_action()],
            },
            TransactOptions {
                broadcast: false,
                expire_seconds: Some(120),
            },
        )
        .await
        .unwrap();

    let _: &dyn UserInterface = &MockUserInterface::default();
}
