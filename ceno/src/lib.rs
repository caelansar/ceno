mod cli;
mod utils;

pub use cli::*;
use enum_dispatch::enum_dispatch;
use std::collections::HashMap;
use ts_rs::TS;

pub const BUILD_DIR: &str = ".build";

#[derive(Debug, TS)]
pub struct Req {
    pub method: String,
    pub url: String,
    pub query: HashMap<String, String>,
    pub params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Debug, TS)]
pub struct Res {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[allow(async_fn_in_trait)]
#[enum_dispatch]
pub trait CmdExector {
    async fn execute(self) -> anyhow::Result<()>;
}
