use std::{borrow::Cow, marker::PhantomData};

use specta::TypeMap;

use crate::{
    internal::{BaseMiddleware, ProcedureStore},
    is_invalid_router_prefix, Config, Router, RouterBuilder, RouterBuilderLike,
};

use super::{
    procedure::AlphaProcedure, AlphaBaseMiddleware, AlphaRequestLayer, AlphaRouterBuilderLike,
    FutureMarker, ProcedureList, RequestKind, RequestLayerMarker, ResolverFunction,
    StreamLayerMarker, StreamMarker,
};

pub struct AlphaRouter<TCtx>
where
    TCtx: Send + Sync + 'static,
{
    procedures: ProcedureList<TCtx>,
    dangerously_map_types: Option<Box<dyn FnOnce(&mut TypeMap) + Send + Sync + 'static>>,
}

impl<TCtx> Default for AlphaRouter<TCtx>
where
    TCtx: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<TCtx> AlphaRouter<TCtx>
where
    TCtx: Send + Sync + 'static,
{
    /// Creates a new `AlphaRouter`.
    /// Avoid using this directly, use `Rspc::router` instead so the types can be inferred.
    pub fn new() -> Self {
        Self {
            procedures: Vec::new(),
            dangerously_map_types: None,
        }
    }

    pub fn procedure(mut self, key: &'static str, procedure: impl IntoProcedure<TCtx>) -> Self {
        self.procedures
            .push((Cow::Borrowed(key), Box::new(procedure)));
        self
    }

    // TODO
    // pub fn merge(self, prefix: &'static str, r: impl RouterBuilderLike<TCtx>) -> Self {
    //     // TODO: disallow `.` in prefix
    //     let r = r.expose();
    //     todo!();
    // }

    pub fn query<R, RMarker>(
        self,
        builder: R,
    ) -> AlphaProcedure<R, RequestLayerMarker<RMarker>, AlphaBaseMiddleware<TCtx>>
    where
        R: ResolverFunction<RequestLayerMarker<RMarker>, LayerCtx = TCtx>
            + Fn(TCtx, R::Arg) -> R::Result,
        R::Result: AlphaRequestLayer<R::ResultMarker, Type = FutureMarker>,
    {
        AlphaProcedure::new_from_resolver(
            RequestLayerMarker::new(RequestKind::Query),
            AlphaBaseMiddleware::new(),
            builder,
        )
    }

    pub fn mutation<R, RMarker>(
        self,
        builder: R,
    ) -> AlphaProcedure<R, RequestLayerMarker<RMarker>, AlphaBaseMiddleware<TCtx>>
    where
        R: ResolverFunction<RequestLayerMarker<RMarker>, LayerCtx = TCtx>
            + Fn(TCtx, R::Arg) -> R::Result,
        R::Result: AlphaRequestLayer<R::ResultMarker, Type = FutureMarker>,
    {
        AlphaProcedure::new_from_resolver(
            RequestLayerMarker::new(RequestKind::Mutation),
            AlphaBaseMiddleware::new(),
            builder,
        )
    }

    pub fn subscription<R, RMarker>(
        self,
        builder: R,
    ) -> AlphaProcedure<R, StreamLayerMarker<RMarker>, AlphaBaseMiddleware<TCtx>>
    where
        R: ResolverFunction<StreamLayerMarker<RMarker>, LayerCtx = TCtx>
            + Fn(TCtx, R::Arg) -> R::Result,
        R::Result: AlphaRequestLayer<R::RequestMarker, Type = StreamMarker>,
    {
        AlphaProcedure::new_from_resolver(
            StreamLayerMarker::new(),
            AlphaBaseMiddleware::new(),
            builder,
        )
    }

    pub fn merge(
        mut self,
        prefix: &'static str,
        router: impl AlphaRouterBuilderLike<TCtx>,
    ) -> Self {
        let (prefix, prefix_valid) = is_invalid_router_prefix(prefix);
        if prefix_valid {
            panic!(
                "RSPC error: attempted to merge a router with the prefix '{}', however this prefix is not allowed.",
                prefix
            );
        }

        self.procedures.extend(
            router
                .procedures()
                .into_iter()
                .map(|(key, procedure)| (Cow::Owned(format!("{}{}", prefix, key)), procedure)),
        );

        self
    }

    pub fn sd_patch_types_dangerously(
        mut self,
        func: impl FnOnce(&mut TypeMap) + Send + Sync + 'static,
    ) -> Self {
        self.dangerously_map_types = Some(Box::new(func));
        self
    }

    // #[deprecated = "Being removed on v1.0.0 once the new syntax is stable"]
    pub fn compat(self) -> Router<TCtx, ()> {
        // TODO: Eventually take these as an argument so we can access the plugin store from the parent router -> For this we do this for compat
        let mut queries = ProcedureStore::new("queries"); // TODO: Take in as arg
        let mut mutations = ProcedureStore::new("mutations"); // TODO: Take in as arg
        let mut subscriptions = ProcedureStore::new("subscriptions"); // TODO: Take in as arg
        let mut typ_store = TypeMap::default(); // TODO: Take in as arg

        let mut ctx = IntoProcedureCtx {
            ty_store: &mut typ_store,
            queries: &mut queries,
            mutations: &mut mutations,
            subscriptions: &mut subscriptions,
        };

        for (key, mut procedure) in self.procedures.into_iter() {
            // TODO: Pass in the `key` here with the router merging prefixes already applied so it's the final runtime key
            procedure.build(key, &mut ctx);
        }

        Router {
            config: Config::new(),
            queries,
            mutations,
            subscriptions,
            typ_store,
            phantom: PhantomData,
        }
    }

    pub fn build(self, config: Config) -> Router<TCtx, ()> {
        // TODO: Eventually take these as an argument so we can access the plugin store from the parent router -> For this we do this for compat
        let mut queries = ProcedureStore::new("queries"); // TODO: Take in as arg
        let mut mutations = ProcedureStore::new("mutations"); // TODO: Take in as arg
        let mut subscriptions = ProcedureStore::new("subscriptions"); // TODO: Take in as arg
        let mut typ_store = TypeMap::default(); // TODO: Take in as arg

        let mut ctx = IntoProcedureCtx {
            ty_store: &mut typ_store,
            queries: &mut queries,
            mutations: &mut mutations,
            subscriptions: &mut subscriptions,
        };

        for (key, mut procedure) in self.procedures.into_iter() {
            // TODO: Pass in the `key` here with the router merging prefixes already applied so it's the final runtime key
            procedure.build(key, &mut ctx);
        }

        if let Some(dangerously_map_types) = self.dangerously_map_types {
            (dangerously_map_types)(&mut typ_store);
        }

        let router = Router {
            config,
            queries,
            mutations,
            subscriptions,
            typ_store,
            phantom: PhantomData,
        };

        #[cfg(debug_assertions)]
        if let Some(export_path) = &router.config.export_bindings_on_build {
            router
                .export_ts(export_path)
                .expect("Failed to export TypeScript bindings");
        }

        router
    }
}

