/// Server represents the server that the request is coming from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Server {
    /// support for [Axum](https://github.com/tokio-rs/axum)
    Axum,
}

impl Server {
    /// convert the server into a string
    #[allow(unreachable_patterns)]
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Axum => "axum",
            _ => unreachable!(),
        }
    }

    /// check if the server that handled this request supports upgrading the connection to a websocket.
    #[allow(unreachable_patterns)]
    pub fn supports_websockets(&self) -> bool {
        match self {
            Self::Axum => true,
            _ => unreachable!(),
        }
    }
}
