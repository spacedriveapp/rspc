use std::{borrow::Cow, marker::PhantomData, panic::Location, process};

use serde::de::DeserializeOwned;
use specta::{Type, TypeMap};
use specta_typescript as ts;

use crate::{
    internal::{
        BaseMiddleware, BuiltProcedureBuilder, EitherLayer, MiddlewareBuilderLike,
        MiddlewareLayerBuilder, MiddlewareMerger, ProcedureDataType, ProcedureStore, ResolverLayer,
        UnbuiltProcedureBuilder,
    },
    Config, ExecError, MiddlewareBuilder, MiddlewareLike, RequestLayer, Router, StreamRequestLayer,
};

// TODO: Storing procedure names as an `ThinVec<Cow<'static, str>>` instead.
#[doc(hidden)]
// #[deprecated = "Removed in v1.0.0. Is now `<TResolver as ResolverFunction<_>>::typedef`"]
pub fn is_invalid_procedure_name(s: &str) -> bool {
    // TODO: Prevent Typescript reserved keywords
    s.is_empty()
        || s == "ws"
        || s.starts_with("rpc")
        || s.starts_with("rspc")
        || !s
            .chars()
            .all(|c| c.is_alphabetic() || c.is_numeric() || c == '_' || c == '-')
}

// TODO: Storing procedure names as an `ThinVec<Cow<'static, str>>` instead.
pub(crate) fn is_invalid_router_prefix(s: &str) -> (String, bool) {
    // TODO: Prevent Typescript reserved keywords

    let s = if s.ends_with('.') {
        // TODO: Replace this with a hard error in a future release.
        println!(
            "rspc warning: attempted to merge a router using prefix '{s}' which is going to be unsupported in a future release. Please remove the trailing '.' to avoid a hard error in the future."
        );
        s.to_owned()
    } else {
        format!("{}.", s)
    };

    let is_valid = s.is_empty()
        || s == "ws."
        || s.starts_with("rpc.")
        || s.starts_with("rspc.")
        || !s
            .chars()
            .all(|c| c.is_alphabetic() || c.is_numeric() || c == '_');

    (s, is_valid)
}

pub struct RouterBuilder<
    TCtx = (), // The is the context the current router was initialised with
    TMeta = (),
    TMiddleware = BaseMiddleware<TCtx>,
> where
    TCtx: Send + Sync + 'static,
    TMeta: Send + 'static,
    TMiddleware: MiddlewareBuilderLike<TCtx> + Send + 'static,
{
    pub(crate) config: Config,
    pub(crate) middleware: TMiddleware,
    pub(crate) queries: ProcedureStore<TCtx>,
    pub(crate) mutations: ProcedureStore<TCtx>,
    pub(crate) subscriptions: ProcedureStore<TCtx>,
    pub(crate) typ_store: TypeMap,
    pub(crate) phantom: PhantomData<TMeta>,
}

pub trait RouterBuilderLike<TCtx>
where
    TCtx: Send + Sync + 'static,
{
    type Meta: Send + 'static;
    type Middleware: MiddlewareBuilderLike<TCtx> + Send + 'static;

    fn expose(self) -> RouterBuilder<TCtx, Self::Meta, Self::Middleware>;
}

impl<TCtx, TMeta, TMiddleware> RouterBuilderLike<TCtx> for RouterBuilder<TCtx, TMeta, TMiddleware>
where
    TCtx: Send + Sync + 'static,
    TMeta: Send + 'static,
    TMiddleware: MiddlewareBuilderLike<TCtx> + Send + 'static,
{
    type Meta = TMeta;
    type Middleware = TMiddleware;

    fn expose(self) -> RouterBuilder<TCtx, TMeta, Self::Middleware> {
        self
    }
}

#[allow(clippy::new_without_default, clippy::new_ret_no_self)]
impl<TCtx, TMeta> Router<TCtx, TMeta>
where
    TCtx: Send + Sync + 'static,
    TMeta: Send + 'static,
{
    pub fn new() -> RouterBuilder<TCtx, TMeta, BaseMiddleware<TCtx>> {
        RouterBuilder::new()
    }
}

#[allow(clippy::new_without_default)]
impl<TCtx, TMeta> RouterBuilder<TCtx, TMeta, BaseMiddleware<TCtx>>
where
    TCtx: Send + Sync + 'static,
    TMeta: Send + 'static,
{
    pub fn new() -> Self {
        Self {
            config: Config::new(),
            middleware: BaseMiddleware::default(),
            queries: ProcedureStore::new("query"),
            mutations: ProcedureStore::new("mutation"),
            subscriptions: ProcedureStore::new("subscription"),
            typ_store: TypeMap::default(),
            phantom: PhantomData,
        }
    }
}

