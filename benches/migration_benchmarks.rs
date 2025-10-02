use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rpcnet::migration::{
    MigrationServiceImpl, MigrationService, MigrationRequest, ServerInstance, MigrationReason,
    ConnectionStateSnapshot, MigrationToken, ServerEndpoint,
};
use uuid::Uuid;

fn migration_initiate_benchmark(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let service = MigrationServiceImpl::new(Uuid::new_v4());
    
    c.bench_function("migration_initiate", |b| {
        b.iter(|| {
            let source = ServerInstance::new("127.0.0.1".to_string(), 8081, 8080);
            let target = ServerInstance::new("127.0.0.2".to_string(), 8081, 8080);
            let request = MigrationRequest::new(
                Uuid::new_v4(),
                source,
                target,
                MigrationReason::LoadBalancing,
                "bench".to_string(),
            );
            
            runtime.block_on(async {
                black_box(service.initiate_migration(request).await.unwrap());
            });
        });
    });
}

fn migration_confirm_benchmark(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let service = MigrationServiceImpl::new(Uuid::new_v4());
    
    c.bench_function("migration_confirm", |b| {
        b.iter(|| {
            let issuer = ServerEndpoint::new("127.0.0.1".to_string(), 8080, 8081);
            let mut token = MigrationToken::for_migration(Uuid::new_v4(), issuer);
            token.activate();
            
            runtime.block_on(async {
                black_box(service.confirm_migration(token).await.unwrap());
            });
        });
    });
}

fn state_snapshot_creation_benchmark(c: &mut Criterion) {
    c.bench_function("state_snapshot_creation", |b| {
        b.iter(|| {
            let snapshot = ConnectionStateSnapshot::new(Uuid::new_v4());
            black_box(snapshot);
        });
    });
}

criterion_group!(
    benches,
    migration_initiate_benchmark,
    migration_confirm_benchmark,
    state_snapshot_creation_benchmark
);
criterion_main!(benches);
