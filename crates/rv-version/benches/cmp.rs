use std::{hint::black_box, str::FromStr};

use criterion::{Criterion, criterion_group, criterion_main};
use rv_version::Version;

fn version_cmp_neither_prerelease(c: &mut Criterion) {
    let sa = "1.82";
    let sb = "1.82.0";
    let va = Version::from_str(sa).unwrap();
    let vb = Version::from_str(sb).unwrap();
    c.bench_function(&format!("Compare {sa} and {sb}"), |b| {
        b.iter(|| {
            let _ver = black_box(va.cmp(&vb));
        })
    });
}

fn version_cmp_one_prerelease(c: &mut Criterion) {
    let sa = "1.82";
    let sb = "1.82.alpha1";
    let va = Version::from_str(sa).unwrap();
    let vb = Version::from_str(sb).unwrap();
    c.bench_function(&format!("Compare {sa} and {sb}"), |b| {
        b.iter(|| {
            let _ver = black_box(va.cmp(&vb));
        })
    });
}

fn version_cmp_both_prerelease(c: &mut Criterion) {
    let sa = "1.82.rc.4";
    let sb = "1.82.alpha1";
    let va = Version::from_str(sa).unwrap();
    let vb = Version::from_str(sb).unwrap();
    c.bench_function(&format!("Compare {sa} and {sb}"), |b| {
        b.iter(|| {
            let _ver = black_box(va.cmp(&vb));
        })
    });
}

criterion_group!(
    benches,
    version_cmp_both_prerelease,
    version_cmp_neither_prerelease,
    version_cmp_one_prerelease
);
criterion_main!(benches);
