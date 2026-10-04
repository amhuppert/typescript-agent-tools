use std::{hint::black_box, time::Instant};
mod support;
fn main() {
    let args: Vec<_> = std::env::args()
        .skip(1)
        .filter(|arg| arg != "--bench")
        .collect();
    if args.first().is_some_and(|arg| arg == "--corpus") {
        let sources = support::sources(&args[1..]);
        assert!(
            !sources.is_empty(),
            "Supply TypeScript source paths after --corpus"
        );
        let bytes: usize = sources.iter().map(|source| source.text.len()).sum();
        for source in &sources {
            black_box(support::parse(source));
        }
        let iterations = (64 * 1024 * 1024 / bytes.max(1)).clamp(3, 5_000);
        let start = Instant::now();
        for _ in 0..iterations {
            for source in &sources {
                black_box(support::parse(black_box(source)));
            }
        }
        let elapsed = start.elapsed();
        println!(
            "{} files, {} bytes x {}: {:.2} MiB/s ({:.3} ms/sweep)",
            sources.len(),
            bytes,
            iterations,
            bytes as f64 * iterations as f64 / elapsed.as_secs_f64() / 1_048_576.0,
            elapsed.as_secs_f64() * 1000.0 / iterations as f64
        );
        return;
    }
    let unit = "export function sum<T extends number>(x: T, y: T): number { return x + y; }\n";
    if std::env::args().any(|arg| arg == "--profile") {
        let source = unit.repeat(1_000);
        black_box(typescript_parser::parse(&source).unwrap());
        println!("Ready to profile");
        let start = Instant::now();
        let mut count = 0;
        while start.elapsed().as_secs() < 10 {
            black_box(typescript_parser::parse(black_box(&source)).unwrap());
            count += 1;
        }
        println!("Profiled {count} parses");
        return;
    }
    for repetitions in [1, 1_000, 10_000] {
        let source = unit.repeat(repetitions);
        let iterations = (10_000 / repetitions).max(10);
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(typescript_parser::parse(black_box(&source)).unwrap());
        }
        let elapsed = start.elapsed();
        println!(
            "{} bytes x {}: {:.2} MiB/s ({:.3} ms/parse)",
            source.len(),
            iterations,
            source.len() as f64 * iterations as f64 / elapsed.as_secs_f64() / 1_048_576.0,
            elapsed.as_secs_f64() * 1000.0 / iterations as f64
        );
    }
}
