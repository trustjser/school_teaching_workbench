//! 解析 csv 名册文件为 `StudentImportRow`（与 `xlsx.rs` 共享中英文列名映射规则）。

use csv::StringRecord;

use crate::db::models::StudentImportRow;
use crate::error::{AppError, AppResult};

/// 从 csv 文件解析学生名册（首行为表头，支持中英文列名）。
pub fn parse_csv(path: &str) -> AppResult<Vec<StudentImportRow>> {
    let mut rdr = csv::Reader::from_path(path)
        .map_err(|e| AppError::import(format!("打开 csv 失败: {}", e)))?;

    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| AppError::import(format!("读取表头失败: {}", e)))?
        .iter()
        .map(|h| h.trim().to_lowercase())
        .collect();

    let mut rows: Vec<StudentImportRow> = Vec::new();
    for record in rdr.records() {
        let record: StringRecord =
            record.map_err(|e| AppError::import(format!("读取数据行失败: {}", e)))?;
        let values: Vec<String> = record.iter().map(|v| v.trim().to_string()).collect();
        let row = map_record(&headers, &values)?;
        rows.push(row);
    }
    Ok(rows)
}

/// 按表头把一行文本映射为 `StudentImportRow`（中英文列名均可）。
fn map_record(headers: &[String], values: &[String]) -> AppResult<StudentImportRow> {
    let get = |keys: &[&str]| -> Option<String> {
        headers
            .iter()
            .position(|h| keys.iter().any(|kk| h == kk))
            .and_then(|i| {
                let v = values
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                if v.is_empty() {
                    None
                } else {
                    Some(v)
                }
            })
    };

    let student_no = get(&["student_no", "学号", "学籍号"]).unwrap_or_default();
    let name = get(&["name", "姓名"]).unwrap_or_default();
    if student_no.is_empty() || name.is_empty() {
        return Err(AppError::import("缺少学号或姓名列"));
    }
    let seat_no = get(&["seat_no", "座位号", "座号"]).and_then(|s| s.parse::<i64>().ok());

    Ok(StudentImportRow {
        student_no,
        name,
        gender: get(&["gender", "性别"]),
        grade: get(&["grade", "年级"]),
        class_name: get(&["class_name", "班级", "班别"]),
        seat_no,
        phone: get(&["phone", "电话", "联系电话", "手机"]),
        note: get(&["note", "备注"]),
        class_id: None,
    })
}
