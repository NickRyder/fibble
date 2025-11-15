(() => {
  const WORD_LENGTH = 5;
  const MAX_ATTEMPTS = {
    wordle: 6,
    fibble: 9,
  };
  const ENTROPY_GUESS_LIMIT = 10;
  const TILE_STATE_CODE = {
    correct: "2",
    present: "1",
    absent: "0",
  };
  const NOTEBOOK_TILE_STATES = ["absent", "present", "correct"];

  const data = {
    allowed: [],
    allowedSet: new Set(),
    secrets: [],
    ready: false,
  };

  const state = {
    gameMode: "wordle",
    playMode: "play",
    secret: "",
    guesses: [],
    complete: false,
  };

  const entropyCache = {
    key: "",
    results: [],
  };

  const elements = {
    board: document.getElementById("board"),
    status: document.getElementById("status-message"),
    attempts: document.getElementById("attempts-remaining"),
    newGameForm: document.getElementById("new-game-form"),
    guessForm: document.getElementById("guess-form"),
    guessInput: document.getElementById("guess-input"),
    guessButton: document.querySelector("#guess-form button[type='submit']"),
    gameModeSelect: document.getElementById("game-mode-select"),
    playModeSelect: document.getElementById("play-mode-select"),
    secretInput: document.getElementById("secret-input"),
    entropyList: document.getElementById("entropy-list"),
  };

  init();

  async function init() {
    elements.newGameForm.addEventListener("submit", (event) => {
      event.preventDefault();
      startNewGame();
    });

    elements.guessForm.addEventListener("submit", handleGuessSubmit);

    elements.gameModeSelect.addEventListener("change", () => {
      syncSecretInputState();
    });

    elements.playModeSelect.addEventListener("change", () => {
      syncSecretInputState();
    });

    elements.guessInput.addEventListener("input", () => {
      elements.guessInput.value = elements.guessInput.value.toUpperCase();
    });

    syncSecretInputState();

    try {
      const [allowed, secrets] = await Promise.all([
        loadWordList("assets/wordle_allowed.txt"),
        loadWordList("assets/wordle_secrets.txt"),
      ]);
      data.allowed = allowed;
      data.allowedSet = new Set(allowed);
      data.secrets = secrets;
      data.ready = true;
      setMessage("Word lists loaded. Pick a game and style to get started!");
      renderBoard();
      updateGuessFormState();
    } catch (error) {
      console.error(error);
      setMessage("Failed to load the word lists. Refresh to try again.");
      updateGuessFormState();
      return;
    }

    startNewGame();
  }

  function startNewGame() {
    if (!data.ready) {
      return;
    }

    const selectedGameMode = elements.gameModeSelect.value;
    const selectedPlayMode = elements.playModeSelect.value;

    if (selectedPlayMode === "notebook") {
      state.gameMode = selectedGameMode;
      state.playMode = selectedPlayMode;
      state.guesses = [];
      state.complete = false;
      entropyCache.key = "";
      entropyCache.results = [];
      state.secret = "";
      renderBoard();
      updateAttemptsText();
      updateGuessFormState();
      const notebookMessage =
        selectedGameMode === "fibble"
          ? "Fibble notebook: enter your real guesses, click tiles (including the fibble lie) to match the feedback, and we’ll suggest what to try next."
          : "Wordle notebook: enter your real guesses, click tiles to match the feedback, and we’ll suggest what to try next.";
      setMessage(notebookMessage);
      focusGuessInput();
      return;
    }

    const manualSecret = elements.secretInput.value.trim().toUpperCase();
    let chosenSecret = "";

    if (manualSecret.length > 0) {
      if (!/^[A-Z]{5}$/.test(manualSecret)) {
        setMessage("Secret words must be exactly five letters (A–Z).");
        return;
      }
      if (!data.allowedSet.has(manualSecret)) {
        setMessage("Secret word must be on the allowed list.");
        return;
      }
      chosenSecret = manualSecret;
    } else {
      chosenSecret = pickRandom(data.secrets);
      elements.secretInput.value = "";
    }

    state.gameMode = selectedGameMode;
    state.playMode = selectedPlayMode;
    state.secret = chosenSecret;
    state.guesses = [];
    state.complete = false;
    entropyCache.key = "";
    entropyCache.results = [];

    renderBoard();
    updateAttemptsText();
    updateGuessFormState();
    focusGuessInput();

    if (selectedGameMode === "fibble") {
      performFibbleAutoGuess();
    } else {
      setMessage("Guess the secret word in six tries.");
    }
  }

  function handleGuessSubmit(event) {
    event.preventDefault();
    if (!data.ready) {
      return;
    }

    if (!state.secret && state.playMode !== "notebook") {
      setMessage("Start a new game first.");
      return;
    }

    if (state.playMode !== "notebook" && state.complete) {
      setMessage("Game over. Start a new one to keep playing!");
      return;
    }

    const guess = elements.guessInput.value.trim().toUpperCase();
    elements.guessInput.value = "";

    if (guess.length !== WORD_LENGTH) {
      setMessage("Guesses must be exactly five letters.");
      return;
    }

    if (!data.allowedSet.has(guess)) {
      setMessage("That word is not on the allowed list.");
      return;
    }

    recordGuess(guess, { auto: false });
  }

  function performFibbleAutoGuess() {
    let opener = pickRandom(data.secrets);
    if (data.secrets.length > 1) {
      let guard = 0;
      while (opener === state.secret && guard < 10) {
        opener = pickRandom(data.secrets);
        guard += 1;
      }
    }
    recordGuess(opener, { auto: true });
  }

  function recordGuess(guess, { auto }) {
    let letters;
    let solved = false;

    if (state.playMode === "notebook") {
      letters = guess.split("").map((letter) => ({
        letter,
        state: NOTEBOOK_TILE_STATES[0],
      }));
    } else {
      letters = scoreGuess(state.secret, guess);
      solved = guess === state.secret;
      if (state.gameMode === "fibble") {
        applyFibbleLie(letters);
      }
    }

    state.guesses.push({
      guess,
      letters,
      auto,
    });

    renderBoard();
    updateAttemptsText();

    if (state.playMode === "notebook") {
      const helperMessage =
        state.gameMode === "fibble"
          ? "Fibble notebook: click tiles to match the (lying) feedback so the next-guess suggestions stay accurate."
          : "Notebook mode: click any tile to cycle between gray, yellow, and green so the next-guess suggestions stay accurate.";
      setMessage(helperMessage);
    } else if (solved) {
      state.complete = true;
      setMessage(
        `Nice! You solved it in ${state.guesses.length} guess${
          state.guesses.length === 1 ? "" : "es"
        }.`
      );
    } else if (state.guesses.length >= maxAttempts()) {
      state.complete = true;
      setMessage(`Out of guesses! The word was ${state.secret}.`);
    } else if (auto) {
      setMessage(`Automatic opener: ${guess}. Expect one lie per row.`);
    } else {
      setMessage("Keep going!");
    }

    updateGuessFormState();
    focusGuessInput();
  }

  function scoreGuess(secret, guess) {
    const letters = [];
    const leftovers = new Array(26).fill(0);

    for (let idx = 0; idx < WORD_LENGTH; idx += 1) {
      const secretChar = secret[idx];
      const guessChar = guess[idx];
      if (guessChar === secretChar) {
        letters[idx] = {
          letter: guessChar,
          state: "correct",
        };
      } else {
        letters[idx] = {
          letter: guessChar,
          state: "absent",
        };
        leftovers[letterIndex(secretChar)] += 1;
      }
    }

    for (let idx = 0; idx < WORD_LENGTH; idx += 1) {
      if (letters[idx].state === "correct") {
        continue;
      }
      const guessChar = guess[idx];
      const lookup = letterIndex(guessChar);
      if (leftovers[lookup] > 0) {
        letters[idx].state = "present";
        leftovers[lookup] -= 1;
      }
    }

    return letters;
  }

  function applyFibbleLie(letters) {
    if (!letters.length) {
      return;
    }
    const lieIndex = Math.floor(Math.random() * letters.length);
    const current = letters[lieIndex];
    letters[lieIndex] = {
      letter: current.letter,
      state: randomLieState(current.state),
    };
  }

  function randomLieState(state) {
    if (state === "correct") {
      return Math.random() < 0.5 ? "present" : "absent";
    }
    if (state === "present") {
      return Math.random() < 0.5 ? "correct" : "absent";
    }
    return Math.random() < 0.5 ? "correct" : "present";
  }

  function cycleNotebookTile(rowIndex, columnIndex) {
    if (state.playMode !== "notebook") {
      return;
    }
    const guess = state.guesses[rowIndex];
    const entry = guess?.letters?.[columnIndex];
    if (!entry) {
      return;
    }
    const currentIndex = NOTEBOOK_TILE_STATES.indexOf(entry.state);
    const nextIndex =
      (currentIndex + 1 + NOTEBOOK_TILE_STATES.length) %
      NOTEBOOK_TILE_STATES.length;
    entry.state = NOTEBOOK_TILE_STATES[nextIndex];
    renderBoard();
  }

  function renderBoard() {
    const totalRows =
      state.playMode === "notebook"
        ? Math.max(maxAttempts(), state.guesses.length + 1)
        : state.secret
        ? maxAttempts()
        : MAX_ATTEMPTS.wordle;
    elements.board.innerHTML = "";

    let possibleSecretsForRender = null;
    let fibbleLieHints = null;

    if (state.gameMode === "fibble") {
      possibleSecretsForRender = computePossibleSecrets();
      fibbleLieHints = computeFibbleLieHints(possibleSecretsForRender);
    }

    for (let row = 0; row < totalRows; row += 1) {
      const rowElement = document.createElement("div");
      rowElement.className = "guess-row";
      const guess = state.guesses[row];
      const rowHints = fibbleLieHints ? fibbleLieHints[row] : null;

      for (let col = 0; col < WORD_LENGTH; col += 1) {
        const tile = document.createElement("div");
        tile.className = "tile";

        if (guess && guess.letters[col]) {
          tile.textContent = guess.letters[col].letter;
          tile.dataset.state = guess.letters[col].state;
          if (rowHints) {
            const hint = rowHints[col];
            if (hint?.isAlwaysLie) {
              tile.classList.add("tile--lie-confirmed");
            } else if (hint?.isNeverLie) {
              tile.classList.add("tile--lie-ruled-out");
            }
          }

          if (state.playMode === "notebook") {
            tile.dataset.editable = "true";
            tile.setAttribute("role", "button");
            tile.tabIndex = 0;
            tile.setAttribute(
              "aria-label",
              `Cycle color for ${guess.letters[col].letter}`
            );
            tile.title = "Click to cycle colors";
            tile.addEventListener("click", () =>
              cycleNotebookTile(row, col)
            );
            tile.addEventListener("keydown", (event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                cycleNotebookTile(row, col);
              }
            });
          }
        } else {
          tile.textContent = "";
          tile.removeAttribute("data-state");
        }

        rowElement.appendChild(tile);
      }

      elements.board.appendChild(rowElement);
    }

    renderEntropySuggestions(possibleSecretsForRender);
  }

  function renderEntropySuggestions(precomputedSecrets) {
    if (!elements.entropyList) {
      return;
    }

    if (!data.ready) {
      setEntropyListMessage("Loading suggestions…");
      return;
    }

    if (!state.secret && state.playMode !== "notebook") {
      setEntropyListMessage("Start a game to see suggestions.");
      return;
    }

    const possibleSecrets = Array.isArray(precomputedSecrets)
      ? precomputedSecrets
      : computePossibleSecrets();
    if (!possibleSecrets.length) {
      const message =
        state.gameMode === "fibble"
          ? "Fibble lies make suggestions unreliable."
          : "No words match the given clues.";
      setEntropyListMessage(message);
      return;
    }

    const cacheKey = buildEntropyCacheKey();
    if (entropyCache.key === cacheKey && entropyCache.results.length) {
      renderEntropyList(entropyCache.results);
      return;
    }

    const ranked = rankGuessesByEntropy(possibleSecrets);
    entropyCache.key = cacheKey;
    entropyCache.results = ranked;
    renderEntropyList(ranked);
  }

  function renderEntropyList(items) {
    if (!elements.entropyList) {
      return;
    }
    elements.entropyList.innerHTML = "";

    if (!items.length) {
      setEntropyListMessage("No suggestions available.");
      return;
    }

    for (const item of items) {
      const li = document.createElement("li");
      const row = document.createElement("div");
      row.className = "entropy-item";

      const word = document.createElement("span");
      word.className = "entropy-word";
      word.dataset.possible = item.possible ? "true" : "false";
      word.textContent = item.guess;

      const entropyValue = document.createElement("span");
      entropyValue.className = "entropy-value";
      entropyValue.textContent = `${item.entropy.toFixed(2)} bits`;

      row.appendChild(word);
      row.appendChild(entropyValue);
      li.appendChild(row);
      elements.entropyList.appendChild(li);
    }
  }

  function setEntropyListMessage(message) {
    if (!elements.entropyList) {
      return;
    }
    elements.entropyList.innerHTML = "";
    const li = document.createElement("li");
    li.className = "entropy-empty";
    li.textContent = message;
    elements.entropyList.appendChild(li);
  }

  function computePossibleSecrets() {
    if (!data.ready) {
      return [];
    }
    if (!state.guesses.length) {
      return data.secrets.slice();
    }
    if (state.gameMode === "fibble") {
      return data.secrets.filter((candidate) =>
        fibbleHistoryMatches(candidate, state.guesses)
      );
    }
    return data.secrets.filter((candidate) =>
      state.guesses.every((guess) => {
        const observedPattern = patternFromLetters(guess.letters);
        const candidatePattern = calculatePattern(candidate, guess.guess);
        return candidatePattern === observedPattern;
      })
    );
  }

  function fibbleHistoryMatches(secret, guesses) {
    return guesses.every((guess) => fibbleGuessMatches(secret, guess));
  }

  function fibbleGuessMatches(secret, guess) {
    const actual = scoreGuess(secret, guess.guess);
    let lies = 0;
    for (let idx = 0; idx < WORD_LENGTH; idx += 1) {
      if (actual[idx].state !== guess.letters[idx].state) {
        lies += 1;
        if (lies > 1) {
          return false;
        }
      }
    }
    return lies === 1;
  }

  function computeFibbleLieHints(possibleSecrets) {
    if (
      state.gameMode !== "fibble" ||
      !Array.isArray(possibleSecrets) ||
      !possibleSecrets.length ||
      !state.guesses.length
    ) {
      return null;
    }

    const totals = new Array(state.guesses.length).fill(0);
    const lieCounts = state.guesses.map(() => new Array(WORD_LENGTH).fill(0));

    for (const secret of possibleSecrets) {
      const lieIndexes = new Array(state.guesses.length);
      let valid = true;

      for (let rowIdx = 0; rowIdx < state.guesses.length; rowIdx += 1) {
        const guess = state.guesses[rowIdx];
        const actual = scoreGuess(secret, guess.guess);
        const lieIndex = findLieIndex(actual, guess.letters);
        if (lieIndex < 0) {
          valid = false;
          break;
        }
        lieIndexes[rowIdx] = lieIndex;
      }

      if (!valid) {
        continue;
      }

      for (let rowIdx = 0; rowIdx < lieIndexes.length; rowIdx += 1) {
        const lieIndex = lieIndexes[rowIdx];
        lieCounts[rowIdx][lieIndex] += 1;
        totals[rowIdx] += 1;
      }
    }

    return lieCounts.map((counts, rowIdx) =>
      counts.map((count) => ({
        isAlwaysLie: totals[rowIdx] > 0 && count === totals[rowIdx],
        isNeverLie: totals[rowIdx] > 0 && count === 0,
      }))
    );
  }

  function findLieIndex(actual, reported) {
    if (!Array.isArray(actual) || !Array.isArray(reported)) {
      return -1;
    }
    let lieIndex = -1;
    for (let idx = 0; idx < WORD_LENGTH; idx += 1) {
      if (!actual[idx] || !reported[idx]) {
        return -1;
      }
      if (actual[idx].state !== reported[idx].state) {
        if (lieIndex !== -1) {
          return -1;
        }
        lieIndex = idx;
      }
    }
    return lieIndex;
  }

  function rankGuessesByEntropy(possibleSecrets) {
    if (!possibleSecrets.length) {
      return [];
    }
    const remainingSet = new Set(possibleSecrets);
    const scored = data.secrets.map((guess) => ({
      guess,
      entropy: calculateEntropyForGuess(guess, possibleSecrets),
      possible: remainingSet.has(guess),
    }));

    scored.sort((a, b) => {
      if (b.entropy !== a.entropy) {
        return b.entropy - a.entropy;
      }
      if (a.possible !== b.possible) {
        return a.possible ? -1 : 1;
      }
      return a.guess.localeCompare(b.guess);
    });

    return scored.slice(0, ENTROPY_GUESS_LIMIT);
  }

  function calculateEntropyForGuess(guess, possibleSecrets) {
    const total = possibleSecrets.length;
    if (!total) {
      return 0;
    }

    const distribution = new Map();
    for (const secret of possibleSecrets) {
      const pattern = calculatePattern(secret, guess);
      distribution.set(pattern, (distribution.get(pattern) || 0) + 1);
    }

    let entropy = 0;
    distribution.forEach((count) => {
      const probability = count / total;
      entropy -= probability * Math.log2(probability);
    });
    return entropy;
  }

  function calculatePattern(secret, guess) {
    if (!secret || !guess) {
      return "";
    }
    const pattern = new Array(WORD_LENGTH);
    const leftovers = new Array(26).fill(0);

    for (let idx = 0; idx < WORD_LENGTH; idx += 1) {
      const secretChar = secret[idx];
      const guessChar = guess[idx];
      if (secretChar === guessChar) {
        pattern[idx] = TILE_STATE_CODE.correct;
      } else {
        pattern[idx] = TILE_STATE_CODE.absent;
        leftovers[letterIndex(secretChar)] += 1;
      }
    }

    for (let idx = 0; idx < WORD_LENGTH; idx += 1) {
      if (pattern[idx] === TILE_STATE_CODE.correct) {
        continue;
      }
      const lookup = letterIndex(guess[idx]);
      if (leftovers[lookup] > 0) {
        pattern[idx] = TILE_STATE_CODE.present;
        leftovers[lookup] -= 1;
      }
    }

    return pattern.join("");
  }

  function patternFromLetters(letters) {
    if (!Array.isArray(letters)) {
      return "";
    }
    return letters
      .map((entry) => TILE_STATE_CODE[entry.state] ?? TILE_STATE_CODE.absent)
      .join("");
  }

  function buildEntropyCacheKey() {
    const patternKey = state.guesses
      .map((guess) => `${guess.guess}-${patternFromLetters(guess.letters)}`)
      .join("|");
    return `${state.gameMode}:${state.playMode}:${patternKey}`;
  }

  function pickRandom(list) {
    return list[Math.floor(Math.random() * list.length)];
  }

  function maxAttempts() {
    return MAX_ATTEMPTS[state.gameMode] ?? MAX_ATTEMPTS.wordle;
  }

  function updateAttemptsText() {
    if (state.playMode === "notebook") {
      const label =
        state.gameMode === "fibble"
          ? "Fibble notebook • Click tiles (one lie per row) to match the feedback so suggestions update."
          : "Notebook mode • Click tiles to match the feedback so suggestions update.";
      elements.attempts.textContent = label;
      return;
    }

    if (!state.secret) {
      elements.attempts.textContent = "";
      return;
    }
    const attemptsUsed = state.guesses.length;
    const remaining = Math.max(maxAttempts() - attemptsUsed, 0);
    const label = `${remaining} attempt${remaining === 1 ? "" : "s"} remaining`;
    const suffix =
      state.gameMode === "fibble" ? " • Fibble lies once per row." : "";
    elements.attempts.textContent = `${label}${suffix}`;
  }

  function updateGuessFormState() {
    const disabled =
      !data.ready ||
      (state.playMode !== "notebook" && (!state.secret || state.complete));
    elements.guessInput.disabled = disabled;
    elements.guessButton.disabled = disabled;
  }

  function focusGuessInput() {
    if (elements.guessInput.disabled) {
      return;
    }
    elements.guessInput.focus();
  }

  function syncSecretInputState() {
    const isNotebookMode = elements.playModeSelect.value === "notebook";
    elements.secretInput.disabled = isNotebookMode;
    elements.secretInput.placeholder = isNotebookMode
      ? "not used in notebook mode"
      : "random";
    if (isNotebookMode) {
      elements.secretInput.value = "";
    }
  }

  function setMessage(message) {
    elements.status.textContent = message;
  }

  function letterIndex(letter) {
    return letter.charCodeAt(0) - 65;
  }

  async function loadWordList(path) {
    const response = await fetch(path);
    if (!response.ok) {
      throw new Error(`Unable to load ${path}`);
    }
    const text = await response.text();
    return text
      .split(/\r?\n/)
      .map((line) => line.trim().toUpperCase())
      .filter((word) => word.length === WORD_LENGTH);
  }
})();
