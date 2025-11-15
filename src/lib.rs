use once_cell::sync::Lazy;
use rand::{thread_rng, Rng};
use std::collections::HashSet;
use std::fmt;

/// The fixed Wordle word length.
pub const WORD_LENGTH: usize = 5;
const ALPHABET_SIZE: usize = 26;
const PATTERN_SPACE: usize = 3usize.pow(WORD_LENGTH as u32);
const PATTERN_ABSENT: u8 = 0;
const PATTERN_PRESENT: u8 = 1;
const PATTERN_CORRECT: u8 = 2;

static WORDLE_ALLOWED_LIST: Lazy<Vec<String>> = Lazy::new(|| {
    include_str!("../data/wordle_allowed.txt")
        .lines()
        .filter_map(|line| {
            let word = line.trim();
            if word.is_empty() {
                return None;
            }

            if word.chars().count() == WORD_LENGTH {
                Some(word.to_ascii_uppercase())
            } else {
                None
            }
        })
        .collect()
});

static WORDLE_ALLOWED_SET: Lazy<HashSet<String>> =
    Lazy::new(|| WORDLE_ALLOWED_LIST.iter().cloned().collect());

static WORDLE_SECRET_LIST: Lazy<Vec<String>> = Lazy::new(|| {
    include_str!("../data/wordle_secrets.txt")
        .lines()
        .filter_map(|line| {
            let word = line.trim();
            if word.is_empty() {
                return None;
            }

            if word.chars().count() == WORD_LENGTH {
                if !word.chars().all(|ch| ch.is_ascii_alphabetic()) {
                    return None;
                }
                let uppercase = word.to_ascii_uppercase();
                if !WORDLE_ALLOWED_SET.contains(&uppercase) {
                    panic!("secret word {word} missing from allowed list");
                }
                Some(uppercase)
            } else {
                None
            }
        })
        .collect()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Wordle,
    Fibble,
}

/// Represents a full Wordle game, keeping track of the secret word and guess history.
#[derive(Debug, Clone)]
pub struct Wordle {
    secret: String,
    mode: GameMode,
    guesses: Vec<GuessResult>,
}

impl Wordle {
    /// Creates a new game with the provided secret word (case-insensitive).
    pub fn new(secret: &str) -> Result<Self, WordleError> {
        Self::new_with_mode(secret, GameMode::Wordle)
    }

    /// Creates a new game with a specific ruleset.
    pub fn new_with_mode(secret: &str, mode: GameMode) -> Result<Self, WordleError> {
        let normalized = normalize(secret)?;
        ensure_allowed(&normalized)?;
        Ok(Self {
            secret: normalized,
            mode,
            guesses: Vec::new(),
        })
    }

    /// Records a guess, returning the scored row so callers can inspect or display it.
    pub fn submit_guess(&mut self, guess: &str) -> Result<&GuessResult, WordleError> {
        let normalized_guess = normalize(guess)?;
        ensure_allowed(&normalized_guess)?;
        let mut letters = score(&self.secret, &normalized_guess);
        if matches!(self.mode, GameMode::Fibble) {
            apply_fibble_lie(&mut letters);
        }
        self.guesses.push(GuessResult {
            guess: normalized_guess,
            letters,
        });
        Ok(self.guesses.last().expect("just pushed"))
    }

    /// Returns the guesses made so far, in submission order.
    pub fn guesses(&self) -> &[GuessResult] {
        &self.guesses
    }

    /// Returns the hidden solution word in its normalized (uppercase) form.
    pub fn secret(&self) -> &str {
        &self.secret
    }

    /// Returns the current game mode.
    pub fn mode(&self) -> GameMode {
        self.mode
    }
}

/// The per-letter states emitted by Wordle scoring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LetterState {
    Correct(char),
    Present(char),
    Absent(char),
}

impl LetterState {
    /// Returns the uppercase character that originated this state.
    pub fn letter(&self) -> char {
        match self {
            LetterState::Correct(c) | LetterState::Present(c) | LetterState::Absent(c) => *c,
        }
    }

    fn color_code(&self) -> &'static str {
        match self {
            LetterState::Correct(_) => "\x1b[48;5;34m\x1b[97m", // green background, bright text
            LetterState::Present(_) => "\x1b[48;5;178m\x1b[30m", // yellow background, dark text
            LetterState::Absent(_) => "\x1b[48;5;240m\x1b[97m", // gray background, bright text
        }
    }

    fn colored_block(&self) -> String {
        format!("{} {} \x1b[0m", self.color_code(), self.letter())
    }
}

