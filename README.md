# Unpackerr for lemonfiber

**Extracts archived releases so they can be imported.**
Unpackerr is one of the parts the stack lemonfiber ships is built from.
Without it: Some downloads never import.

## How lemonfiber carries it

The default bundle in
[lemonfiber-plugins](https://github.com/lemonfiber/lemonfiber-plugins) pins this
repository by a revision and the digests of its manifest and its tree, and
lemonfiber compiles that tree in.
lemonfiber assembles Unpackerr into the stack it ships as a stack service.
Unpackerr's image is its own. The adapter that speaks lemonfiber's
`archive.extract@1` contract in front of it is built from this repository, and
its image is published from every commit on `main` and from the release train's
tag. The catalogue's pin moves by a reviewed pull request.

## What is here

| Path | What it is |
| --- | --- |
| `plugin.toml` | The part's manifest |
| `Cargo.toml`, `src/` | The adapter, built on `lemonfiber-adapter-kit` |
| `Dockerfile` | The adapter's image |
| `targets.toml` | The lemonfiber its checks run against |
| `proofs.json` | What those checks said, committed |

## Checks

The plugin verdict is one job of spec's
[`first-party-plugin`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-plugin.yml)
workflow: the manifest as the lemonfiber `targets.toml` names reads it, every
claim against its recording, and every pinned digest. `rust.yml` holds the
adapter to its format, lints, tests, every line covered and its dependency
policy. This repository holds no Python of its own. `just ci` runs the gates
that need nothing but this tree.

## Licence

The data in this repository is under the licence in `LICENSE`.
Unpackerr itself is under MIT and is not distributed here: this
repository names an image, it does not contain one.
