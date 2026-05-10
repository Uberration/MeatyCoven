use std::{borrow::Cow, path::Path};

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{control_plane, daemon::DaemonStatus, project, store};

pub const COVEN_API_VERSION: &str = "v1";
pub const SUPPORTED_API_VERSIONS: [&str; 1] = [COVEN_API_VERSION];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub api_version: String,
    pub supported_api_versions: Vec<String>,
    pub ok: bool,
    pub daemon: Option<DaemonStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiResponse {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionLaunch {
    pub id: String,
    pub project_root: String,
    pub cwd: String,
    pub harness: String,
    pub prompt: String,
    pub title: String,
}

pub trait SessionRuntime {
    fn launch_session(&self, launch: &SessionLaunch) -> Result<()>;
    fn send_input(&self, session_id: &str, payload: &Value) -> Result<()>;
    fn kill_session(&self, session_id: &str) -> Result<()>;
}

pub struct NoopSessionRuntime;

impl SessionRuntime for NoopSessionRuntime {
    fn launch_session(&self, _launch: &SessionLaunch) -> Result<()> {
        Ok(())
    }

    fn send_input(&self, _session_id: &str, _payload: &Value) -> Result<()> {
        Ok(())
    }

    fn kill_session(&self, _session_id: &str) -> Result<()> {
        Ok(())
    }
}

pub fn health_response(daemon: Option<DaemonStatus>) -> HealthResponse {
    HealthResponse {
        api_version: COVEN_API_VERSION.to_string(),
        supported_api_versions: SUPPORTED_API_VERSIONS.map(str::to_string).to_vec(),
        ok: true,
        daemon,
    }
}

#[allow(dead_code)]
pub fn handle_request(
    method: &str,
    path: &str,
    coven_home: &Path,
    daemon: Option<DaemonStatus>,
) -> Result<ApiResponse> {
    handle_request_with_body(method, path, coven_home, daemon, None)
}

pub fn handle_request_with_body(
    method: &str,
    path: &str,
    coven_home: &Path,
    daemon: Option<DaemonStatus>,
    body: Option<&str>,
) -> Result<ApiResponse> {
    handle_request_with_runtime(method, path, coven_home, daemon, body, &NoopSessionRuntime)
}

pub fn handle_request_with_runtime(
    method: &str,
    path: &str,
    coven_home: &Path,
    daemon: Option<DaemonStatus>,
    body: Option<&str>,
    runtime: &dyn SessionRuntime,
) -> Result<ApiResponse> {
    let (route, query) = split_path_query(path);
    let route = match normalize_api_route(route) {
        ApiRoute::Route(route) => route,
        ApiRoute::Unsupported(version) => {
            return json_response(
                404,
                &json!({
                    "error": "unsupported API version",
                    "apiVersion": version,
                    "supportedApiVersions": SUPPORTED_API_VERSIONS,
                }),
            );
        }
        ApiRoute::Malformed => return json_response(404, &json!({ "error": "not found" })),
    };
    match (method, route.as_ref()) {
        ("GET", "/api-version") => json_response(
            200,
            &json!({
                "apiVersion": COVEN_API_VERSION,
                "supportedApiVersions": SUPPORTED_API_VERSIONS,
            }),
        ),
        ("GET", "/health") => json_response(200, &health_response(daemon)),
        ("GET", "/capabilities") => json_response(200, &control_plane::capabilities()),
        ("POST", "/actions") => {
            let payload = match parse_body(body) {
                Ok(payload) => payload,
                Err(error) => {
                    return json_response(
                        400,
                        &control_plane::rejected_action("(unknown)", error.to_string()),
                    );
                }
            };
            let (status, response) = control_plane::route_action(payload);
            json_response(status, &response)
        }
        ("GET", "/sessions") => {
            let conn = store::open_store(&store_path(coven_home))?;
            let sessions = store::list_sessions(&conn)?;
            json_response(200, &sessions)
        }
        ("POST", "/sessions") => launch_session(coven_home, body, runtime),
        ("POST", path) if path.starts_with("/sessions/") && path.ends_with("/input") => {
            let session_id = session_action_id(path, "/input");
            record_input(coven_home, session_id, body, runtime)
        }
        ("POST", path) if path.starts_with("/sessions/") && path.ends_with("/kill") => {
            let session_id = session_action_id(path, "/kill");
            kill_session(coven_home, session_id, runtime)
        }
        ("GET", path) if path.starts_with("/sessions/") => {
            let session_id = path.trim_start_matches("/sessions/");
            let conn = store::open_store(&store_path(coven_home))?;
            match store::get_session(&conn, session_id)? {
                Some(session) => json_response(200, &session),
                None => json_response(404, &json!({ "error": "session not found" })),
            }
        }
        ("GET", "/events") => {
            let session_id = query_param(query.unwrap_or_default(), "sessionId");
            match session_id {
                Some(session_id) => list_session_events(coven_home, session_id),
                None => json_response(400, &json!({ "error": "sessionId is required" })),
            }
        }
        _ => json_response(404, &json!({ "error": "not found" })),
    }
}

enum ApiRoute<'a> {
    Route(Cow<'a, str>),
    Unsupported(String),
    Malformed,
}

fn normalize_api_route(route: &str) -> ApiRoute<'_> {
    let Some(rest) = route.strip_prefix("/api/") else {
        return ApiRoute::Route(Cow::Borrowed(route));
    };
    let Some((version, suffix)) = rest.split_once('/') else {
        return ApiRoute::Malformed;
    };
    if version != COVEN_API_VERSION {
        return ApiRoute::Unsupported(version.to_string());
    }
    if suffix.is_empty() {
        return ApiRoute::Malformed;
    }
    ApiRoute::Route(Cow::Owned(format!("/{suffix}")))
}

