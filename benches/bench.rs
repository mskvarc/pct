use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use pct::{PctStr, PctString, UriReserved};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    hint::black_box,
};

fn ascii_unreserved(len: usize) -> String {
    let sample = b"abcdefghijklmnopqrstuvwxyz0123456789-._~";
    (0..len).map(|i| sample[i % sample.len()] as char).collect()
}

fn ascii_must_encode(len: usize) -> String {
    let sample = b" !\"#$&'()*+,/:;<=>?@[\\]^`{|}";
    (0..len).map(|i| sample[i % sample.len()] as char).collect()
}

fn mixed_ascii(len: usize) -> String {
    let half = ascii_unreserved(len / 2);
    let enc = ascii_must_encode(len - half.len());
    let mut out = String::with_capacity(len);
    let mut a = half.chars();
    let mut b = enc.chars();
    loop {
        match (a.next(), b.next()) {
            (Some(x), Some(y)) => {
                out.push(x);
                out.push(y);
            }
            (Some(x), None) => out.push(x),
            (None, Some(y)) => out.push(y),
            (None, None) => break,
        }
    }
    out
}

fn cjk_emoji(len_chars: usize) -> String {
    let sample = ['中', '文', '한', '日', '本', '😀', '🚀', '🌍'];
    (0..len_chars).map(|i| sample[i % sample.len()]).collect()
}

fn encoded_no_percent(len: usize) -> String {
    ascii_unreserved(len)
}

fn encoded_all_percent(byte_len: usize) -> String {
    let triplets = byte_len / 3;
    let mut out = String::with_capacity(triplets * 3);
    for i in 0..triplets {
        let b = (i % 128) as u8;
        out.push_str(&format!("%{:02X}", b));
    }
    out
}

fn encoded_mixed(byte_len: usize) -> String {
    let mut out = String::with_capacity(byte_len);
    let mut i = 0;
    while out.len() + 3 <= byte_len {
        if i % 2 == 0 {
            out.push('a');
            out.push('b');
            out.push('c');
        } else {
            out.push_str("%20");
        }
        i += 1;
    }
    out
}

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");
    for &size in &[16usize, 65536] {
        let unr = ascii_unreserved(size);
        let must = ascii_must_encode(size);
        let mix = mixed_ascii(size);
        let utf8 = cjk_emoji(size / 4);

        group.throughput(Throughput::Bytes(unr.len() as u64));
        group.bench_with_input(BenchmarkId::new("unreserved", size), &unr, |b, s| {
            b.iter(|| PctString::encode(black_box(s).chars(), UriReserved::Any))
        });
        group.throughput(Throughput::Bytes(must.len() as u64));
        group.bench_with_input(BenchmarkId::new("must_encode", size), &must, |b, s| {
            b.iter(|| PctString::encode(black_box(s).chars(), UriReserved::Any))
        });
        group.throughput(Throughput::Bytes(mix.len() as u64));
        group.bench_with_input(BenchmarkId::new("mixed", size), &mix, |b, s| {
            b.iter(|| PctString::encode(black_box(s).chars(), UriReserved::Any))
        });
        group.throughput(Throughput::Bytes(utf8.len() as u64));
        group.bench_with_input(BenchmarkId::new("utf8", size), &utf8, |b, s| {
            b.iter(|| PctString::encode(black_box(s).chars(), UriReserved::Any))
        });
    }
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");
    for &size in &[64usize, 65536] {
        let np = encoded_no_percent(size);
        let ap = encoded_all_percent(size);
        let mx = encoded_mixed(size);

        let np_s = PctString::new(np.clone()).unwrap();
        let ap_s = PctString::new(ap.clone()).unwrap();
        let mx_s = PctString::new(mx.clone()).unwrap();

        group.throughput(Throughput::Bytes(np_s.as_bytes().len() as u64));
        group.bench_with_input(BenchmarkId::new("no_percent", size), &np_s, |b, s| b.iter(|| black_box(s).decode()));
        group.throughput(Throughput::Bytes(ap_s.as_bytes().len() as u64));
        group.bench_with_input(BenchmarkId::new("all_percent", size), &ap_s, |b, s| b.iter(|| black_box(s).decode()));
        group.throughput(Throughput::Bytes(mx_s.as_bytes().len() as u64));
        group.bench_with_input(BenchmarkId::new("mixed", size), &mx_s, |b, s| b.iter(|| black_box(s).decode()));
    }
    group.finish();
}

