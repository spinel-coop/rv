use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use rv_lockfile::parse;

fn run_bench(c: &mut Criterion, name: &str) {
    let filepath = format!("crates/rv-lockfile/tests/inputs/{name}");
    let cwd = std::env::current_dir().unwrap().display().to_string();
    println!("benching {cwd}/{filepath}");
    let contents = std::fs::read_to_string(filepath).unwrap();
    c.bench_function(&format!("parse {name}"), |b| {
        b.iter(|| {
            let _out = black_box(parse(&contents));
        })
    });
}

fn parse_gitlab(c: &mut Criterion) {
    run_bench(c, "Gemfile.lock.gitlab");
}

fn parse_feedyouremail(c: &mut Criterion) {
    run_bench(c, "Gemfile.lock.feedyouremail");
}

criterion_group!(benches, parse_gitlab, parse_feedyouremail);
criterion_main!(benches);
