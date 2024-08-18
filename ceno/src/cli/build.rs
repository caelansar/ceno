use crate::utils::calc_project_hash;
use crate::{CmdExector, BUILD_DIR};
use bundler::run_bundle;
use clap::Parser;
use std::fs::File;
use std::path::Path;
use std::{env, fs, io};

#[derive(Debug, Parser)]
pub struct BuildOpts {}

impl CmdExector for BuildOpts {
    async fn execute(self) -> anyhow::Result<()> {
        let cur_dir = env::current_dir()?.display().to_string();
        let filename = build_project(&cur_dir)?;
        eprintln!("Build success: {}", filename);

        Ok(())
    }
}

pub(crate) fn build_project(dir: &str) -> anyhow::Result<String> {
    let hash = calc_project_hash(dir)?;

    fs::remove_dir_all(BUILD_DIR)?;
    fs::create_dir_all(BUILD_DIR)?;

    let filename = format!("{}/{}.js", BUILD_DIR, hash);
    let config = format!("{}/{}.yml", BUILD_DIR, hash);
    let dst = Path::new(&filename);
    // if the file already exists, skip building
    if dst.exists() {
        return Ok(filename);
    }

    // build the project
    let content = run_bundle("main.ts", &Default::default())?;
    fs::write(dst, content)?;
    let mut dst = File::create(config)?;
    let mut src = File::open("config.yml")?;
    io::copy(&mut src, &mut dst)?;

    Ok(filename)
}
