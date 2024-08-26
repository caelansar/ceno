use super::build::build_project;
use crate::{CmdExector, BUILD_DIR};
use ceno_server::{
    start_server, ProjectConfig, SwappableAppRouter, SwappableThreadPool, TenentRouter,
};
use clap::Parser;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, DebouncedEvent, Debouncer};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_sdk::trace::Config;
use opentelemetry_sdk::Resource;
use std::fs;
use std::mem::ManuallyDrop;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc::{channel, Receiver};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{Stream, StreamExt};
use tracing::level_filters::LevelFilter;
use tracing::{info, Level};
use tracing_subscriber::filter::Targets;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _, Layer as _};

const MONITOR_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, PartialEq, Eq)]
pub struct FileChangedEvent {
    pub files: Vec<PathBuf>,
}

impl FileChangedEvent {
    pub fn new(files: Vec<PathBuf>) -> Self {
        Self { files }
    }
}

pub trait SwapWatcher {
    fn recv(self) -> anyhow::Result<impl Stream<Item = FileChangedEvent>>;
}

#[allow(unused)]
struct CompositedWatcher {
    fs: FsWatcher,
    channel: Receiver<FileChangedEvent>,
}

#[allow(unused)]
impl CompositedWatcher {
    pub fn new(fs: FsWatcher, channel: Receiver<FileChangedEvent>) -> Self {
        Self { fs, channel }
    }
}

impl SwapWatcher for CompositedWatcher {
    fn recv(self) -> anyhow::Result<impl Stream<Item = FileChangedEvent>> {
        let fs_stream = self.fs.recv()?;

        let channel_stream = ReceiverStream::new(self.channel);

        Ok(fs_stream.merge(channel_stream))

        //Ok(async_stream::stream! {
        //    loop {
        //        select! {
        //            Some(r1) = fs_stream.next() => {
        //                yield r1
        //            }
        //            Some(r2) = channel_stream.next() => {
        //                yield r2
        //            }
        //        }
        //    }
        //})
    }
}

struct FsWatcher {
    _debouncer: ManuallyDrop<Debouncer<RecommendedWatcher>>,
    rx: Receiver<Vec<DebouncedEvent>>,
}

impl FsWatcher {
    pub fn try_new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let (tx, rx) = channel(10);

        let mut debouncer = new_debouncer(MONITOR_INTERVAL, move |res: DebounceEventResult| {
            println!("send {:?}", res);
            if let Ok(res) = res {
                tx.blocking_send(res).unwrap();
            }
        })?;

        debouncer
            .watcher()
            .watch(path.as_ref(), RecursiveMode::Recursive)?;

        Ok(Self {
            _debouncer: ManuallyDrop::new(debouncer),
            rx,
        })
    }
}

impl SwapWatcher for FsWatcher {
    fn recv(self) -> anyhow::Result<impl Stream<Item = FileChangedEvent>> {
        Ok(ReceiverStream::new(self.rx)
            .map(|events| FileChangedEvent::new(events.into_iter().map(|e| e.path).collect())))
    }
}

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

        let notifier = FsWatcher::try_new(format!("./{}", BUILD_DIR))?;

        let pool = SwappableThreadPool::new(&code);
        let pools = vec![("localhost".to_string(), pool.clone())];

        tokio::spawn(async move {
            let stream = notifier.recv()?;

            handle_swap(router, pool, stream).await
        });

        start_server(self.port, routers, pools).await?;

        Ok(())
    }
}

async fn handle_swap(
    router: SwappableAppRouter,
    pool: SwappableThreadPool,
    mut stream: impl Stream<Item = FileChangedEvent> + Unpin,
) -> anyhow::Result<()> {
    while let Some(event) = stream.next().await {
        let mut need_swap = false;
        // config.yml change, or any ".ts" / ".js" file change
        for path in event.files {
            info!("path: {:?}", path);
            let ext = path.extension().unwrap_or_default();
            if path.ends_with("config.yml") || ext == "ts" || ext == "js" {
                info!("File changed: {}", path.display());
                need_swap = true;
                break;
            }
        }

        if need_swap {
            let (code, config) = get_code_and_config()?;
            pool.swap(&code);
            router.swap(code, config.routes)?;
        }
    }
    Ok(())
}

fn get_code_and_config() -> anyhow::Result<(String, ProjectConfig)> {
    let filename = build_project(".", false)?;
    let config = filename.replace(".js", ".yml");
    let code = fs::read_to_string(filename)?;
    let config = ProjectConfig::load(config)?;
    Ok((code, config))
}

#[cfg(test)]
mod tests {
    use std::{pin::pin, str::FromStr};

    use tokio::sync::mpsc::channel;

    use super::*;

    #[tokio::test]
    async fn notifier_should_work() {
        let fs_notify = FsWatcher::try_new(".").unwrap();

        let (tx, rx) = channel(10);

        tokio::spawn(async move {
            tx.send(FileChangedEvent::new(
                vec![PathBuf::from_str("aa").unwrap()],
            ))
            .await
            .unwrap();
        });

        let mixed_notify = CompositedWatcher::new(fs_notify, rx);

        let mut stream = mixed_notify.recv().unwrap();

        assert_eq!(
            Some(FileChangedEvent::new(
                vec![PathBuf::from_str("aa").unwrap()]
            )),
            pin!(stream).next().await
        );
    }
}
