# Fibble

Play classic Wordle or chaotic Fibble either in the terminal or inside your browser. The repository started as a command-line solver/assistant written in Rust and now includes a static site that runs directly from the `docs/` folder so it can be hosted on GitHub Pages.

## Command-line usage

```bash
cargo run --release -- [--mode wordle|fibble] [--secret WORD]
```

- `--mode wordle` (default) gives you six traditional Wordle guesses.
- `--mode fibble` gives you nine guesses but one tile in every row lies about its color. The CLI plays a random opener automatically in this mode.
- `--secret WORD` lets you supply the hidden word for practice sessions.

## Browser version

The `docs/` directory contains a completely static site (`index.html`, `styles.css`, `script.js`) plus copies of the Wordle word lists under `docs/assets/`. You can preview it locally with any static file server:

```bash
cd docs
python3 -m http.server 8000
```

Then open <http://localhost:8000/> and start playing. Wordle mode behaves just like the CLI, while Fibble mode plays a random automatic opener and applies one random lie to every guess.

### Deploying on GitHub Pages

1. Commit/push the repository with the `docs/` folder.
2. In the repository settings on GitHub, open **Pages** and choose the **Deploy from a branch** option.
3. Select the `main` branch and the `/docs` folder, then save.
4. GitHub will build and host the site automatically. Your site will be available at `https://<username>.github.io/<repo>/` once the build finishes.

Because everything is static there is nothing else to build—refresh the page after you push changes to update the live site.