#[allow(clippy::unwrap_used)] // TODO: Remove this
impl<TCtx, TLayerCtx, TMeta, TMiddleware> RouterBuilder<TCtx, TMeta, TMiddleware>
where
    TCtx: Send + Sync + 'static,
    TMeta: Send + 'static,
    TLayerCtx: Send + Sync + 'static,
    TMiddleware: MiddlewareBuilderLike<TCtx, LayerContext = TLayerCtx> + Send + 'static,
{
    #[doc(hidden)]
    #[cfg(feature = "unstable")]
    pub fn queries(&mut self) -> &mut ProcedureStore<TCtx> {
        &mut self.queries
    }

    #[doc(hidden)]
    #[cfg(feature = "unstable")]
    pub fn mutations(&mut self) -> &mut ProcedureStore<TCtx> {
        &mut self.mutations
    }

    #[doc(hidden)]
    #[cfg(feature = "unstable")]
    pub fn subscriptions(&mut self) -> &mut ProcedureStore<TCtx> {
        &mut self.subscriptions
    }

    #[doc(hidden)]
    #[cfg(feature = "unstable")]
    pub fn typ_store(&mut self) -> &mut TypeMap {
        &mut self.typ_store
    }

    #[doc(hidden)]
    #[cfg(feature = "unstable")]
    pub fn prev_middleware(&mut self) -> &mut TMiddleware {
        &mut self.middleware
    }

    /// Attach a configuration to the router. Calling this multiple times will overwrite the previous config.
    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    pub fn middleware<TNewMiddleware, TNewLayerCtx>(
        self,
        builder: impl Fn(MiddlewareBuilder<TLayerCtx>) -> TNewMiddleware,
    ) -> RouterBuilder<
        TCtx,
        TMeta,
        MiddlewareLayerBuilder<TCtx, TLayerCtx, TNewLayerCtx, TMiddleware, TNewMiddleware>,
    >
    where
        TNewLayerCtx: Send + Sync + 'static,
        TNewMiddleware: MiddlewareLike<TLayerCtx, NewCtx = TNewLayerCtx> + Send + Sync + 'static,
    {
        let Self {
            config,
            middleware,
            queries,
            mutations,
            subscriptions,
            typ_store,
            ..
        } = self;

        let mw = builder(MiddlewareBuilder(PhantomData));
        RouterBuilder {
            config,
            middleware: MiddlewareLayerBuilder {
                middleware,
                mw,
                phantom: PhantomData,
            },
            queries,
            mutations,
            subscriptions,
            typ_store,
            phantom: PhantomData,
        }
    }

    #[track_caller]
    pub fn query<TResolver, TArg, TResult, TResultMarker>(
        mut self,
        key: &'static str,
        builder: impl FnOnce(
            UnbuiltProcedureBuilder<TLayerCtx, TResolver>,
        ) -> BuiltProcedureBuilder<TResolver>,
    ) -> Self
    where
        TArg: DeserializeOwned + Type,
        TResult: RequestLayer<TResultMarker>,
        TResolver: Fn(TLayerCtx, TArg) -> TResult + Send + Sync + 'static,
    {
        if is_invalid_procedure_name(key) {
            eprintln!(
                "{}: rspc error: attempted to attach a query with the key '{}', however this name is not allowed. ",
                Location::caller(),
                key
            );
            process::exit(1);
        }

        let resolver = builder(UnbuiltProcedureBuilder::default()).resolver;
        self.queries.append(
            key.into(),
            self.middleware.build(ResolverLayer {
                func: move |ctx, input, _| {
                    resolver(
                        ctx,
                        serde_json::from_value(input).map_err(ExecError::DeserializingArgErr)?,
                    )
                    .into_layer_result()
                },
                phantom: PhantomData,
            }),
            typedef::<TArg, TResult::Result>(Cow::Borrowed(key), &mut self.typ_store).unwrap(),
        );
        self
    }

    #[track_caller]
    pub fn mutation<TResolver, TArg, TResult, TResultMarker>(
        mut self,
        key: &'static str,
        builder: impl FnOnce(
            UnbuiltProcedureBuilder<TLayerCtx, TResolver>,
        ) -> BuiltProcedureBuilder<TResolver>,
    ) -> Self
    where
        TArg: DeserializeOwned + Type,
        TResult: RequestLayer<TResultMarker>,
        TResolver: Fn(TLayerCtx, TArg) -> TResult + Send + Sync + 'static,
    {
        if is_invalid_procedure_name(key) {
            eprintln!(
                "{}: rspc error: attempted to attach a mutation with the key '{}', however this name is not allowed. ",
                Location::caller(),
                key
            );
            process::exit(1);
        }

        let resolver = builder(UnbuiltProcedureBuilder::default()).resolver;
        self.mutations.append(
            key.into(),
            self.middleware.build(ResolverLayer {
                func: move |ctx, input, _| {
                    resolver(
                        ctx,
                        serde_json::from_value(input).map_err(ExecError::DeserializingArgErr)?,
                    )
                    .into_layer_result()
                },
                phantom: PhantomData,
            }),
            typedef::<TArg, TResult::Result>(Cow::Borrowed(key), &mut self.typ_store).unwrap(),
        );
        self
    }

    #[track_caller]
    pub fn subscription<F, TArg, TResult, TResultMarker>(
        mut self,
        key: &'static str,
        builder: impl FnOnce(UnbuiltProcedureBuilder<TLayerCtx, F>) -> BuiltProcedureBuilder<F>,
    ) -> Self
    where
        F: Fn(TLayerCtx, TArg) -> TResult + Send + Sync + 'static,
        TArg: DeserializeOwned + Type,
        TResult: StreamRequestLayer<TResultMarker>,
    {
        if is_invalid_procedure_name(key) {
            eprintln!(
                "{}: rspc error: attempted to attach a subscription with the key '{}', however this name is not allowed. ",
                Location::caller(),
                key
            );
            process::exit(1);
        }

        let resolver = builder(UnbuiltProcedureBuilder::default()).resolver;
        self.subscriptions.append(
            key.into(),
            self.middleware.build(ResolverLayer {
                func: move |ctx, input, _| {
                    resolver(
                        ctx,
                        serde_json::from_value(input).map_err(ExecError::DeserializingArgErr)?,
                    )
                    .into_layer_result()
                },
                phantom: PhantomData,
            }),
            typedef::<TArg, TResult::Result>(Cow::Borrowed(key), &mut self.typ_store).unwrap(),
        );
        self
    }

    #[track_caller]
    pub fn merge<TNewLayerCtx, TIncomingMiddleware>(
        mut self,
        prefix: &'static str,
        router: impl RouterBuilderLike<TLayerCtx, Meta = TMeta, Middleware = TIncomingMiddleware>,
    ) -> Self
    where
        TNewLayerCtx: 'static,
        TIncomingMiddleware:
            MiddlewareBuilderLike<TLayerCtx, LayerContext = TNewLayerCtx> + Send + 'static,
    {
        let router = router.expose();

        // let (prefix, prefix_valid) = is_invalid_router_prefix(prefix);
        // #[allow(clippy::panic)]
        // if prefix_valid {
        //     eprintln!(
        //         "{}: rspc error: attempted to merge a router with the prefix '{}', however this prefix is not allowed. ",
        //         Location::caller(),
        //         prefix
        //     );
        //     process::exit(1);
        // }

        // TODO: The `data` field has gotta flow from the root router to the leaf routers so that we don't have to merge user defined types.

        for (key, query) in router.queries.store {
            // query.ty.key = format!("{}{}", prefix, key);
            match query.exec {
                EitherLayer::Legacy(exec) => {
                    self.queries.append(
                        format!("{}{}", prefix, key),
                        self.middleware.build(exec),
                        query.ty,
                    );
                }
                #[cfg(feature = "alpha")]
                EitherLayer::Alpha(_) => todo!(),
            }
        }

        for (key, mutation) in router.mutations.store {
            // mutation.ty.key = format!("{}{}", prefix, key);
            match mutation.exec {
                EitherLayer::Legacy(exec) => {
                    self.mutations.append(
                        format!("{}{}", prefix, key),
                        self.middleware.build(exec),
                        mutation.ty,
                    );
                }
                #[cfg(feature = "alpha")]
                EitherLayer::Alpha(_) => todo!(),
            }
        }

        for (key, subscription) in router.subscriptions.store {
            // subscription.ty.key = format!("{}{}", prefix, key);

            match subscription.exec {
                EitherLayer::Legacy(exec) => {
                    self.subscriptions.append(
                        format!("{}{}", prefix, key),
                        self.middleware.build(exec),
                        subscription.ty,
                    );
                }
                #[cfg(feature = "alpha")]
                EitherLayer::Alpha(_) => todo!(),
            }
        }

        for (name, typ) in router.typ_store.iter() {
            self.typ_store.insert(name, typ.clone());
        }

        self
    }

    /// `legacy_merge` maintains the `merge` functionality prior to release 0.1.3
    /// It will flow the `TMiddleware` and `TCtx` out of the child router to the parent router.
    /// This was a confusing behavior and is generally not useful so it has been deprecated.
    ///
    /// This function will be remove in a future release. If you are using it open a GitHub issue to discuss your use case and longer term solutions for it.
    #[track_caller]
    pub fn legacy_merge<TNewLayerCtx, TIncomingMiddleware>(
        self,
        prefix: &'static str,
        router: impl RouterBuilderLike<TLayerCtx, Meta = TMeta, Middleware = TIncomingMiddleware>,
    ) -> RouterBuilder<
        TCtx,
        TMeta,
        MiddlewareMerger<TCtx, TLayerCtx, TNewLayerCtx, TMiddleware, TIncomingMiddleware>,
    >
    where
        TNewLayerCtx: 'static,
        TIncomingMiddleware:
            MiddlewareBuilderLike<TLayerCtx, LayerContext = TNewLayerCtx> + Send + 'static,
    {
        let router = router.expose();

        let (prefix, prefix_valid) = is_invalid_router_prefix(prefix);
        #[allow(clippy::panic)]
        if prefix_valid {
            eprintln!(
                "{}: rspc error: attempted to merge a router with the prefix '{}', however this prefix is not allowed. ",
                Location::caller(),
                prefix
            );
            process::exit(1);
        }

        let Self {
            config,
            middleware,
            mut queries,
            mut mutations,
            mut subscriptions,
            mut typ_store,
            ..
        } = self;

        for (key, query) in router.queries.store {
            match query.exec {
                EitherLayer::Legacy(exec) => {
                    queries.append(
                        format!("{}{}", prefix, key),
                        middleware.build(exec),
                        query.ty,
                    );
                }
                #[cfg(feature = "alpha")]
                EitherLayer::Alpha(_) => todo!(),
            }
        }

        for (key, mutation) in router.mutations.store {
            match mutation.exec {
                EitherLayer::Legacy(exec) => {
                    mutations.append(
                        format!("{}{}", prefix, key),
                        middleware.build(exec),
                        mutation.ty,
                    );
                }
                #[cfg(feature = "alpha")]
                EitherLayer::Alpha(_) => todo!(),
            }
        }

        for (key, subscription) in router.subscriptions.store {
            match subscription.exec {
                EitherLayer::Legacy(exec) => {
                    subscriptions.append(
                        format!("{}{}", prefix, key),
                        middleware.build(exec),
                        subscription.ty,
                    );
                }
                #[cfg(feature = "alpha")]
                EitherLayer::Alpha(_) => todo!(),
            }
        }

        for (name, typ) in router.typ_store.iter() {
            typ_store.insert(name, typ.clone());
        }

        RouterBuilder {
            config,
            middleware: MiddlewareMerger {
                middleware,
                middleware2: router.middleware,
                phantom: PhantomData,
            },
            queries,
            mutations,
            subscriptions,
            typ_store,
            phantom: PhantomData,
        }
    }

    pub fn build(self) -> Router<TCtx, TMeta> {
        let Self {
            config,
            queries,
            mutations,
            subscriptions,
            typ_store,
            ..
        } = self;

        let export_path = config.export_bindings_on_build.clone();
        let router = Router {
            config,
            queries,
            mutations,
            subscriptions,
            typ_store,
            phantom: PhantomData,
        };

        #[cfg(debug_assertions)]
        #[allow(clippy::unwrap_used)]
        if let Some(export_path) = export_path {
            router.export_ts(export_path).unwrap();
        }

        router
    }
}

// #[deprecated = "Removed in v1.0.0. Is now `<TResolver as ResolverFunction<_>>::typedef`"]
#[doc(hidden)]
pub fn typedef<TArg: Type, TResult: Type>(
    key: Cow<'static, str>,
    type_map: &mut TypeMap,
) -> Result<ProcedureDataType, ts::ExportError> {
    Ok(ProcedureDataType {
        key,
        input: <TArg as Type>::reference(type_map, &[]).inner,
        result: <TResult as Type>::reference(type_map, &[]).inner,
    })
}
