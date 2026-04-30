# Release

Publishing to crates.io is automated via GitHub Actions but requires a manual
tag push to initiate.

## Prerequisites

The release workflow uses [crates.io Trusted Publishing][trusted-publishing]
to authenticate via OIDC, so no long-lived `CARGO_REGISTRY_TOKEN` is required.
The following one-time setup must be in place:

1. The repository's `release` GitHub Environment exists (Settings →
   Environments). Required reviewers can be configured here to gate the
   publish step on a manual approval.
2. A Trusted Publisher entry exists on
   [crates.io for `arcjet-gravity`](https://crates.io/crates/arcjet-gravity/settings)
   with:
   - Repository owner: `arcjet`
   - Repository name: `gravity`
   - Workflow filename: `release.yml`
   - Environment: `release`

   This must be added by a crate owner. Members of the `arcjet/rust-team`
   GitHub team inherit owner access via the team owner on the crate.

[trusted-publishing]: https://crates.io/docs/trusted-publishing

## Process

1. Update the version in `cmd/gravity/Cargo.toml` and land the change on `main`.
2. Tag the release commit and push the tag:
   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```
3. The [Release workflow](.github/workflows/release.yml) runs automatically:
   1. **Test** — runs the full test suite (cargo test, go test, snapshot tests).
   2. **Publish** — authenticates to crates.io via Trusted Publishing and
      publishes `arcjet-gravity`. If required reviewers are configured on the
      `release` environment, this step waits for approval first.
   3. **GitHub Release** — creates a GitHub Release with auto-generated notes.
4. Verify the release on [crates.io](https://crates.io/crates/arcjet-gravity)
   and on the repository's
   [Releases page](https://github.com/arcjet/gravity/releases).

## Versioning

This project follows [Semantic Versioning](https://semver.org/). While the
project is in early development (`0.x`), minor version bumps may include
breaking changes.

## Troubleshooting

- **Publish fails with an authentication error**: confirm the Trusted Publisher
  entry on crates.io matches the repository, workflow filename, and environment
  name exactly. The `permissions: id-token: write` block must be present on
  the `publish` job.
- **Publish fails with version conflict**: the version in `Cargo.toml` must be
  greater than the latest published version on crates.io. You cannot re-publish
  a version that has already been published.
- **Tests fail**: the release is aborted. Fix the issue on `main`, delete the
  tag (`git push origin :refs/tags/v0.1.0`), and start again.
