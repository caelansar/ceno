use anyhow::Result;
use glob::glob;
use std::{collections::BTreeSet, fs::File, path::PathBuf};

use crate::BUILD_DIR;

// get all files with certain extension in a directory
pub(crate) fn get_files_with_exts(dir: &str, exts: &[&str]) -> Result<BTreeSet<PathBuf>> {
    // glob all targets files and exclude BUILD_DIR
    let mut files = BTreeSet::new();
    for ext in exts {
        let rule = format!("{}/**/*.{}", dir, ext);
        let paths = glob(&rule)?
            .filter_map(|p| p.ok())
            .filter(|p| {
                p.parent()
                    .and_then(|parent| parent.to_str())
                    .map_or(false, |parent_str| !parent_str.contains(BUILD_DIR))
            })
            .collect::<BTreeSet<PathBuf>>();
        files.extend(paths);
    }
    Ok(files)
}

pub(crate) fn calc_project_hash(dir: &str) -> Result<String> {
    calc_hash_for_files(dir, &["ts", "js", "json"], 16)
}

pub(crate) fn calc_hash_for_files(dir: &str, exts: &[&str], len: usize) -> Result<String> {
    let files = get_files_with_exts(dir, exts)?;
    let mut hasher = blake3::Hasher::new();
    for file in files {
        hasher.update_reader(File::open(file)?)?;
    }
    let mut ret = hasher.finalize().to_string();
    ret.truncate(len);
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_files_with_exts_should_work() -> Result<()> {
        let files = get_files_with_exts("testdata/project", &["ts", "js", "json"])?;
        assert_eq!(
            files.into_iter().collect::<Vec<_>>(),
            [
                PathBuf::from("testdata/project/a.ts"),
                PathBuf::from("testdata/project/test1/b.ts"),
                PathBuf::from("testdata/project/test1/c.js"),
                PathBuf::from("testdata/project/test2/test3/d.json"),
            ]
        );
        Ok(())
    }

    #[test]
    fn calc_hash_for_files_should_work() -> Result<()> {
        let hash = calc_hash_for_files("testdata/project", &["ts", "js", "json"], 12)?;
        assert_eq!(hash, "af1349b9f5f9");
        Ok(())
    }
}
