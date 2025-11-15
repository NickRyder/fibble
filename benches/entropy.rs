use criterion::{Criterion, black_box, criterion_group, criterion_main};
use fibble::{allowed_words, analyze_guess_against, secret_words};

fn entropy_benchmark(c: &mut Criterion) {
    let secrets = secret_words();
    let guesses = ["ARISE", "SOARE", "SLATE"];

    c.bench_function("analyze_guess/all_secrets", |b| {
        b.iter(|| {
            for guess in guesses {
                analyze_guess_against(black_box(guess), secrets.iter().map(|word| word.as_str()))
                    .expect("valid guess");
            }
        });
    });

    let allowed = allowed_words();
    let random_guess = allowed
        .get(1234)
        .map(|word| word.as_str())
        .expect("allowed words not empty");
    c.bench_function("analyze_guess_random_secret_subset", |b| {
        let sample: Vec<&str> = secrets.iter().take(500).map(|word| word.as_str()).collect();
        b.iter(|| {
            analyze_guess_against(black_box(random_guess), sample.iter().copied())
                .expect("valid guess")
        });
    });
}

criterion_group!(benches, entropy_benchmark);
criterion_main!(benches);