/// A scored guess row including letter-by-letter states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuessResult {
    guess: String,
    letters: Vec<LetterState>,
}

impl GuessResult {
    /// Returns the normalized (uppercase) guess string.
    pub fn guess(&self) -> &str {
        &self.guess
    }

    /// Returns the per-letter feedback for this guess.
    pub fn letters(&self) -> &[LetterState] {
        &self.letters
    }

    /// Whether the guess matched the secret completely.
    pub fn is_correct(&self) -> bool {
        self.letters
            .iter()
            .all(|state| matches!(state, LetterState::Correct(_)))
    }

    /// Converts the scored row into a colored string ready for terminal output.
    pub fn colored_string(&self) -> String {
        self.letters
            .iter()
            .map(LetterState::colored_block)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl fmt::Display for GuessResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.colored_string())
    }
}

/// Summary information about a guess evaluated against every possible secret word.
#[derive(Debug, Clone)]
pub struct GuessEntropy {
    guess: String,
    pattern_counts: [usize; PATTERN_SPACE],
}

impl GuessEntropy {
    /// Returns the normalized (uppercase) guess word.
    pub fn guess(&self) -> &str {
        &self.guess
    }

    /// Returns the number of secrets considered when computing the entropy.
    pub fn total_secrets(&self) -> usize {
        self.pattern_counts.iter().sum()
    }

    /// Returns each color pattern and how many secrets yield it.
    pub fn pattern_counts(&self) -> Vec<(String, usize)> {
        self.pattern_counts
            .iter()
            .enumerate()
            .filter(|(_, count)| **count > 0)
            .map(|(code, count)| (pattern_code_to_string(code), *count))
            .collect()
    }

    /// Returns how many distinct patterns were observed.
    pub fn distinct_patterns(&self) -> usize {
        self.pattern_counts
            .iter()
            .filter(|count| **count > 0)
            .count()
    }

    /// Computes the Shannon entropy (in bits) of the pattern distribution.
    pub fn entropy_bits(&self) -> f64 {
        let total = self.total_secrets() as f64;
        self.pattern_counts.iter().fold(0.0, |acc, count| {
            if *count == 0 {
                acc
            } else {
                let probability = *count as f64 / total;
                acc - probability * probability.log2()
            }
        })
    }
}

/// Errors that can occur while creating a game or submitting guesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordleError {
    InvalidLength { expected: usize, found: usize },
    UnknownWord { word: String },
}

impl fmt::Display for WordleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WordleError::InvalidLength { expected, found } => write!(
                f,
                "expected a {expected}-letter word, but found {found} letters"
            ),
            WordleError::UnknownWord { .. } => write!(f, "that word is not in the Wordle list"),
        }
    }
}

impl std::error::Error for WordleError {}

fn normalize(word: &str) -> Result<String, WordleError> {
    let len = word.chars().count();
    if len != WORD_LENGTH {
        return Err(WordleError::InvalidLength {
            expected: WORD_LENGTH,
            found: len,
        });
    }

    Ok(word.to_ascii_uppercase())
}

fn ensure_allowed(word: &str) -> Result<(), WordleError> {
    if WORDLE_ALLOWED_SET.contains(word) {
        Ok(())
    } else {
        Err(WordleError::UnknownWord {
            word: word.to_string(),
        })
    }
}

fn score(secret: &str, guess: &str) -> Vec<LetterState> {
    let pattern_digits = compute_pattern_digits(secret.as_bytes(), guess.as_bytes());
    guess
        .as_bytes()
        .iter()
        .zip(pattern_digits.iter())
        .map(|(&guess_byte, &digit)| {
            let ch = char::from(guess_byte);
            match digit {
                PATTERN_CORRECT => LetterState::Correct(ch),
                PATTERN_PRESENT => LetterState::Present(ch),
                _ => LetterState::Absent(ch),
            }
        })
        .collect()
}

fn apply_fibble_lie(letters: &mut [LetterState]) {
    if letters.is_empty() {
        return;
    }
    let mut rng = thread_rng();
    let lie_index = rng.gen_range(0..letters.len());
    let original = letters[lie_index].clone();
    letters[lie_index] = random_lie_state(&original, &mut rng);
}

