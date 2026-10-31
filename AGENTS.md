# Working on Quiddity

Notes for anyone (person or agent) changing this repository. The design is in
[docs/design.md](docs/design.md); the layout and commands are in the
[README](README.md). Crates with non-obvious behaviour have their own
`AGENTS.md`, and so do the apps (`apps/ui`, `apps/app`): read it before
changing that crate or app. Each one has a `CLAUDE.md` beside it that imports
it with `@AGENTS.md`, so Claude Code loads it too.

## Git

- **Stacked pull requests, one commit each.** Every PR is a single,
  self-contained commit that builds and passes on its own. A change that
  depends on another goes in a PR stacked on top of it.
- **Fix-ups are squashed** into the commit (and PR) they fix, not added as
  new commits.
- **Rebase, never merge.** History is linear: no merge commits on branches.
  Fetch `main` immediately before rebasing onto it, since it moves often, and
  push with `--force-with-lease`.
- Commit messages say why, not just what, in plain prose.

## Writing

British English, in code, comments, docs and commit messages. Comments
explain what isn't obvious from the code; doc comments say what a thing is
for.

## Documentation

`docs/` is a graph in Quiddity's own format, and CI publishes it to GitHub
Pages as the UI showing its own documentation (see
[Publishing](docs/Publishing.md)). Keep it current as you change things:

- **Update the pages a change affects in the same commit.** A page per topic,
  named for the topic (`docs/Static build.md`), linking to related pages with
  `[[double brackets]]`. A page that is no longer true is worse than none.
- **Add a journal entry** in `docs/journals/YYYY-MM-DD.md` for the day's work,
  or add to the day's entry if it exists: what changed and why, and what the
  next person should know, linking to the pages it touches. The format is the
  Markdown outline of the text export: `- ` bullets, children indented with a
  tab.
- Check it imports and builds with `mise run docs`.

## Testing

Tests should be useful, not numerous: check each behaviour once, at the
layer that owns it.

- **Each layer is tested on its own.** `cargo mutants` runs each mutant
  against its own crate's tests only, so a crate can't lean on another
  crate's tests to catch its bugs.
- **Property tests first**, with generators weighted towards the inputs that
  matter (the characters a parser treats specially, say), so that a few
  dozen cases find the edges. A handful of named examples cover what a
  generator is unlikely to hit.
- **Fuzzing** covers arbitrary input: nothing panics, and formatting
  reaches a fixed point. A fuzz crash becomes a seed in `fuzz/seeds/` and a
  regression test in the crate it's about.
- **Proptest regressions** (`*.proptest-regressions`) are committed: they
  replay past failures.
- Check a new test fails without the fix before trusting it.

```sh
cargo nextest run                               # the suite
PROPTEST_CASES=5000 cargo nextest run --release # deeper
PROPTEST_CASES=32 cargo mutants -p <crate>      # are the tests any good?
cargo +nightly fuzz run --target x86_64-unknown-linux-gnu <target>
```

### Mutants

CI runs `cargo mutants` over the lines each PR changes; nothing may be
missed. When a mutant survives:

1. If it changes behaviour, add the test that notices: usually a stronger
   property or a better-weighted generator rather than one more example.
2. If it can't change behaviour, the code is redundant: simplify it (a
   guard another check already implies, a branch equal to the default).
3. Only if neither applies, exclude it in `.cargo/mutants.toml`, with the
   reason in a comment.

A timeout counts as caught, but costs the run 20 seconds a time: if a test
sends a mutant into a loop, check for the fast failure first.

### Tools

- `mise tasks` lists the project's tasks: `mise run app` builds and launches
  the app, `mise run check` runs what CI does apart from fuzzing and mutation
  testing. They are defined in `mise.toml`; `scripts/doctor.sh` checks what mise
  can't install (Tauri's system libraries on Linux) and is the first step of the
  tasks that need it. Keep the tasks in step with CI when either changes.

- The UI embeds the core compiled to WASM, so it needs the
  `wasm32-unknown-unknown` target and a `wasm-bindgen` CLI of exactly the
  version `crates/wasm` pins (`mise install` gets both). Browser tests need
  Playwright's Chromium: see [apps/ui](apps/ui/AGENTS.md).
- `cargo mutants` copies the source tree, build output included: delete
  `apps/app/src-tauri/target` first, or use `-j 1`, if the disk is small.

- `cargo fuzz` installed as a prebuilt binary targets musl; pass
  `--target x86_64-unknown-linux-gnu`.
- The pre-commit hooks include zizmor, which needs the GitHub API: in a
  sandbox without it, it fails without auditing anything. CI runs it.
