use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use rv_lockfile::parse;

fn run_bench(c: &mut Criterion, name: &str) {
    let filepath = format!("crates/rv-lockfile/tests/inputs/{name}");
    let contents = std::fs::read_to_string(filepath).unwrap();
    c.bench_function(&format!("parse {name}"), |b| {
        b.iter(|| {
            let _out = black_box(parse(&contents));
        })
    });
}

fn parse_example0(c: &mut Criterion) {
    run_bench(c, "Gemfile.lock.test0");
}

fn parse_feedyouremail(c: &mut Criterion) {
    run_bench(c, "Gemfile.lock.feedyouremail");
}

criterion_group!(benches, parse_example0, parse_feedyouremail);
criterion_main!(benches);
