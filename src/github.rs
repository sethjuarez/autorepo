use std::{process::Command, time::Duration};

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tokio::time::sleep;

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

#[derive(Debug, Clone)]
pub struct LiveGitHubClient {
    http: reqwest::Client,
    token: String,
}

impl LiveGitHubClient {
    pub fn from_env() -> Result<Self> {
        let token = std::env::var("GH_TOKEN")
            .or_else(|_| std::env::var("GITHUB_TOKEN"))
            .or_else(|_| gh_auth_token())
            .context("set GH_TOKEN/GITHUB_TOKEN or authenticate with gh before running --yes")?;

        Ok(Self {
            http: reqwest::Client::new(),
            token,
        })
    }

    pub async fn repo_is_empty(&self, repo: &RepoRef) -> Result<bool> {
        let branches: Vec<BranchRef> = self
            .send(Method::GET, repo, "branches?per_page=1", None)
            .await?;
        Ok(branches.is_empty())
    }

    pub async fn ensure_label(
        &self,
        repo: &RepoRef,
        name: &str,
        color: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        let path = format!("labels/{}", urlencoding::encode(name));
        if self
            .get_optional::<LabelResponse>(repo, &path)
            .await?
            .is_some()
        {
            println!("skip label {name}");
            return Ok(());
        }

        let body = json!({
            "name": name,
            "color": color.unwrap_or("ededed"),
            "description": description.unwrap_or(""),
        });
        self.send_empty(Method::POST, repo, "labels", Some(body))
            .await?;
        println!("create label {name}");
        Ok(())
    }

    pub async fn ensure_milestone(
        &self,
        repo: &RepoRef,
        title: &str,
        description: Option<&str>,
    ) -> Result<u64> {
        if let Some(existing) = self.find_milestone(repo, title).await? {
            println!("skip milestone {title}");
            return Ok(existing.number);
        }

        let body = json!({
            "title": title,
            "description": description.unwrap_or(""),
        });
        let created: MilestoneResponse = self
            .send(Method::POST, repo, "milestones", Some(body))
            .await?;
        println!("create milestone {title}");
        Ok(created.number)
    }

    pub async fn ensure_file(
        &self,
        repo: &RepoRef,
        path: &str,
        content: &str,
        marker: &str,
        branch: Option<&str>,
    ) -> Result<()> {
        let api_path = contents_path(path, branch);
        if let Some(existing) = self
            .get_optional::<ContentResponse>(repo, &api_path)
            .await?
        {
            let decoded = decode_content(&existing.content)?;
            if decoded.contains(marker) {
                println!("skip file {path}");
                return Ok(());
            }
            bail!("file '{path}' already exists without autorepo marker");
        }

        let mut body = json!({
            "message": format!("chore: add {path}"),
            "content": STANDARD.encode(content),
        });
        if let Some(branch) = branch {
            body["branch"] = json!(branch);
        }
        self.send_empty(
            Method::PUT,
            repo,
            &format!("contents/{}", encode_path(path)),
            Some(body),
        )
        .await?;
        println!("create file {path}");
        Ok(())
    }

    pub async fn ensure_branch(
        &self,
        repo: &RepoRef,
        id: &str,
        name: &str,
        marker: &str,
    ) -> Result<()> {
        if self.get_ref(repo, name).await?.is_some() {
            let marker_path = branch_marker_path(id);
            let api_path = contents_path(&marker_path, Some(name));
            if let Some(existing) = self
                .get_optional::<ContentResponse>(repo, &api_path)
                .await?
            {
                let decoded = decode_content(&existing.content)?;
                if decoded.contains(marker) {
                    println!("skip branch {name}");
                    return Ok(());
                }
            }
            bail!("branch '{name}' already exists without autorepo marker");
        }

        let default_branch = self.default_branch(repo).await?;
        let base = self
            .get_ref(repo, &default_branch)
            .await?
            .with_context(|| format!("default branch '{default_branch}' does not have a ref"))?;
        let body = json!({
            "ref": format!("refs/heads/{name}"),
            "sha": base.object.sha,
        });
        self.send_empty(Method::POST, repo, "git/refs", Some(body))
            .await?;

        let marker_path = branch_marker_path(id);
        let content = format!("{marker}\n\nThis branch was created by autorepo.\n");
        self.ensure_file(repo, &marker_path, &content, marker, Some(name))
            .await?;
        println!("create branch {name}");
        Ok(())
    }

