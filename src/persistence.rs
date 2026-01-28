use crate::error::Result;
use crate::session::SessionStatus;
use rusqlite::{Connection, params, Row, OptionalExtension, backup::Backup};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use tracing::{info, warn, debug};
use async_trait::async_trait;

/// Database schema version for migration management
const CURRENT_SCHEMA_VERSION: i32 = 2;

/// Persistent session data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentSession {
    pub session_id: String,
    pub instance_id: String,
    pub region: String,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub connection_count: u32,
    pub total_duration_seconds: u64,
    pub process_id: Option<u32>,
    pub is_stale: bool,
    pub recovery_attempts: u32,
}

/// Persistent performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentPerformanceMetrics {
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub connection_time_ms: u64,
    pub latency_ms: u64,
    pub throughput_mbps: f64,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
}

/// Database migration information
#[derive(Debug, Clone)]
pub struct MigrationInfo {
    pub version: i32,
    pub description: String,
    pub applied_at: DateTime<Utc>,
}

/// Trait for session and metrics persistence
#[async_trait]
pub trait PersistenceManager {
    /// Initialize the database and run migrations
    async fn initialize(&self) -> Result<()>;
    
    /// Save session state
    async fn save_session(&self, session: &PersistentSession) -> Result<()>;
    
    /// Load session by ID
    async fn load_session(&self, session_id: &str) -> Result<Option<PersistentSession>>;
    
    /// Load all active sessions
    async fn load_active_sessions(&self) -> Result<Vec<PersistentSession>>;
    
    /// Update session status
    async fn update_session_status(&self, session_id: &str, status: SessionStatus) -> Result<()>;
    
    /// Update session activity
    async fn update_session_activity(&self, session_id: &str) -> Result<()>;
    
    /// Delete session
    async fn delete_session(&self, session_id: &str) -> Result<()>;
    
    /// Save performance metrics
    async fn save_performance_metrics(&self, metrics: &PersistentPerformanceMetrics) -> Result<()>;
    
    /// Load performance metrics for session
    async fn load_performance_metrics(&self, session_id: &str, limit: Option<u32>) -> Result<Vec<PersistentPerformanceMetrics>>;
    
    /// Get performance statistics
    async fn get_performance_statistics(&self, session_id: &str) -> Result<PerformanceStatistics>;
    
    /// Clean up old data
    async fn cleanup_old_data(&self, retention_days: u32) -> Result<u32>;
    
    /// Get database info
    async fn get_database_info(&self) -> Result<DatabaseInfo>;
    
    /// Application restart recovery methods
    
    /// Mark sessions as potentially stale on application startup
    async fn mark_sessions_as_stale(&self) -> Result<u32>;
    
    /// Restore session state after application restart
    async fn restore_session_state(&self, session_id: &str) -> Result<Option<SessionRecoveryInfo>>;
    
    /// Save application state for crash recovery
    async fn save_application_state(&self, state: &ApplicationState) -> Result<()>;
    
    /// Load application state for recovery
    async fn load_application_state(&self) -> Result<Option<ApplicationState>>;
    
    /// Backup database to specified path
    async fn backup_database(&self, backup_path: &std::path::Path) -> Result<()>;
    
    /// Restore database from backup
    async fn restore_from_backup(&self, backup_path: &std::path::Path) -> Result<()>;
    
    /// Validate database integrity
    async fn validate_integrity(&self) -> Result<IntegrityReport>;
}

/// SQLite implementation of persistence manager
pub struct SqlitePersistenceManager {
    db_path: PathBuf,
}

