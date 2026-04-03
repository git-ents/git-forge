# `forge server` subcommand

## Problem

`forge-server` is a standalone binary that syncs forge refs with GitHub.
Users must discover, install, and run it separately from the `forge` CLI.
Making it a subcommand gives single-binary distribution and discoverability.

## Approach

### 1. Extract forge-server logic into a library

Add `lib.rs` to the `forge-server` crate exposing the core sync loop.
Keep `main.rs` as a thin wrapper so the standalone binary still works.

- `forge-server/src/lib.rs` — exports `run()`, `SYNC_REF_PREFIXES`,
  `sync_one()`, `fetch_forge_refs()`, `push_forge_refs()`, and
  `ServerConfig` (a plain struct replacing the clap `Args`)
- `forge-server/src/main.rs` — parses clap args, builds `ServerConfig`,
  calls `forge_server::run()`

### 2. Add `daemonix` to workspace

`daemonix` is a maintained fork of the `daemonize` crate.
It handles fork, pidfile, and stdio redirect.

- Add `daemonix` to `[workspace.dependencies]` in root `Cargo.toml`
- Add `daemonix` as optional dep to `git-forge`

### 3. Add `server` feature to git-forge

```toml
server = ["cli", "dep:forge-server", "dep:forge-github", "dep:tokio", "dep:daemonix"]
default = ["cli", "exe", "server"]
```

### 4. Add `ServerCommand` to CLI

Behind `#[cfg(feature = "server")]`:

```rust
Server {
    #[command(subcommand)]
    command: ServerCommand,
}
```

```rust
enum ServerCommand {
    Start {
        poll_interval: u64,   // default 60
        remote: String,       // default "origin"
        no_sync_refs: bool,
        once: bool,
        foreground: bool,
    },
    Stop,
    Status,
}
```

### 5. Implement in executor

Behind `#[cfg(feature = "server")]`:

- **start**: prompt "Start forge sync daemon?" (unless `--foreground` or
  `--once`), write pidfile to `.git/forge-server.pid`, daemonize (unless
  foreground/once), init tokio runtime, call `forge_server::run()`
- **stop**: read pidfile, send SIGTERM, remove pidfile
- **status**: read pidfile, check if process alive, print status

### Files touched

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `daemonix` workspace dep |
| `crates/forge-server/src/lib.rs` | **New** — extract sync logic |
| `crates/forge-server/src/main.rs` | Thin wrapper calling lib |
| `crates/git-forge/Cargo.toml` | Add `server` feature + optional deps |
| `crates/git-forge/src/cli.rs` | Add `ServerCommand` |
| `crates/git-forge/src/exe.rs` | Add server dispatch |

### Commit plan

1. **refactor: extract forge-server sync logic into library** — lib.rs + thin main.rs
2. **feat: add `forge server` subcommand with daemonize support** — daemonix dep, feature flag, CLI, executor
