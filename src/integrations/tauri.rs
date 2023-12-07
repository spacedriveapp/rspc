use std::{
    borrow::Cow,
    collections::{hash_map::DefaultHasher, HashMap},
    future::{ready, Ready},
    hash::{Hash, Hasher},
    marker::PhantomData,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use futures::executor::block_on;
use serde_json::Value;
use tauri::{
    async_runtime::spawn,
    plugin::{Builder, TauriPlugin},
    Window, WindowEvent, Wry,
};
use tokio::sync::oneshot;

use crate::{
    internal::jsonrpc::{
        self, handle_json_rpc, OwnedSender, RequestId, Sender, SubscriptionUpgrade,
    },
    Router,
};

type SubscriptionMap = Arc<futures_locks::Mutex<HashMap<RequestId, oneshot::Sender<()>>>>;

pub struct TauriSender(Window<Wry>, SubscriptionMap);

impl<'a> Sender<'a> for TauriSender {
    type SendFut = Ready<()>;
    type SubscriptionMap = SubscriptionMap;
    type OwnedSender = TauriOwnedSender;

    fn subscription(self) -> SubscriptionUpgrade<'a, Self> {
        SubscriptionUpgrade::Supported(TauriOwnedSender(self.0.clone(), Instant::now()), self.1)
    }

    fn send(self, resp: jsonrpc::Response) -> Self::SendFut {
        let time = std::time::Instant::now();
        self.0
            .emit("plugin:rspc:transport:resp", resp)
            .map_err(|err| {
                #[cfg(feature = "tracing")]
                tracing::error!("failed to emit JSON-RPC response: {}", err);
            })
            .ok();
        println!("plugin:rspc:transport - send - {:?}", time.elapsed());
        ready(())
    }
}

pub struct TauriOwnedSender(Window<Wry>, Instant);

impl OwnedSender for TauriOwnedSender {
    type SendFut<'a> = Ready<()>;

    fn send(&mut self, resp: jsonrpc::Response) -> Self::SendFut<'_> {
        self.0
            .emit("plugin:rspc:transport:resp", resp)
            .map_err(|err| {
                #[cfg(feature = "tracing")]
                tracing::error!("failed to emit JSON-RPC response: {}", err);
            })
            .ok();

        // println!("TauriOwnedSender - sent {:?}", self.1.elapsed()); // This is for subscriptions not useful data
        ready(())
    }
}

struct WindowManager<TCtxFn, TCtx, TMeta>
where
    TCtx: Send + Sync + 'static,
    TMeta: Send + Sync + 'static,
    TCtxFn: Fn(Window<Wry>) -> TCtx + Send + Sync + 'static,
{
    router: Arc<Router<TCtx, TMeta>>,
    ctx_fn: TCtxFn,
    windows: Mutex<HashMap<u64, SubscriptionMap>>,
}

