use antelope::chain::abi::ABI;
use wharfkit_cli::codegen::{
    emit_abi_helpers, emit_actions_module, emit_header, emit_tables_module, emit_types_module,
    resolve_chain, EmitContext,
};

#[test]
fn eosio_token_codegen_shape() {
    let abi_json = include_str!("fixtures/eosio.token.abi.json");
    let abi: ABI = serde_json::from_str(abi_json).expect("parse fixture");
    let chain = resolve_chain("jungle4");
    let ctx = EmitContext {
        abi: &abi,
        chain: &chain,
        account: "eosio.token",
        abi_hash: "test_hash",
        tool_version: "0.1.0",
    };
    let mut out = String::new();
    out.push_str(&emit_header(&ctx));
    out.push_str(&emit_types_module(&ctx).expect("types"));
    out.push_str(&emit_actions_module(&ctx).expect("actions"));
    out.push_str(&emit_tables_module(&ctx).expect("tables"));
    out.push_str(&emit_abi_helpers(&ctx).expect("abi helpers"));

    assert!(
        out.contains("pub const ACCOUNT"),
        "ACCOUNT constant present"
    );
    assert!(
        out.contains("pub const ABI_HASH"),
        "ABI_HASH constant present"
    );
    assert!(out.contains("pub mod types"), "types module present");
    assert!(out.contains("pub mod actions"), "actions module present");
    assert!(out.contains("pub mod tables"), "tables module present");
    assert!(
        out.contains("pub struct Transfer"),
        "Transfer struct present"
    );
    assert!(
        out.contains("pub fn transfer"),
        "transfer factory fn present"
    );
    assert!(
        out.contains("pub fn accounts(scope: Name)"),
        "accounts table accessor present"
    );
    assert!(out.contains("pub fn abi_blob()"), "abi_blob() fn present");
    assert!(out.contains("pub fn abi()"), "abi() fn present");
}
