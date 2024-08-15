use anyhow::Result;
use ceno::{CmdExector, Opts};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opts::parse();
    opts.cmd.execute().await?;
    Ok(())
}
