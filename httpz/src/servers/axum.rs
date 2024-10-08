use std::sync::Arc;

use axum::{
    body::to_bytes,
    extract::{Request, State},
    routing::{on, MethodFilter},
    Router,
};
use http::{header::CONTENT_LENGTH, HeaderMap, StatusCode};

use crate::{Endpoint, HttpEndpoint, HttpResponse, Server};

pub use axum;

impl<TEndpoint> Endpoint<TEndpoint>
where
    TEndpoint: HttpEndpoint,
{
    /// is called to mount the endpoint onto an Axum router.
    pub fn axum<S>(mut self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let (url, methods) = self.endpoint.register();
        let endpoint = Arc::new(self.endpoint);

        let mut method_filter: Option<MethodFilter> = None;
        for method in methods.as_ref().iter() {
            let method = MethodFilter::try_from(method.clone())
                .expect("Error converting method to MethodFilter");
            if let Some(ref mut filter) = method_filter {
                *filter = filter.or(method);
            } else {
                method_filter = Some(method);
            }
        }

        Router::<S>::new().route(
            url.as_ref(),
            on(
                method_filter.expect("No methods found"),
                |State(state): State<S>, request: Request| async move {
                    let (mut parts, body) = request.into_parts();
                    parts.extensions.insert(state);

                    let content_length = parts
                        .headers
                        .get(CONTENT_LENGTH)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(10 * 1024 * 1024); // Default to 10MB if not present

                    let body = match to_bytes(body, content_length).await {
                        Ok(body) => body.to_vec(),
                        Err(err) => {
                            return (
                                StatusCode::BAD_REQUEST,
                                HeaderMap::new(),
                                err.to_string().as_bytes().to_vec(),
                            );
                        }
                    };

                    let body = Request::from_parts(parts, body);

                    match endpoint
                        .handler(crate::Request::new(body, Server::Axum))
                        .await
                        .into_response()
                    {
                        Ok(resp) => {
                            let (parts, body) = resp.into_parts();
                            (parts.status, parts.headers, body)
                        }
                        Err(err) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            HeaderMap::new(),
                            err.to_string().as_bytes().to_vec(),
                        ),
                    }
                },
            ),
        )
    }
}

impl crate::Request {
    /// TODO
    pub fn get_axum_state<S>(&self) -> Option<&S>
    where
        S: Clone + Send + Sync + 'static,
    {
        self.extensions().get::<State<S>>().map(|state| &state.0)
    }
}
