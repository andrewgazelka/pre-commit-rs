# TODO: Extract to Separate Repository

## Goal
Move this composite action to its own dedicated repository for better discoverability and GitHub Marketplace publishing.

## Why
- Cleaner reference: `andrewgazelka/setup-pre-commit@v1` instead of `andrewgazelka/pre-commit-rs/.github/actions/setup-pre-commit@main`
- Can publish to GitHub Marketplace
- Independent versioning and release cycle
- More discoverable for users
- Follows GitHub Actions best practices for public actions

## Steps
1. Create new repository: `andrewgazelka/setup-pre-commit`
2. Move `action.yml` and `README.md` to the root of the new repo
3. Set up versioning with git tags (v1, v1.0.0, etc.)
4. Optionally publish to GitHub Marketplace
5. Update documentation to reference the new action location
6. Keep this action in sync or deprecate it in favor of the standalone repo

## References
- [GitHub Docs: Publishing actions in GitHub Marketplace](https://docs.github.com/en/actions/sharing-automations/creating-actions/publishing-actions-in-github-marketplace)
- [Best practices for composite actions](https://multiprojectdevops.github.io/tutorials/2_composite_actions/)
