-- =============================================================================
--  迁移 002：年级 / 班级目录 + 学生 class_id 关联
--  目标: 本项目捆绑的 SQLite 版本较旧（< 3.35），不支持
--        `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` 语法。
--  约定: 建表语句保持 `IF NOT EXISTS`（旧版已支持）；`ADD COLUMN` 的幂等由
--        Rust 侧 run_migrations 在执行前先查列是否存在来保证，故此处不写
--        `IF NOT EXISTS`，也能在每次启动重复执行而不报错。
-- =============================================================================

-- -----------------------------------------------------------------------------
-- 1. grades —— 年级（教务端统一维护，全校唯一）
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS grades (
    id          TEXT    NOT NULL PRIMARY KEY,                  -- UUID
    grade_no    TEXT,                                          -- 年级编号，如 '3' / '2023'
    grade_name  TEXT    NOT NULL,                              -- 展示名，如 '三年级'
    sort_order  INTEGER NOT NULL DEFAULT 0,                    -- 排序（数字越小越靠前）
    remark      TEXT,                                          -- 备注
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER,
    sync_state  TEXT    NOT NULL DEFAULT 'pending'
                          CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty       INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1))
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_grades_name
    ON grades(grade_name) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_grades_order ON grades(sort_order, grade_name);

-- -----------------------------------------------------------------------------
-- 2. classes —— 班级（归属某个年级，教务端统一维护）
--    grade_no / grade_name 冗余存储，便于考勤/统计聚合时免 join。
-- -----------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS classes (
    id          TEXT    NOT NULL PRIMARY KEY,                  -- UUID
    grade_id    TEXT,                                          -- 关联 grades.id（软删时置空）
    grade_no    TEXT,                                          -- 冗余：年级编号
    grade_name  TEXT,                                          -- 冗余：年级展示名
    class_no    TEXT,                                          -- 班号，如 '2'
    class_name  TEXT    NOT NULL,                              -- 展示名，如 '三年级二班'（唯一）
    head_teacher TEXT,                                         -- 班主任
    sort_order  INTEGER NOT NULL DEFAULT 0,                    -- 班级排序
    remark      TEXT,                                          -- 备注
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER,
    sync_state  TEXT    NOT NULL DEFAULT 'pending'
                          CHECK (sync_state IN ('local','pending','synced','conflict')),
    dirty       INTEGER NOT NULL DEFAULT 1 CHECK (dirty IN (0, 1)),
    FOREIGN KEY (grade_id) REFERENCES grades(id) ON DELETE SET NULL
);
-- class_name 仅用于展示，跨学年可能重复；这里使用普通索引。
-- 旧版唯一索引由 003_school_year 清理，不能在重复历史数据上重新创建唯一索引。
CREATE INDEX IF NOT EXISTS ix_classes_name
    ON classes(class_name) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_classes_grade ON classes(grade_id, sort_order, class_name);
CREATE INDEX IF NOT EXISTS ix_classes_grade_name ON classes(grade_name, class_name);

-- -----------------------------------------------------------------------------
-- 3. students.class_id —— 关联 classes.id（可选；为空时回退用 grade/class_name 匹配）
-- -----------------------------------------------------------------------------
ALTER TABLE students ADD COLUMN IF NOT EXISTS class_id TEXT;
CREATE INDEX IF NOT EXISTS ix_students_class_id
    ON students(class_id) WHERE deleted_at IS NULL;
