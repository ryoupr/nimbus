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
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Configuration-related errors
#[derive(Error, Debug, Clone)]
pub enum ConfigError {
    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },
    
    #[error("Invalid configuration: {message}")]
    Invalid { message: String },
    
    #[error("Configuration validation failed: {field}")]
    ValidationFailed { field: String },
    
    #[error("Permission denied accessing config file: {path}")]
    PermissionDenied { path: String },
}

/// AWS-related errors
#[derive(Error, Debug, Clone)]
pub enum AwsError {
    #[error("Authentication failed: {message}")]
    AuthenticationFailed { message: String },
    
    #[error("Invalid credentials")]
    InvalidCredentials,
    
    #[error("Region not found: {region}")]
    RegionNotFound { region: String },
    
    #[error("Instance not found: {instance_id}")]
    InstanceNotFound { instance_id: String },
    
    #[error("SSM service error: {message}")]
    SsmServiceError { message: String },
    
    #[error("EC2 service error: {message}")]
    Ec2ServiceError { message: String },
    
    #[error("Network error: {message}")]
    NetworkError { message: String },
    
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
    
    #[error("Session already exists: {session_id}")]
    AlreadyExists { session_id: String },
    
    #[error("Session limit exceeded: max {max_sessions}")]
    LimitExceeded { max_sessions: u32 },
    
    #[error("Resource limit exceeded for {resource}: {current} > {limit}")]
    ResourceLimitExceeded { resource: String, current: f64, limit: f64 },
    
    #[error("Session unhealthy: {session_id}")]
    Unhealthy { session_id: String },
    
    #[error("Session timeout: {session_id}")]
    Timeout { session_id: String },
    
    #[error("Session terminated: {session_id}")]
    Terminated { session_id: String },
    
    #[error("Reconnection failed: {session_id}, attempts: {attempts}")]
    ReconnectionFailed { session_id: String, attempts: u32 },
}

/// Connection-related errors
#[derive(Error, Debug, Clone)]
pub enum ConnectionError {
    #[error("Connection failed: {target}")]
    Failed { target: String },
    
    #[error("Connection timeout: {target}")]
    Timeout { target: String },
    
    #[error("Connection refused: {target}")]
    Refused { target: String },
    
    #[error("Port already in use: {port}")]
    PortInUse { port: u16 },
    
    #[error("Network unreachable: {target}")]
    NetworkUnreachable { target: String },
    
    #[error("DNS resolution failed: {hostname}")]
    DnsResolutionFailed { hostname: String },
    
    #[error("SSL/TLS error: {message}")]
    SslError { message: String },
    
    #[error("Preventive check failed: {reason}")]
    PreventiveCheckFailed { reason: String, issues: Vec<String> },
}

/// Resource management errors
#[derive(Error, Debug, Clone)]
pub enum ResourceError {
    #[error("Memory limit exceeded: {current_mb}MB > {limit_mb}MB")]
    MemoryLimitExceeded { current_mb: u64, limit_mb: u64 },
    
    #[error("CPU limit exceeded: {current_percent}% > {limit_percent}%")]
    CpuLimitExceeded { current_percent: f64, limit_percent: f64 },
    
    #[error("Disk space insufficient: {available_mb}MB available")]
    DiskSpaceInsufficient { available_mb: u64 },
    
    #[error("Resource monitoring failed: {resource}")]
    MonitoringFailed { resource: String },
    
    #[error("Resource cleanup failed: {resource}")]
    CleanupFailed { resource: String },
}

/// UI-related errors
#[derive(Error, Debug, Clone)]
pub enum UiError {
    #[error("Terminal initialization failed")]
    TerminalInitFailed,
    
    #[error("Terminal rendering failed: {message}")]
    RenderingFailed { message: String },
    
    #[error("Input handling failed: {message}")]
    InputHandlingFailed { message: String },
    
    #[error("UI update failed: {component}")]
    UpdateFailed { component: String },
    
    #[error("Notification failed: {message}")]
    NotificationFailed { message: String },
}

/// VS Code integration errors
#[derive(Error, Debug, Clone)]
pub enum VsCodeError {
    #[error("VS Code not found: {message}")]
    NotFound { message: String },
    
    #[error("VS Code launch failed: {message}")]
    LaunchFailed { message: String },
    
    #[error("SSH configuration error: {message}")]
    SshConfigError { message: String },
    
    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },
    
    #[error("Integration failed: {message}")]
    IntegrationFailed { message: String },
    
    #[error("Notification failed: {message}")]
    NotificationFailed { message: String },
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
            Ec2ConnectError::Connection(ConnectionError::Timeout { .. }) => true,
            Ec2ConnectError::Connection(ConnectionError::NetworkUnreachable { .. }) => true,
            Ec2ConnectError::Connection(ConnectionError::Failed { .. }) => true,
            Ec2ConnectError::Aws(AwsError::NetworkError { .. }) => true,
            Ec2ConnectError::Aws(AwsError::Timeout { .. }) => true,
            Ec2ConnectError::Aws(AwsError::SsmServiceError { .. }) => true,
            Ec2ConnectError::Session(SessionError::Unhealthy { .. }) => true,
            Ec2ConnectError::Session(SessionError::Timeout { .. }) => true,
            Ec2ConnectError::Session(SessionError::CreationFailed { .. }) => true,
            Ec2ConnectError::Io(_) => true, // IO errors are often temporary
            _ => false,
        }
    }
    
    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Ec2ConnectError::Config(_) => ErrorSeverity::High,
            Ec2ConnectError::Aws(AwsError::AuthenticationFailed { .. }) => ErrorSeverity::High,
            Ec2ConnectError::Aws(AwsError::InvalidCredentials) => ErrorSeverity::High,
            Ec2ConnectError::Resource(ResourceError::MemoryLimitExceeded { .. }) => ErrorSeverity::High,
            Ec2ConnectError::Resource(ResourceError::CpuLimitExceeded { .. }) => ErrorSeverity::Medium,
            Ec2ConnectError::Connection(_) => ErrorSeverity::Medium,
            Ec2ConnectError::Session(_) => ErrorSeverity::Medium,
            Ec2ConnectError::Ui(_) => ErrorSeverity::Low,
            _ => ErrorSeverity::Medium,
        }
    }
    
    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            Ec2ConnectError::Config(ConfigError::FileNotFound { path }) => {
                format!("設定ファイルが見つかりません: {}\nデフォルト設定で続行します。", path)
            },
            Ec2ConnectError::Aws(AwsError::AuthenticationFailed { .. }) => {
                "AWS認証に失敗しました。AWS認証情報を確認してください。".to_string()
            },
            Ec2ConnectError::Aws(AwsError::InstanceNotFound { instance_id }) => {
                format!("EC2インスタンスが見つかりません: {}\nインスタンスIDを確認してください。", instance_id)
            },
            Ec2ConnectError::Connection(ConnectionError::PortInUse { port }) => {
                format!("ポート{}は既に使用されています。別のポートを指定してください。", port)
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
            Ec2ConnectError::VsCode(VsCodeError::SshConfigError { .. }) => {
                "SSH設定の更新に失敗しました。~/.ssh/configファイルの権限を確認してください。".to_string()
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
    Critical,
}

impl ErrorSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorSeverity::Low => "LOW",
            ErrorSeverity::Medium => "MEDIUM",
            ErrorSeverity::High => "HIGH",
            ErrorSeverity::Critical => "CRITICAL",
        }
    }
}