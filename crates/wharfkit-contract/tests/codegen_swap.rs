//! Verify that the codegen'd `eosio_token` module produced by wharfkit-cli is
//! consumable as a drop-in replacement for the hand-written `AccountRow` from
//! Slice 1.
//!
//! The codegen artifact lives at `tests/generated/eosio_token.rs` and is
//! pulled in as a module via `#[path = ...]`. Regenerate with:
//!
//!   cargo run --release --package wharfkit-cli -- \
//!     codegen --chain jungle4 --account eosio.token \
//!     --out crates/wharfkit-contract/tests/generated/eosio_token.rs

// Reference the generated file as a module. Codegen output is wide-surface
// (all structs/actions/tables from the ABI emit even when this test exercises
// only `accounts` + `Account`); silence dead-code warnings at the import edge
// so workspace clippy stays at -D warnings.
#[allow(dead_code, unused_imports)]
#[path = "generated/eosio_token.rs"]
mod eosio_token;

use antelope::api::client::{APIClient, DefaultProvider};
use antelope::chain::name::Name;
use eosio_token::tables::accounts;
use eosio_token::types::Account;
use std::sync::Arc;

fn jungle4_client() -> Arc<APIClient<DefaultProvider>> {
    Arc::new(
        APIClient::<DefaultProvider>::default_provider(
            "https://jungle4.greymass.com".to_string(),
            None,
        )
        .unwrap(),
    )
}

#[test]
fn codegen_account_factory_returns_typed_table() {
    // Constructing the table via the codegen'd factory must yield a
    // `Table<Account>` with the right contract / table / scope wiring —
    // no network needed.
    let table = accounts(Name::new_from_str("teamgreymass"));
    assert_eq!(table.contract().to_string(), "eosio.token");
    assert_eq!(table.table_name().to_string(), "accounts");
    assert_eq!(table.scope().to_string(), "teamgreymass");
}

#[tokio::test]
#[ignore = "network; run manually"]
async fn codegen_account_row_balance_fetch() {
    let client = jungle4_client();
    let table = accounts(Name::new_from_str("teamgreymass"));
    // Primary key for accounts table = symbol code as u64 (EOS = 5459781).
    let row: Option<Account> = table.get(5459781, &client).await.expect("table.get");
    assert!(row.is_some(), "EOS balance row should exist");
    let balance = row.unwrap().balance;
    println!("teamgreymass EOS balance: {balance}");
    assert!(balance.amount() >= 0);
}
