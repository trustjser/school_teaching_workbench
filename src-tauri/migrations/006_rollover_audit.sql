-- 006 rollover 审计（2026-09-11 新学年换届流水线）
-- 只用 CREATE TABLE/INDEX IF NOT EXISTS（本机 SQLite < 3.35，禁 ALTER IF NOT EXISTS）。
CREATE TABLE IF NOT EXISTS rollover_executions (
    id             TEXT PRIMARY KEY,
    executed_at    INTEGER NOT NULL,
    mode           TEXT    NOT NULL,           -- 'init' | 'rollover' | 'rebind'
    source_year_id TEXT,                        -- init 模式为 NULL
    new_year_id    TEXT    NOT NULL,
    summary_json   TEXT    NOT NULL,           -- RolloverExcelReport 序列化快照
    created_at     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS ix_rollover_exec_time ON rollover_executions(executed_at DESC);