    pub async fn ensure_issue(
        &self,
        repo: &RepoRef,
        title: &str,
        body: &str,
        marker: &str,
        labels: &[String],
        milestone: Option<u64>,
    ) -> Result<u64> {
        if let Some(existing) = self.find_marked_issue(repo, marker).await? {
            println!("skip issue {title}");
            return Ok(existing.number);
        }
        if self.find_unmarked_issue_title(repo, title).await?.is_some() {
            bail!("issue '{title}' already exists without autorepo marker");
        }

        let mut request = json!({
            "title": title,
            "body": marked_body(body, marker),
            "labels": labels,
        });
        if let Some(milestone) = milestone {
            request["milestone"] = json!(milestone);
        }

        let created: IssueResponse = self
            .send(Method::POST, repo, "issues", Some(request))
            .await?;
        println!("create issue {title}");
        Ok(created.number)
    }

    pub async fn ensure_pull_request(
        &self,
        repo: &RepoRef,
        title: &str,
        body: &str,
        marker: &str,
        branch: &str,
        labels: &[String],
    ) -> Result<u64> {
        if let Some(existing) = self.find_marked_pr(repo, marker).await? {
            println!("skip pull_request {title}");
            return Ok(existing.number);
        }
        if self.find_unmarked_pr_title(repo, title).await?.is_some() {
            bail!("pull request '{title}' already exists without autorepo marker");
        }

        let default_branch = self.default_branch(repo).await?;
        let request = json!({
            "title": title,
            "body": marked_body(body, marker),
            "head": branch,
            "base": default_branch,
        });
        let created: PullResponse = self
            .send(Method::POST, repo, "pulls", Some(request))
            .await?;
        if !labels.is_empty() {
            self.add_issue_labels(repo, created.number, labels).await?;
        }
        println!("create pull_request {title}");
        Ok(created.number)
    }

    async fn add_issue_labels(&self, repo: &RepoRef, number: u64, labels: &[String]) -> Result<()> {
        let body = json!({ "labels": labels });
        self.send_empty(
            Method::POST,
            repo,
            &format!("issues/{number}/labels"),
            Some(body),
        )
        .await
    }

    async fn default_branch(&self, repo: &RepoRef) -> Result<String> {
        let info: RepoResponse = self.send(Method::GET, repo, "", None).await?;
        Ok(info.default_branch)
    }

    async fn get_ref(&self, repo: &RepoRef, branch: &str) -> Result<Option<GitRefResponse>> {
        self.get_optional(repo, &format!("git/ref/heads/{branch}"))
            .await
    }

    async fn find_milestone(
        &self,
        repo: &RepoRef,
        title: &str,
    ) -> Result<Option<MilestoneResponse>> {
        let milestones: Vec<MilestoneResponse> = self
            .send(Method::GET, repo, "milestones?state=all&per_page=100", None)
            .await?;
        Ok(milestones
            .into_iter()
            .find(|milestone| milestone.title == title))
    }

    async fn find_marked_issue(
        &self,
        repo: &RepoRef,
        marker: &str,
    ) -> Result<Option<IssueResponse>> {
        let issues = self.list_issues(repo).await?;
        Ok(issues
            .into_iter()
            .filter(|issue| issue.pull_request.is_none())
            .find(|issue| issue.body.as_deref().unwrap_or("").contains(marker)))
    }

    async fn find_unmarked_issue_title(
        &self,
        repo: &RepoRef,
        title: &str,
    ) -> Result<Option<IssueResponse>> {
        let issues = self.list_issues(repo).await?;
        Ok(issues
            .into_iter()
            .filter(|issue| issue.pull_request.is_none())
            .find(|issue| issue.title == title))
    }

    async fn find_marked_pr(&self, repo: &RepoRef, marker: &str) -> Result<Option<PullResponse>> {
        let pulls = self.list_pulls(repo).await?;
        Ok(pulls
            .into_iter()
            .find(|pull| pull.body.as_deref().unwrap_or("").contains(marker)))
    }

    async fn find_unmarked_pr_title(
        &self,
        repo: &RepoRef,
        title: &str,
    ) -> Result<Option<PullResponse>> {
        let pulls = self.list_pulls(repo).await?;
        Ok(pulls.into_iter().find(|pull| pull.title == title))
    }

