use anyhow::{Result, bail};

use crate::github::{RepoRef, gh_auth_token};

pub async fn run(repo: &str) -> Result<()> {
    let repo = RepoRef::parse(repo)?;

    println!("Repository: {}/{}", repo.owner, repo.name);
    println!("Authentication: {}", auth_status());
    println!("Writes: disabled during doctor");
    Ok(())
}

fn auth_status() -> &'static str {
    if ["AUTOREPO_GITHUB_TOKEN", "GH_TOKEN", "GITHUB_TOKEN"]
        .into_iter()
        .any(|name| {
            std::env::var(name)
                .ok()
                .map(|token| !token.trim().is_empty())
                .unwrap_or(false)
        })
    {
        "token present"
    } else if gh_auth_token().is_ok() {
        "GitHub CLI token present"
    } else {
        "no AUTOREPO_GITHUB_TOKEN, GH_TOKEN, or GITHUB_TOKEN found"
    }
}

pub fn ensure_empty_repo(empty: bool, allow_non_empty: bool) -> Result<()> {
    if !empty && !allow_non_empty {
        bail!("refusing to prepare a non-empty repository without --allow-non-empty");
    }

    Ok(())
}
