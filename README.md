# Actix-Web openapi generator for rust.

Generates an actix-web server from openapi configuration.

It is based on generation of:
1. api service trait
    A trait containing all of the methods described on the openapi
    and response models that are rust structs supporting serialize/deserialize

    The models for parsing api parameters will also be generated.

    This trait guarantees that any object implementing it and registered
    into actix web scope will adhere to the openapi specification.

2. router - that would create actix web scope from an implementation of the trait

The code that you must provide yourself is running the server itself.
It is very application-specific and thus is out of scope of automatic generation.

You may find the default code for running server that would utilise the
generated spec in the examples folder



Generated API supports custom error types and natively maps them to rust error enums.
A convenience macro `apibail` can be used to return quickly an error to user


```
apibail!(
    HelloUserError::InvalidCharacters,
    "Found non-ascii-alphanumeric characters"
)
```

# Installation

To install this script, first checkout it
`git clone <this repo>`

And then install

`cargo install --path .`

# Usage

First, create the following directory structure
```
src
    server
        static
            openapi.yaml
        mod.rs
```

You can find default openapi.yaml and mod.rs file in the examples folder.


And then run command

`cargo actix-openapi src/server/static src/server/api.rs`

It will generate file `api.rs` and `docs.html` and you will have the following structure:

```
src
    server
        static
            docs.html
            openapi.yaml
        api.rs
        mod.rs
```

After that, consider file `api.rs` to be fully maintained by this automatic tool.

Making any migrations and re-running the generator will create new models in the `api.rs`
file.

Your job will be to implement methods of trait `ApiService` defined in `api.rs` 

As an input it accepts path to `static` directory

