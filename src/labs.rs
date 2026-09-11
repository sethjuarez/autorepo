use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::github::RepoRef;

const SNAPSHOT_FILE: &str = "autorepo-session-snapshot.yml";

#[derive(Debug)]
pub struct InspectSessionArgs {
    pub copilot_home: Option<PathBuf>,
    pub session_id: String,
}

#[derive(Debug)]
pub struct CaptureSessionArgs {
    pub copilot_home: Option<PathBuf>,
    pub session_id: String,
    pub out: PathBuf,
    pub include_transcripts: bool,
}

#[derive(Debug)]
pub struct RehydrateSessionArgs {
    pub copilot_home: Option<PathBuf>,
    pub repo: String,
    pub snapshot: PathBuf,
    pub workspace: PathBuf,
    pub branch: Option<String>,
    pub dry_run: bool,
    pub allow_live_copilot_home: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionSnapshot {
    schema: u32,
    kind: String,
    source_session_id: String,
    title: String,
    summary: Option<String>,
    mode: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<String>,
    repository: Option<String>,
    branch: Option<String>,
    cwd: Option<String>,
    captured_at: String,
    turns: Vec<SnapshotTurn>,
    checkpoint: Option<SnapshotCheckpoint>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SnapshotTurn {
    turn_index: i64,
    user_message: Option<String>,
    assistant_response: Option<String>,
    timestamp: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SnapshotCheckpoint {
    title: Option<String>,
    overview: Option<String>,
    history: Option<String>,
    work_done: Option<String>,
    technical_details: Option<String>,
    important_files: Option<String>,
    next_steps: Option<String>,
}

#[derive(Debug, Serialize)]
struct SessionInspection {
    session_id: String,
    session_folder_exists: bool,
    events: EventSummary,
    app_index: Option<AppSessionSummary>,
    history_index: Option<HistorySessionSummary>,
}

#[derive(Debug, Default, Serialize)]
struct EventSummary {
    count: usize,
    first_timestamp: Option<String>,
    last_timestamp: Option<String>,
}

#[derive(Debug, Serialize)]
struct AppSessionSummary {
    title: Option<String>,
    session_type: Option<String>,
    mode: Option<String>,
    is_running: bool,
    workspace_id: Option<String>,
    workspace_name: Option<String>,
    workspace_branch: Option<String>,
    created_pr: Option<String>,
}

#[derive(Debug, Serialize)]
struct HistorySessionSummary {
    cwd: Option<String>,
    repository: Option<String>,
    branch: Option<String>,
    summary: Option<String>,
    turn_count: i64,
    checkpoint_count: i64,
}

pub fn inspect_session(args: InspectSessionArgs) -> Result<()> {
    let home = copilot_home(args.copilot_home)?;
    let inspection = inspect(&home, &args.session_id)?;
    println!("{}", serde_json::to_string_pretty(&inspection)?);
    Ok(())
}

pub fn capture_session(args: CaptureSessionArgs) -> Result<()> {
    if !args.include_transcripts {
        bail!(
            "capturing a rehydratable session includes transcript text; pass --include-transcripts when the session is safe to store as a fixture"
        );
    }
    let home = copilot_home(args.copilot_home)?;
    let snapshot = capture(&home, &args.session_id)?;

    fs::create_dir_all(&args.out)
        .with_context(|| format!("failed to create {}", args.out.display()))?;
    let path = args.out.join(SNAPSHOT_FILE);
    fs::write(&path, serde_yaml::to_string(&snapshot)?)
        .with_context(|| format!("failed to write {}", path.display()))?;

    println!(
        "Captured session '{}' with {} turns to {}.",
        snapshot.source_session_id,
        snapshot.turns.len(),
        path.display()
    );
    println!(
        "Review this fixture before sharing or committing it; it contains transcript text from the captured session."
    );
    Ok(())
}

pub fn rehydrate_session(args: RehydrateSessionArgs) -> Result<()> {
    let default_home = copilot_home(None)?;
    let home = copilot_home(args.copilot_home)?;
    if !args.dry_run && same_existing_path(&home, &default_home)? && !args.allow_live_copilot_home {
        bail!(
            "refusing to write the default live Copilot home {}; pass --copilot-home or --allow-live-copilot-home",
            home.display()
        );
    }

    let repo = RepoRef::parse(&args.repo)?;
    let snapshot = read_snapshot(&args.snapshot)?;
    validate_snapshot(&snapshot)?;
    let workspace = absolute_existing_path(&args.workspace)?;
    validate_workspace_repo(&workspace, &repo)?;
    let project_id = find_project_id(&home, &repo, &workspace)?;
    let plan = RehydratePlan::new(snapshot, repo, project_id, workspace, args.branch)?;
    plan.preflight(&home)?;

    if args.dry_run {
        println!("{}", serde_json::to_string_pretty(&plan.summary())?);
        return Ok(());
    }

    plan.apply(&home)?;
    println!(
        "Rehydrated session '{}' for {} into {}.",
        plan.session_id,
        plan.repo_full_name,
        home.display()
    );
    Ok(())
}

fn inspect(home: &Path, session_id: &str) -> Result<SessionInspection> {
    Ok(SessionInspection {
        session_id: session_id.to_owned(),
        session_folder_exists: home.join("session-state").join(session_id).is_dir(),
        events: summarize_events(
            &home
                .join("session-state")
                .join(session_id)
                .join("events.jsonl"),
        )?,
        app_index: app_session_summary(home, session_id)?,
        history_index: history_session_summary(home, session_id)?,
    })
}

fn capture(home: &Path, session_id: &str) -> Result<SessionSnapshot> {
    let app = app_session_summary(home, session_id)?;
    let history = history_session_summary(home, session_id)?
        .with_context(|| format!("session '{session_id}' was not found in session-store.db"))?;
    let turns = read_turns(home, session_id)?;
    if turns.is_empty() {
        bail!("session '{session_id}' has no captured turns");
    }

    let checkpoint = read_latest_checkpoint(home, session_id)?;
    Ok(SessionSnapshot {
        schema: 1,
        kind: "copilot_app_session".to_owned(),
        source_session_id: session_id.to_owned(),
        title: app
            .as_ref()
            .and_then(|app| app.title.clone())
            .or_else(|| history.summary.clone())
            .unwrap_or_else(|| "autorepo captured session".to_owned()),
        summary: history.summary,
        mode: app.and_then(|app| app.mode),
        model: read_app_session_field(home, session_id, "model")?,
        reasoning_effort: read_app_session_field(home, session_id, "reasoning_effort")?,
        repository: history.repository,
        branch: history.branch,
        cwd: None,
        captured_at: now_rfc3339()?,
        turns,
        checkpoint,
    })
}

fn app_session_summary(home: &Path, session_id: &str) -> Result<Option<AppSessionSummary>> {
    let db = home.join("data.db");
    if !db.is_file() {
        return Ok(None);
    }
    let conn = open_readonly(&db)?;
    let app = conn
        .query_row(
            r#"
            select s.title, s.session_type, s.mode, s.is_running,
                   a.workspace_id, w.name, w.branch
            from sessions s
            left join workspace_session_aliases a on a.session_id = s.id
            left join workspaces w on w.id = a.workspace_id
            where s.id = ?1
            "#,
            params![session_id],
            |row| {
                Ok(AppSessionSummary {
                    title: row.get(0)?,
                    session_type: row.get(1)?,
                    mode: row.get(2)?,
                    is_running: row.get::<_, i64>(3)? != 0,
                    workspace_id: row.get(4)?,
                    workspace_name: row.get(5)?,
                    workspace_branch: row.get(6)?,
                    created_pr: None,
                })
            },
        )
        .optional()?;
    Ok(app)
}

fn history_session_summary(home: &Path, session_id: &str) -> Result<Option<HistorySessionSummary>> {
    let db = home.join("session-store.db");
    if !db.is_file() {
        return Ok(None);
    }
    let conn = open_readonly(&db)?;
    let history = conn
        .query_row(
            r#"
            select s.cwd, s.repository, s.branch, s.summary,
                   (select count(*) from turns where session_id = s.id),
                   (select count(*) from checkpoints where session_id = s.id)
            from sessions s
            where s.id = ?1
            "#,
            params![session_id],
            |row| {
                Ok(HistorySessionSummary {
                    cwd: row.get(0)?,
                    repository: row.get(1)?,
                    branch: row.get(2)?,
                    summary: row.get(3)?,
                    turn_count: row.get(4)?,
                    checkpoint_count: row.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(history)
}

fn read_turns(home: &Path, session_id: &str) -> Result<Vec<SnapshotTurn>> {
    let conn = open_readonly(&home.join("session-store.db"))?;
    let mut stmt = conn.prepare(
        "select turn_index, user_message, assistant_response, timestamp from turns where session_id = ?1 order by turn_index",
    )?;
    let turns = stmt
        .query_map(params![session_id], |row| {
            Ok(SnapshotTurn {
                turn_index: row.get(0)?,
                user_message: row.get(1)?,
                assistant_response: row.get(2)?,
                timestamp: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(turns)
}

fn read_latest_checkpoint(home: &Path, session_id: &str) -> Result<Option<SnapshotCheckpoint>> {
    let conn = open_readonly(&home.join("session-store.db"))?;
    let checkpoint = conn
        .query_row(
            r#"
            select title, overview, history, work_done, technical_details, important_files, next_steps
            from checkpoints
            where session_id = ?1
            order by checkpoint_number desc
            limit 1
            "#,
            params![session_id],
            |row| {
                Ok(SnapshotCheckpoint {
                    title: row.get(0)?,
                    overview: row.get(1)?,
                    history: row.get(2)?,
                    work_done: row.get(3)?,
                    technical_details: row.get(4)?,
                    important_files: row.get(5)?,
                    next_steps: row.get(6)?,
                })
            },
        )
        .optional()?;
    Ok(checkpoint)
}

fn read_app_session_field(home: &Path, session_id: &str, field: &str) -> Result<Option<String>> {
    let db = home.join("data.db");
    if !db.is_file() {
        return Ok(None);
    }
    let conn = open_readonly(&db)?;
    let value = conn
        .query_row(
            &format!("select {field} from sessions where id = ?1"),
            params![session_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value)
}

#[derive(Debug)]
struct RehydratePlan {
    session_id: String,
    workspace_id: String,
    worktree_id: Option<String>,
    project_id: String,
    repo_full_name: String,
    title: String,
    mode: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    workspace_path: PathBuf,
    branch: String,
    checkout_kind: CheckoutKind,
    created_at: String,
    turns: Vec<SnapshotTurn>,
    checkpoint: Option<SnapshotCheckpoint>,
}

impl RehydratePlan {
    fn new(
        snapshot: SessionSnapshot,
        repo: RepoRef,
        project_id: String,
        workspace_path: PathBuf,
        branch: Option<String>,
    ) -> Result<Self> {
        let branch = match branch {
            Some(branch) => branch,
            None => current_checkout_branch(&workspace_path)?,
        };
        if branch.trim().is_empty() {
            bail!("branch must not be empty");
        }
        let checkout_kind = CheckoutKind::detect(&workspace_path);
        Ok(Self {
            session_id: Uuid::new_v4().to_string(),
            workspace_id: Uuid::new_v4().to_string(),
            worktree_id: checkout_kind
                .writes_worktree_row()
                .then(|| Uuid::new_v4().to_string()),
            project_id,
            repo_full_name: format!("{}/{}", repo.owner, repo.name),
            title: snapshot.title,
            mode: snapshot.mode.unwrap_or_else(|| "interactive".to_owned()),
            model: snapshot.model,
            reasoning_effort: snapshot.reasoning_effort,
            workspace_path,
            branch,
            checkout_kind,
            created_at: now_rfc3339()?,
            turns: snapshot.turns,
            checkpoint: snapshot.checkpoint,
        })
    }

    fn summary(&self) -> serde_json::Value {
        let mut writes = vec![
            "session-state/<session_id>/workspace.yaml",
            "session-state/<session_id>/events.jsonl",
            "session-state/<session_id>/plan.md",
            "data.db:sessions",
            "data.db:workspaces",
            "data.db:workspace_session_aliases",
            "data.db:workspace_checkout_bindings",
            "session-store.db:sessions",
            "session-store.db:turns",
            "session-store.db:checkpoints",
        ];
        if self.checkout_kind.writes_worktree_row() {
            writes.insert(5, "data.db:worktrees");
        }

        json!({
            "session_id": self.session_id,
            "workspace_id": self.workspace_id,
            "worktree_id": self.worktree_id,
            "project_id": self.project_id,
            "repo": self.repo_full_name,
            "title": self.title,
            "mode": self.mode,
            "workspace": self.workspace_path,
            "branch": self.branch,
            "workspace_type": self.checkout_kind.workspace_type(),
            "checkout_kind": self.checkout_kind.checkout_kind(),
            "turns": self.turns.len(),
            "has_checkpoint": self.checkpoint.is_some(),
            "writes": writes
        })
    }

    fn apply(&self, home: &Path) -> Result<()> {
        let result = (|| {
            self.write_session_folder(home)?;
            self.write_app_index(home)?;
            self.write_history_index(home)?;
            Ok(())
        })();
        if result.is_err() {
            self.cleanup_partial(home);
        }
        result
    }

    fn preflight(&self, home: &Path) -> Result<()> {
        require_db_schema(
            &home.join("data.db"),
            &[
                (
                    "sessions",
                    &[
                        "id",
                        "title",
                        "created_at",
                        "updated_at",
                        "session_type",
                        "mode",
                        "is_running",
                        "was_interrupted",
                        "model",
                        "reasoning_effort",
                        "execution_location",
                        "title_source",
                    ][..],
                ),
                (
                    "worktrees",
                    &["id", "project_id", "path", "branch", "created_at"][..],
                ),
                (
                    "workspaces",
                    &[
                        "id",
                        "project_id",
                        "worktree_id",
                        "workspace_type",
                        "branch",
                        "name",
                        "created_at",
                        "updated_at",
                        "session_id",
                        "host_id",
                        "name_source",
                        "is_initialized",
                    ][..],
                ),
                (
                    "workspace_checkout_bindings",
                    &[
                        "workspace_id",
                        "repo_path",
                        "repo_full_name",
                        "display_name",
                        "checkout_kind",
                        "checkout_path",
                        "worktree_id",
                    ][..],
                ),
                (
                    "workspace_session_aliases",
                    &["session_id", "workspace_id", "created_at"][..],
                ),
            ],
        )?;
        require_db_schema(
            &home.join("session-store.db"),
            &[
                (
                    "sessions",
                    &[
                        "id",
                        "cwd",
                        "repository",
                        "branch",
                        "summary",
                        "created_at",
                        "updated_at",
                        "host_type",
                    ][..],
                ),
                (
                    "turns",
                    &[
                        "session_id",
                        "turn_index",
                        "user_message",
                        "assistant_response",
                        "timestamp",
                    ][..],
                ),
                (
                    "checkpoints",
                    &[
                        "session_id",
                        "checkpoint_number",
                        "title",
                        "overview",
                        "history",
                        "work_done",
                        "technical_details",
                        "important_files",
                        "next_steps",
                        "created_at",
                    ][..],
                ),
            ],
        )?;
        if !self.workspace_path.is_dir() {
            bail!(
                "workspace path '{}' does not exist",
                self.workspace_path.display()
            );
        }
        if !self.workspace_path.join(".git").exists() {
            bail!(
                "workspace path '{}' is not a git checkout",
                self.workspace_path.display()
            );
        }
        let session_root = home.join("session-state").join(&self.session_id);
        if session_root.exists() {
            bail!("session folder already exists: {}", session_root.display());
        }
        Ok(())
    }

    fn write_session_folder(&self, home: &Path) -> Result<()> {
        let root = home.join("session-state").join(&self.session_id);
        if root.exists() {
            bail!("session folder already exists: {}", root.display());
        }
        fs::create_dir_all(root.join("files"))?;
        fs::create_dir_all(root.join("checkpoints"))?;
        fs::create_dir_all(root.join("research"))?;
        fs::write(root.join("workspace.yaml"), self.workspace_yaml())?;
        fs::write(root.join("events.jsonl"), self.events_jsonl()?)?;
        fs::write(root.join("plan.md"), self.plan_markdown())?;
        Ok(())
    }

    fn write_app_index(&self, home: &Path) -> Result<()> {
        let mut conn = open_write(&home.join("data.db"))?;
        let tx = conn.transaction()?;
        tx.execute(
            r#"
            insert into sessions
                (id, title, created_at, updated_at, session_type, mode, is_running,
                 was_interrupted, model, reasoning_effort, execution_location, title_source)
            values (?1, ?2, ?3, ?3, 'project', ?4, 0, 0, ?5, ?6, 'local', 'agent')
            "#,
            params![
                self.session_id,
                self.title,
                self.created_at,
                self.mode,
                self.model,
                self.reasoning_effort
            ],
        )?;
        if let Some(worktree_id) = &self.worktree_id {
            tx.execute(
                r#"
                insert into worktrees (id, project_id, path, branch, created_at)
                values (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    worktree_id,
                    self.project_id,
                    self.workspace_path.display().to_string(),
                    self.branch,
                    self.created_at
                ],
            )?;
        }
        tx.execute(
            r#"
            insert into workspaces
                (id, project_id, worktree_id, workspace_type, branch, name, created_at, updated_at,
                 session_id, host_id, name_source, is_initialized)
            values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, 'local', 'agent', 1)
            "#,
            params![
                self.workspace_id,
                self.project_id,
                self.worktree_id,
                self.checkout_kind.workspace_type(),
                self.branch,
                self.title,
                self.created_at,
                self.session_id
            ],
        )?;
        tx.execute(
            "insert into workspace_session_aliases (session_id, workspace_id, created_at) values (?1, ?2, ?3)",
            params![self.session_id, self.workspace_id, self.created_at],
        )?;
        tx.execute(
            r#"
            insert into workspace_checkout_bindings
                (workspace_id, repo_path, repo_full_name, display_name, checkout_kind, checkout_path, worktree_id)
            values (?1, ?2, ?3, ?4, ?5, ?2, ?6)
            "#,
            params![
                self.workspace_id,
                self.workspace_path.display().to_string(),
                self.repo_full_name,
                self.title,
                self.checkout_kind.checkout_kind(),
                self.worktree_id
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn write_history_index(&self, home: &Path) -> Result<()> {
        let mut conn = open_write(&home.join("session-store.db"))?;
        let tx = conn.transaction()?;
        tx.execute(
            r#"
            insert into sessions (id, cwd, repository, branch, summary, created_at, updated_at, host_type)
            values (?1, ?2, ?3, ?4, ?5, ?6, ?6, 'github')
            "#,
            params![
                self.session_id,
                self.workspace_path.display().to_string(),
                self.repo_full_name,
                self.branch,
                self.title,
                self.created_at
            ],
        )?;
        for turn in &self.turns {
            tx.execute(
                r#"
                insert into turns (session_id, turn_index, user_message, assistant_response, timestamp)
                values (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    self.session_id,
                    turn.turn_index,
                    turn.user_message,
                    turn.assistant_response,
                    turn.timestamp.as_deref().unwrap_or(&self.created_at)
                ],
            )?;
        }
        if let Some(checkpoint) = &self.checkpoint {
            tx.execute(
                r#"
                insert into checkpoints
                    (session_id, checkpoint_number, title, overview, history, work_done,
                     technical_details, important_files, next_steps, created_at)
                values (?1, 0, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    self.session_id,
                    checkpoint.title,
                    checkpoint.overview,
                    checkpoint.history,
                    checkpoint.work_done,
                    checkpoint.technical_details,
                    checkpoint.important_files,
                    checkpoint.next_steps,
                    self.created_at
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn workspace_yaml(&self) -> String {
        #[derive(Serialize)]
        struct WorkspaceYaml<'a> {
            id: &'a str,
            cwd: String,
            client_name: &'a str,
            name: &'a str,
            user_named: bool,
            summary_count: u32,
            fork_count: u32,
            created_at: &'a str,
            updated_at: &'a str,
        }

        serde_yaml::to_string(&WorkspaceYaml {
            id: &self.session_id,
            cwd: self.workspace_path.display().to_string(),
            client_name: "autorepo/labs",
            name: &self.title,
            user_named: true,
            summary_count: 0,
            fork_count: 0,
            created_at: &self.created_at,
            updated_at: &self.created_at,
        })
        .expect("serializing workspace metadata should not fail")
    }

    fn events_jsonl(&self) -> Result<String> {
        let mut lines = Vec::new();
        lines.push(serde_json::to_string(&json!({
            "id": Uuid::new_v4().to_string(),
            "parentId": null,
            "timestamp": self.created_at,
            "type": "session.start",
            "data": {
                "sessionId": self.session_id,
                "version": 1,
                "producer": "autorepo-labs",
                "startTime": self.created_at,
                "selectedModel": self.model,
                "reasoningEffort": self.reasoning_effort,
                "context": { "cwd": self.workspace_path },
                "remoteSteerable": false,
                "alreadyInUse": false
            }
        }))?);

        for turn in &self.turns {
            let timestamp = turn.timestamp.as_deref().unwrap_or(&self.created_at);
            let turn_id = turn.turn_index.to_string();
            if let Some(content) = &turn.user_message {
                lines.push(serde_json::to_string(&json!({
                    "id": Uuid::new_v4().to_string(),
                    "parentId": null,
                    "timestamp": timestamp,
                    "type": "user.message",
                    "data": {
                        "content": content,
                        "transformedContent": content,
                        "messageId": Uuid::new_v4().to_string(),
                        "agentMode": self.mode,
                        "delivery": "live",
                        "interactionId": Uuid::new_v4().to_string(),
                        "turnId": turn_id
                    }
                }))?);
            }
            if let Some(content) = &turn.assistant_response {
                lines.push(serde_json::to_string(&json!({
                    "id": Uuid::new_v4().to_string(),
                    "parentId": null,
                    "timestamp": timestamp,
                    "type": "assistant.message",
                    "data": {
                        "messageId": Uuid::new_v4().to_string(),
                        "model": self.model,
                        "content": content,
                        "toolRequests": [],
                        "interactionId": Uuid::new_v4().to_string(),
                        "turnId": turn_id,
                        "phase": "final"
                    }
                }))?);
            }
        }

        Ok(format!("{}\n", lines.join("\n")))
    }

    fn plan_markdown(&self) -> String {
        format!(
            "# Rehydrated autorepo session\n\nRepository: `{}`\n\nThis session was rehydrated from an `autorepo labs session` snapshot.\n",
            self.repo_full_name
        )
    }

    fn cleanup_partial(&self, home: &Path) {
        let _ = fs::remove_dir_all(home.join("session-state").join(&self.session_id));
        if let Ok(mut conn) = open_write(&home.join("data.db"))
            && let Ok(tx) = conn.transaction()
        {
            let _ = tx.execute(
                "delete from workspace_checkout_bindings where workspace_id = ?1",
                params![self.workspace_id],
            );
            let _ = tx.execute(
                "delete from workspace_session_aliases where session_id = ?1",
                params![self.session_id],
            );
            let _ = tx.execute(
                "delete from workspaces where id = ?1",
                params![self.workspace_id],
            );
            if let Some(worktree_id) = &self.worktree_id {
                let _ = tx.execute("delete from worktrees where id = ?1", params![worktree_id]);
            }
            let _ = tx.execute(
                "delete from sessions where id = ?1",
                params![self.session_id],
            );
            let _ = tx.commit();
        }
        if let Ok(mut conn) = open_write(&home.join("session-store.db"))
            && let Ok(tx) = conn.transaction()
        {
            let _ = tx.execute(
                "delete from checkpoints where session_id = ?1",
                params![self.session_id],
            );
            let _ = tx.execute(
                "delete from turns where session_id = ?1",
                params![self.session_id],
            );
            let _ = tx.execute(
                "delete from sessions where id = ?1",
                params![self.session_id],
            );
            let _ = tx.commit();
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CheckoutKind {
    InPlace,
    Worktree,
}

impl CheckoutKind {
    fn detect(workspace: &Path) -> Self {
        if workspace.join(".git").is_file() {
            Self::Worktree
        } else {
            Self::InPlace
        }
    }

    fn workspace_type(self) -> &'static str {
        match self {
            Self::InPlace => "branch",
            Self::Worktree => "worktree",
        }
    }

    fn checkout_kind(self) -> &'static str {
        match self {
            Self::InPlace => "in_place",
            Self::Worktree => "worktree",
        }
    }

    fn writes_worktree_row(self) -> bool {
        matches!(self, Self::Worktree)
    }
}

fn find_project_id(home: &Path, repo: &RepoRef, workspace: &Path) -> Result<String> {
    let data_db = home.join("data.db");
    if !data_db.is_file() {
        bail!(
            "Copilot home '{}' is not seeded: expected data.db. Use a copied Copilot home with data.db and session-store.db, or open the target repo as a project in the app first.",
            home.display()
        );
    }
    let conn = open_readonly(&data_db)?;
    let mut stmt = conn.prepare(
        "select id, main_repo_path from projects where github_owner = ?1 and github_repo = ?2 order by last_opened_at desc",
    )?;
    let candidates = stmt
        .query_map(params![repo.owner, repo.name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if candidates.is_empty() {
        bail!(
            "target Copilot home has no configured project for {}/{}; create/open the project once, then retry",
            repo.owner,
            repo.name
        );
    }

    let mut exact_matches = Vec::new();
    for (id, main_repo_path) in &candidates {
        if same_existing_path(workspace, Path::new(main_repo_path))? {
            exact_matches.push(id.clone());
        }
    }
    if exact_matches.len() == 1 {
        return Ok(exact_matches.remove(0));
    }
    if exact_matches.len() > 1 {
        bail!(
            "target Copilot home has multiple configured projects for workspace '{}'",
            workspace.display()
        );
    }
    if candidates.len() == 1 {
        return Ok(candidates[0].0.clone());
    }

    bail!(
        "target Copilot home has multiple configured projects for {}/{}; pass a workspace that matches one configured project path",
        repo.owner,
        repo.name
    )
}

fn validate_workspace_repo(workspace: &Path, repo: &RepoRef) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(workspace)
        .args(["remote", "get-url", "origin"])
        .output()
        .with_context(|| {
            format!(
                "failed to read origin remote for workspace '{}'",
                workspace.display()
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "failed to read origin remote for workspace '{}': {stderr}",
            workspace.display()
        );
    }
    let remote = String::from_utf8_lossy(&output.stdout);
    let Some(remote_repo) = parse_github_remote(remote.trim()) else {
        bail!(
            "workspace '{}' origin remote is not a GitHub OWNER/REPO URL",
            workspace.display()
        );
    };
    if remote_repo != format!("{}/{}", repo.owner, repo.name) {
        bail!(
            "workspace '{}' origin remote points to {}, not {}/{}",
            workspace.display(),
            remote_repo,
            repo.owner,
            repo.name
        );
    }
    Ok(())
}

fn parse_github_remote(remote: &str) -> Option<String> {
    let trimmed = remote.trim_end_matches(".git");
    if let Some(path) = trimmed.strip_prefix("https://github.com/") {
        return owner_repo_from_path(path);
    }
    if let Some(path) = trimmed.strip_prefix("git@github.com:") {
        return owner_repo_from_path(path);
    }
    if let Some(path) = trimmed.strip_prefix("ssh://git@github.com/") {
        return owner_repo_from_path(path);
    }
    None
}

fn owner_repo_from_path(path: &str) -> Option<String> {
    let mut parts = path.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("{owner}/{name}"))
}

fn require_db_schema(db: &Path, tables: &[(&str, &[&str])]) -> Result<()> {
    if !db.is_file() {
        bail!(
            "required Copilot database '{}' does not exist",
            db.display()
        );
    }
    let conn = open_readonly(db)?;
    for (table, columns) in tables {
        let exists: Option<String> = conn
            .query_row(
                "select name from sqlite_master where type = 'table' and name = ?1",
                params![table],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            bail!(
                "Copilot database '{}' is missing table '{}'",
                db.display(),
                table
            );
        }

        let mut stmt = conn.prepare(&format!("pragma table_info({table})"))?;
        let found = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<std::collections::HashSet<_>>>()?;
        for column in *columns {
            if !found.contains(*column) {
                bail!(
                    "Copilot database '{}' table '{}' is missing column '{}'",
                    db.display(),
                    table,
                    column
                );
            }
        }
    }
    Ok(())
}

fn same_existing_path(left: &Path, right: &Path) -> Result<bool> {
    let left = normalize_path_for_compare(left)?;
    let right = normalize_path_for_compare(right)?;
    #[cfg(windows)]
    {
        Ok(left
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy()))
    }
    #[cfg(not(windows))]
    {
        Ok(left == right)
    }
}

fn normalize_path_for_compare(path: &Path) -> Result<PathBuf> {
    let expanded = expand_home(path)?;
    if expanded.exists() {
        return Ok(expanded.canonicalize()?);
    }
    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        Ok(env::current_dir()?.join(expanded))
    }
}

fn expand_home(path: &Path) -> Result<PathBuf> {
    let text = path.to_string_lossy();
    if text == "~" || text.starts_with("~/") || text.starts_with("~\\") {
        let home = env::var_os("USERPROFILE")
            .or_else(|| env::var_os("HOME"))
            .context("failed to find USERPROFILE or HOME for ~ expansion")?;
        let suffix = text
            .strip_prefix("~/")
            .or_else(|| text.strip_prefix("~\\"))
            .unwrap_or("");
        return Ok(PathBuf::from(home).join(suffix));
    }
    Ok(path.to_path_buf())
}

fn absolute_existing_path(path: &Path) -> Result<PathBuf> {
    let expanded = expand_home(path)?;
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        env::current_dir()?.join(expanded)
    };
    let canonical = absolute
        .canonicalize()
        .with_context(|| format!("path '{}' does not exist", absolute.display()))?;
    Ok(strip_windows_verbatim_prefix(canonical))
}

#[cfg(windows)]
fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(stripped) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(stripped);
    }
    path
}

#[cfg(not(windows))]
fn strip_windows_verbatim_prefix(path: PathBuf) -> PathBuf {
    path
}

fn current_checkout_branch(workspace: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(workspace)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .with_context(|| {
            format!(
                "failed to read current branch for workspace '{}'",
                workspace.display()
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "failed to read current branch for workspace '{}': {stderr}",
            workspace.display()
        );
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if branch.is_empty() || branch == "HEAD" {
        bail!(
            "workspace '{}' is not on a named branch; pass --branch explicitly",
            workspace.display()
        );
    }
    Ok(branch)
}

fn summarize_events(path: &Path) -> Result<EventSummary> {
    if !path.is_file() {
        return Ok(EventSummary::default());
    }

    let mut summary = EventSummary::default();
    for line in fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?
        .lines()
    {
        let value: serde_json::Value = serde_json::from_str(line)
            .with_context(|| format!("failed to parse event in {}", path.display()))?;
        summary.count += 1;
        let timestamp = value
            .get("timestamp")
            .and_then(|value| value.as_str())
            .map(str::to_owned);
        if summary.first_timestamp.is_none() {
            summary.first_timestamp = timestamp.clone();
        }
        summary.last_timestamp = timestamp;
    }
    Ok(summary)
}

fn read_snapshot(path: &Path) -> Result<SessionSnapshot> {
    let path = if path.is_dir() {
        path.join(SNAPSHOT_FILE)
    } else {
        path.to_path_buf()
    };
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn validate_snapshot(snapshot: &SessionSnapshot) -> Result<()> {
    if snapshot.schema != 1 {
        bail!("unsupported session snapshot schema {}", snapshot.schema);
    }
    if snapshot.kind != "copilot_app_session" {
        bail!("unsupported session snapshot kind '{}'", snapshot.kind);
    }
    if snapshot.turns.is_empty() {
        bail!("session snapshot has no turns");
    }
    Ok(())
}

fn copilot_home(value: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(value) = value {
        return expand_home(&value);
    }
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .context("failed to find USERPROFILE or HOME for default Copilot home")?;
    Ok(PathBuf::from(home).join(".copilot"))
}

fn open_readonly(path: &Path) -> Result<Connection> {
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("failed to open {} read-only", path.display()))
}

fn open_write(path: &Path) -> Result<Connection> {
    if !path.is_file() {
        bail!(
            "required Copilot database '{}' does not exist",
            path.display()
        );
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open {} for writing", path.display()))?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(conn)
}

fn now_rfc3339() -> Result<String> {
    Ok(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn captures_and_rehydrates_session_fixture() {
        let home = TempDir::new().unwrap();
        seed_copilot_home(home.path(), "source-session");

        let out = TempDir::new().unwrap();
        capture_session(CaptureSessionArgs {
            copilot_home: Some(home.path().to_path_buf()),
            session_id: "source-session".to_owned(),
            out: out.path().to_path_buf(),
            include_transcripts: true,
        })
        .unwrap();
        let workspace = home.path().join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&workspace)
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args([
                "-C",
                workspace.to_str().unwrap(),
                "remote",
                "add",
                "origin",
                "https://github.com/sethjuarez/autorepo-test-fixture.git",
            ])
            .status()
            .unwrap();

        rehydrate_session(RehydrateSessionArgs {
            copilot_home: Some(home.path().to_path_buf()),
            repo: "sethjuarez/autorepo-test-fixture".to_owned(),
            snapshot: out.path().to_path_buf(),
            workspace,
            branch: Some("autorepo/session-rehydrate".to_owned()),
            dry_run: false,
            allow_live_copilot_home: false,
        })
        .unwrap();

        let conn = Connection::open(home.path().join("session-store.db")).unwrap();
        let count: i64 = conn
            .query_row(
                "select count(*) from sessions where repository = 'sethjuarez/autorepo-test-fixture' and id <> 'source-session'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let turns: i64 = conn
            .query_row(
                "select count(*) from turns where session_id in (select id from sessions where repository = 'sethjuarez/autorepo-test-fixture' and id <> 'source-session')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(turns, 2);
    }

    #[test]
    fn refuses_live_home_without_explicit_override() {
        let snapshot = TempDir::new().unwrap();
        fs::write(
            snapshot.path().join(SNAPSHOT_FILE),
            r#"
schema: 1
kind: copilot_app_session
source_session_id: source
title: Demo
summary:
mode: interactive
model:
reasoning_effort:
repository:
branch:
cwd:
captured_at: 2026-01-01T00:00:00.000Z
turns:
  - turn_index: 0
    user_message: hi
    assistant_response: hello
    timestamp: 2026-01-01T00:00:00.000Z
checkpoint:
"#
            .trim_start(),
        )
        .unwrap();
        let result = rehydrate_session(RehydrateSessionArgs {
            copilot_home: None,
            repo: "sethjuarez/autorepo-test-fixture".to_owned(),
            snapshot: snapshot.path().to_path_buf(),
            workspace: snapshot.path().join("workspace"),
            branch: Some("demo/test".to_owned()),
            dry_run: false,
            allow_live_copilot_home: false,
        });
        let error = result.unwrap_err().to_string();
        assert!(error.contains("refusing to write the default live Copilot home"));
    }

    fn seed_copilot_home(home: &Path, session_id: &str) {
        fs::create_dir_all(home.join("session-state").join(session_id)).unwrap();
        fs::write(
            home.join("session-state")
                .join(session_id)
                .join("events.jsonl"),
            r#"{"timestamp":"2026-01-01T00:00:00.000Z","type":"session.start"}"#,
        )
        .unwrap();

        let data = Connection::open(home.join("data.db")).unwrap();
        data.execute_batch(
            r#"
            create table projects (
                id text primary key not null,
                name text not null,
                main_repo_path text not null unique,
                default_branch text not null default 'main',
                github_owner text,
                github_repo text,
                last_opened_at text
            );
            create table sessions (
                id text primary key not null,
                title text,
                created_at text not null,
                updated_at text not null,
                session_type text not null default 'workspace',
                mode text,
                is_running integer not null default 0,
                was_interrupted integer not null default 0,
                model text,
                reasoning_effort text,
                execution_location text not null default 'local',
                title_source text not null default 'auto'
            );
            create table workspaces (
                id text primary key not null,
                project_id text not null,
                worktree_id text,
                workspace_type text not null,
                branch text not null,
                name text not null,
                created_at text not null,
                updated_at text not null,
                session_id text,
                host_id text not null default 'local',
                name_source text not null default 'auto',
                is_initialized integer not null default 1
            );
            create table worktrees (
                id text primary key not null,
                project_id text not null,
                path text not null,
                branch text not null,
                base_branch text,
                created_at text not null
            );
            create table workspace_checkout_bindings (
                workspace_id text not null,
                repo_path text not null,
                repo_full_name text,
                display_name text not null,
                checkout_kind text not null,
                checkout_path text not null,
                worktree_id text,
                primary key (workspace_id, repo_path)
            );
            create table workspace_session_aliases (
                session_id text primary key not null,
                workspace_id text not null,
                created_at text not null
            );
            create table activity_items (
                id text primary key not null,
                workspace_id text,
                session_id text,
                activity_type text not null,
                preview text not null default '',
                is_read integer not null default 0,
                created_at text not null,
                updated_at text not null,
                metadata_json text not null default '{}'
            );
            insert into projects (id, name, main_repo_path, github_owner, github_repo, last_opened_at)
            values ('project-1', 'autorepo test', 'C:\repo', 'sethjuarez', 'autorepo-test-fixture', '2026-01-01T00:00:00.000Z');
            insert into sessions (id, title, created_at, updated_at, session_type, mode, is_running, was_interrupted, model, reasoning_effort)
            values ('source-session', 'Captured demo', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', 'project', 'interactive', 0, 0, 'gpt-5.5', 'medium');
            insert into worktrees (id, project_id, path, branch, created_at)
            values ('worktree-1', 'project-1', 'C:\repo', 'main', '2026-01-01T00:00:00.000Z');
            insert into workspaces (id, project_id, worktree_id, workspace_type, branch, name, created_at, updated_at, session_id)
            values ('workspace-1', 'project-1', 'worktree-1', 'worktree', 'main', 'Captured demo', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', 'source-session');
            insert into workspace_checkout_bindings (workspace_id, repo_path, repo_full_name, display_name, checkout_kind, checkout_path, worktree_id)
            values ('workspace-1', 'C:\repo', 'sethjuarez/autorepo-test-fixture', 'Captured demo', 'worktree', 'C:\repo', 'worktree-1');
            insert into workspace_session_aliases (session_id, workspace_id, created_at)
            values ('source-session', 'workspace-1', '2026-01-01T00:00:00.000Z');
            "#,
        )
        .unwrap();

        let store = Connection::open(home.join("session-store.db")).unwrap();
        store
            .execute_batch(
                r#"
            create table sessions (
                id text primary key,
                cwd text,
                repository text,
                branch text,
                summary text,
                created_at text,
                updated_at text,
                host_type text
            );
            create table turns (
                id integer primary key autoincrement,
                session_id text not null,
                turn_index integer not null,
                user_message text,
                assistant_response text,
                timestamp text,
                unique(session_id, turn_index)
            );
            create table checkpoints (
                id integer primary key autoincrement,
                session_id text not null,
                checkpoint_number integer not null,
                title text,
                overview text,
                history text,
                work_done text,
                technical_details text,
                important_files text,
                next_steps text,
                created_at text,
                unique(session_id, checkpoint_number)
            );
            insert into sessions values ('source-session', 'C:\repo', 'sethjuarez/autorepo-test-fixture', 'main', 'Captured demo', '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', 'github');
            insert into turns (session_id, turn_index, user_message, assistant_response, timestamp)
            values ('source-session', 0, 'prepare the demo', 'demo is ready', '2026-01-01T00:00:00.000Z');
            insert into turns (session_id, turn_index, user_message, assistant_response, timestamp)
            values ('source-session', 1, 'summarize', 'summary', '2026-01-01T00:01:00.000Z');
            insert into checkpoints (session_id, checkpoint_number, title, overview, created_at)
            values ('source-session', 0, 'checkpoint', 'overview', '2026-01-01T00:01:00.000Z');
            "#,
            )
            .unwrap();
    }
}
