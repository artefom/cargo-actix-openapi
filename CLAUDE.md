# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

cargo-actix-openapi is a Rust CLI tool that generates Actix-web server implementations from OpenAPI 3.0.3 specifications. It produces API traits, request/response models, error types, and route registration code from OpenAPI YAML files.

## Build Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo install --path .         # Install as cargo subcommand
cargo run -- <spec_dir> <out>  # Run directly
```

## Testing

```bash
cargo test                     # Run all tests
cargo test <test_name>         # Run specific test
cargo clippy                   # Lint
```

Tests use `rstest` for parameterized testing. Test specs are in `tests/openapi/` with expected outputs in `tests/expected/`. Set `OVERWRITE = true` in `tests/test_specs.rs` to regenerate expected outputs.

## Architecture

### Code Generation Pipeline

```
OpenAPI YAML → Parse (serde_yaml) → Internal Type System → Tera Templates → Rust Code
```

### Key Modules

- **`main.rs`**: CLI entry point using clap. Scans for `openapi*.yaml` files in spec directory.
- **`generator.rs`**: Main generation logic. Converts OpenAPI specs to internal types, then renders templates.
- **`generator/models.rs`**: Internal type system (`RStruct`, `REnum`, `RApiErr`, `RustOperation`) mapping OpenAPI concepts to Rust constructs.
- **`generator/models/types.rs`**: OpenAPI-to-Rust type mapping (e.g., `string` → `String`, `integer:int32` → `i32`).
- **`openapictx.rs`**: OpenAPI reference resolution (`$ref` dereferencing) and parameter categorization.
- **`generator/templates.rs`**: Tera template engine setup with custom filters (`quote`, `comment`, `indent`).

### Templates (`generator/static/`)

- `api.tera`: API trait and `make_scope()` function
- `struct.tera`, `enum.tera`, `error.tera`: Type definitions
- `default.tera`: Default value implementations
- `docs.html`: Interactive API documentation

### Generated Output

The tool generates:
1. `ApiService` trait with methods for each endpoint
2. Request/response structs with serde derives
3. Enums with discriminator support
4. Error types with HTTP status code mappings
5. `make_scope<T: ApiService>()` function for Actix route registration
