# Coven Capture Design

Date: 2026-05-19
Status: Draft approved for implementation planning
Owner: OpenCoven / Coven daemon

## Executive Summary

Build native Coven Capture as the first mobile-to-Coven input path. The iOS Shortcut pattern from Shiori is the reference UX: install a Shortcut, paste a short auth code into a header, then use the iOS Share Sheet to send links into the system. Coven should not depend on Shiori for this path.

The first implementation should post directly to a Coven-owned capture endpoint over same-LAN or Tailscale. Captures become typed workflow triggers, so mobile share-sheet input can start the same graph/event machinery as manual workflow runs, cron triggers, file watchers, or future Cast Codes.

Assumption locked for v0: URL is required. Title, selected text, notes, source app, and raw shortcut metadata are optional.

## Goals

1. Add a native `/api/v1/captures` ingestion surface to Coven.
2. Support iOS Share Sheet capture over same-LAN or Tailscale without a hosted dependency.
3. Store capture records durably in the Coven SQLite store.
4. Represent captured input as a workflow graph trigger, starting with `trigger.capture.ios`.
5. Emit normal Coven events for capture receipt, graph run lifecycle, node lifecycle, and agent session launches.
6. Keep Shiori as a UX/API reference only; do not depend on Shiori or copy its Shortcut implementation.

## Non-Goals

- Do not build a hosted relay in v0.
- Do not expose the raw Coven Unix socket over TCP.
- Do not build a browser dashboard or public webhook service.
- Do not implement Shiori import in the first pass.
- Do not implement arbitrary Shortcut actions or OS automation.
- Do not require Cast Codes from the phone in v0.
- Do not require the graph canvas UI before capture works.

## Reference Pattern

Shiori's iOS Shortcut flow is useful because it optimizes setup for non-developer mobile use:

- user opens a setup dialog;
- user downloads an iOS Shortcut;
- user copies a short authentication code;
- shortcut sends share-sheet data with a custom auth header;
- app processes links asynchronously.

Coven should adapt the pattern, not the dependency. In Coven terms:

- "shortcode" becomes a Coven capture token;
- "save link" becomes "create capture record and trigger workflow graph";
- "background processing" becomes evented graph/session execution.

Reference docs:

- https://www.shiori.sh/docs/ios-shortcut
- https://www.shiori.sh/docs/api

## Architecture

The trusted daemon remains the authority boundary. The mobile Shortcut never talks to the Unix socket directly. Instead, Coven exposes an explicit capture listener that validates a capture token and forwards accepted records into the same store/event model as the daemon.

Initial flow:

```text
iOS Share Sheet
  -> Coven Capture Shortcut
  -> same-LAN/Tailscale HTTPS or HTTP endpoint
  -> capture listener validates x-coven-capture
  -> Rust authority layer stores capture
  -> workflow graph run starts with trigger.capture.ios
  -> graph nodes emit events
  -> optional agent.session node launches Sage/Cody/etc.
```

Two deployment modes are acceptable for v0:

1. Same-LAN direct: `http://<mac-hostname>.local:<capture-port>/api/v1/captures`
2. Tailscale direct: `http://<tailnet-hostname-or-ip>:<capture-port>/api/v1/captures`

Tailscale direct is the recommended practical MVP. It avoids a hosted relay while working away from home.

## Trust And Auth Model

Coven's existing daemon API is HTTP over Unix socket and assumes same-user local access. Capture is different: it is a network-facing mobile ingress. It therefore needs a separate, narrow auth design.

V0 auth:

- Generate one random capture token with at least 128 bits of entropy.
- Display it once during `coven capture setup` or `coven capture shortcut`.
- Store only a hash in Coven home, never the raw token.
- Require `x-coven-capture: <token>` on every capture request.
- Reject missing, malformed, or invalid tokens with `401`.
- Never log the raw token.
- Allow token rotation with `coven capture token rotate`.

V0 network posture:

- Capture listener is opt-in and disabled by default.
- Bind address defaults to loopback unless the user explicitly enables LAN/Tailscale binding.
- The listener must not proxy arbitrary daemon routes.
- The listener only accepts the capture endpoints documented here.
- Request body size must be capped. Recommended v0 cap: 256 KiB.
- Accepted payload fields are validated and normalized before storage.

