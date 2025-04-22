use criterion::{Criterion, criterion_group, criterion_main};
use std::{hint::black_box, time::Duration};

use hostlist_iter::{Hostlist, Result};

fn hostlist_n(n: u32) -> Result<()> {
    let hostlist_expr = format!("n[1-{n}]");
    let hostlist = Hostlist::new(&hostlist_expr)?;

    let mut i: u32 = 1;
    for host in hostlist {
        let expected = format!("n{i}");
        assert_eq!(host, expected);
        i += 1;
    }

    Ok(())
}

fn criterion_benchmark_100k(c: &mut Criterion) {
    c.bench_function("hostlist 100k", |b| {
        b.iter(|| hostlist_n(black_box(100_000)));
    });
}

fn criterion_benchmark_1m(c: &mut Criterion) {
    c.bench_function("hostlist 1m", |b| {
        b.iter(|| hostlist_n(black_box(1_000_000)));
    });
}

// Custom configuration function
fn custom_criterion() -> Criterion {
    Criterion::default()
        //.sample_size(50)
        // and/or:
        .measurement_time(Duration::from_secs(12))
}

//criterion_group!(benches, criterion_benchmark_100k, criterion_benchmark_1m);
criterion_group! {
    name = benches;
    config = custom_criterion();
    targets = criterion_benchmark_100k, criterion_benchmark_1m
}

criterion_main!(benches);
