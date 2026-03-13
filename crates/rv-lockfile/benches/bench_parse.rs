use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use rv_lockfile::parse;

fn run_bench(c: &mut Criterion, name: &str) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let filepath = format!("{manifest_dir}/tests/inputs/{name}");
    println!("benching {filepath}");
    let contents = std::fs::read_to_string(filepath).unwrap();
    c.bench_function(&format!("parse {name}"), |b| {
        b.iter(|| {
            let _out = black_box(parse(&contents));
        })
    });
}

fn parse_gitlab(c: &mut Criterion) {
    run_bench(c, "Gemfile.gitlab.lock");
}

fn parse_feedyouremail(c: &mut Criterion) {
    run_bench(c, "Gemfile.feedyouremail.lock");
}

criterion_group!(benches, parse_gitlab, parse_feedyouremail);
criterion_main!(benches);
