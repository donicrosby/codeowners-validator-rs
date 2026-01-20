# Benchmarks

Benchmarks use AST-based fixture generation for guaranteed valid CODEOWNERS files.
Fixtures are generated at runtime using deterministic seeds - no external files needed.

## Quick Start

```bash
# Rust core library benchmarks
cargo bench -p codeowners-validator-core --features generate

# CLI benchmarks (requires: cargo install hyperfine)
./benches/cli/benchmark_cli.sh              # Standard checks
./benches/cli/benchmark_cli.sh experimental # Experimental checks
./benches/cli/benchmark_cli.sh all          # All checks

# Python benchmarks
cd crates/codeowners-python-bindings
uv run pytest tests/test_benchmarks.py --benchmark-only
```

## Fixture Sizes

| Name    | Rules  | Approx Size |
|---------|--------|-------------|
| small   | 10     | ~500 bytes  |
| medium  | 100    | ~5 KB       |
| large   | 1,000  | ~50 KB      |
| xlarge  | 10,000 | ~500 KB     |
| max     | ~60k   | ~3 MB       |

## Benchmark Groups

### Standard Checks
- `syntax` - Syntax validation
- `duppatterns` - Duplicate pattern detection  
- `files` - Pattern-to-file matching

### Experimental Checks
- `notowned` - Files not covered by any rule
- `avoid-shadowing` - Pattern shadowing detection

## Filtering Benchmarks

```bash
# Rust - filter by group
cargo bench -p codeowners-validator-core --features generate -- "parsing"
cargo bench -p codeowners-validator-core --features generate -- "checks/standard"
cargo bench -p codeowners-validator-core --features generate -- "checks/experimental"

# Python - filter by marker
uv run pytest tests/test_benchmarks.py --benchmark-only -k "parse"
uv run pytest tests/test_benchmarks.py --benchmark-only -k "experimental"
```

## Adding New Fixture Sizes

1. Add preset method to `GeneratorConfig` in `generate.rs` (e.g., `GeneratorConfig::huge()`)
2. Add `LazyLock` static and entry in `benches/fixtures.rs` (Rust)
3. Add to `PRESETS` array in `generate_fixtures.rs` (CLI)
4. Add to `FIXTURE_CONFIGS` and fixture function in `conftest.py` (Python)

## Comparing Results

```bash
# Python - compare against baseline
uv run pytest tests/test_benchmarks.py --benchmark-compare

# Rust - automatic comparison
cargo bench -p codeowners-validator-core --features generate
```

## How It Works

The generator uses the AST types directly:

1. `GeneratorConfig` specifies rules, comments, seed
2. `generate_ast()` builds a valid `CodeownersFile` AST
3. `Display` impl serializes AST to string
4. Result is always valid (round-trip guaranteed)

This approach:
- Eliminates external file dependencies
- Guarantees syntactically valid output
- Supports deterministic reproducible benchmarks
- Scales to GitHub's 3MB limit

### Temp Repo for File Checks

For file-dependent checks (`FilesCheck`, `NotOwnedCheck`), a temporary directory is created with files matching the generated patterns:

```
temp_repo/
├── file.rs, file.py, file.js, ...     # Root files (*.{ext})
├── src/, lib/, tests/, docs/, ...      # Directories
│   ├── file.rs, file.py, ...           # Files in dirs (/{dir}/*.{ext})
│   └── sub/
│       ├── nested.rs, ...              # Nested files (/{dir}/**)
│       └── test_example.rs, ...        # Test files (test_*.{ext})
├── docs/guide/
│   └── *.md                            # Markdown files
└── vendor/                             # For negation patterns
```

This ensures benchmarks measure real file system operations.

## Architecture

```
codeowners-validator-core/src/generate.rs
    └── GeneratorConfig + generate() + generate_ast()
           │
           ├── Rust Benchmarks (benches/fixtures.rs)
           │   └── LazyLock<String> static fixtures
           │
           ├── CLI Benchmarks (crates/codeowners-cli/src/bin/generate_fixtures.rs)
           │   └── Writes .codeowners files for hyperfine
           │
           └── Python Benchmarks (generate_codeowners_fixture in lib.rs)
               └── conftest.py uses @lru_cache for efficient fixture generation
```

## Feature Flags

The `generate` feature controls fixture generation:

- **codeowners-validator-core**: Optional (off by default)
- **codeowners-cli**: Optional (needed for `generate-fixtures` binary)
- **codeowners-python-bindings**: Enabled by default (for benchmarking convenience)

To build without generation:
```bash
cargo build --no-default-features -p codeowners-python-bindings
```
