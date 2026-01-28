// Criterion benchmarks for Nimbus v3.0
// Task 17: Final Integration and Performance Testing
// Requirements: 5.1, 5.2

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use nimbus::*;
use nimbus::manager::{SessionManager, DefaultSessionManager};
use nimbus::health::{HealthChecker, DefaultHealthChecker};
use nimbus::persistence::{PersistenceManager, SqlitePersistenceManager, PersistentSession};
use nimbus::session::{SessionStatus, SessionConfig};
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark session manager operations
fn bench_session_manager(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("session_manager");
    
    // Benchmark session search
    group.bench_function("find_existing_sessions", |b| {
        b.iter(|| {
            rt.block_on(async {
                let session_manager = DefaultSessionManager::new(3).await
                    .expect("Failed to create session manager");
                session_manager.find_existing_sessions(
                    black_box("i-test123456789"),
                    black_box(8080)
                ).await.unwrap()
            })
        });
    });
    
    // Benchmark resource monitoring
    group.bench_function("monitor_resource_usage", |b| {
        b.iter(|| {
            rt.block_on(async {
                let session_manager = DefaultSessionManager::new(3).await
                    .expect("Failed to create session manager");
                session_manager.monitor_resource_usage().await.unwrap()
            })
        });
    });
    
    group.finish();
}

/// Benchmark resource monitor operations
fn bench_resource_monitor(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("resource_monitor");
    
    // Benchmark current usage check
    group.bench_function("get_current_usage", |b| {
        b.iter(|| {
            rt.block_on(async {
                let resource_monitor = resource::ResourceMonitor::new();
                resource_monitor.get_current_usage().await.unwrap()
            })
        });
    });
    
    // Benchmark efficiency metrics
    group.bench_function("get_efficiency_metrics", |b| {
        b.iter(|| {
            rt.block_on(async {
                let resource_monitor = resource::ResourceMonitor::new();
                resource_monitor.get_efficiency_metrics().await.unwrap()
            })
        });
    });
    
    group.finish();
}

/// Benchmark health checker operations
fn bench_health_checker(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("health_checker");
    
    // Benchmark resource availability check
    group.bench_function("check_resource_availability", |b| {
        b.iter(|| {
            rt.block_on(async {
                let health_checker = DefaultHealthChecker::new(Duration::from_secs(30));
                health_checker.check_resource_availability().await.unwrap()
            })
        });
    });
    
    group.finish();
}

/// Benchmark persistence operations
fn bench_persistence(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("persistence");
    
    // Create test session
    let test_session = PersistentSession {
        session_id: "bench-session".to_string(),
        instance_id: "i-bench123".to_string(),
        region: "us-east-1".to_string(),
        status: SessionStatus::Active,
        created_at: chrono::Utc::now(),
        last_activity: chrono::Utc::now(),
        connection_count: 5,
        total_duration_seconds: 300,
        process_id: Some(std::process::id()),
        is_stale: false,
        recovery_attempts: 0,
    };
    
    // Benchmark save operation
    group.bench_function("save_session", |b| {
        b.iter(|| {
            rt.block_on(async {
                let persistence = SqlitePersistenceManager::with_default_path()
                    .expect("Failed to create persistence manager");
                persistence.initialize().await.expect("Failed to initialize database");
                persistence.save_session(black_box(&test_session)).await.unwrap()
            })
        });
    });
    
    // Benchmark load operation
    group.bench_function("load_active_sessions", |b| {
        b.iter(|| {
            rt.block_on(async {
                let persistence = SqlitePersistenceManager::with_default_path()
                    .expect("Failed to create persistence manager");
                persistence.initialize().await.expect("Failed to initialize database");
                persistence.load_active_sessions().await.unwrap()
            })
        });
    });
    
    group.finish();
}

/// Benchmark configuration operations
fn bench_config(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("config");
    
    // Benchmark config loading
    group.bench_function("load_default_config", |b| {
        b.iter(|| {
            rt.block_on(async {
                config::Config::load(black_box(None)).await.unwrap()
            })
        });
    });
    
    // Benchmark config validation
    group.bench_function("validate_config", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = config::Config::default();
                config.validate().unwrap()
            })
        });
    });
    
    group.finish();
}

/// Benchmark session creation workflow
fn bench_session_workflow(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("session_workflow");
    
    // Benchmark complete session creation workflow
    group.bench_function("create_session_workflow", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Create session manager
                let session_manager = DefaultSessionManager::new(3).await
                    .expect("Failed to create session manager");
                
                // Create session config
                let session_config = SessionConfig::new(
                    black_box("i-workflow123".to_string()),
                    black_box(8080),
                    black_box(80),
                    black_box(None),
                    black_box("us-east-1".to_string()),
                );
                
                // Search for existing sessions
                let _existing = session_manager
                    .find_existing_sessions(&session_config.instance_id, session_config.local_port)
                    .await
                    .expect("Failed to search sessions");
                
                // Monitor resource usage
                let _usage = session_manager
                    .monitor_resource_usage()
                    .await
                    .expect("Failed to monitor resources");
                
                // Note: We don't actually create the session to avoid AWS API calls
            })
        });
    });
    
    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("memory_patterns");
    
    // Benchmark memory usage with different session counts
    for session_count in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::new("session_manager_memory", session_count),
            session_count,
            |b, &session_count| {
                b.iter(|| {
                    rt.block_on(async move {
                        let session_manager = DefaultSessionManager::new(session_count).await
                            .expect("Failed to create session manager");
                        
                        // Simulate some operations
                        for i in 0..session_count {
                            let _search = session_manager
                                .find_existing_sessions(&format!("i-test{}", i), 8080 + i as u16)
                                .await;
                        }
                    })
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_operations");
    
    // Benchmark concurrent session searches
    for concurrency in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_session_search", concurrency),
            concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    rt.block_on(async move {
                        let session_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
                            DefaultSessionManager::new(10).await
                                .expect("Failed to create session manager")
                        ));
                        
                        let mut handles = Vec::new();
                        
                        for i in 0..concurrency {
                            let manager = session_manager.clone();
                            let handle = tokio::spawn(async move {
                                let mgr = manager.lock().await;
                                mgr.find_existing_sessions(
                                    &format!("i-concurrent{}", i),
                                    8080 + i as u16
                                ).await
                            });
                            handles.push(handle);
                        }
                        
                        for handle in handles {
                            handle.await.unwrap().unwrap();
                        }
                    })
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark startup performance
fn bench_startup_performance(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("startup");
    
    // Benchmark component initialization
    group.bench_function("initialize_all_components", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Initialize all core components
                let _session_manager = DefaultSessionManager::new(3).await
                    .expect("Failed to create session manager");
                let _resource_monitor = resource::ResourceMonitor::new();
                let _health_checker = DefaultHealthChecker::new(Duration::from_secs(30));
                let _persistence = SqlitePersistenceManager::with_default_path()
                    .expect("Failed to create persistence manager");
            })
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_session_manager,
    bench_resource_monitor,
    bench_health_checker,
    bench_persistence,
    bench_config,
    bench_session_workflow,
    bench_memory_patterns,
    bench_concurrent_operations,
    bench_startup_performance
);

criterion_main!(benches);