fn random_lie_state(state: &LetterState, rng: &mut impl Rng) -> LetterState {
    let letter = state.letter();
    match state {
        LetterState::Correct(_) => match rng.gen_range(0..2) {
            0 => LetterState::Present(letter),
            _ => LetterState::Absent(letter),
        },
        LetterState::Present(_) => match rng.gen_range(0..2) {
            0 => LetterState::Correct(letter),
            _ => LetterState::Absent(letter),
        },
        LetterState::Absent(_) => match rng.gen_range(0..2) {
            0 => LetterState::Correct(letter),
            _ => LetterState::Present(letter),
        },
    }
}

/// Computes the entropy of a guess against every known secret word.
pub fn analyze_guess(guess: &str) -> Result<GuessEntropy, WordleError> {
    analyze_guess_against(guess, secret_words().iter().map(|word| word.as_str()))
}

/// Computes the entropy of a guess against an arbitrary list of secret candidates.
pub fn analyze_guess_against<'a>(
    guess: &str,
    secrets: impl IntoIterator<Item = &'a str>,
) -> Result<GuessEntropy, WordleError> {
    let normalized_guess = normalize(guess)?;
    ensure_allowed(&normalized_guess)?;

    let mut pattern_counts = [0usize; PATTERN_SPACE];
    let guess_bytes = normalized_guess.as_bytes();
    for secret in secrets {
        let digits = compute_pattern_digits(secret.as_bytes(), guess_bytes);
        let pattern_code = encode_pattern(&digits);
        pattern_counts[pattern_code] += 1;
    }

    Ok(GuessEntropy {
        guess: normalized_guess,
        pattern_counts,
    })
}

fn compute_pattern_digits(secret: &[u8], guess: &[u8]) -> [u8; WORD_LENGTH] {
    debug_assert_eq!(
        secret.len(),
        WORD_LENGTH,
        "secret words must be {WORD_LENGTH} letters long"
    );
    debug_assert_eq!(
        guess.len(),
        WORD_LENGTH,
        "guess words must be {WORD_LENGTH} letters long"
    );

    let mut digits = [PATTERN_ABSENT; WORD_LENGTH];
    let mut leftovers = [0u8; ALPHABET_SIZE];

    for idx in 0..WORD_LENGTH {
        let secret_byte = secret[idx];
        let guess_byte = guess[idx];
        if guess_byte == secret_byte {
            digits[idx] = PATTERN_CORRECT;
        } else {
            leftovers[letter_index(secret_byte)] += 1;
        }
    }

    for idx in 0..WORD_LENGTH {
        if digits[idx] == PATTERN_CORRECT {
            continue;
        }

        let guess_byte = guess[idx];
        let lookup = letter_index(guess_byte);
        if leftovers[lookup] > 0 {
            digits[idx] = PATTERN_PRESENT;
            leftovers[lookup] -= 1;
        }
    }

    digits
}

fn encode_pattern(digits: &[u8; WORD_LENGTH]) -> usize {
    digits
        .iter()
        .fold(0usize, |acc, digit| acc * 3 + *digit as usize)
}

fn pattern_code_to_string(mut code: usize) -> String {
    let mut chars = [b'B'; WORD_LENGTH];
    for idx in (0..WORD_LENGTH).rev() {
        let digit = code % 3;
        code /= 3;
        chars[idx] = match digit {
            2 => b'G',
            1 => b'Y',
            _ => b'B',
        };
    }
    chars.iter().map(|byte| char::from(*byte)).collect()
}

fn letter_index(letter: u8) -> usize {
    debug_assert!(
        letter.is_ascii_uppercase(),
        "words should use only uppercase ASCII letters"
    );
    (letter - b'A') as usize
}

fn secret_matches_history(secret: &str, game: &Wordle) -> bool {
    match game.mode {
        GameMode::Wordle => game
            .guesses
            .iter()
            .all(|guess| score(secret, guess.guess()) == guess.letters),
        GameMode::Fibble => fibble_history_matches(secret, game.guesses()),
    }
}

fn fibble_history_matches(secret: &str, guesses: &[GuessResult]) -> bool {
    guesses
        .iter()
        .all(|guess| fibble_guess_matches(secret, guess))
}

fn fibble_guess_matches(secret: &str, guess: &GuessResult) -> bool {
    let truth = score(secret, guess.guess());
    let mismatches = truth
        .iter()
        .zip(guess.letters())
        .filter(|(actual, reported)| *actual != *reported)
        .count();
    mismatches == 1
}

