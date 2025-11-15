use fibble::analyze_guess;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let guess = std::env::args().nth(1).ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidInput,
            "usage: fibble-entropy <guess word>",
        )
    })?;

    let analysis = analyze_guess(&guess)?;
    println!("Guess: {}", analysis.guess());
    println!("Total secrets: {}", analysis.total_secrets());
    println!("Distinct patterns: {}", analysis.distinct_patterns());
    println!("Entropy: {:.4} bits", analysis.entropy_bits());

    Ok(())
}
