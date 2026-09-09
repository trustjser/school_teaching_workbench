//! 统一错误类型：`ErrorCode` 枚举 + `AppError`。
//! 序列化到前端的形态固定为 `{ code, message }`，与 `src/constants/errorCodes.ts` 对齐。

use serde::{Serialize, Serializer};
use std::fmt;

/// 错误码。字符串值与 `docs/03-tasks.md` §4.3 错误码表严格一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// 数据库操作失败
    Db,
    /// 网络不可达
    Net,
    /// 签名校验失败
    Sign,
    /// 报文解密失败
    Crypto,
    /// 时间偏差超出窗口
    TsWindow,
    /// 检测到重放请求
    NonceReplay,
    /// 参数校验失败
    Validation,
    /// 记录不存在或已删除
    NotFound,
    /// 当前模式不支持该操作
    Mode,
    /// 无文件或网络权限
    Permission,
    /// 导入数据有误
    Import,
    /// 未知错误
    Unknown,
}

impl ErrorCode {
    /// 返回对外协议字符串，例如 `ERR_DB`。
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::Db => "ERR_DB",
            ErrorCode::Net => "ERR_NET",
            ErrorCode::Sign => "ERR_SIGN",
            ErrorCode::Crypto => "ERR_CRYPTO",
            ErrorCode::TsWindow => "ERR_TS_WINDOW",
            ErrorCode::NonceReplay => "ERR_NONCE_REPLAY",
            ErrorCode::Validation => "ERR_VALIDATION",
            ErrorCode::NotFound => "ERR_NOT_FOUND",
            ErrorCode::Mode => "ERR_MODE",
            ErrorCode::Permission => "ERR_PERMISSION",
            ErrorCode::Import => "ERR_IMPORT",
            ErrorCode::Unknown => "ERR_UNKNOWN",
        }
    }

    /// 对应的 HTTP 状态码。
    pub fn http_status(&self) -> u16 {
        match self {
            ErrorCode::Db => 500,
            ErrorCode::Net => 502,
            ErrorCode::Sign | ErrorCode::Crypto | ErrorCode::TsWindow | ErrorCode::NonceReplay => {
                401
            }
            ErrorCode::Validation => 400,
            ErrorCode::NotFound => 404,
            ErrorCode::Mode => 409,
            ErrorCode::Permission => 403,
            ErrorCode::Import => 422,
            ErrorCode::Unknown => 500,
        }
    }

    /// 默认中文提示。
    pub fn default_message(&self) -> &'static str {
        match self {
            ErrorCode::Db => "数据库操作失败，请重试",
            ErrorCode::Net => "网络不可达，已加入待发队列",
            ErrorCode::Sign => "签名校验失败，请检查共享密钥",
            ErrorCode::Crypto => "报文解密失败，数据可能已损坏",
            ErrorCode::TsWindow => "时间偏差过大，请校准本机时间",
            ErrorCode::NonceReplay => "检测到重复请求，已拒绝",
            ErrorCode::Validation => "参数校验失败",
            ErrorCode::NotFound => "记录不存在或已删除",
            ErrorCode::Mode => "当前运行模式不支持该操作",
            ErrorCode::Permission => "无文件或网络权限",
            ErrorCode::Import => "导入数据有误，请查看错误报告",
            ErrorCode::Unknown => "发生未知错误",
        }
    }

    /// 是否值得重试（同步 worker 判定死信时使用）。
    pub fn retryable(&self) -> bool {
        matches!(self, ErrorCode::Db | ErrorCode::Net)
    }

    /// 由协议字符串反解，未知输入回退 `Unknown`。
    pub fn parse(value: &str) -> ErrorCode {
        match value {
            "ERR_DB" => ErrorCode::Db,
            "ERR_NET" => ErrorCode::Net,
            "ERR_SIGN" => ErrorCode::Sign,
            "ERR_CRYPTO" => ErrorCode::Crypto,
            "ERR_TS_WINDOW" => ErrorCode::TsWindow,
            "ERR_NONCE_REPLAY" => ErrorCode::NonceReplay,
            "ERR_VALIDATION" => ErrorCode::Validation,
            "ERR_NOT_FOUND" => ErrorCode::NotFound,
            "ERR_MODE" => ErrorCode::Mode,
            "ERR_PERMISSION" => ErrorCode::Permission,
            "ERR_IMPORT" => ErrorCode::Import,
            _ => ErrorCode::Unknown,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// 统一应用层错误。所有 Tauri 命令返回 `Result<T, AppError>`。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    /// 错误码。
    pub code: ErrorCode,
    /// 面向用户的中文提示。
    pub message: String,
    /// 可选的技术细节（调试日志用，前端忽略）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AppError {
    /// 以指定错误码与消息构造。
    pub fn new(code: ErrorCode, message: impl Into<String>) -> AppError {
        AppError {
            code,
            message: message.into(),
            detail: None,
        }
    }

    /// 使用错误码的默认中文提示构造。
    pub fn code(code: ErrorCode) -> AppError {
        AppError {
            code,
            message: code.default_message().to_string(),
            detail: None,
        }
    }

    /// 附加技术细节（不展示给用户）。
    pub fn with_detail(mut self, detail: impl Into<String>) -> AppError {
        self.detail = Some(detail.into());
        self
    }

    /// 数据库错误统一入口：保留原因、对外隐藏细节。
    pub fn db(reason: impl fmt::Display) -> AppError {
        AppError::new(ErrorCode::Db, ErrorCode::Db.default_message())
            .with_detail(reason.to_string())
    }

    /// 网络错误统一入口。
    pub fn net(reason: impl fmt::Display) -> AppError {
        AppError::new(ErrorCode::Net, ErrorCode::Net.default_message())
            .with_detail(reason.to_string())
    }

    /// 参数校验失败。
    pub fn validation(message: impl Into<String>) -> AppError {
        AppError::new(ErrorCode::Validation, message)
    }

    /// 记录不存在。
    pub fn not_found(what: impl Into<String>) -> AppError {
        AppError::new(
            ErrorCode::NotFound,
            format!("{}不存在或已删除", what.into()),
        )
    }

    /// 快捷构造：签名错误。
    pub fn sign() -> AppError {
        AppError::code(ErrorCode::Sign)
    }

    /// 快捷构造：解密错误。
    pub fn crypto() -> AppError {
        AppError::code(ErrorCode::Crypto)
    }

    /// 快捷构造：时间戳窗口错误。
    pub fn ts_window() -> AppError {
        AppError::code(ErrorCode::TsWindow)
    }

    /// 快捷构造：重放错误。
    pub fn nonce_replay() -> AppError {
        AppError::code(ErrorCode::NonceReplay)
    }

    /// 快捷构造：模式错误。
    pub fn mode(message: impl Into<String>) -> AppError {
        AppError::new(ErrorCode::Mode, message)
    }

    /// 快捷构造：导入错误。
    pub fn import(message: impl Into<String>) -> AppError {
        AppError::new(ErrorCode::Import, message)
    }

    /// 快捷构造：导出错误。
    pub fn export(message: impl Into<String>) -> AppError {
        AppError::new(ErrorCode::Import, message)
    }

    /// 快捷构造：权限错误。
    pub fn permission(message: impl Into<String>) -> AppError {
        AppError::new(ErrorCode::Permission, message)
    }

    /// 协议字符串形式，便于落 `sync_log.error_code`。
    pub fn code_str(&self) -> &'static str {
        self.code.as_str()
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.detail {
            Some(d) => write!(
                f,
                "[{}] {} | detail: {}",
                self.code.as_str(),
                self.message,
                d
            ),
            None => write!(f, "[{}] {}", self.code.as_str(), self.message),
        }
    }
}

