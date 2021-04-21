# Human Protocol API for Solana

This is implementation of the Human Protocol API, defined [as Swagger documentation](https://app.swaggerhub.com/apis/excerebrose/human-protocol/1.0.0#/).

## Build and run

To build and run this API you will have to install Rust. For Linux and Mac OS run:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

For instruction for other platforms or troubleshooting please visit Rust [installation page](https://www.rust-lang.org/tools/install).

Once Rust is installed build the server binaries:

```
cargo build --release
```

...and run API server:

```
target/release/human-api
```

Or instead run it in debug mode:

```
cargo run
```

## Configuration

Please read [Rocket configuration](https://api.rocket.rs/v0.4/rocket/config/index.html) article for running server in different environments (development, production).