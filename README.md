# `hearth`

*Environments as Git trees.*

<!-- rumdl-disable MD013 -->
[![CI](https://github.com/git-ents/git-hearth/actions/workflows/CI.yml/badge.svg)](https://github.com/git-ents/git-hearth/actions/workflows/CI.yml)
[![CD](https://github.com/git-ents/git-hearth/actions/workflows/CD.yml/badge.svg)](https://github.com/git-ents/git-hearth/actions/workflows/CD.yml)
<!-- rumdl-enable MD013 -->

> [!CAUTION]
> This project is being actively developed!
> Despite this, semantic versioning rules will be respected.
> Expect frequent updates.

## About

Hearth is an environment manager backed by Git's content-addressed object store.
It treats build environments as pure compositions of content-addressed filesystem
trees, providing reproducible, inspectable, and shareable environments with
graduated isolation — from convention-only to full VM-based sandboxing.

See [`docs/design/hearth.md`](docs/design/hearth.md) for the full design specification.

## Crates

| Crate | Description |
|---|---|
| [`hearth`](crates/hearth/) | Environments as Git trees. |
