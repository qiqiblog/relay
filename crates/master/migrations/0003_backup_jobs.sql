CREATE TABLE backup_jobs (
    id           BIGINT      PRIMARY KEY,
    state        TEXT        NOT NULL DEFAULT 'running'
                             CHECK (state IN ('running', 'succeeded', 'failed')),
    triggered_by TEXT        NOT NULL DEFAULT 'schedule'
                             CHECK (triggered_by IN ('schedule', 'manual')),
    object_key   TEXT,
    size_bytes   BIGINT,
    error        TEXT,
    started_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX backup_jobs_started_at_idx ON backup_jobs (started_at DESC);
