use std::fs;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use rv_gem_specification_yaml::parse;

fn load_fixture(name: &str) -> String {
    let fixture_path = format!("crates/rv-gem-specification-yaml/tests/fixtures/{name}.yaml");
    fs::read_to_string(&fixture_path)
        .unwrap_or_else(|_| panic!("Failed to read fixture: {fixture_path}"))
}

fn run_bench(c: &mut Criterion, test_name: &str) {
    let original_yaml = load_fixture(test_name);
    c.bench_function(&format!("Parse {test_name}"), |b| {
        b.iter(|| {
            let _req = black_box(parse(&original_yaml));
        })
    });
}

fn simple_spec(c: &mut Criterion) {
    run_bench(c, "simple_spec");
}

fn complex_spec(c: &mut Criterion) {
    run_bench(c, "complex_spec");
}

fn version_constraints_spec(c: &mut Criterion) {
    run_bench(c, "version_constraints_spec");
}

fn minimal_spec(c: &mut Criterion) {
    run_bench(c, "minimal_spec");
}

fn edge_case_spec(c: &mut Criterion) {
    run_bench(c, "edge_case_spec");
}

criterion_group!(
    benches,
    simple_spec,
    complex_spec,
    version_constraints_spec,
    minimal_spec,
    edge_case_spec,
);
criterion_main!(benches);
