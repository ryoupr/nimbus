use thiserror::Error;

/// Main error type for EC2 Connect
#[derive(Error, Debug, Clone)]
pub enum Ec2ConnectError {
    #[error("Configuration error: {0}")]
    Config(ConfigError),
    
    #[error("AWS error: {0}")]
    Aws(AwsError),
    
    #[error("Session error: {0}")]
    Session(SessionError),
    
    #[error("Connection error: {0}")]
    Connection(ConnectionError),
    
    #[error("Resource error: {0}")]
    Resource(ResourceError),
    
    #[error("UI error: {0}")]
    Ui(UiError),
    
    #[error("VS Code integration error: {0}")]
    VsCode(VsCodeError),
    
    #[error("IO error: {0}")]
    Io(String),
    
    #[error("JSON error: {0}")]
    Json(String),
    
    #[error("TOML error: {0}")]
    Toml(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Anyhow error: {0}")]
    Anyhow(String),
    
    #[error("System error: {0}")]
    System(String),
}

/// Configuration-related errors
#[derive(Error, Debug, Clone)]
pub enum ConfigError {
}

/// AWS-related errors
#[derive(Error, Debug, Clone)]
pub enum AwsError {
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },
    
    #[error("SSM service error: {message}")]
    SsmServiceError { message: String },
    
    #[error("EC2 service error: {message}")]
    Ec2ServiceError { message: String },
    
    #[error("Timeout: {operation}")]
    Timeout { operation: String },
}

/// Session management errors
#[derive(Error, Debug, Clone)]
pub enum SessionError {
    #[error("Session not found: {session_id}")]
    NotFound { session_id: String },
    
    #[error("Session creation failed: {reason}")]
    CreationFailed { reason: String },
    
    #[error("Session limit exceeded: max {max_sessions}")]
    LimitExceeded { max_sessions: u32 },
}

/// Connection-related errors
#[derive(Error, Debug, Clone)]
pub enum ConnectionError {
    #[error("Preventive check failed: {reason}")]
    PreventiveCheckFailed { reason: String, issues: Vec<String> },
}

/// Resource management errors
#[derive(Error, Debug, Clone)]
pub enum ResourceError {
}

/// UI-related errors
#[derive(Error, Debug, Clone)]
pub enum UiError {
}

/// VS Code integration errors
#[derive(Error, Debug, Clone)]
pub enum VsCodeError {
    #[error("VS Code not found: {message}")]
    NotFound { message: String },
    
    #[error("VS Code launch failed: {message}")]
    LaunchFailed { message: String },
    
    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
}

/// Result type alias for EC2 Connect operations
pub type Result<T> = std::result::Result<T, Ec2ConnectError>;

// From trait implementations for error conversion
impl From<ConfigError> for Ec2ConnectError {
    fn from(err: ConfigError) -> Self {
        Ec2ConnectError::Config(err)
    }
}

impl From<AwsError> for Ec2ConnectError {
    fn from(err: AwsError) -> Self {
        Ec2ConnectError::Aws(err)
    }
}

impl From<SessionError> for Ec2ConnectError {
    fn from(err: SessionError) -> Self {
        Ec2ConnectError::Session(err)
    }
}

impl From<ConnectionError> for Ec2ConnectError {
    fn from(err: ConnectionError) -> Self {
        Ec2ConnectError::Connection(err)
    }
}

impl From<ResourceError> for Ec2ConnectError {
    fn from(err: ResourceError) -> Self {
        Ec2ConnectError::Resource(err)
    }
}

impl From<UiError> for Ec2ConnectError {
    fn from(err: UiError) -> Self {
        Ec2ConnectError::Ui(err)
    }
}

impl From<VsCodeError> for Ec2ConnectError {
    fn from(err: VsCodeError) -> Self {
        Ec2ConnectError::VsCode(err)
    }
}

impl From<std::io::Error> for Ec2ConnectError {
    fn from(err: std::io::Error) -> Self {
        Ec2ConnectError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for Ec2ConnectError {
    fn from(err: serde_json::Error) -> Self {
        Ec2ConnectError::Json(err.to_string())
    }
}

impl From<toml::de::Error> for Ec2ConnectError {
    fn from(err: toml::de::Error) -> Self {
        Ec2ConnectError::Toml(err.to_string())
    }
}

impl From<rusqlite::Error> for Ec2ConnectError {
    fn from(err: rusqlite::Error) -> Self {
        Ec2ConnectError::Database(err.to_string())
    }
}

impl From<anyhow::Error> for Ec2ConnectError {
    fn from(err: anyhow::Error) -> Self {
        Ec2ConnectError::Anyhow(err.to_string())
    }
}

impl Ec2ConnectError {
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Ec2ConnectError::Connection(ConnectionError::PreventiveCheckFailed { .. }) => true,
            Ec2ConnectError::Session(SessionError::CreationFailed { .. }) => true,
            Ec2ConnectError::Io(_) => true,
            _ => false,
        }
    }
    
    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Ec2ConnectError::Config(_) => ErrorSeverity::High,
            Ec2ConnectError::Aws(AwsError::AuthenticationFailed { .. }) => ErrorSeverity::High,
            Ec2ConnectError::Resource(_) => ErrorSeverity::High,
            Ec2ConnectError::Connection(_) => ErrorSeverity::Medium,
            Ec2ConnectError::Session(_) => ErrorSeverity::Medium,
            Ec2ConnectError::Ui(_) => ErrorSeverity::Low,
            _ => ErrorSeverity::Medium,
        }
    }
    
    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Ec2ConnectError::Aws(AwsError::AuthenticationFailed { .. }) => {
                "AWS認証に失敗しました。AWS認証情報を確認してください。".to_string()
            },
            Ec2ConnectError::Session(SessionError::LimitExceeded { max_sessions }) => {
                format!("セッション数の上限に達しました（最大{}セッション）。", max_sessions)
            },
            Ec2ConnectError::VsCode(VsCodeError::NotFound { .. }) => {
                "VS Codeが見つかりません。VS Codeをインストールするか、設定でパスを指定してください。".to_string()
            },
            Ec2ConnectError::VsCode(VsCodeError::LaunchFailed { .. }) => {
                "VS Codeの起動に失敗しました。VS Codeが正しくインストールされているか確認してください。".to_string()
            },
            _ => self.to_string(),
        }
    }
}

/// Error severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
}

impl ErrorSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorSeverity::Low => "LOW",
            ErrorSeverity::Medium => "MEDIUM",
            ErrorSeverity::High => "HIGH",
        }
    }
}
