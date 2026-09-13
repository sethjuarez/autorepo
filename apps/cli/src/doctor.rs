use anyhow::{Result, bail};

use crate::github::RepoRef;

pub async fn run(repo: &str) -> Result<()> {
    let repo = RepoRef::parse(repo)?;

    println!("Repository: {}/{}", repo.owner, repo.name);
    println!("Authentication: {}", auth_status());
    println!("Writes: disabled during doctor");
    Ok(())
}

fn auth_status() -> &'static str {
    if std::env::var_os("GITHUB_TOKEN").is_some() || std::env::var_os("GH_TOKEN").is_some() {
        "token present"
    } else {
        "no GITHUB_TOKEN or GH_TOKEN found"
    }
}

pub fn ensure_empty_repo(empty: bool, allow_non_empty: bool) -> Result<()> {
    if !empty && !allow_non_empty {
        bail!("refusing to prepare a non-empty repository without --allow-non-empty");
    }

    Ok(())
}
