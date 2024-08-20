use std::fs;

use crate::CmdExector;
use ceno_server::{start_server, ProjectConfig, SwappableAppRouter, TenentRouter};
use clap::Parser;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_sdk::trace::Config;
use opentelemetry_sdk::Resource;
use tracing::level_filters::LevelFilter;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _, Layer as _};

use super::build::build_project;

#[derive(Debug, Parser)]
pub struct RunOpts {
    #[arg(short, long, default_value = "5000", help = "Port to listen")]
    pub port: u16,
    #[arg(long, default_value_t = false, help = "Enable opentelemetry")]
    pub otlp: bool,
}

impl CmdExector for RunOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let fmt_layer = tracing_subscriber::fmt::Layer::new().with_filter(LevelFilter::INFO);

        let provider = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(opentelemetry_otlp::new_exporter().tonic())
            .with_trace_config(
                Config::default()
                    .with_resource(Resource::new(vec![KeyValue::new("service.name", "ceno")])),
            )
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .expect("Couldn't create OTLP tracer");

        let tracer = provider.tracer("ceno");

        let telemetry_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(Targets::new().with_target("ceno", Level::INFO));

        let r = tracing_subscriber::registry().with(fmt_layer);

        if self.otlp {
            r.with(telemetry_layer).init();
        } else {
            r.init();
        }

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
