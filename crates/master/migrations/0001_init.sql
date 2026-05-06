CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ------------------------------------------------------------------
-- 触发器函数
-- ------------------------------------------------------------------
CREATE OR REPLACE FUNCTION set_updated_at() RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION tunnels_normalize_protocols()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.protocols IS NULL OR cardinality(NEW.protocols) = 0 THEN
        RAISE EXCEPTION 'tunnels.protocols 必须至少包含一个值';
    END IF;
    NEW.protocols := ARRAY(
        SELECT DISTINCT lower(p) FROM unnest(NEW.protocols) AS p ORDER BY 1
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forward_ports_check_port_invariant()
RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forward_ports
        WHERE forward_id = COALESCE(NEW.forward_id, OLD.forward_id)
          AND hop_index  = COALESCE(NEW.hop_index, OLD.hop_index)
        GROUP BY forward_id, hop_index
        HAVING COUNT(DISTINCT listen_port) > 1
    ) THEN
        RAISE EXCEPTION
            'forward_ports invariant 违反：同一 (forward_id=%, hop_index=%) 下 listen_port 必须一致',
            COALESCE(NEW.forward_id, OLD.forward_id),
            COALESCE(NEW.hop_index, OLD.hop_index);
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- ------------------------------------------------------------------
-- users
-- ------------------------------------------------------------------
CREATE TABLE users (
    id            BIGINT      PRIMARY KEY,
    username      TEXT        NOT NULL UNIQUE,
    password_hash TEXT        NOT NULL,
    role          TEXT        NOT NULL CHECK (role IN ('admin', 'user')),
    status        TEXT        NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'disabled', 'expired')),
    expires_at    TIMESTAMPTZ,
    remark        TEXT        NOT NULL DEFAULT '',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX users_status_idx     ON users (status);
CREATE INDEX users_expires_at_idx ON users (expires_at) WHERE expires_at IS NOT NULL;

CREATE TRIGGER users_set_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ------------------------------------------------------------------
-- nodes
-- ------------------------------------------------------------------
CREATE TABLE nodes (
    id                   TEXT        PRIMARY KEY
        CHECK (id ~ '^[a-z0-9][a-z0-9._-]{0,62}$'),
    hostname             TEXT        NOT NULL DEFAULT '',
    version              TEXT        NOT NULL DEFAULT '',
    protocol_version     INT         NOT NULL DEFAULT 0,
    tags                 TEXT[]      NOT NULL DEFAULT '{}',
    enrollment_token     TEXT        UNIQUE,
    enrolled_at          TIMESTAMPTZ,
    last_seen_at         TIMESTAMPTZ,
    last_heartbeat       JSONB,
    tunnels_version      BIGINT      NOT NULL DEFAULT 0,
    last_applied_version BIGINT      NOT NULL DEFAULT 0,
    cert_fingerprint     TEXT,
    cert_serial          TEXT,
    cert_not_after       TIMESTAMPTZ,
    server_ips           TEXT[]      NOT NULL DEFAULT '{}',
    port_range_start     INT         NOT NULL DEFAULT 30000,
    port_range_end       INT         NOT NULL DEFAULT 39999,
    traffic_ratio        DOUBLE PRECISION NOT NULL DEFAULT 1.0
        CHECK (traffic_ratio >= 0),
    tunnel_eligible      BOOLEAN     NOT NULL DEFAULT TRUE,
    expires_at           TIMESTAMPTZ,
    monthly_price        NUMERIC(10, 2),
    website              TEXT        NOT NULL DEFAULT '',
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT nodes_port_range_chk
        CHECK (port_range_start >= 1
           AND port_range_end <= 65535
           AND port_range_start <= port_range_end)
);

CREATE INDEX nodes_last_seen_idx ON nodes (last_seen_at DESC);

CREATE TRIGGER nodes_set_updated_at
    BEFORE UPDATE ON nodes
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ------------------------------------------------------------------
-- tunnels
-- ------------------------------------------------------------------
CREATE TABLE tunnels (
    id            BIGINT      PRIMARY KEY,
    name          TEXT        NOT NULL UNIQUE,
    description   TEXT        NOT NULL DEFAULT '',
    protocols     TEXT[]      NOT NULL DEFAULT ARRAY['tcp','udp']::text[],
    ip_preference TEXT        NOT NULL DEFAULT ''
        CHECK (ip_preference IN ('', 'ipv4', 'ipv6')),
    in_ip         TEXT        NOT NULL DEFAULT '',
    enabled       BOOLEAN     NOT NULL DEFAULT TRUE,
    version       BIGINT      NOT NULL DEFAULT 1,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT tunnels_protocols_check
        CHECK (
            cardinality(protocols) BETWEEN 1 AND 2
            AND protocols <@ ARRAY['tcp','udp']::text[]
        )
);

CREATE TRIGGER tunnels_set_updated_at
    BEFORE UPDATE ON tunnels
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER tunnels_normalize_protocols_trg
    BEFORE INSERT OR UPDATE ON tunnels
    FOR EACH ROW EXECUTE FUNCTION tunnels_normalize_protocols();

