<div align="center"><h1>httpz</h1></div>

<br>

This project is designed around the goals of [spacedrive](https://github.com/spacedriveapp/spacedrive)'s fork of [rspc](https://github.com/spacedriveapp/rspc) and there is no intention to expand it besides that.

## Usage

```rust
    // Define your a single HTTP handler which is supported by all major Rust webservers.
let endpoint = GenericEndpoint::new(
    // Set URL prefix
    "/",
    // Set the supported HTTP methods
    [Method::GET, Method::POST],
    // Define the handler function
    |_req: Request| async move {
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html")
            .body(b"Hello httpz World!".to_vec())?)
    },
);

// Attach your generic endpoint to Axum
let app = axum::Router::new().route("/", endpoint.axum());

// Attach your generic endpoint to Actix Web
HttpServer::new({
    let endpoint = endpoint.actix();
    move || App::new().service(web::scope("/prefix").service(endpoint.mount()))
});

// and so on...
```

## Features

- Write your HTTP handler once and support [Axum](https://github.com/tokio-rs/axum).
- Support for websockets.

## Projects using httpz

httpz is primarily designed to make life easier for library authors. It allows a library author to write and test a HTTP endpoint once and know it will work for Axum.

Libraries using httpz:

- [rspc](https://github.com/spacedriveapp/rspc)
