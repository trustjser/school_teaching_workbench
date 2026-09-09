-- 迁移 004：固定教室与学年班级绑定。
-- 设备是可更换的技术节点，教室是稳定的物理位置；班级绑定按学年生效。
CREATE TABLE IF NOT EXISTS classrooms (
    id            TEXT NOT NULL PRIMARY KEY,
    room_name     TEXT NOT NULL,
    device_id     TEXT,
    remark        TEXT,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    deleted_at    INTEGER,
    sync_state    TEXT NOT NULL DEFAULT 'pending',
    dirty         INTEGER NOT NULL DEFAULT 1
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_classrooms_name
    ON classrooms(room_name) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_classrooms_device
    ON classrooms(device_id) WHERE device_id IS NOT NULL AND deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS classroom_assignments (
    id             TEXT NOT NULL PRIMARY KEY,
    classroom_id   TEXT NOT NULL,
    school_year_id TEXT NOT NULL,
    class_id       TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    deleted_at     INTEGER,
    sync_state     TEXT NOT NULL DEFAULT 'pending',
    dirty          INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (classroom_id) REFERENCES classrooms(id),
    FOREIGN KEY (school_year_id) REFERENCES school_years(id),
    FOREIGN KEY (class_id) REFERENCES classes(id)
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_classroom_assignment_year
    ON classroom_assignments(classroom_id, school_year_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS ix_classroom_assignment_class
    ON classroom_assignments(class_id, school_year_id) WHERE deleted_at IS NULL;
