-- =============================================================================
--  迁移 003：学年 / 届维度 + 班级年隔离
--  目标: 本项目捆绑的 SQLite 版本较旧（< 3.35），不支持
--        `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` 语法。
--  约定: 建表/建索引语句保持 `IF NOT EXISTS`（旧版已支持）；`ADD COLUMN` 的幂等
--        由 Rust 侧 run_migrations 在执行前先查列是否存在来保证，故此处不写
--        `IF NOT EXISTS`，也能在每次启动重复执行而不报错。
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. school_years —— 学年目录（如 2027届 / 2028届）
--    物理机房 / 设备永久不变（device_id 不变），学年只是时间维度：
--    同一教室每学年的班级人员、班主任都不同，但旧数据必须保留。
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS school_years (
    id               TEXT    NOT NULL PRIMARY KEY,                  -- UUID
    school_year_no   TEXT,                                          -- 届号，如 '2027'
    school_year_name TEXT    NOT NULL,                              -- 展示名，如 '2027届'
    start_date       TEXT,                                          -- 开学日期 YYYY-MM-DD
    end_date         TEXT,                                          -- 结束日期 YYYY-MM-DD
    sort_order       INTEGER NOT NULL DEFAULT 0,                    -- 排序（数字越小越靠前）
    remark           TEXT,                                          -- 备注
    created_at       INTEGER,                                       -- 创建时间（毫秒）
    updated_at       INTEGER,                                       -- 更新时间（毫秒）
    deleted_at       INTEGER,                                       -- 软删时间（毫秒）
    sync_state       TEXT    NOT NULL DEFAULT 'pending'
                          CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty            INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1))
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_school_years_name
    ON school_years(school_year_name) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_school_years_order ON school_years(sort_order, school_year_name);

-- -----------------------------------------------------------------------------
-- 2. classes.school_year_id —— 班级年隔离
--    班级真正身份 = (school_year_id, grade_id, class_no)；class_name 退为展示名。
--    唯一索引改为 (school_year_id, grade_id, class_no) 的部分索引。
--    旧版 SQLite (< 3.35) 不支持 ADD COLUMN IF NOT EXISTS，幂等由 Rust 保证。
-- -----------------------------------------------------------------------------
ALTER TABLE classes ADD COLUMN school_year_id TEXT;

-- 新建年隔离唯一索引（部分索引，软删不计入）。
CREATE UNIQUE INDEX IF NOT EXISTS ux_classes_year
    ON classes(school_year_id, grade_id, class_no) WHERE deleted_at IS NULL;

-- 删除旧的唯一索引（按 class_name）：与新索引语义冲突，且年隔离后 class_name 不再唯一。
DROP INDEX IF EXISTS ux_classes_name;