impl SqlitePersistenceManager {
    /// Create new SQLite persistence manager
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }
    
    /// Create with default database path
    pub fn with_default_path() -> Result<Self> {
        let db_dir = dirs::data_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
            .ok_or_else(|| crate::error::NimbusError::Config(
                crate::error::ConfigError::Invalid { 
                    message: "Cannot determine data directory".to_string() 
                }
            ))?
            .join("nimbus");
        
        std::fs::create_dir_all(&db_dir)?;
        let db_path = db_dir.join("sessions.db");
        
        Ok(Self::new(db_path))
    }
    
    /// Get database connection
    fn get_connection(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        
        // Enable foreign keys - this PRAGMA doesn't return results
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        
        // Set WAL mode - this PRAGMA returns the mode, so we need to use query_row or ignore the result
        conn.execute("PRAGMA journal_mode = WAL", []).or_else(|e| {
            match e {
                rusqlite::Error::ExecuteReturnedResults => {
                    // This is expected for journal_mode pragma, ignore it
                    Ok(0)
                },
                _ => Err(e)
            }
        })?;
        
        // Set synchronous mode - this might return results too
        conn.execute("PRAGMA synchronous = NORMAL", []).or_else(|e| {
            match e {
                rusqlite::Error::ExecuteReturnedResults => {
                    // This is expected, ignore it
                    Ok(0)
                },
                _ => Err(e)
            }
        })?;
        
        // Set cache size - this might return results too
        conn.execute("PRAGMA cache_size = 10000", []).or_else(|e| {
            match e {
                rusqlite::Error::ExecuteReturnedResults => {
                    // This is expected, ignore it
                    Ok(0)
                },
                _ => Err(e)
            }
        })?;
        
        Ok(conn)
    }
    
    /// Create database tables
    fn create_tables(&self, conn: &Connection) -> Result<()> {
        // Sessions table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                region TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_activity TEXT NOT NULL,
                connection_count INTEGER DEFAULT 0,
                total_duration_seconds INTEGER DEFAULT 0,
                process_id INTEGER,
                is_stale BOOLEAN DEFAULT 0,
                recovery_attempts INTEGER DEFAULT 0
            )",
            [],
        )?;
        
        // Performance metrics table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS performance_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                connection_time_ms INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL,
                throughput_mbps REAL NOT NULL,
                cpu_usage_percent REAL NOT NULL,
                memory_usage_mb REAL NOT NULL,
                network_bytes_sent INTEGER NOT NULL,
                network_bytes_received INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions (session_id) ON DELETE CASCADE
            )",
            [],
        )?;
        
        // Application state table for crash recovery
        conn.execute(
            "CREATE TABLE IF NOT EXISTS application_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                startup_time TEXT NOT NULL,
                last_heartbeat TEXT NOT NULL,
                active_session_count INTEGER NOT NULL,
                total_memory_usage_mb REAL NOT NULL,
                configuration_hash TEXT NOT NULL,
                recovery_mode BOOLEAN DEFAULT 0
            )",
            [],
        )?;
        
        // Create indexes for better performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions (status)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_last_activity ON sessions (last_activity)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_stale ON sessions (is_stale)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_performance_session_timestamp 
             ON performance_metrics (session_id, timestamp)",
            [],
        )?;
        
        Ok(())
    }
    
    /// Run database migrations
    fn run_migrations(&self, conn: &Connection) -> Result<()> {
        // Get current schema version - schema_version table is already created in initialize()
        // Use query_row which is designed for SELECT statements that return rows
        let current_version: i32 = match conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        ) {
            Ok(version) => version,
            Err(rusqlite::Error::QueryReturnedNoRows) => 0,
            Err(e) => return Err(e.into()),
        };
        
        info!("Current database schema version: {}", current_version);
        
        if current_version < CURRENT_SCHEMA_VERSION {
            info!("Running database migrations from version {} to {}", 
                current_version, CURRENT_SCHEMA_VERSION);
            
            // Migration from version 0 to 1 (initial schema)
            if current_version < 1 {
                self.migrate_to_version_1(conn)?;
            }
            
            // Migration from version 1 to 2 (add recovery fields)
            if current_version < 2 {
                self.migrate_to_version_2(conn)?;
            }
            
            // Future migrations would go here
            // if current_version < 3 {
            //     self.migrate_to_version_3(conn)?;
            // }
            
            info!("Database migrations completed successfully");
        }
        
        Ok(())
    }
    
    /// Migrate to version 1 (initial schema)
    fn migrate_to_version_1(&self, conn: &Connection) -> Result<()> {
        info!("Migrating database to version 1");
        
        // Create the main tables (without new fields)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL,
                region TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_activity TEXT NOT NULL,
                connection_count INTEGER DEFAULT 0,
                total_duration_seconds INTEGER DEFAULT 0
            )",
            [],
        )?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS performance_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                connection_time_ms INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL,
                throughput_mbps REAL NOT NULL,
                cpu_usage_percent REAL NOT NULL,
                memory_usage_mb REAL NOT NULL,
                network_bytes_sent INTEGER NOT NULL,
                network_bytes_received INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions (session_id) ON DELETE CASCADE
            )",
            [],
        )?;
        
        // Create basic indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions (status)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_last_activity ON sessions (last_activity)",
            [],
        )?;
        
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_performance_session_timestamp 
             ON performance_metrics (session_id, timestamp)",
            [],
        )?;
        
        // Record the migration - use execute for INSERT statements
        conn.execute(
            "INSERT INTO schema_version (version, description, applied_at) VALUES (?, ?, ?)",
            params![1, "Initial schema with sessions and performance metrics", Utc::now().to_rfc3339()],
        )?;
        
        Ok(())
    }
    
    /// Migrate to version 2 (add recovery and application state features)
    fn migrate_to_version_2(&self, conn: &Connection) -> Result<()> {
        info!("Migrating database to version 2");
        
        // Add new columns to sessions table
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN process_id INTEGER",
            [],
        ).or_else(|e| {
            // Column might already exist, check if it's a duplicate column error
            match e {
                rusqlite::Error::SqliteFailure(err, _) if err.code == rusqlite::ErrorCode::Unknown => {
                    // This might be a "duplicate column" error, which is OK
                    warn!("Column process_id might already exist, continuing...");
                    Ok(0)
                },
                _ => Err(e)
            }
        })?;
        
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN is_stale BOOLEAN DEFAULT 0",
            [],
        ).or_else(|e| {
            match e {
                rusqlite::Error::SqliteFailure(err, _) if err.code == rusqlite::ErrorCode::Unknown => {
                    warn!("Column is_stale might already exist, continuing...");
                    Ok(0)
                },
                _ => Err(e)
            }
        })?;
        
        conn.execute(
            "ALTER TABLE sessions ADD COLUMN recovery_attempts INTEGER DEFAULT 0",
            [],
        ).or_else(|e| {
            match e {
                rusqlite::Error::SqliteFailure(err, _) if err.code == rusqlite::ErrorCode::Unknown => {
                    warn!("Column recovery_attempts might already exist, continuing...");
                    Ok(0)
                },
                _ => Err(e)
            }
        })?;
        
        // Create application state table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS application_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                startup_time TEXT NOT NULL,
                last_heartbeat TEXT NOT NULL,
                active_session_count INTEGER NOT NULL,
                total_memory_usage_mb REAL NOT NULL,
                configuration_hash TEXT NOT NULL,
                recovery_mode BOOLEAN DEFAULT 0
            )",
            [],
        )?;
        
        // Create new indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_sessions_stale ON sessions (is_stale)",
            [],
        )?;
        
        // Record the migration
        conn.execute(
            "INSERT INTO schema_version (version, description, applied_at) VALUES (?, ?, ?)",
            params![2, "Add recovery features and application state management", Utc::now().to_rfc3339()],
        )?;
        
        Ok(())
    }
    
    /// Convert database row to PersistentSession
    fn row_to_session(row: &Row) -> rusqlite::Result<PersistentSession> {
        Ok(PersistentSession {
            session_id: row.get("session_id")?,
            instance_id: row.get("instance_id")?,
            region: row.get("region")?,
            status: SessionStatus::from_str(&row.get::<_, String>("status")?),
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>("created_at")?)
                .map_err(|_e| rusqlite::Error::InvalidColumnType(0, "created_at".to_string(), rusqlite::types::Type::Text))?
                .with_timezone(&Utc),
            last_activity: DateTime::parse_from_rfc3339(&row.get::<_, String>("last_activity")?)
                .map_err(|_e| rusqlite::Error::InvalidColumnType(0, "last_activity".to_string(), rusqlite::types::Type::Text))?
                .with_timezone(&Utc),
            connection_count: row.get("connection_count")?,
            total_duration_seconds: row.get("total_duration_seconds")?,
            process_id: row.get("process_id")?,
            is_stale: row.get("is_stale")?,
            recovery_attempts: row.get("recovery_attempts")?,
        })
    }
    
    /// Convert database row to PersistentPerformanceMetrics
    fn row_to_performance_metrics(row: &Row) -> rusqlite::Result<PersistentPerformanceMetrics> {
        Ok(PersistentPerformanceMetrics {
            session_id: row.get("session_id")?,
            timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>("timestamp")?)
                .map_err(|_e| rusqlite::Error::InvalidColumnType(0, "timestamp".to_string(), rusqlite::types::Type::Text))?
                .with_timezone(&Utc),
            connection_time_ms: row.get("connection_time_ms")?,
            latency_ms: row.get("latency_ms")?,
            throughput_mbps: row.get("throughput_mbps")?,
            cpu_usage_percent: row.get("cpu_usage_percent")?,
            memory_usage_mb: row.get("memory_usage_mb")?,
            network_bytes_sent: row.get("network_bytes_sent")?,
            network_bytes_received: row.get("network_bytes_received")?,
        })
    }
}

