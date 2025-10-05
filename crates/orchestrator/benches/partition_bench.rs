use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nc_orchestrator::{partition, nir};

fn bench_partition_small(c: &mut Criterion) {
    let g = nir::fixtures::chain(&[8, 16, 32, 64, 128]);
    let targets = ["riscv64gcv_linux"];
    c.bench_function("partition/chain_5_layers", |b| {
        b.iter(|| {
            let plan = partition(black_box(&g), black_box(&targets)).expect("partition ok");
            black_box(plan);
        })
    });
}

fn bench_partition_medium(c: &mut Criterion) {
    let g = nir::fixtures::chain(&[8, 8, 8, 8, 8, 8, 8, 8, 8, 8]);
    let targets = ["riscv64gcv_linux"];
    c.bench_function("partition/chain_10_layers", |b| {
        b.iter(|| {
            let plan = partition(black_box(&g), black_box(&targets)).expect("partition ok");
            black_box(plan);
        })
    });
}

fn bench_partition_star(c: &mut Criterion) {
    let g = nir::fixtures::star(32, 8, 64, 0.5, 1.0);
    let targets = ["riscv64gcv_linux"];
    c.bench_function("partition/star_64_spokes", |b| {
        b.iter(|| {
            let plan = partition(black_box(&g), black_box(&targets)).expect("partition ok");
            black_box(plan);
        })
    });
}

criterion_group!(
    name = partition_benches;
    config = Criterion::default();
    targets = bench_partition_small, bench_partition_medium, bench_partition_star
);
criterion_main!(partition_benches);