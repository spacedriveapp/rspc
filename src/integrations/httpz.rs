use futures::{SinkExt, StreamExt};
use futures_channel::mpsc;
use httpz::{
    axum::axum::extract::FromRequestParts,
    http::{self, Method, Response, StatusCode},
    ws::{Message, WebsocketUpgrade},
    Endpoint, GenericEndpoint, HttpEndpoint, HttpResponse,
};
use serde_json::Value;
use std::{borrow::Cow, collections::HashMap, sync::Arc};

use crate::{
    internal::{
        jsonrpc::{self, handle_json_rpc, RequestId, SubscriptionSender},
        ProcedureKind,
    },
    Router,
};

pub use super::httpz_extractors::*;
/// TODO
///
/// This wraps [httpz::Request] removing any methods that are not safe with rspc such as `body`, `into_parts` and replacing the cookie handling API.
///
#[derive(Debug)]
pub struct Request(httpz::Request);

impl Request {
    pub(crate) fn new(req: httpz::Request) -> Self {
        Self(req)
    }

    /// Get the uri of the request.
    pub fn uri(&self) -> &httpz::http::Uri {
        self.0.uri()
    }

    /// Get the version of the request.
    pub fn version(&self) -> httpz::http::Version {
        self.0.version()
    }

    /// Get the method of the request.
    pub fn method(&self) -> &httpz::http::Method {
        self.0.method()
    }

    /// Get the headers of the request.
    pub fn headers(&self) -> &httpz::http::HeaderMap {
        self.0.headers()
    }

    /// Get the headers of the request.
    pub fn headers_mut(&mut self) -> &mut httpz::http::HeaderMap {
        self.0.headers_mut()
    }

    /// query_pairs returns an iterator of the query parameters.
    pub fn query_pairs(&self) -> Option<httpz::form_urlencoded::Parse<'_>> {
        self.0.query_pairs()
    }

    /// TODO
    pub fn server(&self) -> httpz::Server {
        self.0.server()
    }

    /// Get the extensions of the request.
    pub fn extensions(&self) -> &http::Extensions {
        self.0.extensions()
    }

    /// Get the extensions of the request.
    pub fn extensions_mut(&mut self) -> &mut http::Extensions {
        self.0.extensions_mut()
    }

    /// This methods allows using Axum extractors.
    /// This was previously supported but in Axum 0.6 it's not typesafe anymore so we are going to remove this API.
    // TODO: Remove this API once rspc's official cookie API is more stabilised.
    #[cfg(feature = "axum")]
    pub fn deprecated_extract<E, S>(&mut self) -> Option<Result<E, E::Rejection>>
    where
        E: FromRequestParts<S>,
        S: Clone + Send + Sync + 'static,
    {
        let parts = self.0.parts_mut();

        let state = parts
            .extensions
            .remove::<httpz::axum::axum::extract::State<S>>()?;

        // This is bad but it's a temporary API so I don't care.
        Some(futures::executor::block_on(async {
            let resp = <E as FromRequestParts<S>>::from_request_parts(parts, &state.0).await;
            parts.extensions.insert(state);
            resp
        }))
    }
}

impl<TCtx> Router<TCtx>
where
    TCtx: Send + Sync + 'static,
{
    pub fn endpoint<TCtxFnMarker: Send + Sync + 'static, TCtxFn: TCtxFunc<TCtx, TCtxFnMarker>>(
        self: Arc<Self>,
        ctx_fn: TCtxFn,
    ) -> Endpoint<impl HttpEndpoint> {
        GenericEndpoint::new(
            "/:id", // TODO: I think this is Axum specific. Fix in `httpz`!
            [Method::GET, Method::POST],
            move |req: httpz::Request| {
                // TODO: It would be nice if these clones weren't per request.
                // TODO: Maybe httpz can `Box::leak` a ref to a context type and allow it to be shared.
                let router = self.clone();
                let ctx_fn = ctx_fn.clone();

                async move {
                    match (req.method(), &req.uri().path()[1..]) {
                        (&Method::GET, "ws") => {
                            handle_websocket(ctx_fn, req, router).into_response()
                        }
                        (&Method::GET, _) => {
                            handle_http(ctx_fn, ProcedureKind::Query, req, &router)
                                .await
                                .into_response()
                        }
                        (&Method::POST, "_batch") => handle_http_batch(ctx_fn, req, &router)
                            .await
                            .into_response(),
                        (&Method::POST, _) => {
                            handle_http(ctx_fn, ProcedureKind::Mutation, req, &router)
                                .await
                                .into_response()
                        }
                        _ => unreachable!(),
                    }
                }
            },
        )
    }
}