fn store_path(coven_home: &Path) -> std::path::PathBuf {
    coven_home.join("coven.sqlite3")
}

fn launch_session(
    coven_home: &Path,
    body: Option<&str>,
    runtime: &dyn SessionRuntime,
) -> Result<ApiResponse> {
    let payload = parse_body(body)?;
    let launch = session_launch_from_payload(payload)?;
    let conn = store::open_store(&store_path(coven_home))?;
    let now = current_timestamp();
    let record = store::SessionRecord {
        id: launch.id.clone(),
        project_root: launch.project_root.clone(),
        harness: launch.harness.clone(),
        title: launch.title.clone(),
        status: "running".to_string(),
        exit_code: None,
        archived_at: None,
        created_at: now.clone(),
        updated_at: now,
    };
    store::insert_session(&conn, &record)?;
    if let Err(error) = runtime.launch_session(&launch) {
        store::update_session_status(&conn, &record.id, "failed", None, &current_timestamp())?;
        return Err(error);
    }
    json_response(201, &record)
}

fn session_launch_from_payload(payload: Value) -> Result<SessionLaunch> {
    let project_root = required_string(&payload, "projectRoot")?;
    let cwd = payload.get("cwd").and_then(Value::as_str);
    let canonical_project_root = project::canonical_project_root(Path::new(&project_root))
        .context("failed to resolve projectRoot")?;
    let canonical_cwd = project::resolve_inside_root(&canonical_project_root, cwd.map(Path::new))?;
    let harness = required_string(&payload, "harness")?;
    let prompt = required_string(&payload, "prompt")?;
    let title = payload
        .get("title")
        .and_then(Value::as_str)
        .filter(|title| !title.trim().is_empty())
        .unwrap_or(&prompt)
        .to_string();

    Ok(SessionLaunch {
        id: Uuid::new_v4().to_string(),
        project_root: canonical_project_root.to_string_lossy().into_owned(),
        cwd: canonical_cwd.to_string_lossy().into_owned(),
        harness,
        prompt,
        title,
    })
}

fn required_string(payload: &Value, field: &str) -> Result<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .with_context(|| format!("request body requires string field `{field}`"))
}

fn record_input(
    coven_home: &Path,
    session_id: &str,
    body: Option<&str>,
    runtime: &dyn SessionRuntime,
) -> Result<ApiResponse> {
    let conn = store::open_store(&store_path(coven_home))?;
    let Some(session) = store::get_session(&conn, session_id)? else {
        return json_response(404, &json!({ "error": "session not found" }));
    };
    if session.status != "running" {
        return session_not_live_response(session_id);
    }

    let payload = parse_body(body)?;
    if let Err(error) = runtime.send_input(session_id, &payload) {
        if error.to_string().contains("not live") {
            return session_not_live_response(session_id);
        }
        return Err(error);
    }
    insert_event(&conn, session_id, "input", payload)?;
    json_response(202, &json!({ "ok": true, "accepted": true }))
}

