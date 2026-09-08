//! 解析 xlsx 名册文件为 `StudentImportRow`（前端也可直接解析，此函数供后端导入命令复用）。

use calamine::{open_workbook_auto, Data, Reader};

use crate::db::models::StudentImportRow;
use crate::error::{AppError, AppResult};

/// 从 xlsx 文件解析学生名册（首行为表头，支持中英文列名）。
pub fn parse_xlsx(path: &str) -> AppResult<Vec<StudentImportRow>> {
    let mut wb = open_workbook_auto(path)
        .map_err(|e| AppError::import(format!("打开 xlsx 失败: {}", e)))?;
    let range = wb
        .worksheet_range_at(0)
        .ok_or_else(|| AppError::import("xlsx 中未找到工作表"))?
        .map_err(|e| AppError::import(format!("读取工作表失败: {}", e)))?;

    let mut rows: Vec<StudentImportRow> = Vec::new();
    let mut header: Option<Vec<String>> = None;
    for (i, row) in range.rows().enumerate() {
        if row.is_empty() {
            continue;
        }
        if i == 0 {
            header = Some(row.iter().map(cell_text).collect());
            continue;
        }
        let rec = map_record(header.as_deref(), row.iter().map(cell_text).collect::<Vec<_>>())?;
        rows.push(rec);
    }
    Ok(rows)
}

/// 单元格转文本。
fn cell_text(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        other => other.to_string().trim().to_string(),
    }
}

/// 按表头把一行文本映射为 `StudentImportRow`（中英文列名均可）。
fn map_record(header: Option<&[String]>, values: Vec<String>) -> AppResult<StudentImportRow> {
    let mut map: Vec<(String, String)> = Vec::with_capacity(values.len());
    match header {
        Some(h) => {
            for (k, v) in h.iter().zip(values.iter()) {
                map.push((k.trim().to_lowercase(), v.clone()));
            }
        }
        None => {
            for (idx, v) in values.iter().enumerate() {
                map.push((format!("col{}", idx), v.clone()));
            }
        }
    }

    let get = |keys: &[&str]| -> Option<String> {
        map.iter()
            .find(|(k, _)| keys.iter().any(|kk| k == kk))
            .and_then(|(_, v)| if v.is_empty() { None } else { Some(v.clone()) })
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
