# Release Process

This project uses [cargo-release](https://github.com/crate-ci/cargo-release) to automate the release process. When you create a release, it will automatically:

1. Bump the version in `Cargo.toml`
2. Update version references in `README.md`
3. Create a signed git commit
4. Create a signed git tag (e.g., `v0.2.0`)
5. Push the commit and tag to GitHub
6. Trigger GitHub Actions to build and publish Docker images
7. Create a GitHub Release with notes

## Prerequisites

### Install cargo-release

```bash
cargo install cargo-release
```

### Verify Git Configuration

Since you're using SSH signing for commits (vigilance mode), ensure your git is configured:

```bash
# Verify SSH signing is configured
git config --get gpg.format
# Should output: ssh

git config --get user.signingkey
# Should output your SSH key path or key

git config --get commit.gpgsign
# Should output: true

git config --get tag.gpgsign
# Should output: true
```

If not configured, set it up:

```bash
git config --global gpg.format ssh
git config --global user.signingkey ~/.ssh/your_signing_key.pub
git config --global commit.gpgsign true
git config --global tag.gpgsign true
```

## Creating a Release

### 1. Ensure you're on the main branch and up to date

```bash
git checkout main
git pull
```

### 2. Run cargo-release

For a patch release (0.1.0 → 0.1.1):
```bash
cargo release patch --execute
```

For a minor release (0.1.0 → 0.2.0):
```bash
cargo release minor --execute
```

For a major release (0.1.0 → 1.0.0):
```bash
cargo release major --execute
```

### 3. What happens automatically

When you run `cargo release`:

1. **Local changes:**
   - Updates version in `Cargo.toml` (e.g., `0.1.0` → `0.2.0`)
   - Updates `README.md` to reference the new version
   - Creates a commit: `chore: Release reddit-notifier version 0.2.0`
   - Creates a signed tag: `v0.2.0`
   - Pushes both commit and tag to GitHub

2. **GitHub Actions automatically:**
   - Detects the `v*.*.*` tag
   - Runs the `release.yml` workflow (the CI workflow `ci.yml` will skip building since it detects the release tag)
   - Creates a GitHub Release at `https://github.com/mandreko/reddit-notifier/releases/tag/v0.2.0`
   - Builds the Docker image
   - Pushes Docker images with tags:
     - `ghcr.io/mandreko/reddit-notifier:0.2.0`
     - `ghcr.io/mandreko/reddit-notifier:0.2`
     - `ghcr.io/mandreko/reddit-notifier:0`
     - `ghcr.io/mandreko/reddit-notifier:latest`

   **Note:** The CI workflow is smart enough to skip building when a release tag is present, so you won't build Docker images twice.

### 4. Verify the release

After running `cargo release`, you can verify:

```bash
# Check the git log
git log --oneline -5

# Check tags
git tag -l

# View the tag signature
git tag -v v0.2.0
```

Then check GitHub:
- Visit `https://github.com/mandreko/reddit-notifier/releases` to see the release
- Visit `https://github.com/mandreko/reddit-notifier/pkgs/container/reddit-notifier` to see Docker images
- Check the Actions tab to monitor the Docker build

## Dry Run (Preview Mode)

To see what would happen without actually making changes:

```bash
cargo release patch --dry-run
```

This will show you:
- What version changes would be made
- What files would be modified
- What commits and tags would be created
- But won't actually push anything

## Rolling Back a Release

If you need to undo a release that was pushed but before the GitHub Actions complete:

```bash
# Delete the remote tag
git push origin :refs/tags/v0.2.0

# Delete the local tag
git tag -d v0.2.0

# Reset to the commit before the release
git reset --hard HEAD~1
git push origin main --force
```

**Note:** Only do this immediately after pushing. Once Docker images are published and users may have pulled them, you should create a new patch release instead.

## Troubleshooting

### Error: "failed to sign tag"

This means your SSH signing isn't configured properly. Verify:
```bash
git config --get gpg.format
git config --get user.signingkey
```

### Error: "failed to push"

Ensure you have push access to the repository and your SSH key is added to GitHub.

### GitHub Actions doesn't trigger

Ensure:
1. The tag format is `v*.*.*` (e.g., `v0.2.0`)
2. Check the Actions tab for any errors
3. Verify the workflow file exists at `.github/workflows/release.yml`

## Advanced: Custom Versioning

You can also specify an exact version:

```bash
cargo release 1.2.3 --execute
```

Or bump pre-release versions:

```bash
cargo release patch --pre-release alpha --execute  # 0.1.0 → 0.1.1-alpha.1
```

## More Information

For more cargo-release options:
```bash
cargo release --help
```

Official documentation: https://github.com/crate-ci/cargo-release