    async fn list_issues(&self, repo: &RepoRef) -> Result<Vec<IssueResponse>> {
        self.send(Method::GET, repo, "issues?state=all&per_page=100", None)
            .await
    }

    async fn list_pulls(&self, repo: &RepoRef) -> Result<Vec<PullResponse>> {
        self.send(Method::GET, repo, "pulls?state=all&per_page=100", None)
            .await
    }

    async fn get_optional<T: DeserializeOwned>(
        &self,
        repo: &RepoRef,
        path: &str,
    ) -> Result<Option<T>> {
        let url = repo_url(repo, path);
        let response = self
            .request(Method::GET, &url)
            .send()
            .await
            .with_context(|| format!("GitHub GET {url} failed"))?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        parse_response(response, &url).await.map(Some)
    }

    async fn send_empty(
        &self,
        method: Method,
        repo: &RepoRef,
        path: &str,
        body: Option<Value>,
    ) -> Result<()> {
        let _: Value = self.send(method, repo, path, body).await?;
        Ok(())
    }

    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        repo: &RepoRef,
        path: &str,
        body: Option<Value>,
    ) -> Result<T> {
        let url = repo_url(repo, path);
        let mut attempt = 0;
        loop {
            let mut request = self.request(method.clone(), &url);
            if let Some(body) = &body {
                request = request.json(body);
            }
            let response = request
                .send()
                .await
                .with_context(|| format!("GitHub {} {url} failed", method.as_str()))?;

            if matches!(
                response.status(),
                StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
            ) && attempt < 3
            {
                attempt += 1;
                sleep(Duration::from_secs(2_u64.pow(attempt))).await;
                continue;
            }

            return parse_response(response, &url).await;
        }
    }

    fn request(&self, method: Method, url: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .bearer_auth(&self.token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "autorepo")
    }
}

fn gh_auth_token() -> Result<String> {
    let output = Command::new("gh").args(["auth", "token"]).output()?;
    if !output.status.success() {
        bail!("gh auth token failed");
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

async fn parse_response<T: DeserializeOwned>(response: reqwest::Response, url: &str) -> Result<T> {
    let status = response.status();
    let text = response.text().await?;
    if !status.is_success() {
        bail!("GitHub request to {url} failed with {status}: {text}");
    }
    if text.trim().is_empty() {
        return serde_json::from_str("null").context("failed to parse empty GitHub response");
    }
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse GitHub response from {url}"))
}

fn repo_url(repo: &RepoRef, path: &str) -> String {
    let suffix = if path.is_empty() {
        String::new()
    } else {
        format!("/{path}")
    };
    format!(
        "https://api.github.com/repos/{}/{}{}",
        repo.owner, repo.name, suffix
    )
}

fn contents_path(path: &str, branch: Option<&str>) -> String {
    let encoded = encode_path(path);
    match branch {
        Some(branch) => format!("contents/{encoded}?ref={}", urlencoding::encode(branch)),
        None => format!("contents/{encoded}"),
    }
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(urlencoding::encode)
        .collect::<Vec<_>>()
        .join("/")
}

fn decode_content(content: &str) -> Result<String> {
    let compact = content.replace(['\n', '\r'], "");
    let bytes = STANDARD.decode(compact)?;
    String::from_utf8(bytes).context("GitHub file content is not UTF-8")
}

fn marked_body(body: &str, marker: &str) -> String {
    if body.contains(marker) {
        body.to_string()
    } else {
        format!("{body}\n\n{marker}")
    }
}

fn branch_marker_path(id: &str) -> String {
    format!(".autorepo/branches/{id}.md")
}

#[derive(Debug, Deserialize)]
struct RepoResponse {
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct BranchRef {
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct LabelResponse {
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct MilestoneResponse {
    number: u64,
    title: String,
}

#[derive(Debug, Deserialize)]
struct ContentResponse {
    content: String,
}

#[derive(Debug, Deserialize)]
struct GitRefResponse {
    object: GitObject,
}

#[derive(Debug, Deserialize)]
struct GitObject {
    sha: String,
}

#[derive(Debug, Deserialize)]
struct IssueResponse {
    number: u64,
    title: String,
    body: Option<String>,
    pull_request: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct PullResponse {
    number: u64,
    title: String,
    body: Option<String>,
}
