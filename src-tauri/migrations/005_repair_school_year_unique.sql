-- 修复早期版本创建的非部分唯一索引：软删除学年不应阻塞同名新学年。
DROP INDEX IF EXISTS ux_school_years_name;
CREATE UNIQUE INDEX IF NOT EXISTS ux_school_years_name
    ON school_years(school_year_name) WHERE deleted_at IS NULL;