fn bench_validate(c: &mut Criterion) {
    let mut group = c.benchmark_group("validate");
    for &size in &[64usize, 65536] {
        let np = encoded_no_percent(size);
        let ap = encoded_all_percent(size);
        let mx = encoded_mixed(size);

        group.throughput(Throughput::Bytes(np.len() as u64));
        group.bench_with_input(BenchmarkId::new("no_percent", size), &np, |b, s| {
            b.iter(|| PctStr::new(black_box(s.as_str())).is_ok())
        });
        group.throughput(Throughput::Bytes(ap.len() as u64));
        group.bench_with_input(BenchmarkId::new("all_percent", size), &ap, |b, s| {
            b.iter(|| PctStr::new(black_box(s.as_str())).is_ok())
        });
        group.throughput(Throughput::Bytes(mx.len() as u64));
        group.bench_with_input(BenchmarkId::new("mixed", size), &mx, |b, s| {
            b.iter(|| PctStr::new(black_box(s.as_str())).is_ok())
        });
    }
    group.finish();
}

fn bench_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("eq");
    let size = 4096;
    let base = encoded_mixed(size);
    let a = PctString::new(base.clone()).unwrap();
    let b_same = PctString::new(base.clone()).unwrap();
    let lower: String = base
        .chars()
        .scan(0usize, |i, c| {
            let r = if *i > 0 && (c.is_ascii_uppercase() && c.is_ascii_hexdigit()) {
                c.to_ascii_lowercase()
            } else {
                c
            };
            *i += 1;
            Some(r)
        })
        .collect();
    let b_hex_case = PctString::new(lower).unwrap();
    let mut early = base.clone();
    if !early.is_empty() {
        let mut bytes = early.into_bytes();
        bytes[0] = b'Z';
        early = String::from_utf8(bytes).unwrap();
    }
    let b_early = PctString::new(early).unwrap();
    let mut late = base.clone();
    if late.len() > 1 {
        let mut bytes = late.into_bytes();
        let last = bytes.len() - 1;
        bytes[last] = b'Z';
        late = String::from_utf8(bytes).unwrap();
    }
    let b_late = PctString::new(late).unwrap();

    group.bench_function("byte_equal", |bn| bn.iter(|| black_box(&a) == black_box(&b_same)));
    group.bench_function("hex_case_different", |bn| bn.iter(|| black_box(&a) == black_box(&b_hex_case)));
    group.bench_function("unequal_early", |bn| bn.iter(|| black_box(&a) == black_box(&b_early)));
    group.bench_function("unequal_late", |bn| bn.iter(|| black_box(&a) == black_box(&b_late)));
    group.finish();
}

fn bench_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash");
    for &size in &[64usize, 4096] {
        let mx = encoded_mixed(size);
        let s = PctString::new(mx).unwrap();
        group.throughput(Throughput::Bytes(s.as_bytes().len() as u64));
        group.bench_with_input(BenchmarkId::new("mixed", size), &s, |b, v| {
            b.iter(|| {
                let mut h = DefaultHasher::new();
                black_box(v).hash(&mut h);
                h.finish()
            })
        });
    }
    group.finish();
}

fn bench_len(c: &mut Criterion) {
    let mut group = c.benchmark_group("len");
    let size = 4096;
    let ascii = PctString::new(encoded_mixed(size)).unwrap();
    let utf8_src: String = cjk_emoji(size / 4);
    let utf8 = PctString::encode(utf8_src.chars(), UriReserved::Any);

    group.bench_function("ascii", |bn| bn.iter(|| black_box(&ascii).len()));
    group.bench_function("utf8", |bn| bn.iter(|| black_box(&utf8).len()));
    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode, bench_validate, bench_eq, bench_hash, bench_len);
criterion_main!(benches);
