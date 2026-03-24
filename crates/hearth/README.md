# 🔥 `hearth`

*Environments as Git trees.*

> [!CAUTION]
> This project is in active development and has not yet been published to crates.io.
> Please file a [new issue] for any misbehaviors you find!

[new issue]: https://github.com/git-ents/git-forge/issues/new

## Overview

Hearth is an environment manager backed by Git's content-addressed object store.
It treats environments as compositions of filesystem trees — toolchains, SDKs, and other dependencies are imported once, stored as Git tree objects, and merged on demand into a single environment tree that can be materialized to disk as a hardlink farm.

Key ideas:

- **Content-addressed** — every tree and blob is identified by its hash;
  identical content is stored once.
- **Composable** — environments are built by overlaying named toolchains and
  raw trees in a declared order.
- **Graduated isolation** — level 0 inherits the host, level 1 replaces
  `PATH` with declared inputs only, level 2 additionally makes the store
  read-only and captures writes.
- **Inspectable** — environments are plain Git trees; `git ls-tree`, `git
  diff-tree`, and `hearth diff` all work.

### Import sources

| Source | Command | Notes |
|---|---|---|
| Local directory | `hearth import dir <path>` | Blob writes parallelized across cores |
| Tarball | `hearth import tarball <path>` | `.tar` and `.tar.gz`; supports `--strip-prefix` |
| OCI image layout | `hearth import oci <path>` | Whiteout handling; registry pull not yet implemented |

### Configuration

<!-- rumdl-disable MD013 -->

Hearth reads two files from `.forge/` in the project root:

**`.forge/toolchains.toml`** declares named toolchains with a source URI and a
content-addressed tree hash managed by hearth:

```toml
[rust]
source = "https://static.rust-lang.org/dist/rust-1.87.0-x86_64-unknown-linux-gnu.tar.gz"
oid = "a3f1c9d..."

[python]
source = "git://kiln-packages/python@3.12"
oid = "b72e4f8..."
```

**`.forge/environment.toml`** references toolchains by name and may also list
raw tree hashes as an escape hatch:

```toml
[project]
default = "rust"

[env.rust]
toolchains = ["rust"]
isolation = 1

[env.dev]
extends = "rust"
toolchains = ["python"]
extras = ["/usr/bin"]
```

<!-- rumdl-enable MD013 -->

## Example: Rust toolchain with isolation 1

<!-- rumdl-disable MD013 MD014 -->

The following is a complete, copy-paste-able walkthrough that imports a Rust
toolchain, configures an environment, and builds this workspace inside it with
isolation level 1 (declared inputs only — `PATH` is replaced so only tools from
the environment tree are available).

```sh
# 1. Download and unpack the Rust toolchain.
curl -LO https://static.rust-lang.org/dist/rust-1.87.0-x86_64-unknown-linux-gnu.tar.gz

# 2. Import the tarball into the hearth store.
#    --strip-prefix 1 removes the top-level directory inside the archive.
RUST_TREE=$(hearth import tarball rust-1.87.0-x86_64-unknown-linux-gnu.tar.gz --strip-prefix 1)
echo "imported rust tree: $RUST_TREE"

# 3. Write the toolchains config.
mkdir -p .forge
cat > .forge/toolchains.toml <<EOF
[rust]
source = "https://static.rust-lang.org/dist/rust-1.87.0-x86_64-unknown-linux-gnu.tar.gz"
oid = "$RUST_TREE"
EOF

# 4. Write the environment config.
cat > .forge/environment.toml <<EOF
[project]
default = "rust"

[env.rust]
toolchains = ["rust"]
isolation = 1
EOF

# 5. Enter the environment with isolation 1.
#    PATH is replaced with <store>/bin — only Rust tooling is available.
hearth enter --isolation 1

# 6. Inside the hearth shell, build the workspace.
cargo build --workspace
```

<!-- rumdl-enable MD013 MD014 -->

## Installation

### CLI

The `hearth` command can be installed with `cargo install`.

```shell
cargo install --locked --git https://github.com/git-ents/git-forge.git hearth
```

If `~/.cargo/bin` is on your `PATH`:

```shell
hearth -h
```

### Library

The `hearth` library can be added to your Rust project via `cargo add`.

```shell
cargo add --git https://github.com/git-ents/git-forge.git hearth
```

### Library usage

```rust
use std::path::Path;
use hearth::store::Store;
use hearth::import::import_dir;
use hearth::env::{load_config, load_toolchains, resolve_env, resolve_extras};
use hearth::exe::{self, Isolation};

// Open (or initialize) the default store at ~/.hearth/.
let store = Store::open_default().unwrap();

// Import a local directory as a component tree.
let tree_oid = import_dir(&store, Path::new("/path/to/toolchain")).unwrap();
println!("imported: {tree_oid}");

// Resolve and enter a configured environment.
let cfg = load_config(Path::new(".forge/environment.toml")).unwrap();
let tc = load_toolchains(Path::new(".forge/toolchains.toml")).unwrap();
let env_name = cfg.default_env();
let extras = resolve_extras(&cfg, env_name).unwrap();
let merged = resolve_env(&store, &cfg, Some(&tc), env_name).unwrap();

// Materialize the tree to disk and get its path.
let env_path = store.materialize(merged).unwrap();
println!("environment at: {}", env_path.display());
```
