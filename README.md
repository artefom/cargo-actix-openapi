# Actix-Web OpenAPI Generator for Rust

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Generates an actix-web server from OpenAPI 3.0.3 specifications.

Its purpose is to provide a FastAPI-like ease of use to Rust web servers.

It generates two things:

1. **API Service Trait** - A trait containing all methods described in the OpenAPI spec, along with request/response models as Rust structs supporting serialize/deserialize. This trait guarantees that any implementation registered into an Actix-web scope will adhere to the OpenAPI specification.

2. **Scope Creator** - A `make_scope` function that creates an Actix-web scope from your trait implementation.

## Example

Here is a simplified example of what this package allows you to do:

```rust
use actix_web::{web, App, HttpServer};
use actix_web_prom::PrometheusMetricsBuilder;

// Your application state
struct AppState {
    // your state fields
}

struct MyServer;

// ApiService is a trait auto-generated from your openapi.yaml spec
// The state type parameter allows access to your application state
impl api::ApiService<AppState> for MyServer {
    async fn health(
        data: web::Data<AppState>,
    ) -> web::Json<HealthResponse> {
        web::Json(HealthResponse::Ok)
    }

    async fn hello_user(
        data: web::Data<AppState>,
        path: web::Path<HelloUserPath>,
    ) -> Result<web::Json<String>, Detailed<HelloUserError>> {
        let user = &path.user;

        if !user.chars().all(|c| c.is_ascii_alphanumeric()) {
            apibail!(
                HelloUserError::InvalidCharacters,
                "Found non-ascii-alphanumeric characters"
            );
        }

        Ok(web::Json(format!("Hello, {}!", user)))
    }
}

// In your main function:
let prometheus = PrometheusMetricsBuilder::new("api")
    .endpoint("/metrics")
    .build()
    .unwrap();

// make_scope is auto-generated - pass your implementation type and state type
let scope = api::make_scope::<MyServer, AppState>(prometheus);

App::new()
    .app_data(web::Data::new(AppState { /* ... */ }))
    .service(scope)
```

The code for running the server itself is application-specific and not auto-generated. See the `examples/` folder for a complete working example.

## Error Handling

Generated API supports custom error types mapped to HTTP status codes. Two convenience macros are generated:

**`apibail!`** - Return an error immediately:
```rust
apibail!(
    HelloUserError::InvalidCharacters,
    "Found non-ascii-alphanumeric characters"
)
```

**`detailed!`** - Create a detailed error without returning:
```rust
let err = detailed!(HelloUserError::InvalidCharacters, "Invalid input");
```

## Interactive Documentation

The generator creates a `docs.html` file that provides interactive API documentation. It's automatically served at `/docs` and `/v1/docs` endpoints alongside your API.

## Installation

Clone and install:

```bash
git clone <this repo>
cargo install --path .
```

## Usage

Create the following directory structure:

```
src/
    server/
        static/
            openapi.yaml
        mod.rs
```

You can find example `openapi.yaml` and implementation files in the `examples/` folder.

Run the generator:

```bash
cargo actix-openapi src/server/static src/server/api.rs
```

This generates `api.rs` and `docs.html`:

```
src/
    server/
        static/
            docs.html      # Generated interactive docs
            openapi.yaml
        api.rs             # Generated API code
        mod.rs
```

**Important:** The `api.rs` file is fully managed by this tool. Do not manually edit it - your changes will be overwritten when you regenerate.

Your job is to implement the `ApiService` trait defined in `api.rs`.

## Required Dependencies

The generated code requires these dependencies in your `Cargo.toml`:

```toml
[dependencies]
actix-web = "4"
actix-web-prom = "0.6"  # For PrometheusMetrics
async-trait = "0.1"      # For async trait support
serde = { version = "1", features = ["derive"] }
```

## Module Path Constraint

The generated `apibail!` and `detailed!` macros reference `$crate::server::api::Detailed`. This means your generated `api.rs` must be located at `src/server/api.rs` for the macros to work correctly.
