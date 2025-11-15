use dirs::cache_dir;
use fibble::{
    allowed_words, analyze_guess_against, remaining_secrets, secret_words, GameMode, Wordle,
    WordleError, WORD_LENGTH,
};
use indicatif::{ProgressBar, ProgressStyle};
use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::PathBuf;

const WORDLE_MAX_ATTEMPTS: usize = 6;
const FIBBLE_MAX_ATTEMPTS: usize = 9;
const FIRST_GUESS_CACHE_VERSION: u32 = 1;
const FIRST_GUESS_CACHE_FILE: &str = "first_guess_entropies.json";

struct Config {
    mode: GameMode,
    secret: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let mut game = Wordle::new_with_mode(&config.secret, config.mode)?;
    let max_attempts = max_attempts(config.mode);

    println!("Welcome to Fibble!");
    println!(
        "Try to guess the {WORD_LENGTH}-letter word in {max_attempts} attempts. Type 'quit' to exit."
    );
    if config.mode == GameMode::Fibble {
        println!("Fibble mode: expect one lied tile per guess, and enjoy the automatic opener.");
    }
    println!();

    if config.mode == GameMode::Fibble {
        perform_fibble_auto_guess(&mut game)?;
    }

    while game.guesses().len() < max_attempts {
        let analysis = best_guess_with_progress(&game);
        print_guess_summary("Suggested guess", &analysis);

        let attempt = game.guesses().len() + 1;
        print!("Guess {attempt}/{max_attempts}: ");
        io::stdout().flush()?;

        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            println!("\nNo input detected, exiting.");
            return Ok(());
        }

        let guess = line.trim();
        if guess.eq_ignore_ascii_case("quit") {
            println!("Come back soon!");
            return Ok(());
        }

        if guess.chars().count() != WORD_LENGTH {
            println!("Please enter a {WORD_LENGTH}-letter word.");
            continue;
        }

        let secret_word = game.secret().to_string();
        match game.submit_guess(guess) {
            Ok(row) => {
                println!("{row}");
                if row.guess() == secret_word {
                    println!(
                        "Nice! You solved it in {attempt} guess{}.",
                        if attempt == 1 { "" } else { "es" }
                    );
                    return Ok(());
                }
            }
            Err(WordleError::InvalidLength { .. }) => {
                println!("Please enter a {WORD_LENGTH}-letter word.");
            }
            Err(WordleError::UnknownWord { .. }) => {
                println!("That's not one of the allowed Wordle guesses.");
            }
        }
    }

    println!("Out of guesses! The word was {}.", game.secret());
    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut idx = 0;
    let mut mode = GameMode::Wordle;
    let mut secret: Option<String> = None;

    while idx < args.len() {
        let arg = &args[idx];
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--mode" => {
                idx += 1;
                let value = args
                    .get(idx)
                    .ok_or_else(|| String::from("missing value for --mode (wordle or fibble)"))?;
                mode = parse_mode(value)?;
            }
            "--secret" => {
                idx += 1;
                let value = args.get(idx).ok_or_else(|| {
                    String::from("missing value for --secret; supply a five-letter word")
                })?;
                secret = Some(value.clone());
            }
            _ if arg.starts_with('-') => {
                return Err(format!("unknown argument: {arg}").into());
            }
            _ => {
                if secret.is_none() {
                    secret = Some(arg.clone());
                } else {
                    return Err(String::from("multiple secrets provided").into());
                }
            }
        }
        idx += 1;
    }

    let selected_secret = secret.unwrap_or_else(random_secret);
    Ok(Config {
        mode,
        secret: selected_secret,
    })
}

fn parse_mode(value: &str) -> Result<GameMode, Box<dyn Error>> {
    match value.to_ascii_lowercase().as_str() {
        "wordle" => Ok(GameMode::Wordle),
        "fibble" => Ok(GameMode::Fibble),
        _ => Err(format!("unknown mode: {value}").into()),
    }
}

