use crate::CmdExector;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct RunOpts {
    // port to listen
    #[arg(short, long, default_value = "5000")]
    pub port: u16,
}

impl CmdExector for RunOpts {
    async fn execute(self) -> anyhow::Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn path_end_with_should_work() {
        let p1 = Path::new("/private/tmp/ceno-test/main.ts");
        let p2 = Path::new("/private/tmp/ceno-test/config.yml");
        let ext = p1.extension().unwrap_or_default();
        assert!(ext == "ts" || ext == "js");
        assert!(p2.ends_with("config.yml"));
    }
}
