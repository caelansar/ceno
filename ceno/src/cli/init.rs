use crate::CmdExector;
use anyhow::Result;
use askama::Template;
use ceno_server::{Req, Res};
use clap::Parser;
use dialoguer::Input;
use git2::Repository;
use std::{fs, path::Path};
use ts_rs::TS as _;

#[derive(Debug, Parser)]
pub struct InitOpts {}

#[derive(Template)]
#[template(path = "config.yml.j2")]
struct ConfigFile {
    name: String,
}

#[derive(Template)]
#[template(path = "main.ts.j2")]
struct MainTsFile {}

#[derive(Template)]
#[template(path = ".gitignore.j2")]
struct GitIgnoreFile {}

impl CmdExector for InitOpts {
    async fn execute(self) -> Result<()> {
        let name: String = Input::new().with_prompt("Project name").interact_text()?;

        // if current dir is empty then init project, otherwise create new dir and init project
        let cur = Path::new(".");
        if fs::read_dir(cur)?.next().is_none() {
            init_project(&name, cur)?;
        } else {
            let path = cur.join(&name);
            init_project(&name, &path)?;
        }

        Ok(())
    }
}

fn init_project(name: &str, path: &Path) -> Result<()> {
    // init git repository
    Repository::init(path)?;
    // init config file
    let config = ConfigFile {
        name: name.to_string(),
    };
    fs::write(path.join("config.yml"), config.render()?)?;
    // init main.ts file
    fs::write(path.join("main.ts"), MainTsFile {}.render()?)?;
    // init .gitignore file
    fs::write(path.join(".gitignore"), GitIgnoreFile {}.render()?)?;

    // init types.d.ts
    let mut s = String::new();
    s.push_str(&Req::decl());
    s.push('\n');
    s.push_str(&Res::decl());
    s.push('\n');
    s.push_str("export {Req, Res}\n");
    fs::write(path.join("types.d.ts"), s)?;

    Ok(())
}
