use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::models::{Class, Classroom, ClassroomAssignment, Grade, SchoolYear};
use crate::db::repo::{class_repo, classroom_repo, grade_repo, school_year_repo, settings_repo};
use crate::error::AppResult;

/// 教务端可供班级端重复拉取的完整工作目录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySnapshot {
    pub school_years: Vec<SchoolYear>,
    pub grades: Vec<Grade>,
    pub classes: Vec<Class>,
    pub classrooms: Vec<Classroom>,
    pub assignments: Vec<ClassroomAssignment>,
    /// 权威当前学年（教务端换届执行后写入）。None = 尚未换届确认。
    #[serde(default)]
    pub current_school_year_id: Option<String>,
}

/// 班级端自动切换守护（设计 §5.1）的切换结果，供前端提示「已切到新学年班级」。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoSwitchInfo {
    pub school_year_id: String,
    pub school_year_name: String,
    pub class_id: String,
    pub class_name: String,
    pub grade_name: Option<String>,
}

/// 目录同步结果，用于前端反馈本次实际取得的数据量。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySyncReport {
    pub school_years: usize,
    pub grades: usize,
    pub classes: usize,
    pub classrooms: usize,
    pub assignments: usize,
    /// 目录应用后自动切绑的结果（None = 未触发切换）。
    #[serde(default)]
    pub auto_switched: Option<AutoSwitchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassroomClaimResponse {
    pub classroom: Classroom,
    pub assignment: ClassroomAssignment,
}

pub async fn build_snapshot(pool: &SqlitePool) -> AppResult<DirectorySnapshot> {
    let school_years = school_year_repo::list(pool).await?;
    let grades = grade_repo::list(pool).await?;
    let classes = class_repo::list(pool, None, None).await?;
    let classrooms = classroom_repo::list(pool).await?;
    let raw_assignments = classroom_repo::list_assignments(pool, None).await?;
    // 目录快照必须是自洽的活跃集合：学年/班级/教室「软删后重建」时，
    // `class_repo::soft_delete` 与 `school_year_repo::soft_delete` 不会级联到
    // `classroom_assignments`（仅 `classroom_repo::soft_delete` 会），因此母实体
    // 已软删、绑定仍活跃的行会出现在 list_assignments 里，但母实体并不在
    // school_years / classes 列表中。若不剔除，班级端 apply_snapshot 插入该绑定时
    // 会撞外键（code 787, FOREIGN KEY constraint failed）。这里按三张母表的活动
    // 集合过滤，保证快照内部引用一致。
    let year_ids: std::collections::HashSet<&str> =
        school_years.iter().map(|y| y.id.as_str()).collect();
    let class_ids: std::collections::HashSet<&str> =
        classes.iter().map(|c| c.id.as_str()).collect();
    let room_ids: std::collections::HashSet<&str> =
        classrooms.iter().map(|r| r.id.as_str()).collect();
    let assignments: Vec<ClassroomAssignment> = raw_assignments
        .into_iter()
        .filter(|a| {
            year_ids.contains(a.school_year_id.as_str())
                && class_ids.contains(a.class_id.as_str())
                && room_ids.contains(a.classroom_id.as_str())
        })
        .collect();
    let current_school_year_id = settings_repo::get_string(pool, "current_school_year_id", "")
        .await
        .ok()
        .filter(|s| !s.is_empty());
    Ok(DirectorySnapshot {
        school_years,
        grades,
        classes,
        classrooms,
        assignments,
        current_school_year_id,
    })
}

