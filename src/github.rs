use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}

impl RepoRef {
    pub fn parse(value: &str) -> Result<Self> {
        let Some((owner, name)) = value.split_once('/') else {
            bail!("repository must be in OWNER/REPO form");
        };

        if owner.is_empty() || name.is_empty() || value.matches('/').count() != 1 {
            bail!("repository must be in OWNER/REPO form");
        }

        Ok(Self {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct GitHubClient;

impl GitHubClient {
    pub fn repo_is_empty_fixture(&self, _repo: &RepoRef) -> bool {
        true
    }
}
