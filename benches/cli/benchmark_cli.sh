#!/bin/bash
# CLI benchmarks using hyperfine
#
# Note: Uses bash for arrays, but avoids bash 4+ features like associative arrays
# for macOS compatibility (which ships with bash 3.x)
#
# Usage:
#   ./benchmark_cli.sh              # Standard checks
#   ./benchmark_cli.sh experimental # Experimental checks
#   ./benchmark_cli.sh all          # All checks

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/../.."
FIXTURES_DIR="$SCRIPT_DIR/fixtures"

# Check profiles (avoiding associative arrays for bash 3.x compatibility)
get_checks() {
    case "$1" in
        standard)     echo "syntax,duppatterns,files" ;;
        experimental) echo "notowned,avoid-shadowing" ;;
        all)          echo "syntax,duppatterns,files,notowned,avoid-shadowing" ;;
        *)            return 1 ;;
    esac
}

PROFILE="${1:-standard}"
CHECKS=$(get_checks "$PROFILE") || {
    echo "Unknown profile: $PROFILE"
    echo "Available: standard, experimental, all"
    exit 1
}

# Check for required tools
if ! command -v hyperfine &> /dev/null; then
    echo "Error: hyperfine is not installed"
    echo "Install with: cargo install hyperfine"
    echo "Or see: https://github.com/sharkdp/hyperfine#installation"
    exit 1
fi

echo "Profile: $PROFILE ($CHECKS)"

# Build release binaries
echo "Building..."
cargo build --release -p codeowners-cli --features generate

BINARY="$PROJECT_ROOT/target/release/codeowners-validator"
GENERATOR="$PROJECT_ROOT/target/release/generate-fixtures"

# Generate fixtures
echo "Generating fixtures..."
mkdir -p "$FIXTURES_DIR"
"$GENERATOR" "$FIXTURES_DIR"

# Discover fixtures
# Note: Glob expands in sorted order; fixture names are controlled (no spaces)
FIXTURES=("$FIXTURES_DIR"/*.codeowners)

# Check if glob matched anything (bash sets array to literal pattern if no match)
if [[ ! -f "${FIXTURES[0]:-}" ]]; then
    echo "No fixtures found in $FIXTURES_DIR"
    exit 1
fi

echo "Found ${#FIXTURES[@]} fixtures: ${FIXTURES[*]##*/}"

# Create temp repos
TEMP_DIRS=()
cleanup() {
    for d in "${TEMP_DIRS[@]}"; do
        rm -rf "$d"
    done
}
trap cleanup EXIT

# Build hyperfine command
HYPERFINE_ARGS=(
    --warmup 3
    --min-runs 10
    --export-json "$SCRIPT_DIR/results-${PROFILE}.json"
    --export-markdown "$SCRIPT_DIR/results-${PROFILE}.md"
)

for fixture in "${FIXTURES[@]}"; do
    name=$(basename "$fixture" .codeowners)
    tmp=$(mktemp -d)
    TEMP_DIRS+=("$tmp")
    mkdir -p "$tmp/.github"
    cp "$fixture" "$tmp/.github/CODEOWNERS"
    
    # Build the command based on profile
    case "$PROFILE" in
        standard)
            HYPERFINE_ARGS+=(-n "$name" "$BINARY --checks $CHECKS $tmp")
            ;;
        experimental)
            HYPERFINE_ARGS+=(-n "$name" "$BINARY --experimental-checks $CHECKS $tmp")
            ;;
        all)
            # For 'all', split standard and experimental checks
            HYPERFINE_ARGS+=(-n "$name" "$BINARY --checks syntax,duppatterns,files --experimental-checks notowned,avoid-shadowing $tmp")
            ;;
    esac
done

echo "Running hyperfine..."
hyperfine "${HYPERFINE_ARGS[@]}"

echo ""
echo "Results saved to:"
echo "  $SCRIPT_DIR/results-${PROFILE}.json"
echo "  $SCRIPT_DIR/results-${PROFILE}.md"