pub async fn apply_snapshot(
    pool: &SqlitePool,
    snapshot: &DirectorySnapshot,
) -> AppResult<DirectorySyncReport> {
    // 外键依赖顺序：学年/年级 -> 班级/教室 -> 教室绑定。
    for item in &snapshot.school_years {
        school_year_repo::merge_remote(pool, item).await?;
    }
    for item in &snapshot.grades {
        grade_repo::merge_remote(pool, item).await?;
    }
    for item in &snapshot.classes {
        class_repo::merge_remote(pool, item).await?;
    }
    for item in &snapshot.classrooms {
        classroom_repo::merge_remote(pool, item).await?;
    }
    for item in &snapshot.assignments {
        classroom_repo::merge_remote_assignment(pool, item).await?;
    }
    Ok(DirectorySyncReport {
        school_years: snapshot.school_years.len(),
        grades: snapshot.grades.len(),
        classes: snapshot.classes.len(),
        classrooms: snapshot.classrooms.len(),
        assignments: snapshot.assignments.len(),
        auto_switched: None,
    })
}

/// 班级端自动切换守护（设计 §5.1）：比较「权威年 + 认领教室在权威年的绑定」与
/// 本机绑定，目标绑定对 (schoolYearId, classId) 一致即切绑——同年重绑也会对齐。
/// 各前置条件不满足时返回 `Ok(None)` 且不落任何写入；只有切绑成功才写三个
/// settings key（school_year_id / class_id / bound_class_id）。
pub async fn auto_switch_if_ready(
    pool: &SqlitePool,
    current_school_year_id: Option<&str>,
) -> AppResult<Option<AutoSwitchInfo>> {
    let Some(year_id) = current_school_year_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(None); // 权威年缺失：尚未换届确认，永不触发。
    };
    let device_id = settings_repo::get_string(pool, "device_id", "")
        .await
        .unwrap_or_default();
    if device_id.is_empty() {
        return Ok(None); // 本机未认领设备。
    }
    let bound_year = settings_repo::get_string(pool, "school_year_id", "")
        .await
        .unwrap_or_default();
    let bound_class = settings_repo::get_string(pool, "bound_class_id", "")
        .await
        .unwrap_or_default();
    if bound_year == year_id && bound_class.is_empty() {
        return Ok(None); // 已在权威年但尚未绑定班级，留给手动绑定流程。
    }
    let Some(room) = classroom_repo::list(pool)
        .await?
        .into_iter()
        .find(|r| r.device_id.as_deref() == Some(device_id.as_str()))
    else {
        return Ok(None); // 设备未认领教室。
    };
    let Some(a) = classroom_repo::list_assignments(pool, None)
        .await?
        .into_iter()
        .find(|a| a.classroom_id == room.id && a.school_year_id == year_id)
    else {
        return Ok(None); // 权威年下该教室无绑定。
    };
    if bound_year == year_id && bound_class == a.class_id {
        return Ok(None); // 已对齐。
    }
    let Some(klass) = class_repo::get(pool, &a.class_id)
        .await?
        .filter(|c| c.deleted_at.is_none() && c.school_year_id.as_deref() == Some(year_id))
    else {
        return Ok(None); // 目标班级不存在或已软删或跨年不符。
    };
    let roster: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM students WHERE class_id = ? AND deleted_at IS NULL")
            .bind(&a.class_id)
            .fetch_one(pool)
            .await?;
    if roster.0 == 0 {
        return Ok(None); // 名册未就绪。
    }
    settings_repo::set_raw(pool, "school_year_id", Some(year_id), "string").await?;
    settings_repo::set_raw(pool, "class_id", Some(a.class_id.as_str()), "string").await?;
    settings_repo::set_raw(pool, "bound_class_id", Some(a.class_id.as_str()), "string").await?;
    let year_name = school_year_repo::get(pool, year_id)
        .await?
        .map(|y| y.school_year_name)
        .unwrap_or_else(|| year_id.to_string());
    Ok(Some(AutoSwitchInfo {
        school_year_id: year_id.to_string(),
        school_year_name: year_name,
        class_id: a.class_id.clone(),
        class_name: klass.class_name,
        grade_name: klass.grade_name,
    }))
}

#[cfg(test)]
mod tests {
    use crate::db::models::{Class, Classroom, ClassroomAssignment, Grade, SchoolYear, Student};
    use crate::db::repo::{
        class_repo, classroom_repo, grade_repo, school_year_repo, settings_repo, student_repo,
    };
    use crate::db::{create_pool, run_migrations};

