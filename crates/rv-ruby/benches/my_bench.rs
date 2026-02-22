use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use rv_ruby::request::RubyRequest;
use rv_ruby::version::ReleasedRubyVersion;

fn parse_ruby_req(c: &mut Criterion) {
    let req = "3.4.1".to_owned();
    c.bench_function(&format!("parse {req} into RubyRequest"), |b| {
        b.iter(|| {
            let _req: RubyRequest = black_box(req.parse().unwrap());
        })
    });
}

fn ruby_req_to_string(c: &mut Criterion) {
    let req: RubyRequest = "3.4.1".parse().unwrap();
    c.bench_function(&format!("Call \"{req}\".to_string()"), |b| {
        b.iter(|| {
            let _req = black_box(req.to_string());
        })
    });
}

fn ruby_ver_number(c: &mut Criterion) {
    let ver: ReleasedRubyVersion = "3.4.1".parse().unwrap();
    c.bench_function(&format!("Call {ver}.number()"), |b| {
        b.iter(|| {
            let _ver = black_box(ver.number());
        })
    });
}

criterion_group!(benches, parse_ruby_req, ruby_req_to_string, ruby_ver_number);
criterion_main!(benches);