#[async_trait]
impl PersistenceManager for SqlitePersistenceManager {
    /// Initialize the database and run migrations
    async fn initialize(&self) -> Result<()> {
        info!("Initializing SQLite database at: {:?}", self.db_path);
        
        // Ensure parent directory exists
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let conn = self.get_connection()?;
        
        // First create the schema_version table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TEXT NOT NULL
            )",
            [],
        )?;
        
        // Then run migrations which will create other tables
        self.run_migrations(&conn)?;
        
        info!("Database initialization completed successfully");
        Ok(())
    }
    
    /// Save session state
    async fn save_session(&self, session: &PersistentSession) -> Result<()> {
        debug!("Saving session: {}", session.session_id);
        
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO sessions 
             (session_id, instance_id, region, status, created_at, last_activity, 
              connection_count, total_duration_seconds, process_id, is_stale, recovery_attempts)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                session.session_id,
                session.instance_id,
                session.region,
                session.status.to_string(),
                session.created_at.to_rfc3339(),
                session.last_activity.to_rfc3339(),
                session.connection_count,
                session.total_duration_seconds,
                session.process_id,
                session.is_stale,
                session.recovery_attempts
            ],
        )?;
        
        debug!("Session saved successfully: {}", session.session_id);
        Ok(())
    }
    
    /// Load session by ID
    async fn load_session(&self, session_id: &str) -> Result<Option<PersistentSession>> {
        debug!("Loading session: {}", session_id);
        
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT session_id, instance_id, region, status, created_at, last_activity, 
                    connection_count, total_duration_seconds, process_id, is_stale, recovery_attempts
             FROM sessions WHERE session_id = ?"
        )?;
        
        let session = stmt.query_row(params![session_id], Self::row_to_session)
            .optional()?;
        
        if session.is_some() {
            debug!("Session loaded successfully: {}", session_id);
        } else {
            debug!("Session not found: {}", session_id);
        }
        
        Ok(session)
    }
    
    /// Load all active sessions
    async fn load_active_sessions(&self) -> Result<Vec<PersistentSession>> {
        debug!("Loading active sessions");
        
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT session_id, instance_id, region, status, created_at, last_activity,
                    connection_count, total_duration_seconds, process_id, is_stale, recovery_attempts
             FROM sessions 
             WHERE status IN ('Active', 'Connecting', 'Reconnecting')
             ORDER BY last_activity DESC"
        )?;
        
        let sessions: Result<Vec<_>> = stmt.query_map([], Self::row_to_session)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into);
        
        let sessions = sessions?;
        info!("Loaded {} active sessions", sessions.len());
        
        Ok(sessions)
    }
    
    /// Update session status
    async fn update_session_status(&self, session_id: &str, status: SessionStatus) -> Result<()> {
        debug!("Updating session status: {} -> {:?}", session_id, status);
        
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "UPDATE sessions SET status = ?, last_activity = ? WHERE session_id = ?",
            params![status.to_string(), Utc::now().to_rfc3339(), session_id],
        )?;
        
        if rows_affected == 0 {
            warn!("No session found to update status: {}", session_id);
        } else {
            debug!("Session status updated successfully: {}", session_id);
        }
        
        Ok(())
    }
    
    /// Update session activity
    async fn update_session_activity(&self, session_id: &str) -> Result<()> {
        debug!("Updating session activity: {}", session_id);
        
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "UPDATE sessions SET last_activity = ? WHERE session_id = ?",
            params![Utc::now().to_rfc3339(), session_id],
        )?;
        
        if rows_affected == 0 {
            warn!("No session found to update activity: {}", session_id);
        }
        
        Ok(())
    }
    
    /// Delete session
    async fn delete_session(&self, session_id: &str) -> Result<()> {
        debug!("Deleting session: {}", session_id);
        
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "DELETE FROM sessions WHERE session_id = ?",
            params![session_id],
        )?;
        
        if rows_affected == 0 {
            warn!("No session found to delete: {}", session_id);
        } else {
            info!("Session deleted successfully: {}", session_id);
        }
        
        Ok(())
    }
    
    /// Save performance metrics
    async fn save_performance_metrics(&self, metrics: &PersistentPerformanceMetrics) -> Result<()> {
        debug!("Saving performance metrics for session: {}", metrics.session_id);
        
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO performance_metrics 
             (session_id, timestamp, connection_time_ms, latency_ms, throughput_mbps,
              cpu_usage_percent, memory_usage_mb, network_bytes_sent, network_bytes_received)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                metrics.session_id,
                metrics.timestamp.to_rfc3339(),
                metrics.connection_time_ms,
                metrics.latency_ms,
                metrics.throughput_mbps,
                metrics.cpu_usage_percent,
                metrics.memory_usage_mb,
                metrics.network_bytes_sent,
                metrics.network_bytes_received
            ],
        )?;
        
        debug!("Performance metrics saved successfully for session: {}", metrics.session_id);
        Ok(())
    }
    
    /// Load performance metrics for session
    async fn load_performance_metrics(&self, session_id: &str, limit: Option<u32>) -> Result<Vec<PersistentPerformanceMetrics>> {
        debug!("Loading performance metrics for session: {} (limit: {:?})", session_id, limit);
        
        let conn = self.get_connection()?;
        let query = if let Some(limit) = limit {
            format!(
                "SELECT session_id, timestamp, connection_time_ms, latency_ms, throughput_mbps,
                        cpu_usage_percent, memory_usage_mb, network_bytes_sent, network_bytes_received
                 FROM performance_metrics 
                 WHERE session_id = ? 
                 ORDER BY timestamp DESC 
                 LIMIT {}",
                limit
            )
        } else {
            "SELECT session_id, timestamp, connection_time_ms, latency_ms, throughput_mbps,
                    cpu_usage_percent, memory_usage_mb, network_bytes_sent, network_bytes_received
             FROM performance_metrics 
             WHERE session_id = ? 
             ORDER BY timestamp DESC".to_string()
        };
        
        let mut stmt = conn.prepare(&query)?;
        let metrics: Result<Vec<_>> = stmt.query_map(params![session_id], Self::row_to_performance_metrics)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into);
        
        let metrics = metrics?;
        debug!("Loaded {} performance metrics for session: {}", metrics.len(), session_id);
        
        Ok(metrics)
    }
    
    /// Get performance statistics
    async fn get_performance_statistics(&self, session_id: &str) -> Result<PerformanceStatistics> {
        debug!("Getting performance statistics for session: {}", session_id);
        
        let conn = self.get_connection()?;
        let stats = conn.query_row(
            "SELECT 
                COUNT(*) as count,
                AVG(connection_time_ms) as avg_connection_time,
                MIN(connection_time_ms) as min_connection_time,
                MAX(connection_time_ms) as max_connection_time,
                AVG(latency_ms) as avg_latency,
                MIN(latency_ms) as min_latency,
                MAX(latency_ms) as max_latency,
                AVG(throughput_mbps) as avg_throughput,
                MAX(throughput_mbps) as max_throughput,
                AVG(cpu_usage_percent) as avg_cpu_usage,
                MAX(cpu_usage_percent) as max_cpu_usage,
                AVG(memory_usage_mb) as avg_memory_usage,
                MAX(memory_usage_mb) as max_memory_usage
             FROM performance_metrics 
             WHERE session_id = ?",
            params![session_id],
            |row| {
                Ok(PerformanceStatistics {
                    session_id: session_id.to_string(),
                    total_measurements: row.get::<_, i64>("count")? as u32,
                    avg_connection_time_ms: row.get::<_, Option<f64>>("avg_connection_time")?.unwrap_or(0.0) as u64,
                    min_connection_time_ms: row.get::<_, Option<i64>>("min_connection_time")?.unwrap_or(0) as u64,
                    max_connection_time_ms: row.get::<_, Option<i64>>("max_connection_time")?.unwrap_or(0) as u64,
                    avg_latency_ms: row.get::<_, Option<f64>>("avg_latency")?.unwrap_or(0.0) as u64,
                    min_latency_ms: row.get::<_, Option<i64>>("min_latency")?.unwrap_or(0) as u64,
                    max_latency_ms: row.get::<_, Option<i64>>("max_latency")?.unwrap_or(0) as u64,
                    avg_throughput_mbps: row.get::<_, Option<f64>>("avg_throughput")?.unwrap_or(0.0),
                    max_throughput_mbps: row.get::<_, Option<f64>>("max_throughput")?.unwrap_or(0.0),
                    avg_cpu_usage_percent: row.get::<_, Option<f64>>("avg_cpu_usage")?.unwrap_or(0.0),
                    max_cpu_usage_percent: row.get::<_, Option<f64>>("max_cpu_usage")?.unwrap_or(0.0),
                    avg_memory_usage_mb: row.get::<_, Option<f64>>("avg_memory_usage")?.unwrap_or(0.0),
                    max_memory_usage_mb: row.get::<_, Option<f64>>("max_memory_usage")?.unwrap_or(0.0),
                })
            },
        )?;
        
        debug!("Performance statistics calculated for session: {}", session_id);
        Ok(stats)
    }
    
    /// Clean up old data
    async fn cleanup_old_data(&self, retention_days: u32) -> Result<u32> {
        info!("Cleaning up data older than {} days", retention_days);
        
        let cutoff_date = Utc::now() - chrono::Duration::days(retention_days as i64);
        let conn = self.get_connection()?;
        
        // Delete old performance metrics
        let metrics_deleted = conn.execute(
            "DELETE FROM performance_metrics WHERE timestamp < ?",
            params![cutoff_date.to_rfc3339()],
        )?;
        
        // Delete old inactive sessions
        let sessions_deleted = conn.execute(
            "DELETE FROM sessions 
             WHERE last_activity < ? 
             AND status NOT IN ('Active', 'Connecting', 'Reconnecting')",
            params![cutoff_date.to_rfc3339()],
        )?;
        
        let total_deleted = metrics_deleted + sessions_deleted;
        info!("Cleanup completed: {} records deleted ({} metrics, {} sessions)", 
            total_deleted, metrics_deleted, sessions_deleted);
        
        Ok(total_deleted as u32)
    }
    
    /// Get database info
    async fn get_database_info(&self) -> Result<DatabaseInfo> {
        debug!("Getting database information");
        
        let conn = self.get_connection()?;
        
        // Get schema version
        let schema_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        
        // Get table counts
        let session_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap_or(0);
        
        let metrics_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM performance_metrics", [], |row| row.get(0))
            .unwrap_or(0);
        
        // Get database file size
        let file_size = std::fs::metadata(&self.db_path)
            .map(|m| m.len())
            .unwrap_or(0);
        
        let info = DatabaseInfo {
            db_path: self.db_path.clone(),
            schema_version,
            session_count: session_count as u32,
            metrics_count: metrics_count as u32,
            file_size_bytes: file_size,
        };
        
        debug!("Database info: {:?}", info);
        Ok(info)
    }
    
    /// Mark sessions as potentially stale on application startup
    async fn mark_sessions_as_stale(&self) -> Result<u32> {
        info!("Marking active sessions as potentially stale");
        
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "UPDATE sessions SET is_stale = 1 
             WHERE status IN ('Active', 'Connecting', 'Reconnecting')",
            [],
        )?;
        
        info!("Marked {} sessions as potentially stale", rows_affected);
        Ok(rows_affected as u32)
    }
    
    /// Restore session state after application restart
    async fn restore_session_state(&self, session_id: &str) -> Result<Option<SessionRecoveryInfo>> {
        debug!("Restoring session state for: {}", session_id);
        
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT session_id, instance_id, region, status, created_at, last_activity,
                    connection_count, total_duration_seconds, process_id, recovery_attempts
             FROM sessions WHERE session_id = ?"
        )?;
        
        let recovery_info = stmt.query_row(params![session_id], |row| {
            let session = PersistentSession {
                session_id: row.get("session_id")?,
                instance_id: row.get("instance_id")?,
                region: row.get("region")?,
                status: SessionStatus::from_str(&row.get::<_, String>("status")?),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>("created_at")?)
                    .map_err(|_e| rusqlite::Error::InvalidColumnType(0, "created_at".to_string(), rusqlite::types::Type::Text))?
                    .with_timezone(&Utc),
                last_activity: DateTime::parse_from_rfc3339(&row.get::<_, String>("last_activity")?)
                    .map_err(|_e| rusqlite::Error::InvalidColumnType(0, "last_activity".to_string(), rusqlite::types::Type::Text))?
                    .with_timezone(&Utc),
                connection_count: row.get("connection_count")?,
                total_duration_seconds: row.get("total_duration_seconds")?,
                process_id: row.get("process_id")?,
                is_stale: false, // Default value for recovery info
                recovery_attempts: row.get("recovery_attempts")?,
            };
            
            let process_id = session.process_id;
            let recovery_attempts = session.recovery_attempts;
            
            // Determine recovery actions based on session state
            let mut recovery_actions = Vec::new();
            
            if process_id.is_some() {
                recovery_actions.push(RecoveryAction::ValidateProcess);
            }
            
            recovery_actions.push(RecoveryAction::CheckPortBinding);
            
            if recovery_attempts < 3 {
                recovery_actions.push(RecoveryAction::AttemptReconnection);
            } else {
                recovery_actions.push(RecoveryAction::MarkAsTerminated);
            }
            
            let estimated_uptime = Utc::now()
                .signed_duration_since(session.created_at)
                .num_seconds()
                .max(0) as u64;
            
            Ok(SessionRecoveryInfo {
                session,
                last_known_process_id: process_id,
                recovery_actions,
                estimated_uptime_seconds: estimated_uptime,
            })
        }).optional()?;
        
        if recovery_info.is_some() {
            debug!("Session recovery info prepared for: {}", session_id);
        } else {
            debug!("No session found for recovery: {}", session_id);
        }
        
        Ok(recovery_info)
    }
    
    /// Save application state for crash recovery
    async fn save_application_state(&self, state: &ApplicationState) -> Result<()> {
        debug!("Saving application state");
        
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO application_state 
             (id, startup_time, last_heartbeat, active_session_count, total_memory_usage_mb, 
              configuration_hash, recovery_mode)
             VALUES (1, ?, ?, ?, ?, ?, ?)",
            params![
                state.startup_time.to_rfc3339(),
                state.last_heartbeat.to_rfc3339(),
                state.active_session_count,
                state.total_memory_usage_mb,
                state.configuration_hash,
                state.recovery_mode
            ],
        )?;
        
        debug!("Application state saved successfully");
        Ok(())
    }
    
    /// Load application state for recovery
    async fn load_application_state(&self) -> Result<Option<ApplicationState>> {
        debug!("Loading application state");
        
        let conn = self.get_connection()?;
        let state = conn.query_row(
            "SELECT startup_time, last_heartbeat, active_session_count, 
                    total_memory_usage_mb, configuration_hash, recovery_mode
             FROM application_state WHERE id = 1",
            [],
            |row| {
                Ok(ApplicationState {
                    startup_time: DateTime::parse_from_rfc3339(&row.get::<_, String>("startup_time")?)
                        .map_err(|_e| rusqlite::Error::InvalidColumnType(0, "startup_time".to_string(), rusqlite::types::Type::Text))?
                        .with_timezone(&Utc),
                    last_heartbeat: DateTime::parse_from_rfc3339(&row.get::<_, String>("last_heartbeat")?)
                        .map_err(|_e| rusqlite::Error::InvalidColumnType(0, "last_heartbeat".to_string(), rusqlite::types::Type::Text))?
                        .with_timezone(&Utc),
                    active_session_count: row.get("active_session_count")?,
                    total_memory_usage_mb: row.get("total_memory_usage_mb")?,
                    configuration_hash: row.get("configuration_hash")?,
                    recovery_mode: row.get("recovery_mode")?,
                })
            },
        ).optional()?;
        
        if state.is_some() {
            debug!("Application state loaded successfully");
        } else {
            debug!("No application state found");
        }
        
        Ok(state)
    }
    
    /// Backup database to specified path
    async fn backup_database(&self, backup_path: &std::path::Path) -> Result<()> {
        info!("Creating database backup at: {:?}", backup_path);
        
        // Ensure backup directory exists
        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Create backup using SQLite backup API
        let source_conn = self.get_connection()?;
        let mut backup_conn = Connection::open(backup_path)?;
        
        let backup = Backup::new(&source_conn, &mut backup_conn)?;
        backup.run_to_completion(5, std::time::Duration::from_millis(250), None)?;
        
        info!("Database backup completed successfully");
        Ok(())
    }
    
    /// Restore database from backup
    async fn restore_from_backup(&self, backup_path: &std::path::Path) -> Result<()> {
        info!("Restoring database from backup: {:?}", backup_path);
        
        if !backup_path.exists() {
            return Err(crate::error::NimbusError::Config(
                crate::error::ConfigError::Invalid {
                    message: format!("Backup file not found: {:?}", backup_path),
                }
            ).into());
        }
        
        // Create backup of current database before restore
        let current_backup_path = self.db_path.with_extension("db.pre-restore");
        if self.db_path.exists() {
            std::fs::copy(&self.db_path, &current_backup_path)?;
            info!("Current database backed up to: {:?}", current_backup_path);
        }
        
        // Restore from backup
        let backup_conn = Connection::open(backup_path)?;
        let mut target_conn = Connection::open(&self.db_path)?;
        
        let backup = Backup::new(&backup_conn, &mut target_conn)?;
        backup.run_to_completion(5, std::time::Duration::from_millis(250), None)?;
        
        info!("Database restored from backup successfully");
        Ok(())
    }
    
    /// Validate database integrity
    async fn validate_integrity(&self) -> Result<IntegrityReport> {
        info!("Validating database integrity");
        
        let conn = self.get_connection()?;
        let mut issues = Vec::new();
        let mut recommendations = Vec::new();
        
        // Check database integrity
        let integrity_check: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        
        if integrity_check != "ok" {
            issues.push(IntegrityIssue {
                severity: IssueSeverity::Critical,
                description: format!("Database integrity check failed: {}", integrity_check),
                table_name: None,
                suggested_fix: Some("Consider restoring from backup".to_string()),
            });
        }
        
        // Check foreign key constraints
        let fk_violations: Vec<String> = conn.prepare("PRAGMA foreign_key_check")?
            .query_map([], |row| {
                Ok(format!("Foreign key violation in table: {}", row.get::<_, String>(0)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        
        for violation in fk_violations {
            issues.push(IntegrityIssue {
                severity: IssueSeverity::Error,
                description: violation,
                table_name: None,
                suggested_fix: Some("Clean up orphaned records".to_string()),
            });
        }
        
        // Check for orphaned performance metrics
        let orphaned_metrics: i64 = conn.query_row(
            "SELECT COUNT(*) FROM performance_metrics pm 
             LEFT JOIN sessions s ON pm.session_id = s.session_id 
             WHERE s.session_id IS NULL",
            [],
            |row| row.get(0),
        )?;
        
        if orphaned_metrics > 0 {
            issues.push(IntegrityIssue {
                severity: IssueSeverity::Warning,
                description: format!("{} orphaned performance metrics found", orphaned_metrics),
                table_name: Some("performance_metrics".to_string()),
                suggested_fix: Some("Run cleanup_old_data() to remove orphaned records".to_string()),
            });
            recommendations.push("Consider running regular data cleanup".to_string());
        }
        
        // Check for very old sessions
        let old_sessions: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sessions 
             WHERE last_activity < datetime('now', '-30 days')",
            [],
            |row| row.get(0),
        )?;
        
        if old_sessions > 0 {
            issues.push(IntegrityIssue {
                severity: IssueSeverity::Info,
                description: format!("{} sessions older than 30 days found", old_sessions),
                table_name: Some("sessions".to_string()),
                suggested_fix: Some("Consider archiving or removing old sessions".to_string()),
            });
            recommendations.push("Set up automatic data retention policy".to_string());
        }
        
        let is_valid = !issues.iter().any(|issue| matches!(issue.severity, IssueSeverity::Critical | IssueSeverity::Error));
        
        let report = IntegrityReport {
            is_valid,
            issues,
            recommendations,
            last_check: Utc::now(),
        };
        
        info!("Database integrity validation completed. Valid: {}", report.is_valid);
        Ok(report)
    }
}

/// Performance statistics summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStatistics {
    pub session_id: String,
    pub total_measurements: u32,
    pub avg_connection_time_ms: u64,
    pub min_connection_time_ms: u64,
    pub max_connection_time_ms: u64,
    pub avg_latency_ms: u64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub avg_throughput_mbps: f64,
    pub max_throughput_mbps: f64,
    pub avg_cpu_usage_percent: f64,
    pub max_cpu_usage_percent: f64,
    pub avg_memory_usage_mb: f64,
    pub max_memory_usage_mb: f64,
}

/// Database information
#[derive(Debug, Clone)]
pub struct DatabaseInfo {
    pub db_path: PathBuf,
    pub schema_version: i32,
    pub session_count: u32,
    pub metrics_count: u32,
    pub file_size_bytes: u64,
}

/// Session recovery information for application restart
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecoveryInfo {
    pub session: PersistentSession,
    pub last_known_process_id: Option<u32>,
    pub recovery_actions: Vec<RecoveryAction>,
    pub estimated_uptime_seconds: u64,
}

/// Recovery actions to take for a session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecoveryAction {
    ValidateProcess,
    CheckPortBinding,
    AttemptReconnection,
    MarkAsTerminated,
}