This is intentionally not a general remote Coven API.

## Data Model

Add a `captures` table to the existing Coven store:

```sql
CREATE TABLE IF NOT EXISTS captures (
    id TEXT PRIMARY KEY NOT NULL,
    source TEXT NOT NULL,
    url TEXT NOT NULL,
    title TEXT,
    selected_text TEXT,
    note TEXT,
    source_app TEXT,
    status TEXT NOT NULL,
    workflow_run_id TEXT,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_captures_created_at
    ON captures(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_captures_status_created_at
    ON captures(status, created_at DESC);
```

Capture status values:

- `pending`: stored but no workflow run has started yet.
- `running`: associated workflow run is active.
- `completed`: workflow run finished successfully.
- `failed`: workflow run failed.
- `archived`: user hid the capture from active queues.

Rust record shape:

```rust
pub struct CaptureRecord {
    pub id: String,
    pub source: String,
    pub url: String,
    pub title: Option<String>,
    pub selected_text: Option<String>,
    pub note: Option<String>,
    pub source_app: Option<String>,
    pub status: String,
    pub workflow_run_id: Option<String>,
    pub payload_json: String,
    pub created_at: String,
    pub updated_at: String,
}
```

## API Contract

Additive endpoints under `/api/v1`:

### POST `/api/v1/captures`

Headers:

```http
Content-Type: application/json
x-coven-capture: <capture-token>
```

Request body:

```json
{
  "source": "ios-shortcut",
  "url": "https://example.com/article",
  "title": "Example Article",
  "selectedText": "Optional highlighted text.",
  "note": "Optional note from the share sheet.",
  "sourceApp": "Safari",
  "receivedAt": "2026-05-19T14:00:00Z",
  "shortcutVersion": 1
}
```

Validation:

- `url` is required and must parse as `http` or `https`.
- `source` defaults to `ios-shortcut`.
- `title` is optional and capped at 500 characters.
- `selectedText` is optional and capped at 20,000 characters.
- `note` is optional and capped at 4,000 characters.
- `sourceApp` is optional and capped at 100 characters.
- Unknown fields are preserved inside `payload_json` but ignored for routing.

Success response:

```json
{
  "success": true,
  "capture": {
    "id": "capture_...",
    "status": "pending",
    "url": "https://example.com/article",
    "createdAt": "2026-05-19T14:00:00Z"
  },
  "workflowRun": {
    "id": "run_...",
    "status": "running"
  }
}
```

If workflow auto-start is disabled, `workflowRun` is `null` and the capture remains `pending`.

### GET `/api/v1/captures`

Purpose: list captures for CLI/TUI debugging and future UI inboxes.

Query parameters:

- `status`: optional filter.
- `limit`: default 50, max 500.
- `offset`: default 0.

### GET `/api/v1/captures/:id`

Purpose: fetch one capture record.

### POST `/api/v1/captures/:id/archive`

Purpose: hide a capture from active queues. This is non-destructive.

## Workflow Graph Integration

Capture should be implemented as a graph trigger, not as a one-off agent launch path.

Initial graph node:

```json
{
  "id": "capture",
  "type": "trigger.capture.ios",
  "outputs": {
    "url": "state",
    "title": "state",
    "selectedText": "state",
    "note": "state",
    "received": "signal"
  }
}
```

First built-in workflow:

```json
{
  "id": "workflow.capture.sage-review",
  "name": "Save link for Sage review",
  "nodes": [
    { "id": "capture", "type": "trigger.capture.ios" },
    { "id": "template", "type": "prompt.template" },
    { "id": "agent", "type": "agent.session" }
  ],
  "edges": [
    { "from": "capture.url", "to": "template.url" },
    { "from": "capture.title", "to": "template.title" },
    { "from": "capture.selectedText", "to": "template.selectedText" },
    { "from": "capture.note", "to": "template.note" },
    { "from": "template.prompt", "to": "agent.prompt" }
  ]
}
```

Template prompt:

