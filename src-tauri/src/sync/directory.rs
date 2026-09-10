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
}
