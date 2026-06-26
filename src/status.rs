use serde::Serialize;
use std::fmt;

#[derive(Serialize, Clone, Copy, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatusCode {
    ConfigReadFailed,
    JsonParseFailed,
    FstabWriteFailed,
    SnapshotsDirOpenFailed,
    SourceDirOpenFailed,
    HashCollisionDetected,
    KernelIoctlFailed,
    PropertySetFailed,
    TimeRetrievalFailed,
    RootRequired,
    LockFileOpenFailed,
    ProcessLocked,
    RootSnapshotNotFound,
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for StatusCode {}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuccessCode {
    SnapshotCreated,
    SequenceFinished,
}

#[derive(Serialize)]
struct JsonErrorPayload {
    pub error_type: StatusCode,
    pub message: &'static str,
}

#[derive(Serialize)]
struct JsonSuccessPayload<'a> {
    pub event: SuccessCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
    pub message: &'static str,
}

pub fn emit_error(code: StatusCode) {
    let msg = match code {
        StatusCode::ConfigReadFailed => "Config read failed",
        StatusCode::JsonParseFailed => "Wrong json schema",
        StatusCode::FstabWriteFailed => "Fstab write failed",
        StatusCode::SnapshotsDirOpenFailed => "Snapshots storage path missing",
        StatusCode::SourceDirOpenFailed => "Failed to open target path",
        StatusCode::HashCollisionDetected => "Snapshot variant already exists",
        StatusCode::KernelIoctlFailed => "Kernel snapshot creation failed",
        StatusCode::PropertySetFailed => "Failed to toggle properties",
        StatusCode::TimeRetrievalFailed => "System time backwards",
        StatusCode::RootRequired => "Root required",
        StatusCode::LockFileOpenFailed => "Lock initialization failed",
        StatusCode::ProcessLocked => "Instance locked",
        StatusCode::RootSnapshotNotFound => "Root snapshot missing in targets",
    };

    let line = serde_json::to_string(&JsonErrorPayload {
        error_type: code,
        message: msg,
    })
    .unwrap(); // UNWRAP: Infallible static structural text schema mapping
    eprintln!("{line}");
}

pub fn emit_success_snapshot(source: &str, name: &str) {
    let line = serde_json::to_string(&JsonSuccessPayload {
        event: SuccessCode::SnapshotCreated,
        hash: None,
        source: Some(source),
        name: Some(name),
        message: "Snapshot created",
    })
    .unwrap(); // UNWRAP: Infallible static structural text schema mapping
    println!("{line}");
}

pub fn emit_success_finished(hash: &str) {
    let line = serde_json::to_string(&JsonSuccessPayload {
        event: SuccessCode::SequenceFinished,
        hash: Some(hash),
        source: None,
        name: None,
        message: "Sequence finished",
    })
    .unwrap(); // UNWRAP: Infallible static structural text schema mapping
    println!("{line}");
}
