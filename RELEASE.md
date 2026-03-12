# Release

Publishing to crates.io is automated via GitHub Actions but requires a manual
tag push to initiate.

## Prerequisites

A `CARGO_REGISTRY_TOKEN` secret must be configured in the repository's GitHub
settings (Settings → Secrets and variables → Actions). This token must have
publish permissions for the `arcjet-gravity` crate on crates.io.

## Process

It looks like this:

1. Update the version in `cmd/gravity/Cargo.toml` and land the change on `main`.
2. Tag the release commit and push the tag:
   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. The [Release workflow](.github/workflows/release.yml) runs automatically:
   1. **Test** — runs the full test suite (cargo test, go test, snapshot tests).
   2. **Publish** — publishes `arcjet-gravity` to crates.io.
   3. **GitHub Release** — creates a GitHub Release with auto-generated notes.
4. Verify the release on [crates.io](https://crates.io/crates/arcjet-gravity)
   and on the repository's
   [Releases page](https://github.com/arcjet/gravity/releases).

## Versioning

This project follows [Semantic Versioning](https://semver.org/). While the
project is in early development (`0.x`), minor version bumps may include
breaking changes.

## Troubleshooting

- **Publish fails with permission error**: check that the `CARGO_REGISTRY_TOKEN`
  secret is set and the token has not expired.
- **Publish fails with version conflict**: the version in `Cargo.toml` must be
  greater than the latest published version on crates.io. You cannot re-publish
  a version that has already been published.
- **Tests fail**: the release is aborted. Fix the issue on `main`, delete the
  tag (`git push origin :refs/tags/v0.1.0`), and start again.
