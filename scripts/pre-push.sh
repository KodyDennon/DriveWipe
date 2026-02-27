#!/bin/bash

# DriveWipe Pre-push Hook
# Automates versioning based on commit messages and LOC safety triggers.

echo "🚀 Running DriveWipe versioning check..."

# Run the xtask versioning tool
cargo run --package xtask -- bump

# Check if any Cargo.toml files were modified
MODIFIED_FILES=$(git status --porcelain | grep "Cargo.toml")

if [ -n "$MODIFIED_FILES" ]; then
    echo "📝 Version bumps detected. Committing changes locally..."
    git add crates/*/Cargo.toml
    git commit -m "chore(version): automated version bump [skip ci]"
    echo "⚠️  Versions updated locally. Please run 'git push' again to include the version bump."
    exit 1
fi

echo "✅ Version check complete."
exit 0
