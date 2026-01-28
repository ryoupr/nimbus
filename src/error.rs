use thiserror::Error;

/// Main error type for Nimbus
#[derive(Error, Debug, Clone)]
pub enum NimbusError {
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

/// Result type alias for Nimbus operations
pub type Result<T> = std::result::Result<T, NimbusError>;

// From trait implementations for error conversion
impl From<ConfigError> for NimbusError {
    fn from(err: ConfigError) -> Self {
        NimbusError::Config(err)
    }
}

impl From<AwsError> for NimbusError {
    fn from(err: AwsError) -> Self {
        NimbusError::Aws(err)
    }
}

impl From<SessionError> for NimbusError {
    fn from(err: SessionError) -> Self {
        NimbusError::Session(err)
    }
}

impl From<ConnectionError> for NimbusError {
    fn from(err: ConnectionError) -> Self {
        NimbusError::Connection(err)
    }
}

impl From<ResourceError> for NimbusError {
    fn from(err: ResourceError) -> Self {
        NimbusError::Resource(err)
    }
}

impl From<UiError> for NimbusError {
    fn from(err: UiError) -> Self {
        NimbusError::Ui(err)
    }
}

impl From<VsCodeError> for NimbusError {
    fn from(err: VsCodeError) -> Self {
        NimbusError::VsCode(err)
    }
}

impl From<std::io::Error> for NimbusError {
    fn from(err: std::io::Error) -> Self {
        NimbusError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for NimbusError {
    fn from(err: serde_json::Error) -> Self {
        NimbusError::Json(err.to_string())
    }
}

impl From<toml::de::Error> for NimbusError {
    fn from(err: toml::de::Error) -> Self {
        NimbusError::Toml(err.to_string())
    }
}

impl From<rusqlite::Error> for NimbusError {
    fn from(err: rusqlite::Error) -> Self {
        NimbusError::Database(err.to_string())
    }
}

impl From<anyhow::Error> for NimbusError {
    fn from(err: anyhow::Error) -> Self {
        NimbusError::Anyhow(err.to_string())
    }
}

impl NimbusError {
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            NimbusError::Connection(ConnectionError::PreventiveCheckFailed { .. }) => true,
            NimbusError::Session(SessionError::CreationFailed { .. }) => true,
            NimbusError::Io(_) => true,
            _ => false,
        }
    }
    
    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            NimbusError::Config(_) => ErrorSeverity::High,
            NimbusError::Aws(AwsError::AuthenticationFailed { .. }) => ErrorSeverity::High,
            NimbusError::Resource(_) => ErrorSeverity::High,
            NimbusError::Connection(_) => ErrorSeverity::Medium,
            NimbusError::Session(_) => ErrorSeverity::Medium,
            NimbusError::Ui(_) => ErrorSeverity::Low,
            _ => ErrorSeverity::Medium,
        }
    }
    
    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            NimbusError::Aws(AwsError::AuthenticationFailed { .. }) => {
                "AWS認証に失敗しました。AWS認証情報を確認してください。".to_string()
            },
            NimbusError::Session(SessionError::LimitExceeded { max_sessions }) => {
                format!("セッション数の上限に達しました（最大{}セッション）。", max_sessions)
            },
            NimbusError::VsCode(VsCodeError::NotFound { .. }) => {
                "VS Codeが見つかりません。VS Codeをインストールするか、設定でパスを指定してください。".to_string()
            },
            NimbusError::VsCode(VsCodeError::LaunchFailed { .. }) => {
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
