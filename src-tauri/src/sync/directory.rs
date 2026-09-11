use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::models::{Class, Classroom, ClassroomAssignment, Grade, SchoolYear};
use crate::db::repo::{class_repo, classroom_repo, grade_repo, school_year_repo};
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassroomClaimResponse {
    pub classroom: Classroom,
    pub assignment: ClassroomAssignment,
}

pub async fn build_snapshot(pool: &SqlitePool) -> AppResult<DirectorySnapshot> {
    Ok(DirectorySnapshot {
        school_years: school_year_repo::list(pool).await?,
        grades: grade_repo::list(pool).await?,
        classes: class_repo::list(pool, None, None).await?,
        classrooms: classroom_repo::list(pool).await?,
        assignments: classroom_repo::list_assignments(pool, None).await?,
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
    })
}

#[cfg(test)]
mod tests {
    use crate::db::models::{Class, Classroom, Grade, SchoolYear};
    use crate::db::repo::{class_repo, classroom_repo, grade_repo, school_year_repo};
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
            let dir =
                std::env::temp_dir().join(format!("lanwb_recreate_{}_{}", tag, uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&dir).expect("temp dir");
            dir
        };
        let master_dir = mk_dir("master");
        let client_dir = mk_dir("client");
        let master = create_pool(&master_dir.join("db.sqlite")).await.expect("master pool");
        let client = create_pool(&client_dir.join("db.sqlite")).await.expect("client pool");
        run_migrations(&master).await.expect("master migrations");
        run_migrations(&client).await.expect("client migrations");

        // ---- 第一版目录：2028届(A) + 一年级 + 一年级1班(C1) + 教室101(R1, 设备D) ----
        let year_a = school_year_repo::upsert(
            &master,
            SchoolYear { school_year_name: "2028届".into(), ..Default::default() },
        )
        .await
        .expect("year a");
        let grade = grade_repo::upsert(
            &master,
            Grade { grade_name: "一年级".into(), ..Default::default() },
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
        super::apply_snapshot(&client, &snapshot1).await.expect("apply snapshot1");

        // ---- 教务端删了重建：软删旧实体，同名/同业务键新建（全新 id） ----
        class_repo::soft_delete(&master, &class_c1.id).await.expect("del c1");
        school_year_repo::soft_delete(&master, &year_a.id).await.expect("del year a");
        classroom_repo::soft_delete(&master, &room_r1.id).await.expect("del r1");

        let year_b = school_year_repo::upsert(
            &master,
            SchoolYear { school_year_name: "2028届".into(), ..Default::default() },
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
        let classes = class_repo::list_by_year(&client, &year_b.id).await.expect("classes");
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].id, class_c2.id);
        let rooms = classroom_repo::list(&client).await.expect("rooms");
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].id, room_r2.id);

        // ---- 二轮：班级也删了重建（同业务键、新 id） ----
        class_repo::soft_delete(&master, &class_c2.id).await.expect("del c2");
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
        let classes = class_repo::list_by_year(&client, &year_b.id).await.expect("classes");
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].id, class_c3.id);

        master.close().await;
        client.close().await;
        std::fs::remove_dir_all(master_dir).ok();
        std::fs::remove_dir_all(client_dir).ok();
    }
}