/// Application state for crash recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationState {
    pub startup_time: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
    pub active_session_count: u32,
    pub total_memory_usage_mb: f64,
    pub configuration_hash: String,
    pub recovery_mode: bool,
}

/// Database integrity report
#[derive(Debug, Clone)]
pub struct IntegrityReport {
    pub is_valid: bool,
    pub issues: Vec<IntegrityIssue>,
    pub recommendations: Vec<String>,
    pub last_check: DateTime<Utc>,
}

/// Database integrity issue
#[derive(Debug, Clone)]
pub struct IntegrityIssue {
    pub severity: IssueSeverity,
    pub description: String,
    pub table_name: Option<String>,
    pub suggested_fix: Option<String>,
}

/// Issue severity levels
#[derive(Debug, Clone)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Helper trait for SessionStatus string conversion
impl SessionStatus {
    pub fn from_str(s: &str) -> Self {
        match s {
            "Active" => SessionStatus::Active,
            "Connecting" => SessionStatus::Connecting,
            "Inactive" => SessionStatus::Inactive,
            "Reconnecting" => SessionStatus::Reconnecting,
            "Terminated" => SessionStatus::Terminated,
            _ => SessionStatus::Inactive, // Default fallback
        }
    }
    
    pub fn to_string(&self) -> String {
        match self {
            SessionStatus::Active => "Active".to_string(),
            SessionStatus::Connecting => "Connecting".to_string(),
            SessionStatus::Inactive => "Inactive".to_string(),
            SessionStatus::Reconnecting => "Reconnecting".to_string(),
            SessionStatus::Terminated => "Terminated".to_string(),
        }
    }
}