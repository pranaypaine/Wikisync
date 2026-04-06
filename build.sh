#!/usr/bin/env bash
# build.sh — build the complete wiki into a single binary
set -e

echo "==> Building React frontend..."
cd frontend
npm ci --silent
npm run build
cd ..

echo "==> Building Rust backend (release)..."
cargo build --release

echo ""
echo "==> Done! Binary at: target/release/wiki-server"
echo ""
echo "Run with:"
echo "  DATABASE_URL=sqlite:wiki.db JWT_SECRET=your-secret ./target/release/wiki-server"