impl<TCtxFn, TCtx, TMeta> WindowManager<TCtxFn, TCtx, TMeta>
where
    TCtx: Send + Sync + 'static,
    TMeta: Send + Sync + 'static,
    TCtxFn: Fn(Window<Wry>) -> TCtx + Send + Sync + 'static,
{
    pub fn new(ctx_fn: TCtxFn, router: Arc<Router<TCtx, TMeta>>) -> Arc<Self> {
        Arc::new(Self {
            router,
            ctx_fn,
            windows: Mutex::new(HashMap::new()),
        })
    }

    pub fn on_page_load(self: Arc<Self>, window: Window<Wry>) {
        let mut hasher = DefaultHasher::new();
        window.hash(&mut hasher);
        let window_hash = hasher.finish();

        let mut windows = self.windows.lock().unwrap();
        // Shutdown all subscriptions for the previously loaded page is there was one
        if let Some(subscriptions) = windows.get(&window_hash) {
            let mut subscriptions = block_on(subscriptions.lock());
            for (_, tx) in subscriptions.drain() {
                tx.send(()).ok();
            }
        } else {
            let subscriptions = SubscriptionMap::default();
            windows.insert(window_hash, subscriptions.clone());
            drop(windows);

            window.listen("plugin:rspc:transport", {
                let window = window.clone();
                move |event| {
                    println!(
                        "tauri-ipc-listener - {:?}",
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("Time went backwards")
                            .as_millis()
                    );

                    let time = std::time::Instant::now();

                    let reqs = match event.payload() {
                        Some(v) => {
                            let v = match serde_json::from_str::<serde_json::Value>(v) {
                                Ok(v) => match v {
                                    Value::String(s) => {
                                        match serde_json::from_str::<serde_json::Value>(&s) {
                                            Ok(v) => v,
                                            Err(err) => {
                                                #[cfg(feature = "tracing")]
                                                tracing::error!(
                                                    "failed to parse JSON-RPC request: {}",
                                                    err
                                                );
                                                return;
                                            }
                                        }
                                    }
                                    v => v,
                                },
                                Err(err) => {
                                    #[cfg(feature = "tracing")]
                                    tracing::error!("failed to parse JSON-RPC request: {}", err);
                                    return;
                                }
                            };

                            match if v.is_array() {
                                serde_json::from_value::<Vec<jsonrpc::Request>>(v)
                            } else {
                                serde_json::from_value::<jsonrpc::Request>(v).map(|v| vec![v])
                            } {
                                Ok(v) => v,
                                Err(err) => {
                                    #[cfg(feature = "tracing")]
                                    tracing::error!("failed to parse JSON-RPC request: {}", err);
                                    return;
                                }
                            }
                        }
                        None => {
                            #[cfg(feature = "tracing")]
                            tracing::error!("Tauri event payload is empty");

                            return;
                        }
                    };

                    println!("plugin:rspc:transport - decoded - {:?}", time.elapsed());

                    for req in reqs {
                        let ctx = (self.ctx_fn)(window.clone());
                        let router = self.router.clone();
                        let window = window.clone();

                        let req2 = req.clone();
                        let fut = handle_json_rpc(
                            ctx,
                            req,
                            Cow::Owned(router),
                            TauriSender(window, subscriptions.clone()),
                        );
                        spawn(async move {
                            println!(
                                "plugin:rspc:transport - thread({:?}) - start - {:?}",
                                req2,
                                time.elapsed()
                            );
                            let time = std::time::Instant::now();

                            let result = fut.await;

                            println!(
                                "plugin:rspc:transport - thread({:?}) - end - {:?}",
                                req2,
                                time.elapsed()
                            );
                            result
                        });
                    }

                    println!("plugin:rspc:transport - {:?}", time.elapsed());
                }
            });
        }
    }

    pub fn close_requested(&self, window: &Window<Wry>) {
        let mut hasher = DefaultHasher::new();
        window.hash(&mut hasher);
        let window_hash = hasher.finish();

        if let Some(rspc_window) = self.windows.lock().unwrap().remove(&window_hash) {
            spawn(async move {
                let mut subscriptions = rspc_window.lock().await;
                for (_, tx) in subscriptions.drain() {
                    tx.send(()).ok();
                }
            });
        }
    }
}

// #[deprecated("Use `plugin_with_ctx` instead")]
pub fn plugin<TCtx, TMeta>(
    router: Arc<Router<TCtx, TMeta>>,
    ctx_fn: impl Fn() -> TCtx + Send + Sync + 'static,
) -> TauriPlugin<Wry>
where
    TCtx: Send + Sync + 'static,
    TMeta: Send + Sync + 'static,
{
    let manager = WindowManager::new(move |_| ctx_fn(), router);
    Builder::new("rspc")
        .on_page_load(move |window, _page| {
            manager.clone().on_page_load(window.clone());

            window.on_window_event({
                let window = window.clone();
                let manager = manager.clone();
                move |event| match event {
                    WindowEvent::CloseRequested { .. } => {
                        manager.close_requested(&window);
                    }
                    _ => {}
                }
            })
        })
        .build()
}

pub fn plugin_with_ctx<TCtx, TMeta>(
    router: Arc<Router<TCtx, TMeta>>,
    ctx_fn: impl Fn(Window<Wry>) -> TCtx + Send + Sync + 'static,
) -> TauriPlugin<Wry>
where
    TCtx: Send + Sync + 'static,
    TMeta: Send + Sync + 'static,
{
    let manager = WindowManager::new(ctx_fn, router);
    Builder::new("rspc")
        .on_page_load(move |window, _page| {
            manager.clone().on_page_load(window.clone());

            window.on_window_event({
                let window = window.clone();
                let manager = manager.clone();
                move |event| match event {
                    WindowEvent::CloseRequested { .. } => {
                        manager.close_requested(&window);
                    }
                    _ => {}
                }
            })
        })
        .build()
}