/// Returns the list of remaining possible secret words for the provided game state.
pub fn remaining_secrets(game: &Wordle) -> Vec<&'static str> {
    WORDLE_SECRET_LIST
        .iter()
        .map(|word| word.as_str())
        .filter(|secret| secret_matches_history(secret, game))
        .collect()
}

/// Returns the guess from the allowed list that maximizes the expected information gain.
pub fn best_information_guess(game: &Wordle) -> Option<GuessEntropy> {
    let candidates = remaining_secrets(game);
    if candidates.is_empty() {
        return None;
    }

    allowed_words()
        .iter()
        .filter_map(|guess| analyze_guess_against(guess, candidates.iter().copied()).ok())
        .max_by(|a, b| {
            a.entropy_bits()
                .partial_cmp(&b.entropy_bits())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_words_not_in_wordle_list() {
        assert_eq!(
            Wordle::new("zzzzz").unwrap_err(),
            WordleError::UnknownWord {
                word: "ZZZZZ".into()
            }
        );

        let mut game = Wordle::new("cigar").unwrap();
        assert_eq!(
            game.submit_guess("zzzzz").unwrap_err(),
            WordleError::UnknownWord {
                word: "ZZZZZ".into()
            }
        );
    }

    #[test]
    fn rejects_words_of_incorrect_length() {
        assert!(Wordle::new("tool").is_err());
        let mut game = Wordle::new("cigar").unwrap();
        assert!(game.submit_guess("longer").is_err());
    }

    #[test]
    fn scores_guesses_with_duplicate_letters() {
        let letters = score("APPLE", "ALLOT");
        use LetterState::*;
        assert_eq!(
            letters,
            vec![
                Correct('A'),
                Present('L'),
                Absent('L'),
                Absent('O'),
                Absent('T')
            ]
        );
    }

    #[test]
    fn records_history_and_detects_wins() {
        let mut game = Wordle::new("cigar").unwrap();
        assert!(!game.submit_guess("cairn").unwrap().is_correct());
        assert!(game.submit_guess("cigar").unwrap().is_correct());
        assert_eq!(game.guesses().len(), 2);
    }

    #[test]
    fn colored_string_contains_ansi_sequences() {
        let mut game = Wordle::new("cigar").unwrap();
        let guess = game.submit_guess("cairn").unwrap();
        let colored = guess.colored_string();
        assert!(colored.contains("\x1b[")); // basic sanity check for ANSI output
    }

    #[test]
    fn remaining_secrets_keeps_actual_solution() {
        let mut game = Wordle::new("cigar").unwrap();
        game.submit_guess("cairn").unwrap();
        let secrets = remaining_secrets(&game);
        assert!(secrets.contains(&"CIGAR"));
    }

    #[test]
    fn remaining_secrets_collapses_after_solution_found() {
        let mut game = Wordle::new("cigar").unwrap();
        game.submit_guess("cigar").unwrap();
        let secrets = remaining_secrets(&game);
        assert_eq!(secrets, vec!["CIGAR"]);
    }

    #[test]
    fn entropy_bits_ignores_zero_probabilities() {
        let entropy = analyze_guess_against("cigar", vec!["CIGAR"]).unwrap();
        assert_eq!(entropy.total_secrets(), 1);
        assert!(entropy.entropy_bits().is_finite());
        assert_eq!(entropy.entropy_bits(), 0.0);
    }

    #[test]
    fn fibble_history_requires_single_lie() {
        let mut game = Wordle::new_with_mode("cigar", GameMode::Fibble).unwrap();
        game.guesses.push(GuessResult {
            guess: "CIGAR".into(),
            letters: vec![
                LetterState::Correct('C'),
                LetterState::Correct('I'),
                LetterState::Correct('G'),
                LetterState::Correct('A'),
                LetterState::Present('R'),
            ],
        });
        let secrets = remaining_secrets(&game);
        assert!(secrets.contains(&"CIGAR"));
        assert!(!secrets.contains(&"TIGAR"));
    }
}

/// Returns the uppercase list of allowed Wordle guesses.
pub fn allowed_words() -> &'static [String] {
    WORDLE_ALLOWED_LIST.as_slice()
}

/// Returns the uppercase list of canonical Wordle solutions.
pub fn secret_words() -> &'static [String] {
    WORDLE_SECRET_LIST.as_slice()
}
