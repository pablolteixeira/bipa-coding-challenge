## 1. README

- [ ] 1.1 Overview + quick start (`docker compose up --build`) + `curl` example with output
- [ ] 1.2 Local dev path (`docker compose up -d db` + `cargo run`) and configuration table
- [ ] 1.3 Testing section: unit vs integration, DB requirement (`docker compose up -d db` + `DATABASE_URL`), `cargo test -- --ignored` for the live test
- [ ] 1.4 Template sections: build tools & versions, steps to run, focus, time spent, trade-offs (capacity as dot-separated string; snapshot replace; single instance; no CI), weakest part, other info
- [ ] 1.5 Link to `docs/architecture.md`

## 2. Final review

- [ ] 2.1 Follow the README steps from a fresh clone in a temp dir (compose up, curl, tests)
- [ ] 2.2 Run `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` locally
- [ ] 2.3 Review comments and `///` docs on public items. Check `cargo doc --no-deps` has no warnings
- [ ] 2.4 `grep -rn "unwrap()\|expect(" src/` shows only test code