    #[tokio::test]
    async fn snapshot_repairs_directory_created_before_client_connects() {
        let master_dir =
            std::env::temp_dir().join(format!("lanwb_directory_master_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&master_dir).expect("master temp dir");
        let master = create_pool(&master_dir.join("master.db"))
            .await
            .expect("master pool");
        run_migrations(&master).await.expect("master migrations");

        let year = school_year_repo::upsert(
            &master,
            SchoolYear {
                school_year_no: "2026".into(),
                school_year_name: "2026学年".into(),
                ..Default::default()
            },
        )
        .await
        .expect("create year");
        let grade = grade_repo::upsert(
            &master,
            Grade {
                grade_no: "1".into(),
                grade_name: "一年级".into(),
                ..Default::default()
            },
        )
        .await
        .expect("create grade");
        let class = class_repo::upsert(
            &master,
            Class {
                grade_id: Some(grade.id.clone()),
                school_year_id: Some(year.id.clone()),
                class_no: Some("1".into()),
                class_name: "一年级1班".into(),
                ..Default::default()
            },
        )
        .await
        .expect("create class");
        let room = classroom_repo::upsert(
            &master,
            Classroom {
                id: String::new(),
                room_name: "1楼101".into(),
                device_id: None,
                remark: None,
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                sync_state: String::new(),
                dirty: false,
            },
        )
        .await
        .expect("create classroom");
        classroom_repo::assign(&master, &room.id, &year.id, &class.id)
            .await
            .expect("create assignment");

        let snapshot = super::build_snapshot(&master)
            .await
            .expect("build snapshot");

        let client_dir =
            std::env::temp_dir().join(format!("lanwb_directory_client_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&client_dir).expect("client temp dir");
        let client = create_pool(&client_dir.join("client.db"))
            .await
            .expect("client pool");
        run_migrations(&client).await.expect("client migrations");
        super::apply_snapshot(&client, &snapshot)
            .await
            .expect("apply snapshot");

        assert_eq!(
            class_repo::list(&client, None, None).await.unwrap().len(),
            1
        );
        assert_eq!(classroom_repo::list(&client).await.unwrap().len(), 1);
        assert_eq!(
            classroom_repo::list_assignments(&client, None)
                .await
                .unwrap()
                .len(),
            1
        );
        master.close().await;
        client.close().await;
        std::fs::remove_dir_all(master_dir).ok();
        std::fs::remove_dir_all(client_dir).ok();
    }

    /// 回归：教务端对学年/班级/教室「删了重建」（新 id 占据同一业务键）后，
    /// 班级端再次拉取快照不得撞业务键唯一索引
    /// （用户实测报错：UNIQUE constraint failed: school_years.school_year_name，2067）。
    #[tokio::test]
    async fn snapshot_apply_survives_master_delete_and_recreate() {
        let mk_dir = |tag: &str| {
            let dir = std::env::temp_dir().join(format!(
                "lanwb_recreate_{}_{}",
                tag,
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&dir).expect("temp dir");
            dir
        };
        let master_dir = mk_dir("master");
        let client_dir = mk_dir("client");
        let master = create_pool(&master_dir.join("db.sqlite"))
            .await
            .expect("master pool");
        let client = create_pool(&client_dir.join("db.sqlite"))
            .await
            .expect("client pool");
        run_migrations(&master).await.expect("master migrations");
        run_migrations(&client).await.expect("client migrations");

        // ---- 第一版目录：2028届(A) + 一年级 + 一年级1班(C1) + 教室101(R1, 设备D) ----
        let year_a = school_year_repo::upsert(
            &master,
            SchoolYear {
                school_year_name: "2028届".into(),
                ..Default::default()
            },
        )
        .await
        .expect("year a");
        let grade = grade_repo::upsert(
            &master,
            Grade {
                grade_name: "一年级".into(),
                ..Default::default()
            },
        )
        .await
        .expect("grade");
        let class_c1 = class_repo::upsert(
            &master,
            Class {
                grade_id: Some(grade.id.clone()),
                school_year_id: Some(year_a.id.clone()),
                class_no: Some("1".into()),
                class_name: "一年级1班".into(),
                ..Default::default()
            },
        )
        .await
        .expect("class c1");
        let room_r1 = classroom_repo::upsert(
            &master,
            Classroom {
                id: String::new(),
                room_name: "101".into(),
                device_id: Some("device-d".into()),
                remark: None,
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                sync_state: String::new(),
                dirty: false,
            },
        )
        .await
        .expect("room r1");
        classroom_repo::assign(&master, &room_r1.id, &year_a.id, &class_c1.id)
            .await
            .expect("assign");

        let snapshot1 = super::build_snapshot(&master).await.expect("snapshot1");
        super::apply_snapshot(&client, &snapshot1)
            .await
            .expect("apply snapshot1");

        // ---- 教务端删了重建：软删旧实体，同名/同业务键新建（全新 id） ----
        class_repo::soft_delete(&master, &class_c1.id)
            .await
            .expect("del c1");
        school_year_repo::soft_delete(&master, &year_a.id)
            .await
            .expect("del year a");
        classroom_repo::soft_delete(&master, &room_r1.id)
            .await
            .expect("del r1");

        let year_b = school_year_repo::upsert(
            &master,
            SchoolYear {
                school_year_name: "2028届".into(),
                ..Default::default()
            },
        )
        .await
        .expect("year b");
        let class_c2 = class_repo::upsert(
            &master,
            Class {
                grade_id: Some(grade.id.clone()),
                school_year_id: Some(year_b.id.clone()),
                class_no: Some("1".into()),
                class_name: "一年级1班".into(),
                ..Default::default()
            },
        )
        .await
        .expect("class c2");
        let room_r2 = classroom_repo::upsert(
            &master,
            Classroom {
                id: String::new(),
                room_name: "101".into(),
                device_id: Some("device-d".into()),
                remark: None,
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                sync_state: String::new(),
                dirty: false,
            },
        )
        .await
        .expect("room r2");
        classroom_repo::assign(&master, &room_r2.id, &year_b.id, &class_c2.id)
            .await
            .expect("assign2");

        let snapshot2 = super::build_snapshot(&master).await.expect("snapshot2");
        super::apply_snapshot(&client, &snapshot2)
            .await
            .expect("重建后的快照不得撞业务键唯一索引（回归：2067）");

        // 客户端最终态：学年只剩重建行，班级/教室指向新 id。
        let years = school_year_repo::list(&client).await.expect("years");
        assert_eq!(years.len(), 1);
        assert_eq!(years[0].id, year_b.id);
        let classes = class_repo::list_by_year(&client, &year_b.id)
            .await
            .expect("classes");
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].id, class_c2.id);
        let rooms = classroom_repo::list(&client).await.expect("rooms");
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].id, room_r2.id);

        // ---- 二轮：班级也删了重建（同业务键、新 id） ----
        class_repo::soft_delete(&master, &class_c2.id)
            .await
            .expect("del c2");
        let class_c3 = class_repo::upsert(
            &master,
            Class {
                grade_id: Some(grade.id.clone()),
                school_year_id: Some(year_b.id.clone()),
                class_no: Some("1".into()),
                class_name: "一年级1班".into(),
                ..Default::default()
            },
        )
        .await
        .expect("class c3");
        let snapshot3 = super::build_snapshot(&master).await.expect("snapshot3");
        super::apply_snapshot(&client, &snapshot3)
            .await
            .expect("班级重建不得撞 ux_classes_year");
        let classes = class_repo::list_by_year(&client, &year_b.id)
            .await
            .expect("classes");
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].id, class_c3.id);

