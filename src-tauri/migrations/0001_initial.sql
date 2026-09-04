-- Initial T-ide schema (data-model.md).
-- Every entity lives in a single SQLite database file (t-ide.db) on the
-- developer machine.

PRAGMA foreign_keys = ON;

CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL UNIQUE,
    auto_approve_read_only INTEGER NOT NULL DEFAULT 0,
    available INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_projects_path ON projects (path);

CREATE TABLE model_providers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('external', 'local')),
    api_base_url TEXT NOT NULL,
    default_model TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE paired_devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_name TEXT NOT NULL,
    auth_token TEXT NOT NULL UNIQUE,
    paired_at TEXT NOT NULL,
    last_seen_at TEXT,
    revoked INTEGER NOT NULL DEFAULT 0,
    revoked_at TEXT
);

CREATE INDEX idx_devices_auth_token ON paired_devices (auth_token);

CREATE TABLE agent_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects (id) ON DELETE RESTRICT,
    provider_id INTEGER NOT NULL REFERENCES model_providers (id) ON DELETE RESTRICT,
    status TEXT NOT NULL CHECK (
        status IN ('idle', 'running', 'waiting_approval', 'interrupted', 'complete')
    ),
    prompt_text TEXT NOT NULL,
    origin_device_id INTEGER REFERENCES paired_devices (id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE INDEX idx_sessions_status ON agent_sessions (status);
CREATE INDEX idx_sessions_project_id ON agent_sessions (project_id);

CREATE TABLE transcript_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES agent_sessions (id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL,
    entry_type TEXT NOT NULL CHECK (
        entry_type IN (
            'prompt',
            'response',
            'file_change',
            'command',
            'approval_decision',
            'error'
        )
    ),
    origin_device_id INTEGER REFERENCES paired_devices (id) ON DELETE RESTRICT,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_transcript_session_order ON transcript_entries (session_id, order_index);

CREATE TABLE approval_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES agent_sessions (id) ON DELETE CASCADE,
    request_type TEXT NOT NULL CHECK (request_type IN ('file_change', 'command')),
    details TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    resolved INTEGER NOT NULL DEFAULT 0,
    resolved_by_device_id INTEGER REFERENCES paired_devices (id) ON DELETE RESTRICT,
    resolution TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_approval_session ON approval_requests (session_id, resolved);
CREATE INDEX idx_approval_expires ON approval_requests (expires_at);

CREATE TABLE session_client_connections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id INTEGER NOT NULL REFERENCES agent_sessions (id) ON DELETE CASCADE,
    device_id INTEGER REFERENCES paired_devices (id) ON DELETE RESTRICT,
    connected_at TEXT NOT NULL,
    disconnected_at TEXT
);

CREATE INDEX idx_client_session ON session_client_connections (session_id, disconnected_at);