-- ------------------------------------------------------------------
-- tunnel_hops（支持 DAG 多节点层，同层可有多个节点）
-- ------------------------------------------------------------------
CREATE TABLE tunnel_hops (
    tunnel_id  BIGINT      NOT NULL REFERENCES tunnels(id) ON DELETE CASCADE,
    hop_index  INTEGER     NOT NULL CHECK (hop_index >= 0),
    node_id    TEXT        NOT NULL REFERENCES nodes(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tunnel_id, hop_index, node_id),
    CONSTRAINT tunnel_hops_tunnel_node_unique UNIQUE (tunnel_id, node_id)
);

CREATE INDEX tunnel_hops_node_idx ON tunnel_hops (node_id);

COMMENT ON TABLE tunnel_hops IS
    'Tunnel topology. Each (tunnel_id, hop_index) layer may contain multiple nodes (DAG fan-in/fan-out). Same node cannot appear in multiple layers within one tunnel.';

-- ------------------------------------------------------------------
-- user_groups（套餐级配额）
-- ------------------------------------------------------------------
CREATE TABLE user_groups (
    id               BIGINT      PRIMARY KEY,
    name             TEXT        NOT NULL UNIQUE,
    remark           TEXT        NOT NULL DEFAULT '',
    flow_limit_bytes BIGINT      NOT NULL DEFAULT 0 CHECK (flow_limit_bytes >= 0),
    speed_limit_kbps BIGINT      NOT NULL DEFAULT 0 CHECK (speed_limit_kbps >= 0),
    tunnel_limit     INTEGER     NOT NULL DEFAULT 0 CHECK (tunnel_limit >= 0),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER user_groups_set_updated_at
    BEFORE UPDATE ON user_groups
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ------------------------------------------------------------------
-- group_members
-- ------------------------------------------------------------------
CREATE TABLE group_members (
    group_id   BIGINT      NOT NULL REFERENCES user_groups(id) ON DELETE CASCADE,
    user_id    BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (group_id, user_id)
);

CREATE INDEX group_members_user_idx ON group_members (user_id);

-- ------------------------------------------------------------------
-- group_tunnels（组内隧道授权，配额由 user_groups 统一控制）
-- ------------------------------------------------------------------
CREATE TABLE group_tunnels (
    id         BIGINT      PRIMARY KEY,
    group_id   BIGINT      NOT NULL REFERENCES user_groups(id) ON DELETE CASCADE,
    tunnel_id  BIGINT      NOT NULL REFERENCES tunnels(id) ON DELETE RESTRICT,
    enabled    BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (group_id, tunnel_id)
);

CREATE INDEX group_tunnels_group_idx  ON group_tunnels (group_id);
CREATE INDEX group_tunnels_tunnel_idx ON group_tunnels (tunnel_id);

CREATE TRIGGER group_tunnels_set_updated_at
    BEFORE UPDATE ON group_tunnels
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ------------------------------------------------------------------
-- user_tunnels（per-user 隧道分配与配额）
-- ------------------------------------------------------------------
CREATE TABLE user_tunnels (
    id               BIGINT      PRIMARY KEY,
    user_id          BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tunnel_id        BIGINT      NOT NULL REFERENCES tunnels(id) ON DELETE RESTRICT,
    flow_limit_bytes BIGINT      NOT NULL DEFAULT 0 CHECK (flow_limit_bytes >= 0),
    speed_limit_kbps BIGINT      NOT NULL DEFAULT 0 CHECK (speed_limit_kbps >= 0),
    expires_at       TIMESTAMPTZ,
    enabled          BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, tunnel_id)
);

CREATE INDEX user_tunnels_user_idx   ON user_tunnels (user_id);
CREATE INDEX user_tunnels_tunnel_idx ON user_tunnels (tunnel_id);