        master.close().await;
        client.close().await;
        std::fs::remove_dir_all(master_dir).ok();
        std::fs::remove_dir_all(client_dir).ok();
    }

    /// 回归：教务端软删学年/班级后，其教室绑定仍活跃，但母实体已不在快照的
    /// 活跃集合中；`build_snapshot` 必须剔除这类悬空绑定，否则班级端
    /// `apply_snapshot` 插入时会撞外键（code 787, FOREIGN KEY constraint failed）。
    #[tokio::test]
    async fn snapshot_excludes_assignments_with_dangling_parents() {
        let mk = || {
            let d = std::env::temp_dir().join(format!("lanwb_dangling_{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&d).unwrap();
            d
        };
        let master_dir = mk();
        let client_dir = mk();
        let master = create_pool(&master_dir.join("m.db")).await.unwrap();
        let client = create_pool(&client_dir.join("c.db")).await.unwrap();
        run_migrations(&master).await.unwrap();
        run_migrations(&client).await.unwrap();

        let year = school_year_repo::upsert(
            &master,
            SchoolYear {
                school_year_name: "2028届".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let grade = grade_repo::upsert(
            &master,
            Grade {
                grade_name: "一年级".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let class = class_repo::upsert(
            &master,
            Class {
                grade_id: Some(grade.id.clone()),
                school_year_id: Some(year.id.clone()),
                class_name: "一年级1班".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let room = classroom_repo::upsert(
            &master,
            Classroom {
                id: String::new(),
                room_name: "101".into(),
                device_id: None,
                remark: None,
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                sync_state: String::new(),
                dirty: false,
            },
        )
        .await
        .unwrap();
        classroom_repo::assign(&master, &room.id, &year.id, &class.id)
            .await
            .unwrap();

        // 软删学年：school_year_repo::soft_delete 不级联到 classroom_assignments。
        school_year_repo::soft_delete(&master, &year.id)
            .await
            .unwrap();

        let snapshot = super::build_snapshot(&master).await.unwrap();
        // 学年已软删 → 不在 school_years 列表；绑定引用了它 → 必须被剔除。
        assert_eq!(snapshot.school_years.len(), 0);
        assert_eq!(
            snapshot.assignments.len(),
            0,
            "悬空绑定不得进入快照（回归：787 FK）"
        );

        // 即便快照为空，apply 也不应撞外键。
        super::apply_snapshot(&client, &snapshot)
            .await
            .expect("空快照不得报 787");

        master.close().await;
        client.close().await;
        std::fs::remove_dir_all(master_dir).ok();
        std::fs::remove_dir_all(client_dir).ok();
    }

    /// 回归：即便快照本身携带悬空绑定（母实体 id 在客户端不存在），`apply_snapshot`
    /// 也必须跳过它而非整体失败（code 787）。班级端是只读镜像，丢弃悬空引用即可，
    /// 不必让整次目录同步崩溃（与 3168a60 的「镜像不撞唯一索引」同思路）。
    #[tokio::test]
    async fn apply_snapshot_skips_dangling_assignment_without_fk_error() {
        let client_dir =
            std::env::temp_dir().join(format!("lanwb_dangling_client_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&client_dir).unwrap();
        let client = create_pool(&client_dir.join("c.db")).await.unwrap();
        run_migrations(&client).await.unwrap();

        // 客户端已有真实学年/班级/教室，但快照里的绑定却指向不存在的母实体 id。
        let year = school_year_repo::upsert(
            &client,
            SchoolYear {
                school_year_name: "2028届".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let grade = grade_repo::upsert(
            &client,
            Grade {
                grade_name: "一年级".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let class = class_repo::upsert(
            &client,
            Class {
                grade_id: Some(grade.id.clone()),
                school_year_id: Some(year.id.clone()),
                class_name: "一年级1班".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let room = classroom_repo::upsert(
            &client,
            Classroom {
                id: String::new(),
                room_name: "101".into(),
                device_id: None,
                remark: None,
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                sync_state: String::new(),
                dirty: false,
            },
        )
        .await
        .unwrap();

        let dangling = ClassroomAssignment {
            id: uuid::Uuid::new_v4().to_string(),
            classroom_id: room.id.clone(),
            school_year_id: "nonexistent-year".into(),
            class_id: class.id.clone(),
            created_at: 1,
            updated_at: 1,
            deleted_at: None,
            sync_state: String::new(),
            dirty: false,
        };
        let snapshot = super::DirectorySnapshot {
            school_years: vec![],
            grades: vec![],
            classes: vec![],
            classrooms: vec![],
            assignments: vec![dangling],
            current_school_year_id: None,
        };
        let report = super::apply_snapshot(&client, &snapshot)
            .await
            .expect("悬空绑定必须被跳过而非报 787");
        assert_eq!(report.assignments, 1);

        // 客户端不应多插入任何绑定行（悬空绑定被丢弃）。
        let remaining = classroom_repo::list_assignments(&client, None)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 0, "悬空绑定不得落库");

        client.close().await;
        std::fs::remove_dir_all(client_dir).ok();
    }

    /// 回归：快照携带两个同名活跃学年（如重复「删了重建」/ 半途崩溃残留），
    /// 且后到的旧行走更新路径会被 upsert 的 `SET deleted_at = NULL` 复活成活跃——
    /// 修复前会撞唯一索引 ux_school_years_name（code 2067），且因快照顺序而时好时坏。
    /// 现在插入与更新路径都先墓碑化同名异 id 旧行，绝不再撞唯一索引。
    #[tokio::test]
    async fn apply_snapshot_survives_same_named_year_update_path() {
        let client_dir =
            std::env::temp_dir().join(format!("lanwb_same_name_year_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&client_dir).unwrap();
        let client = create_pool(&client_dir.join("c.db")).await.unwrap();
        run_migrations(&client).await.unwrap();

        // 客户端已有一个活跃学年 X（id 固定，模拟早于本次重建即已同步）。
        school_year_repo::upsert(
            &client,
            SchoolYear {
                id: "X-id".into(),
                school_year_name: "2028届".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // 快照携带两个同名活跃学年：Y（新 id，先到）→ X（同 id，后到，走更新路径复活）。
        let y = SchoolYear {
            id: "Y-id".into(),
            school_year_name: "2028届".into(),
            ..Default::default()
        };
        let x = SchoolYear {
            id: "X-id".into(),
            school_year_name: "2028届".into(),
            ..Default::default()
        };
        let snapshot = super::DirectorySnapshot {
            school_years: vec![y, x],
            grades: vec![],
            classes: vec![],
            classrooms: vec![],
            assignments: vec![],
            current_school_year_id: None,
        };
        super::apply_snapshot(&client, &snapshot)
            .await
            .expect("同名学年不得撞 ux_school_years_name（回归：2067 更新路径复活）");

        // 最终应恰好剩一个活跃同名学年（后处理者胜出），不得 2067、不得双活。
        let active = school_year_repo::list(&client).await.unwrap();
        assert_eq!(active.len(), 1, "活跃同名学年应只剩一个");
        assert_eq!(active[0].school_year_name, "2028届");

        client.close().await;
        std::fs::remove_dir_all(client_dir).ok();
    }

    /// 班级端自动切换守护（设计 §5.1）：
    /// ① 名册非空 + 设备认领教室 + 权威年绑定齐备 → 切绑并写三个 settings key；
    /// ② 已对齐 → 不再触发；
    /// ③ 权威年为空 → 永不触发；
    /// ④ 名册为空 → 返回 None 且不改写绑定。
    #[tokio::test]
    async fn auto_switch_fires_when_ready_and_skips_empty_roster() {
        let dir = std::env::temp_dir().join(format!("lanwb_auto_switch_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let pool = create_pool(&dir.join("c.db")).await.expect("pool");
        run_migrations(&pool).await.expect("migrations");

        // 场景数据：设备 d1 认领教室 room1；room1 在新学年 y2 绑定 class2；class2 有 1 名学生。
        school_year_repo::upsert(
            &pool,
            SchoolYear {
                id: "y1".into(),
                school_year_name: "2025届".into(),
                ..Default::default()
            },
        )
        .await
        .expect("year y1");
        school_year_repo::upsert(
            &pool,
            SchoolYear {
                id: "y2".into(),
                school_year_name: "2026届".into(),
                ..Default::default()
            },
        )
        .await
        .expect("year y2");
        let grade = grade_repo::upsert(
            &pool,
            Grade {
                grade_name: "一年级".into(),
                ..Default::default()
            },
        )
        .await
        .expect("grade");
        class_repo::upsert(
            &pool,
            Class {
                id: "class1".into(),
                grade_id: Some(grade.id.clone()),
                school_year_id: Some("y1".into()),
                class_name: "一年级1班".into(),
                ..Default::default()
            },
        )
        .await
        .expect("class1");
        class_repo::upsert(
            &pool,
            Class {
                id: "class2".into(),
                grade_id: Some(grade.id.clone()),
                school_year_id: Some("y2".into()),
                class_name: "一年级2班".into(),
                ..Default::default()
            },
        )
        .await
        .expect("class2");
        classroom_repo::upsert(
            &pool,
            Classroom {
                id: "room1".into(),
                room_name: "101".into(),
                device_id: Some("d1".into()),
                remark: None,
                created_at: 0,
                updated_at: 0,
                deleted_at: None,
                sync_state: String::new(),
                dirty: false,
            },
        )
        .await
        .expect("room1");
        classroom_repo::assign(&pool, "room1", "y2", "class2")
            .await
            .expect("assignment");
        student_repo::upsert(
            &pool,
            Student {
                id: "stu1".into(),
                student_no: "S001".into(),
                name: "张三".into(),
                gender: "male".into(),
                class_id: Some("class2".into()),
                class_name: Some("一年级2班".into()),
                status: "active".into(),
                ..Default::default()
            },
        )
        .await
        .expect("student");

        // 本机绑定 settings（旧学年 y1 / class1）。
        settings_repo::set_raw(&pool, "device_id", Some("d1"), "string")
            .await
            .unwrap();
        settings_repo::set_raw(&pool, "school_year_id", Some("y1"), "string")
            .await
            .unwrap();
        settings_repo::set_raw(&pool, "bound_class_id", Some("class1"), "string")
            .await
            .unwrap();
        settings_repo::set_raw(&pool, "class_id", Some("class1"), "string")
            .await
            .unwrap();

        // ① 名册非空 → 切换成功。
        let info = super::auto_switch_if_ready(&pool, Some("y2"))
            .await
            .unwrap()
            .expect("should switch");
        assert_eq!(info.school_year_id, "y2");
        assert_eq!(info.class_id, "class2");
        assert_eq!(info.school_year_name, "2026届");
        assert_eq!(info.class_name, "一年级2班");
        assert_eq!(
            settings_repo::get_string(&pool, "bound_class_id", "")
                .await
                .unwrap(),
            "class2"
        );
        assert_eq!(
            settings_repo::get_string(&pool, "school_year_id", "")
                .await
                .unwrap(),
            "y2"
        );

        // ② 已对齐 → 不再触发。
        assert!(super::auto_switch_if_ready(&pool, Some("y2"))
            .await
            .unwrap()
            .is_none());

        // ③ 权威年为空 → 永不触发。
        assert!(super::auto_switch_if_ready(&pool, None)
            .await
            .unwrap()
            .is_none());

        // ④ 名册为空 → 返回 None 且不改写绑定。
        // 先把绑定改回旧班（否则 ② 的「已对齐」会短路），再删学生。
        settings_repo::set_raw(&pool, "bound_class_id", Some("class1"), "string")
            .await
            .unwrap();
        settings_repo::set_raw(&pool, "class_id", Some("class1"), "string")
            .await
            .unwrap();
        student_repo::soft_delete(&pool, "stu1").await.unwrap();
        assert!(super::auto_switch_if_ready(&pool, Some("y2"))
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            settings_repo::get_string(&pool, "bound_class_id", "")
                .await
                .unwrap(),
            "class1",
            "名册为空时不得改写绑定"
        );

        pool.close().await;
        std::fs::remove_dir_all(dir).ok();
    }
}