pub async fn handle_http<TCtx, TCtxFn, TCtxFnMarker>(
    ctx_fn: TCtxFn,
    kind: ProcedureKind,
    req: httpz::Request,
    router: &Arc<Router<TCtx>>,
) -> impl HttpResponse
where
    TCtx: Send + Sync + 'static,
    TCtxFn: TCtxFunc<TCtx, TCtxFnMarker>,
{
    // Has to be allocated because `TCtxFn` takes ownership of `req`
    let procedure_name = req.uri().path()[1..].to_string();

    let input = match *req.method() {
        Method::GET => req
            .query_pairs()
            .and_then(|mut params| params.find(|e| e.0 == "input").map(|e| e.1))
            .map(|v| serde_json::from_str(&v))
            .unwrap_or(Ok(None as Option<Value>)),
        Method::POST => (!req.body().is_empty())
            .then(|| serde_json::from_slice(req.body()))
            .unwrap_or(Ok(None)),
        _ => unreachable!(),
    };

    let input = match input {
        Ok(input) => input,
        Err(_err) => {
            tracing::error!(
                "Error passing parameters to operation '{}' with key '{:?}': {}",
                kind.to_str(),
                procedure_name,
                _err
            );

            return Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .header("Content-Type", "application/json")
                .body(b"[]".to_vec())?);
        }
    };

    tracing::debug!(
        "Executing operation '{}' with key '{}' with params {:?}",
        kind.to_str(),
        procedure_name,
        input
    );

    let ctx = ctx_fn.exec(req);

    let ctx = match ctx {
        Ok(v) => v,
        Err(_err) => {
            tracing::error!("Error executing context function: {}", _err);

            return Ok(
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "application/json")
                    .body(b"[]".to_vec())?,
                // TODO: Props just return `None` here so that we don't allocate or need a clone.
            );
        }
    };

    let mut response = None as Option<jsonrpc::Response>;
    handle_json_rpc(
        ctx,
        jsonrpc::Request {
            jsonrpc: None,
            id: RequestId::Null,
            inner: match kind {
                ProcedureKind::Query => jsonrpc::RequestInner::Query {
                    path: procedure_name.to_string(), // TODO: Lifetime instead of allocate?
                    input,
                },
                ProcedureKind::Mutation => jsonrpc::RequestInner::Mutation {
                    path: procedure_name.to_string(), // TODO: Lifetime instead of allocate?
                    input,
                },
                ProcedureKind::Subscription => {
                    tracing::error!("Attempted to execute a subscription operation with HTTP");

                    return Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "application/json")
                        .body(b"[]".to_vec())?);
                }
            },
        },
        Cow::Borrowed(router),
        &mut response,
    )
    .await;

    debug_assert!(response.is_some()); // This would indicate a bug in rspc's jsonrpc_exec code
    let resp = match response {
        Some(resp) => match serde_json::to_vec(&resp) {
            Ok(v) => Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "application/json")
                .body(v)?,
            Err(_err) => {
                tracing::error!("Error serializing response: {}", _err);

                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Content-Type", "application/json")
                    .body(b"[]".to_vec())?
            }
        },
        // This case is unreachable but an error is here just incase.
        None => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .header("Content-Type", "application/json")
            .body(b"[]".to_vec())?,
    };

    Ok(resp)
}