```text
Review this captured link for Val.

URL: {{url}}
Title: {{title}}
Selected text: {{selectedText}}
Note: {{note}}

Return a concise triage:
- what this is
- why it may matter
- whether it belongs in research memory, project notes, or backlog
- one suggested next action
```

`agent.session` should target Sage by default for research captures. The target should be configurable later.

## Event Model

Add capture and graph context to Coven events without breaking existing session/event consumers.

Recommended new event kinds:

- `capture.received`
- `capture.workflow_started`
- `graph.run.started`
- `node.started`
- `node.output`
- `node.completed`
- `node.failed`
- `graph.run.completed`
- `graph.run.failed`

Event payloads should include optional graph metadata:

```json
{
  "capture": {
    "id": "capture_...",
    "source": "ios-shortcut",
    "url": "https://example.com/article"
  },
  "graph": {
    "id": "workflow.capture.sage-review",
    "runId": "run_..."
  },
  "node": {
    "id": "capture",
    "type": "trigger.capture.ios"
  }
}
```

Existing `/api/v1/events?sessionId=...&afterSeq=...` remains the live stream for agent sessions. Capture/workflow events may either attach to the launched session id or use a workflow run event stream once workflow APIs exist. The v0 implementation should choose the smaller path that fits existing store constraints, but must preserve `capture.id`, `graph.id`, and `graph.runId` in payloads.

## CLI Commands

Add a `coven capture` command family:

```text
coven capture setup
coven capture token rotate
coven capture shortcut
coven capture list
coven capture inspect <id>
```

Command behavior:

- `setup`: creates capture token if missing, prints listener mode options, and explains same-LAN/Tailscale URL requirements.
- `token rotate`: rotates token and invalidates the previous Shortcut header.
- `shortcut`: prints exact iOS Shortcut setup instructions and the endpoint/header values. Later this can generate a `.shortcut` file.
- `list`: shows recent captures and status.
- `inspect`: prints one capture with workflow/session references.

The Shortcut setup text must warn that iOS cannot call `127.0.0.1` on the Mac. Users need a reachable Mac hostname, LAN IP, or Tailscale address.

## iOS Shortcut Shape

The generated/manual Shortcut should:

1. Accept URLs and text from the Share Sheet.
2. Extract:
   - URL
   - page title when available
   - selected text when available
   - optional typed note
   - source app when available
3. Send JSON to the configured Coven capture endpoint.
4. Include `x-coven-capture` with the capture token.
5. Show a simple success/failure notification.

V0 can document manual Shortcut construction. A generated `.shortcut` export is useful but not required for the first implementation.

## Error Handling

Use the existing structured error envelope where this runs through the daemon contract:

```json
{
  "error": {
    "code": "capture.invalid_url",
    "message": "Capture URL must be http or https.",
    "details": {
      "url": "shortcuts://..."
    }
  }
}
```

Recommended error codes:

- `capture.unauthorized`
- `capture.invalid_url`
- `capture.payload_too_large`
- `capture.invalid_payload`
- `capture.listener_disabled`
- `capture.workflow_unavailable`

Token failures should not disclose whether a capture token exists.

## Privacy And Storage

Captures can contain private URLs, selected text, and notes. Treat them like prompt logs.

Rules:

- Do not log full request bodies by default.
- Do not print raw capture tokens.
- Do not include raw selected text in terminal success summaries unless explicitly inspecting a capture.
- Preserve raw payload only in `payload_json` inside Coven home.
- Keep request size limits small enough to avoid accidental document dumps.
- Do not sync captures to a hosted service in v0.

## Phased Implementation

### Phase 1: Capture storage and local API

- Add capture token config/storage.
- Add `captures` table migration.
- Add `CaptureRecord` helpers in `store.rs`.
- Add `POST /api/v1/captures`.
- Add focused API/store tests.

### Phase 2: Capture CLI and listener posture

- Add `coven capture setup`.
- Add `coven capture token rotate`.
- Add `coven capture shortcut` setup output.
- Document Tailscale/same-LAN endpoint setup.
- Add body size and bind-address safeguards.

### Phase 3: Workflow trigger integration