CREATE TRIGGER user_tunnels_set_updated_at
    BEFORE UPDATE ON user_tunnels
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ------------------------------------------------------------------
-- forwards
-- ------------------------------------------------------------------
CREATE TABLE forwards (
    id                 BIGINT      PRIMARY KEY,
    user_tunnel_id     BIGINT      NOT NULL REFERENCES user_tunnels(id) ON DELETE RESTRICT,
    name               TEXT        NOT NULL,
    in_port            INTEGER     NOT NULL CHECK (in_port BETWEEN 1 AND 65535),
    remote_addrs       TEXT[]      NOT NULL
        CONSTRAINT forwards_remote_addrs_nonempty CHECK (cardinality(remote_addrs) >= 1),
    lb_strategy        TEXT        NOT NULL DEFAULT 'round_robin'
        CHECK (lb_strategy IN ('round_robin', 'primary_backup')),
    max_connections    INTEGER     NOT NULL DEFAULT 0 CHECK (max_connections >= 0),
    allow_cidrs        TEXT[]      NOT NULL DEFAULT '{}',
    deny_cidrs         TEXT[]      NOT NULL DEFAULT '{}',
    desired_enabled    BOOLEAN     NOT NULL DEFAULT TRUE,
    deploy_generation  BIGINT      NOT NULL DEFAULT 0,
    in_flow_bytes      BIGINT      NOT NULL DEFAULT 0 CHECK (in_flow_bytes >= 0),
    out_flow_bytes     BIGINT      NOT NULL DEFAULT 0 CHECK (out_flow_bytes >= 0),
    last_deploy_error  TEXT,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX forwards_user_tunnel_idx ON forwards (user_tunnel_id);

CREATE TRIGGER forwards_set_updated_at
    BEFORE UPDATE ON forwards
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- ------------------------------------------------------------------
-- forward_ports（支持 DAG 多节点，同层共享 listen_port）
-- ------------------------------------------------------------------
CREATE TABLE forward_ports (
    forward_id  BIGINT      NOT NULL REFERENCES forwards(id) ON DELETE CASCADE,
    hop_index   INTEGER     NOT NULL CHECK (hop_index >= 0),
    node_id     TEXT        NOT NULL REFERENCES nodes(id) ON DELETE RESTRICT,
    protocol    TEXT        NOT NULL CHECK (protocol IN ('tcp', 'udp')),
    listen_port INTEGER     NOT NULL CHECK (listen_port BETWEEN 1 AND 65535),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (forward_id, hop_index, node_id, protocol),
    UNIQUE (node_id, protocol, listen_port)
);

CREATE INDEX forward_ports_node_idx ON forward_ports (node_id);

CREATE CONSTRAINT TRIGGER forward_ports_port_invariant_trg
    AFTER INSERT OR UPDATE OR DELETE ON forward_ports
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION forward_ports_check_port_invariant();

COMMENT ON TABLE forward_ports IS
    'Per-node listener allocations. Same hop_index nodes share the same listen_port (enforced by deferrable trigger). Different hop_index layers each pick their own listen_port.';

-- ------------------------------------------------------------------
-- forward_pause_reasons（多源暂停原因）
-- ------------------------------------------------------------------
CREATE TABLE forward_pause_reasons (
    forward_id BIGINT      NOT NULL REFERENCES forwards(id) ON DELETE CASCADE,
    reason     TEXT        NOT NULL CHECK (reason IN (
        'tunnel_quota_exceeded',
        'user_tunnel_expired',
        'user_tunnel_disabled',
        'tunnel_disabled',
        'user_disabled',
        'user_expired',
        'deploy_failed'
    )),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (forward_id, reason)
);

-- ------------------------------------------------------------------
-- audit_log
-- ------------------------------------------------------------------
CREATE TABLE audit_log (
    id     BIGSERIAL   PRIMARY KEY,
    actor  TEXT,
    action TEXT        NOT NULL,
    target TEXT,
    detail JSONB,
    at     TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX audit_log_at_idx ON audit_log (at DESC);

-- ------------------------------------------------------------------
-- system_config（单行全局配置）
-- ------------------------------------------------------------------
CREATE TABLE system_config (
    id                   INT         PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    announcement_enabled BOOLEAN     NOT NULL DEFAULT FALSE,
    announcement_title   TEXT        NOT NULL DEFAULT '',
    announcement_content TEXT        NOT NULL DEFAULT '',
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO system_config DEFAULT VALUES;

-- ------------------------------------------------------------------
-- node_availability（节点在线历史，保留 90 天）
-- ------------------------------------------------------------------
CREATE TABLE node_availability (
    node_id     TEXT        NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    recorded_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (node_id, recorded_at)
);


-- ------------------------------------------------------------------
-- app_settings（系统级 KV 配置）
-- ------------------------------------------------------------------
CREATE TABLE app_settings (
    key        TEXT        PRIMARY KEY,
    value      TEXT        NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO app_settings (key, value) VALUES ('upgrade_channel', 'stable');

-- ------------------------------------------------------------------
-- upgrade_jobs（节点升级任务）
-- ------------------------------------------------------------------
CREATE TABLE upgrade_jobs (
    id           BIGINT      PRIMARY KEY,
    node_id      TEXT        NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    from_version TEXT,
    target_tag   TEXT        NOT NULL,
    state        TEXT        NOT NULL DEFAULT 'queued'
        CHECK (state IN ('queued', 'dispatched', 'accepted', 'succeeded', 'failed', 'timed_out')),
    error        TEXT,
    requested_by BIGINT      NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    accepted_at  TIMESTAMPTZ,
    completed_at TIMESTAMPTZ
);

CREATE INDEX upgrade_jobs_node_idx
    ON upgrade_jobs (node_id, requested_at DESC);

CREATE INDEX upgrade_jobs_active_idx
    ON upgrade_jobs (state)
    WHERE state IN ('queued', 'dispatched', 'accepted');

CREATE UNIQUE INDEX upgrade_jobs_one_active_per_node
    ON upgrade_jobs (node_id)
    WHERE state IN ('queued', 'dispatched', 'accepted');