fn max_attempts(mode: GameMode) -> usize {
    match mode {
        GameMode::Wordle => WORDLE_MAX_ATTEMPTS,
        GameMode::Fibble => FIBBLE_MAX_ATTEMPTS,
    }
}

fn perform_fibble_auto_guess(game: &mut Wordle) -> Result<(), WordleError> {
    let mut guess = random_secret();
    while guess.eq_ignore_ascii_case(game.secret()) {
        guess = random_secret();
    }
    println!("Automatic opener: {guess}");
    let row = game.submit_guess(&guess)?;
    println!("{row}");
    Ok(())
}

fn random_secret() -> String {
    secret_words()
        .choose(&mut thread_rng())
        .expect("Word list is not empty")
        .clone()
}

fn print_usage() {
    println!("Play Wordle in the terminal.");
    println!("Usage: fibble [--mode MODE] [--secret WORD]");
    println!("Modes: 'wordle' (default) or 'fibble'.");
    println!("Without --secret a random secret word is selected.");
}

fn print_guess_summary(label: &str, insights: &GuessInsights) {
    if let Some(best) = &insights.best_guess {
        println!(
            "{label}: {} ({} possible secrets, {:.2} bits of information)",
            best.word, best.matching_secrets, best.entropy_bits
        );
    } else {
        println!("{label}: (no remaining candidates)");
    }

    if insights.top_secret_guesses.is_empty() {
        println!("Top secret guesses: (no remaining candidates)");
    } else {
        let description = insights
            .top_secret_guesses
            .iter()
            .map(|guess| format!("{} ({:.2} bits)", guess.word, guess.entropy_bits))
            .collect::<Vec<_>>()
            .join(", ");
        println!("Top secret guesses: {description}");
    }
}

fn best_guess_with_progress(game: &Wordle) -> GuessInsights {
    let candidates = remaining_secrets(game);
    match candidates.len() {
        0 => return GuessInsights::default(),
        1 => {
            let only = candidates[0].to_string();
            let suggestion = GuessSuggestion {
                word: only,
                entropy_bits: 0.0,
                matching_secrets: 1,
            };
            return GuessInsights {
                best_guess: Some(suggestion.clone()),
                top_secret_guesses: vec![suggestion],
            };
        }
        _ => {}
    }

    if game.guesses().is_empty() {
        let expected_total = candidates.len();
        if let Some(entries) = load_first_guess_cache(expected_total) {
            return insights_from_cache(&entries, &candidates);
        }

        let GuessCalculation {
            insights,
            all_suggestions,
        } = calculate_guess_suggestions(&candidates, true);
        if let Some(all_suggestions) = all_suggestions {
            if let Err(err) = write_first_guess_cache(all_suggestions, expected_total) {
                eprintln!("Failed to cache first-guess entropies: {err}");
            }
        }
        insights
    } else {
        calculate_guess_suggestions(&candidates, false).insights
    }
}

fn calculate_guess_suggestions(candidates: &[&str], collect_all: bool) -> GuessCalculation {
    let allowed = allowed_words();
    let candidate_lookup: HashSet<&str> = candidates.iter().copied().collect();
    let mut best: Option<GuessSuggestion> = None;
    let mut secret_only: Vec<GuessSuggestion> = Vec::new();
    let mut all_suggestions = if collect_all {
        Some(Vec::with_capacity(allowed.len()))
    } else {
        None
    };

    let bar = ProgressBar::new(allowed.len() as u64);
    bar.set_message("Analyzing guesses");
    bar.set_style(
        ProgressStyle::default_bar()
            .template(
                "{msg:<24} {bar:40.cyan/blue} {pos:>5}/{len:<5} [{elapsed_precise}<{eta_precise}]",
            )
            .expect("valid template"),
    );

    for guess in allowed {
        if let Ok(entropy) = analyze_guess_against(guess, candidates.iter().copied()) {
            let suggestion = GuessSuggestion {
                word: entropy.guess().to_string(),
                entropy_bits: entropy.entropy_bits(),
                matching_secrets: entropy.total_secrets(),
            };

            if best.as_ref().map_or(true, |current| {
                suggestion.entropy_bits > current.entropy_bits
            }) {
                best = Some(suggestion.clone());
            }

            if candidate_lookup.contains(suggestion.word.as_str()) {
                secret_only.push(suggestion.clone());
            }

            if let Some(all) = &mut all_suggestions {
                all.push(suggestion);
            }
        }
        bar.inc(1);
    }

    bar.finish_and_clear();

    secret_only.sort_by(|a, b| {
        b.entropy_bits
            .partial_cmp(&a.entropy_bits)
            .unwrap_or(Ordering::Equal)
    });
    secret_only.truncate(4);

    GuessCalculation {
        insights: GuessInsights {
            best_guess: best,
            top_secret_guesses: secret_only,
        },
        all_suggestions,
    }
}