fn kill_session(
    coven_home: &Path,
    session_id: &str,
    runtime: &dyn SessionRuntime,
) -> Result<ApiResponse> {
    let conn = store::open_store(&store_path(coven_home))?;
    let Some(session) = store::get_session(&conn, session_id)? else {
        return json_response(404, &json!({ "error": "session not found" }));
    };
    if session.status != "running" {
        return session_not_live_response(session_id);
    }

    if let Err(error) = runtime.kill_session(session_id) {
        if error.to_string().contains("not live") {
            return session_not_live_response(session_id);
        }
        return Err(error);
    }
    let now = current_timestamp();
    store::update_session_status(&conn, session_id, "killed", None, &now)?;
    insert_event(&conn, session_id, "kill", json!({ "status": "killed" }))?;
    json_response(202, &json!({ "ok": true, "accepted": true }))
}

fn session_not_live_response(session_id: &str) -> Result<ApiResponse> {
    json_response(
        409,
        &json!({
            "error": "session not live",
            "sessionId": session_id,
        }),
    )
}

fn list_session_events(coven_home: &Path, session_id: &str) -> Result<ApiResponse> {
    let conn = store::open_store(&store_path(coven_home))?;
    if store::get_session(&conn, session_id)?.is_none() {
        return json_response(404, &json!({ "error": "session not found" }));
    }
    let events = store::list_events(&conn, session_id)?;
    json_response(200, &events)
}

fn insert_event(
    conn: &rusqlite::Connection,
    session_id: &str,
    kind: &str,
    payload: Value,
) -> Result<()> {
    store::insert_event(
        conn,
        &store::EventRecord {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            kind: kind.to_string(),
            payload_json: serde_json::to_string(&payload)
                .context("failed to serialize event payload")?,
            created_at: current_timestamp(),
        },
    )
}

fn parse_body(body: Option<&str>) -> Result<Value> {
    match body.filter(|body| !body.trim().is_empty()) {
        Some(body) => serde_json::from_str(body).context("failed to parse request body"),
        None => Ok(json!({})),
    }
}

fn split_path_query(path: &str) -> (&str, Option<&str>) {
    match path.split_once('?') {
        Some((route, query)) => (route, Some(query)),
        None => (path, None),
    }
}

fn query_param<'a>(query: &'a str, key: &str) -> Option<&'a str> {
    query.split('&').find_map(|part| {
        let (candidate, value) = part.split_once('=')?;
        (candidate == key).then_some(value)
    })
}

fn session_action_id<'a>(path: &'a str, suffix: &str) -> &'a str {
    path.trim_start_matches("/sessions/")
        .strip_suffix(suffix)
        .unwrap_or_default()
}

pub(crate) fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Nanos, true)
}

