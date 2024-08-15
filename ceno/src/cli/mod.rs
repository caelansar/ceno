mod build;
mod init;
mod run;

use clap::Parser;
use enum_dispatch::enum_dispatch;

pub use self::{build::BuildOpts, init::InitOpts, run::RunOpts};

#[derive(Debug, Parser)]
#[command(name = "ceno", version, author, about, long_about = None)]
pub struct Opts {
    #[command(subcommand)]
    pub cmd: SubCommand,
}

#[derive(Debug, Parser)]
#[enum_dispatch(CmdExector)]
pub enum SubCommand {
    #[command(name = "init", about = "Init ceno project")]
    Init(InitOpts),
    #[command(name = "build", about = "Build ceno project")]
    Build(BuildOpts),
    #[command(name = "run", about = "Run user's ceno project")]
    Run(RunOpts),
}
