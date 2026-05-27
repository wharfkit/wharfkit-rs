use antelope::chain::abi::ABI;
use wharfkit_cli::codegen::{
    emit_abi_helpers, emit_actions_module, emit_header, emit_tables_module, emit_types_module,
    resolve_chain, EmitContext,
};

#[test]
fn codegen_output_is_byte_identical_across_runs() {
    let abi_json = include_str!("fixtures/eosio.token.abi.json");
    let abi: ABI = serde_json::from_str(abi_json).expect("parse fixture");
    let chain = resolve_chain("jungle4");
    let abi_hash = "fixed_hash_for_test";
    let ctx = EmitContext {
        abi: &abi,
        chain: &chain,
        account: "eosio.token",
        abi_hash,
        tool_version: "0.1.0",
    };

    let run = |ctx: &EmitContext| -> String {
        let mut s = String::new();
        s.push_str(&emit_header(ctx));
        s.push_str(&emit_types_module(ctx).expect("types"));
        s.push_str(&emit_actions_module(ctx).expect("actions"));
        s.push_str(&emit_tables_module(ctx).expect("tables"));
        s.push_str(&emit_abi_helpers(ctx).expect("abi helpers"));
        s
    };

    let out1 = run(&ctx);
    let out2 = run(&ctx);
    assert_eq!(out1, out2, "codegen output differs across runs");
}
