use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "wharfkit-cli", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Generate a typed Rust module from a contract's ABI.
    Codegen {
        /// Chain name (e.g. "jungle4") or URL endpoint.
        #[arg(long)]
        chain: String,
        /// Contract account name (e.g. "eosio.token").
        #[arg(long)]
        account: String,
        /// Output file path.
        #[arg(long)]
        out: std::path::PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Codegen {
            chain,
            account,
            out,
        } => {
            wharfkit_cli::codegen(&chain, &account, &out)
                .await
                .map_err(anyhow::Error::from)?;
            Ok(())
        }
    }
}