impl std::error::Error for AppError {}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> AppError {
        AppError::db(err)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> AppError {
        AppError::new(ErrorCode::Validation, "JSON 序列化/反序列化失败")
            .with_detail(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> AppError {
        AppError::permission(err.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> AppError {
        AppError::net(err)
    }
}

impl From<base64::DecodeError> for AppError {
    fn from(err: base64::DecodeError) -> AppError {
        AppError::new(ErrorCode::Crypto, ErrorCode::Crypto.default_message())
            .with_detail(err.to_string())
    }
}

impl From<hex::FromHexError> for AppError {
    fn from(err: hex::FromHexError) -> AppError {
        AppError::new(ErrorCode::Crypto, ErrorCode::Crypto.default_message())
            .with_detail(err.to_string())
    }
}

/// 业务便捷别名。
pub type AppResult<T> = Result<T, AppError>;

/// 序列化为 `{ code, message }` 的通用错误响应体，供 HTTP 层复用。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    /// 错误码。
    pub code: String,
    /// 中文提示。
    pub message: String,
    /// 追踪 ID（等于信封 nonce）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

impl ErrorBody {
    /// 由 `AppError` 生成响应体。
    pub fn from_error(err: &AppError, trace_id: Option<String>) -> ErrorBody {
        ErrorBody {
            code: err.code.as_str().to_string(),
            message: err.message.clone(),
            trace_id,
        }
    }
}