impl<TCtx: Send + Sync + 'static> AlphaRouterBuilderLike<TCtx> for AlphaRouter<TCtx> {
    fn procedures(self) -> ProcedureList<TCtx> {
        self.procedures
    }
}

pub struct IntoProcedureCtx<'a, TCtx> {
    pub ty_store: &'a mut TypeMap,
    pub queries: &'a mut ProcedureStore<TCtx>,
    pub mutations: &'a mut ProcedureStore<TCtx>,
    pub subscriptions: &'a mut ProcedureStore<TCtx>,
}

pub trait IntoProcedure<TCtx>: 'static {
    fn build(&mut self, key: Cow<'static, str>, ctx: &mut IntoProcedureCtx<'_, TCtx>);
}

impl<TCtx> RouterBuilderLike<TCtx> for AlphaRouter<TCtx>
where
    TCtx: Send + Sync + 'static,
{
    type Meta = ();
    type Middleware = BaseMiddleware<TCtx>;

    fn expose(self) -> RouterBuilder<TCtx, Self::Meta, Self::Middleware> {
        let r = self.compat();
        RouterBuilder {
            config: Config::default(),
            middleware: BaseMiddleware::new(),
            queries: r.queries,
            mutations: r.mutations,
            subscriptions: r.subscriptions,
            typ_store: r.typ_store,
            phantom: PhantomData,
        }
    }
}