fn json_response<T: Serialize>(status: u16, body: &T) -> Result<ApiResponse> {
    Ok(ApiResponse {
        status,
        content_type: "application/json",
        body: serde_json::to_string(body).context("failed to serialize API response")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_health_response() {
        let response = health_response(None);

        assert!(response.ok);
        assert_eq!(response.api_version, COVEN_API_VERSION);
        assert_eq!(response.daemon, None);
    }

    #[test]
    fn routes_health_request_to_json() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let daemon = DaemonStatus {
            pid: 12345,
            started_at: "2026-04-27T10:00:00Z".to_string(),
            socket: temp_dir
                .path()
                .join("coven.sock")
                .to_string_lossy()
                .into_owned(),
        };

        let response = handle_request("GET", "/health", temp_dir.path(), Some(daemon))?;

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "application/json");
        assert!(response.body.contains(r#""ok":true"#));
        assert!(response.body.contains(r#""apiVersion":"v1""#));
        assert!(response.body.contains(r#""pid":12345"#));
        Ok(())
    }

    #[test]
    fn routes_versioned_health_request_to_named_api_contract() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let response = handle_request("GET", "/api/v1/health", temp_dir.path(), None)?;

        assert_eq!(response.status, 200);
        assert!(response.body.contains(r#""apiVersion":"v1""#));
        assert!(response.body.contains(r#""supportedApiVersions":["v1"]"#));
        assert!(response.body.contains(r#""ok":true"#));
        Ok(())
    }

    #[test]
    fn rejects_unknown_api_version_prefixes() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let response = handle_request("GET", "/api/v2/health", temp_dir.path(), None)?;

        assert_eq!(response.status, 404);
        assert!(response.body.contains("unsupported API version"));
        Ok(())
    }

    #[test]
    fn routes_control_capabilities_discovery_to_json() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let response = handle_request("GET", "/api/v1/capabilities", temp_dir.path(), None)?;

        assert_eq!(response.status, 200);
        assert!(response.body.contains(r#""id":"coven.sessions""#));
        assert!(response.body.contains(r#""id":"coven.control.actions""#));
        assert!(response.body.contains(r#""id":"desktop.automation""#));
        assert!(response.body.contains(r#""policy":"requiresApproval""#));
        Ok(())
    }

    #[test]
    fn control_action_routes_safe_capability_refresh() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let body = json!({
            "action": "coven.capabilities.refresh",
            "origin": "open-meow",
            "intentId": "intent-1"
        })
        .to_string();

        let response = handle_request_with_body(
            "POST",
            "/api/v1/actions",
            temp_dir.path(),
            None,
            Some(&body),
        )?;

        assert_eq!(response.status, 200);
        assert!(response.body.contains(r#""accepted":true"#));
        assert!(response
            .body
            .contains(r#""action":"coven.capabilities.refresh""#));
        assert!(response.body.contains(r#""kind":"capabilities.refreshed""#));
        assert!(response.body.contains(r#""origin":"open-meow""#));
        assert!(response.body.contains(r#""intentId":"intent-1""#));
        Ok(())
    }

    #[test]
    fn capabilities_only_advertise_routable_control_actions() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let response = handle_request("GET", "/api/v1/capabilities", temp_dir.path(), None)?;

        assert_eq!(response.status, 200);
        assert!(response
            .body
            .contains(r#""actions":["coven.capabilities.refresh"]"#));
        assert!(!response.body.contains("coven.sessions.launch"));
        assert!(!response.body.contains("desktop.window.focus"));
        Ok(())
    }

    #[test]
    fn malformed_control_actions_fail_closed_with_structured_json() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let malformed = handle_request_with_body(
            "POST",
            "/api/v1/actions",
            temp_dir.path(),
            None,
            Some("{not-json"),
        )?;
        let missing = handle_request_with_body(
            "POST",
            "/api/v1/actions",
            temp_dir.path(),
            None,
            Some(r#"{"origin":"open-meow"}"#),
        )?;
        let empty = handle_request_with_body(
            "POST",
            "/api/v1/actions",
            temp_dir.path(),
            None,
            Some(r#"{"action":"   "}"#),
        )?;
        let non_object = handle_request_with_body(
            "POST",
            "/api/v1/actions",
            temp_dir.path(),
            None,
            Some(r#"["not","an","object"]"#),
        )?;

        assert_eq!(malformed.status, 400);
        assert_eq!(missing.status, 400);
        assert_eq!(empty.status, 400);
        assert_eq!(non_object.status, 400);
        assert!(malformed.body.contains(r#""accepted":false"#));
        assert!(malformed.body.contains(r#""action":"(unknown)""#));
        assert!(missing.body.contains("request body requires string field"));
        assert!(empty.body.contains("request body requires string field"));
        assert!(non_object
            .body
            .contains("request body must be a JSON object"));
        Ok(())
    }

    #[test]
    fn control_action_blocks_unknown_actions_before_adapters_run() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let body = json!({
            "action": "desktop.deleteEverything",
            "origin": "open-meow"
        })
        .to_string();

        let response = handle_request_with_body(
            "POST",
            "/api/v1/actions",
            temp_dir.path(),
            None,
            Some(&body),
        )?;

        assert_eq!(response.status, 400);
        assert!(response.body.contains(r#""accepted":false"#));
        assert!(response.body.contains("unknown action"));
        Ok(())
    }

    #[test]
    fn routes_sessions_list_and_detail_requests_to_json() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let conn = crate::store::open_store(&temp_dir.path().join("coven.sqlite3"))?;
        let session = crate::store::SessionRecord {
            id: "session-1".to_string(),
            project_root: "/repo".to_string(),
            harness: "codex".to_string(),
            title: "hello from coven".to_string(),
            status: "created".to_string(),
            exit_code: None,
            archived_at: None,
            created_at: "2026-04-27T10:00:00Z".to_string(),
            updated_at: "2026-04-27T10:00:00Z".to_string(),
        };
        crate::store::insert_session(&conn, &session)?;

        let list = handle_request("GET", "/sessions", temp_dir.path(), None)?;
        let detail = handle_request("GET", "/sessions/session-1", temp_dir.path(), None)?;

        assert_eq!(list.status, 200);
        assert!(list.body.contains(r#""id":"session-1""#));
        assert_eq!(detail.status, 200);
        assert!(detail.body.contains(r#""title":"hello from coven""#));
        Ok(())
    }

    #[test]
    fn returns_not_found_for_unknown_session() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let response = handle_request("GET", "/sessions/missing", temp_dir.path(), None)?;

        assert_eq!(response.status, 404);
        assert!(response.body.contains("session not found"));
        Ok(())
    }

    #[test]
    fn launch_request_invokes_runtime_and_persists_running_session() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let project_root = temp_dir.path().join("repo");
        let cwd = project_root.join("app");
        std::fs::create_dir_all(&cwd)?;
        let runtime = RecordingRuntime::default();
        let body = json!({
            "projectRoot": project_root,
            "cwd": cwd,
            "harness": "codex",
            "prompt": "hello coven",
            "title": "Demo"
        })
        .to_string();

        let response = handle_request_with_runtime(
            "POST",
            "/sessions",
            temp_dir.path(),
            None,
            Some(&body),
            &runtime,
        )?;
        let list = handle_request("GET", "/sessions", temp_dir.path(), None)?;

        assert_eq!(response.status, 201);
        assert!(response.body.contains(r#""status":"running""#));
        assert_eq!(runtime.launches.borrow().len(), 1);
        assert_eq!(runtime.launches.borrow()[0].harness, "codex");
        assert_eq!(runtime.launches.borrow()[0].prompt, "hello coven");
        assert_eq!(
            runtime.launches.borrow()[0].project_root,
            project_root.canonicalize()?.to_string_lossy()
        );
        assert_eq!(
            runtime.launches.borrow()[0].cwd,
            cwd.canonicalize()?.to_string_lossy()
        );
        assert!(list.body.contains(r#""title":"Demo""#));
        Ok(())
    }

    #[test]
    fn launch_request_persists_failed_status_when_runtime_launch_fails() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let project_root = temp_dir.path().join("repo");
        std::fs::create_dir_all(&project_root)?;
        let runtime = FailingLaunchRuntime;
        let body = json!({
            "projectRoot": project_root,
            "harness": "codex",
            "prompt": "hello"
        })
        .to_string();

        let error = handle_request_with_runtime(
            "POST",
            "/sessions",
            temp_dir.path(),
            None,
            Some(&body),
            &runtime,
        )
        .unwrap_err();
        let sessions = handle_request("GET", "/sessions", temp_dir.path(), None)?;

        assert!(error.to_string().contains("launch failed"));
        assert!(sessions.body.contains(r#""status":"failed""#));
        Ok(())
    }

    #[test]
    fn launch_request_rejects_cwd_outside_project_root() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let project_root = temp_dir.path().join("repo");
        let outside = temp_dir.path().join("outside");
        std::fs::create_dir_all(&project_root)?;
        std::fs::create_dir_all(&outside)?;
        let runtime = RecordingRuntime::default();
        let body = json!({
            "projectRoot": project_root,
            "cwd": outside,
            "harness": "codex",
            "prompt": "hello"
        })
        .to_string();

        let error = handle_request_with_runtime(
            "POST",
            "/sessions",
            temp_dir.path(),
            None,
            Some(&body),
            &runtime,
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("outside the Coven project root"),
            "unexpected error: {error:?}"
        );
        assert!(runtime.launches.borrow().is_empty());
        Ok(())
    }

    #[test]
    fn launch_request_rejects_missing_required_fields() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let runtime = RecordingRuntime::default();

        let error = handle_request_with_runtime(
            "POST",
            "/sessions",
            temp_dir.path(),
            None,
            Some(r#"{"harness":"codex"}"#),
            &runtime,
        )
        .unwrap_err();

        assert!(error.to_string().contains("projectRoot"));
        assert!(runtime.launches.borrow().is_empty());
        Ok(())
    }

    #[test]
    fn input_request_records_session_event() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        insert_test_session(temp_dir.path(), "session-1")?;

        let response = handle_request_with_body(
            "POST",
            "/sessions/session-1/input",
            temp_dir.path(),
            None,
            Some(r#"{"data":"hello coven"}"#),
        )?;
        let events = handle_request("GET", "/events?sessionId=session-1", temp_dir.path(), None)?;

        assert_eq!(response.status, 202);
        assert!(response.body.contains(r#""accepted":true"#));
        assert_eq!(events.status, 200);
        assert!(events.body.contains(r#""kind":"input""#));
        assert!(events.body.contains("hello coven"));
        Ok(())
    }

    #[test]
    fn input_request_invokes_live_runtime_hook() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        insert_test_session(temp_dir.path(), "session-1")?;
        let runtime = RecordingRuntime::default();

        let response = handle_request_with_runtime(
            "POST",
            "/sessions/session-1/input",
            temp_dir.path(),
            None,
            Some(r#"{"data":"hello coven"}"#),
            &runtime,
        )?;

        assert_eq!(response.status, 202);
        assert_eq!(
            runtime.inputs.borrow().as_slice(),
            &["session-1:hello coven"]
        );
        Ok(())
    }

    #[test]
    fn kill_request_marks_session_killed_and_records_event() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        insert_test_session(temp_dir.path(), "session-1")?;

        let response = handle_request("POST", "/sessions/session-1/kill", temp_dir.path(), None)?;
        let detail = handle_request("GET", "/sessions/session-1", temp_dir.path(), None)?;
        let events = handle_request("GET", "/events?sessionId=session-1", temp_dir.path(), None)?;

        assert_eq!(response.status, 202);
        assert!(detail.body.contains(r#""status":"killed""#));
        assert!(events.body.contains(r#""kind":"kill""#));
        Ok(())
    }

    #[test]
    fn kill_request_invokes_live_runtime_hook() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        insert_test_session(temp_dir.path(), "session-1")?;
        let runtime = RecordingRuntime::default();

        let response = handle_request_with_runtime(
            "POST",
            "/sessions/session-1/kill",
            temp_dir.path(),
            None,
            None,
            &runtime,
        )?;

        assert_eq!(response.status, 202);
        assert_eq!(runtime.kills.borrow().as_slice(), &["session-1"]);
        Ok(())
    }

    #[test]
    fn input_and_kill_reject_completed_sessions_as_not_live() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        insert_test_session_with_status(temp_dir.path(), "session-1", "completed")?;

        let input = handle_request_with_body(
            "POST",
            "/sessions/session-1/input",
            temp_dir.path(),
            None,
            Some(r#"{"data":"hello"}"#),
        )?;
        let kill = handle_request("POST", "/sessions/session-1/kill", temp_dir.path(), None)?;

        assert_eq!(input.status, 409);
        assert_eq!(kill.status, 409);
        assert!(input.body.contains("session not live"));
        assert!(kill.body.contains("session not live"));
        Ok(())
    }

    #[test]
    fn input_and_kill_reject_orphaned_sessions_as_not_live() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        insert_test_session_with_status(temp_dir.path(), "session-1", "orphaned")?;

        let input = handle_request_with_body(
            "POST",
            "/sessions/session-1/input",
            temp_dir.path(),
            None,
            Some(r#"{"data":"hello"}"#),
        )?;
        let kill = handle_request("POST", "/sessions/session-1/kill", temp_dir.path(), None)?;

        assert_eq!(input.status, 409);
        assert_eq!(kill.status, 409);
        assert!(input.body.contains("session not live"));
        assert!(kill.body.contains("session not live"));
        Ok(())
    }

    #[test]
    fn runtime_not_live_errors_become_conflict_responses() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        insert_test_session(temp_dir.path(), "session-1")?;
        let runtime = NotLiveRuntime;

        let input = handle_request_with_runtime(
            "POST",
            "/sessions/session-1/input",
            temp_dir.path(),
            None,
            Some(r#"{"data":"hello"}"#),
            &runtime,
        )?;
        let kill = handle_request_with_runtime(
            "POST",
            "/sessions/session-1/kill",
            temp_dir.path(),
            None,
            None,
            &runtime,
        )?;

        assert_eq!(input.status, 409);
        assert_eq!(kill.status, 409);
        assert!(input.body.contains("session not live"));
        assert!(kill.body.contains("session not live"));
        Ok(())
    }

    #[test]
    fn input_and_kill_reject_unknown_sessions() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let input = handle_request_with_body(
            "POST",
            "/sessions/missing/input",
            temp_dir.path(),
            None,
            Some(r#"{"data":"hello"}"#),
        )?;
        let kill = handle_request("POST", "/sessions/missing/kill", temp_dir.path(), None)?;

        assert_eq!(input.status, 404);
        assert_eq!(kill.status, 404);
        assert!(input.body.contains("session not found"));
        assert!(kill.body.contains("session not found"));
        Ok(())
    }

    #[derive(Default)]
    struct RecordingRuntime {
        launches: std::cell::RefCell<Vec<SessionLaunch>>,
        inputs: std::cell::RefCell<Vec<String>>,
        kills: std::cell::RefCell<Vec<String>>,
    }

    impl SessionRuntime for RecordingRuntime {
        fn launch_session(&self, launch: &SessionLaunch) -> Result<()> {
            self.launches.borrow_mut().push(launch.clone());
            Ok(())
        }

        fn send_input(&self, session_id: &str, payload: &Value) -> Result<()> {
            let data = payload
                .get("data")
                .and_then(Value::as_str)
                .unwrap_or_default();
            self.inputs
                .borrow_mut()
                .push(format!("{session_id}:{data}"));
            Ok(())
        }

        fn kill_session(&self, session_id: &str) -> Result<()> {
            self.kills.borrow_mut().push(session_id.to_string());
            Ok(())
        }
    }

    struct FailingLaunchRuntime;

    impl SessionRuntime for FailingLaunchRuntime {
        fn launch_session(&self, _launch: &SessionLaunch) -> Result<()> {
            anyhow::bail!("launch failed")
        }

        fn send_input(&self, _session_id: &str, _payload: &Value) -> Result<()> {
            Ok(())
        }

        fn kill_session(&self, _session_id: &str) -> Result<()> {
            Ok(())
        }
    }

    struct NotLiveRuntime;

    impl SessionRuntime for NotLiveRuntime {
        fn launch_session(&self, _launch: &SessionLaunch) -> Result<()> {
            Ok(())
        }

        fn send_input(&self, session_id: &str, _payload: &Value) -> Result<()> {
            anyhow::bail!("session `{session_id}` is not live in this daemon")
        }

        fn kill_session(&self, session_id: &str) -> Result<()> {
            anyhow::bail!("session `{session_id}` is not live in this daemon")
        }
    }

    fn insert_test_session(coven_home: &std::path::Path, id: &str) -> anyhow::Result<()> {
        insert_test_session_with_status(coven_home, id, "running")
    }

    fn insert_test_session_with_status(
        coven_home: &std::path::Path,
        id: &str,
        status: &str,
    ) -> anyhow::Result<()> {
        let conn = crate::store::open_store(&coven_home.join("coven.sqlite3"))?;
        let session = crate::store::SessionRecord {
            id: id.to_string(),
            project_root: "/repo".to_string(),
            harness: "codex".to_string(),
            title: "hello from coven".to_string(),
            status: status.to_string(),
            exit_code: None,
            archived_at: None,
            created_at: "2026-04-27T10:00:00Z".to_string(),
            updated_at: "2026-04-27T10:00:00Z".to_string(),
        };
        crate::store::insert_session(&conn, &session)?;
        Ok(())
    }
}
