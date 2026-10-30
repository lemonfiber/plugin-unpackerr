# AGENTS.md — plugin-unpackerr

> **Start at the roadmap and board on [lemonfiber.app](https://lemonfiber.app),
> rendered from the report of where every unreleased version stands. Then the
> rules** every repository shares:
> [working in the repositories](https://github.com/lemonfiber/spec/blob/main/50-governance/working-in-the-repositories.md)
> and [the rules for agents](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> This file holds only what is true of this repository.

## What this repo is

Unpackerr as a first-party lemonfiber plugin: `plugin.toml` and the adapter that
speaks the `archive.extract@1` contract in front of Unpackerr. `watch` writes
`unpackerr.conf` into Unpackerr's configuration directory, and `working` answers
from the events Unpackerr posts the adapter. The adapter is the Rust crate at the
root, built on `lemonfiber-adapter-kit` from the core at the commit `Cargo.toml`
pins, into the image the `Dockerfile` builds. `release.yml` publishes that image
from every commit on `main` and from the train's tag. `plugin.toml` runs
Unpackerr alone, and its claim's recordings are Unpackerr's own answers. The
[bundle contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/bundle.md)
says how the catalogue pins this repository and how lemonfiber carries it.

## The rules you cannot break

- **The plugin id is the service's id** (`ARCH-R266`). The container, the
  configuration directory, the settings and the journal keep their names.
- **One service, speaking no contract**, while the bundle assembles it. It
  declares `replaces` naming its own id, which is how it takes the id, the port
  and the hostname the bundle reserves for it (`ARCH-R222`).
- **Its claims' recordings sit under `recordings/unpackerr/`**, and its starting
  files, where it has any, under `config/unpackerr/` (`ARCH-R271`).
- **Every service declares `reaches` and `asks_for`** (`ARCH-R270`).
- **No Python here.** The reader is spec's `first-party-plugin` workflow's.
- **No `unwrap`, `expect`, `panic` or indexing** outside tests. The lints in
  `Cargo.toml` are the core's.
- **`unpackerr.conf` is written owner-only** (`0600`), because it carries every
  curator's key, and through `lemonfiber_adapter_kit::shared`, so a link, a
  hardlink or a file that is not plain is refused by name or replaced and never
  written through.
- **A configuration changed by hand is never written over.** The first line of
  one the adapter writes carries the digest of the rest. A file whose digest no
  longer holds and that tells Unpackerr to watch something or tell someone is
  named in the refusal; the default Unpackerr's image writes sets nothing, and
  is taken as nobody's.
- **Every event Unpackerr posts proves itself with the token in its body**, read
  from the configuration the adapter wrote, or minted from the operating
  system's randomness where none was, and compared in constant time. The
  webhook is logged under a name rather than its address, the body is read up to
  `LARGEST`, one that does not parse is refused, and at most `MOST` downloads are
  held as extracting.
- **The core's crates move with the train.** The `rev` in `Cargo.toml` is the
  core commit `targets.toml` names, the same for every `lemonfiber-*` crate.
- **Moving Unpackerr's digest** means recording every response again against the
  new image in the same change, moving `RELEASE`, `DIGEST` and the digest of
  each platform's default configuration, `IMAGE_DEFAULTS`, in `src/adapter.rs`
  with it, and committing the `proofs.json` the checks write.