fn load_first_guess_cache(expected_total_secrets: usize) -> Option<Vec<FirstGuessCacheEntry>> {
    let path = cache_file_path()?;
    let data = fs::read(&path).ok()?;
    let cache: FirstGuessCacheFile = serde_json::from_slice(&data).ok()?;
    if cache.version != FIRST_GUESS_CACHE_VERSION
        || cache.total_secrets != expected_total_secrets
        || cache.allowed_words != allowed_words().len()
    {
        return None;
    }
    Some(cache.entries)
}

fn write_first_guess_cache(
    suggestions: Vec<GuessSuggestion>,
    total_secrets: usize,
) -> io::Result<()> {
    let path = match cache_file_path() {
        Some(path) => path,
        None => return Ok(()),
    };

    let mut entries: Vec<FirstGuessCacheEntry> = suggestions
        .into_iter()
        .map(|suggestion| FirstGuessCacheEntry {
            guess: suggestion.word,
            entropy_bits: suggestion.entropy_bits,
        })
        .collect();

    entries.sort_by(|a, b| {
        b.entropy_bits
            .partial_cmp(&a.entropy_bits)
            .unwrap_or(Ordering::Equal)
    });

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let cache = FirstGuessCacheFile {
        version: FIRST_GUESS_CACHE_VERSION,
        total_secrets,
        allowed_words: allowed_words().len(),
        entries,
    };

    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, &cache)?;
    Ok(())
}

fn cache_file_path() -> Option<PathBuf> {
    cache_dir().map(|dir| dir.join("fibble").join(FIRST_GUESS_CACHE_FILE))
}

fn insights_from_cache(entries: &[FirstGuessCacheEntry], candidates: &[&str]) -> GuessInsights {
    let matching_secrets = candidates.len();
    let candidate_lookup: HashSet<&str> = candidates.iter().copied().collect();
    let best_guess = entries.first().map(|entry| GuessSuggestion {
        word: entry.guess.clone(),
        entropy_bits: entry.entropy_bits,
        matching_secrets,
    });

    let mut top_secret_guesses = Vec::new();
    for entry in entries {
        if candidate_lookup.contains(entry.guess.as_str()) {
            top_secret_guesses.push(GuessSuggestion {
                word: entry.guess.clone(),
                entropy_bits: entry.entropy_bits,
                matching_secrets,
            });
            if top_secret_guesses.len() == 4 {
                break;
            }
        }
    }

    GuessInsights {
        best_guess,
        top_secret_guesses,
    }
}

#[derive(Default, Clone)]
struct GuessInsights {
    best_guess: Option<GuessSuggestion>,
    top_secret_guesses: Vec<GuessSuggestion>,
}

#[derive(Clone)]
struct GuessSuggestion {
    word: String,
    entropy_bits: f64,
    matching_secrets: usize,
}

struct GuessCalculation {
    insights: GuessInsights,
    all_suggestions: Option<Vec<GuessSuggestion>>,
}

#[derive(Serialize, Deserialize)]
struct FirstGuessCacheFile {
    version: u32,
    total_secrets: usize,
    allowed_words: usize,
    entries: Vec<FirstGuessCacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct FirstGuessCacheEntry {
    guess: String,
    entropy_bits: f64,
}
