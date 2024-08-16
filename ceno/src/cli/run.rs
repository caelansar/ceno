use std::fs;

use crate::CmdExector;
use ceno_server::{start_server, ProjectConfig, SwappableAppRouter, TenentRouter};
use clap::Parser;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    fmt::Layer, layer::SubscriberExt as _, util::SubscriberInitExt as _, Layer as _,
};

use super::build::build_project;

#[derive(Debug, Parser)]
pub struct RunOpts {
    // port to listen
    #[arg(short, long, default_value = "5000")]
    pub port: u16,
}

impl CmdExector for RunOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let layer = Layer::new().with_filter(LevelFilter::INFO);
        tracing_subscriber::registry().with(layer).init();

        let (code, config) = get_code_and_config()?;

        let router = SwappableAppRouter::try_new(&code, config.routes)?;
        let routers = vec![TenentRouter::new("localhost", router.clone())];

        start_server(self.port, routers).await?;

        Ok(())
    }
}

fn get_code_and_config() -> anyhow::Result<(String, ProjectConfig)> {
    let filename = build_project(".")?;
    let config = filename.replace(".js", ".yml");
    let code = fs::read_to_string(filename)?;
    let config = ProjectConfig::load(config)?;
    Ok((code, config))
}

#[cfg(test)]
mod tests {}