- Add `trigger.capture.ios` graph trigger.
- Add built-in `workflow.capture.sage-review`.
- Start a workflow run on capture receipt when enabled.
- Emit capture and graph lifecycle events with capture/graph IDs.

### Phase 4: iOS Shortcut polish

- Provide a downloadable/generated Shortcut file if practical.
- Add share-sheet instructions to docs.
- Add optional note prompt in the Shortcut.
- Add troubleshooting for auth failures, unreachable Mac, and Tailscale DNS.

### Phase 5: Later integrations

- Shiori importer: poll Shiori unread links through its API or MCP and convert new links into captures.
- Cast Code mobile mode: optional Shortcut field for `castCode`.
- Hosted relay: explicit separate auth/pairing design.

## Acceptance Criteria

1. `coven capture setup` creates or reports a capture token without printing stored hashes.
2. `POST /api/v1/captures` rejects missing/invalid tokens.
3. `POST /api/v1/captures` accepts a valid URL payload and stores a capture row.
4. Captures preserve optional title, selected text, note, source app, and raw payload JSON.
5. Invalid URLs, oversized bodies, and malformed JSON return structured errors.
6. A valid capture can trigger the built-in Sage review workflow when workflow auto-start is enabled.
7. Events include capture id plus graph/run/node metadata where applicable.
8. Existing session APIs and event pagination continue to pass tests.
9. Documentation explains same-LAN/Tailscale reachability and explicitly warns that `127.0.0.1` on iOS is the phone, not the Mac.
10. Shiori is credited as reference inspiration only; no dependency or copied Shortcut code is introduced.

## Cody Handoff Prompt

```text
You are Cody working in /Users/buns/Documents/GitHub/OpenCoven/coven.

Goal:
Build native Coven Capture: an opt-in iOS Share Sheet capture path that posts links plus optional selected text/notes to Coven over same-LAN or Tailscale, stores them in Coven SQLite, and prepares them to trigger graph-backed workflows.

Controlling spec:
docs/superpowers/specs/2026-05-19-coven-capture-design.md

Critical constraints:
- Treat Shiori's iOS Shortcut docs as UX reference only. Do not depend on Shiori and do not copy its Shortcut implementation.
- Do not expose the existing Coven Unix socket API over TCP.
- Capture listener must be narrow: capture endpoints only, token-authenticated, opt-in, and size-limited.
- Keep the Rust daemon/store as the authority boundary.
- Do not log raw capture tokens or full request bodies.
- The repo has existing dirty files. Inspect `git status --short` before editing and preserve unrelated changes.

Implement in phases:

Phase 1:
- Add capture token storage/config helpers.
- Add `captures` table migration and store helpers.
- Add `POST /api/v1/captures` with `x-coven-capture` auth.
- Validate URL, title, selectedText, note, sourceApp, and request size.
- Add tests for store helpers and API behavior.

Phase 2:
- Add `coven capture setup`, `coven capture token rotate`, `coven capture shortcut`, `coven capture list`, and `coven capture inspect`.
- `shortcut` should print exact manual iOS Shortcut setup instructions, including endpoint, header name, and the 127.0.0.1 warning.

Phase 3:
- Add or prepare `trigger.capture.ios` as the workflow graph trigger.
- Add the built-in "Save link for Sage review" workflow if the graph runner exists; otherwise store the capture and emit capture events with a clean seam for graph execution.

Verification:
- Run `cargo fmt`.
- Run `cargo test -p coven-cli`.
- Run targeted tests for capture store/API/CLI behavior.

Final report:
- List changed files.
- List tests run and results.
- State whether workflow auto-start is implemented or left as a seam.
- Confirm that Shiori is reference-only and no Shiori code/dependency was introduced.
```

## Self-Review

- Placeholder scan: no placeholder requirements remain.
- Scope check: this is focused on native Coven Capture plus the graph trigger seam. Hosted relay, Shiori importer, and generated Shortcut export are deferred.
- Ambiguity check: URL is required; selected text and notes are optional in v0.
- Safety check: capture is treated as a separate network-facing ingress, not a proxy for the local Unix socket API.