pub async fn handle_http_batch<TCtx, TCtxFn, TCtxFnMarker>(
    ctx_fn: TCtxFn,
    req: httpz::Request,
    router: &Arc<Router<TCtx>>,
) -> impl HttpResponse
where
    TCtx: Send + Sync + 'static,
    TCtxFn: TCtxFunc<TCtx, TCtxFnMarker>,
{
    match serde_json::from_slice::<Vec<jsonrpc::Request>>(req.body()) {
        Ok(reqs) => {
            let mut responses = Vec::with_capacity(reqs.len());
            for op in reqs {
                // TODO: Make `TCtx` require clone and only run the ctx function once for the whole batch.
                let ctx = ctx_fn.exec(req._internal_dangerously_clone());

                let ctx = match ctx {
                    Ok(v) => v,
                    Err(_err) => {
                        tracing::error!("Error executing context function: {}", _err);

                        return Ok(
                            Response::builder()
                                .status(StatusCode::INTERNAL_SERVER_ERROR)
                                .header("Content-Type", "application/json")
                                .body(b"[]".to_vec())?,
                            // TODO: Props just return `None` here so that we don't allocate or need a clone.
                        );
                    }
                };

                // Catch panics so they don't take out the whole batch
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| async {
                    let mut response = None as Option<jsonrpc::Response>;
                    handle_json_rpc(ctx, op, Cow::Borrowed(router), &mut response).await;
                    response
                }));

                match result {
                    Ok(fut) => {
                        if let Some(response) = fut.await {
                            responses.push(response);
                        }
                    }
                    Err(_err) => {
                        tracing::error!(
                            "Panic occurred while executing JSON-RPC handler: {:?}",
                            _err
                        );
                    }
                }
            }

            match serde_json::to_vec(&responses) {
                Ok(v) => Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(v)?),
                Err(_err) => {
                    tracing::error!("Error serializing batch request: {}", _err);

                    Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Content-Type", "application/json")
                        .body(b"[]".to_vec())?)
                }
            }
        }
        Err(_err) => {
            tracing::error!("Error deserializing batch request: {}", _err);

            Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header("Content-Type", "application/json")
                .body(b"[]".to_vec())?)
        }
    }
}

pub fn handle_websocket<TCtx, TCtxFn, TCtxFnMarker>(
    ctx_fn: TCtxFn,
    req: httpz::Request,
    router: Arc<Router<TCtx>>,
) -> impl HttpResponse
where
    TCtx: Send + Sync + 'static,
    TCtxFn: TCtxFunc<TCtx, TCtxFnMarker>,
{
    tracing::debug!("Accepting websocket connection");

    if !req.server().supports_websockets() {
        tracing::debug!("Websocket are not supported on your webserver!");

        // TODO: Make this error be picked up on the frontend and expose it with a logical name
        return Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(vec![])?);
    }

    WebsocketUpgrade::from_req(req, move |req, mut socket| async move {
		let mut subscriptions = HashMap::new();
		let (mut tx, mut rx) = mpsc::channel::<jsonrpc::Response>(100);

		loop {
			tokio::select! {
					biased; // Note: Order is important here
					msg = rx.next() => {
						match socket.send(Message::Text(match serde_json::to_string(&msg) {
							Ok(v) => v,
							Err(_err) => {
								tracing::error!("Error serializing websocket message: {}", _err);

								continue;
							}
						})).await {
							Ok(_) => {}
							Err(_err) => {
								tracing::error!("Error sending websocket message: {}", _err);

								continue;
							}
						}
					}
					msg = socket.next() => {
						match msg {
							Some(Ok(msg) )=> {
							   let res = match msg {
									Message::Text(text) => serde_json::from_str::<Value>(&text),
									Message::Binary(binary) => serde_json::from_slice(&binary),
									Message::Ping(_) | Message::Pong(_) | Message::Close(_) => {
										continue;
									}
									Message::Frame(_) => unreachable!(),
								};

								match res.and_then(|v| match v.is_array() {
										true => serde_json::from_value::<Vec<jsonrpc::Request>>(v),
										false => serde_json::from_value::<jsonrpc::Request>(v).map(|v| vec![v]),
									}) {
									Ok(reqs) => {
										for request in reqs {
											let ctx = ctx_fn.exec(req._internal_dangerously_clone());
											handle_json_rpc(match ctx {
												Ok(v) => v,
												Err(_err) => {
													tracing::error!("Error executing context function: {}", _err);

													continue;
												}
											}, request, Cow::Borrowed(&router), SubscriptionSender(&mut tx, &mut subscriptions)
											).await;
										}
									},
									Err(_err) => {
										tracing::error!("Error parsing websocket message: {}", _err);

										// TODO: Send report of error to frontend

										continue;
									}
								};
							}
							Some(Err(_err)) => {
								tracing::error!("Error in websocket: {}", _err);

								// TODO: Send report of error to frontend

								continue;
							},
							None => {
								tracing::debug!("Shutting down websocket connection");

								// TODO: Send report of error to frontend

								return;
							},
						}
					}
			}
		}
	})
	.into_response()
}
