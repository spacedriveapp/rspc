use std::{future::Future, marker::PhantomData};

use serde::{de::DeserializeOwned, Serialize};
use specta::Type;

use crate::alpha::{AlphaMiddlewareContext, MwV2Result, MwV3};

/// TODO
pub trait MwArgMapper: Send + Sync {
    /// TODO
    type State: Send + Sync + 'static + Default;

    /// TODO
    ///
    /// This is not typesafe. If you get it wrong it will runtime panic!
    type Input<T>: DeserializeOwned + Type + 'static
    where
        T: DeserializeOwned + Type + 'static;

    /// TODO
    fn map<T: Serialize + DeserializeOwned + Type + 'static>(
        arg: Self::Input<T>,
    ) -> (T, Self::State);
}

pub struct MwArgMapperMiddleware<M: MwArgMapper>(PhantomData<M>);

impl<M: MwArgMapper + 'static> Default for MwArgMapperMiddleware<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: MwArgMapper + 'static> MwArgMapperMiddleware<M> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }

    pub fn mount<TLCtx, TNCtx, Fu, R>(
        &self,
        handler: impl Fn(AlphaMiddlewareContext, TLCtx, M::State) -> Fu + Send + Sync + 'static,
    ) -> impl MwV3<TLCtx, NewCtx = TNCtx>
    where
        TLCtx: Send + Sync + 'static,
        TNCtx: Send + Sync + 'static,
        Fu: Future<Output = R> + Send + Sync + 'static,
        R: MwV2Result<Ctx = TNCtx> + Send + 'static,
    {
        // TODO: Make this passthrough to new handler but provide the owned `State` as an arg
        MiddlewareFnWithTypeMapper(
            move |mw: AlphaMiddlewareContext, ctx| {
                let (out, state) = match serde_json::from_value(mw.input) {
                    Ok(val) => M::map::<serde_json::Value>(val),
                    Err(e) => {
                        tracing::error!(
                            "Failed to deserialize middleware input: {:?}, request: {:?}",
                            e,
                            mw.req
                        );
                        // TODO: Find a better way to handle error intead of fallback to default
                        (serde_json::Value::Null, M::State::default())
                    }
                };

                handler(
                    AlphaMiddlewareContext {
                        input: serde_json::to_value(out).unwrap_or_else(|e| {
                            tracing::error!(
                                "Failed to serialize output value: {:?}, request: {:?}",
                                e,
                                mw.req
                            );
                            serde_json::Value::Null
                        }),
                        req: mw.req,
                        _priv: (),
                    },
                    ctx,
                    state,
                )
            },
            PhantomData::<M>,
        )
    }
}

pub struct MiddlewareFnWithTypeMapper<M, F>(F, PhantomData<M>);

impl<M, TLCtx, F, Fu, R> MwV3<TLCtx> for MiddlewareFnWithTypeMapper<M, F>
where
    TLCtx: Send + Sync + 'static,
    F: Fn(AlphaMiddlewareContext, TLCtx) -> Fu + Send + Sync + 'static,
    Fu: Future<Output = R> + Send + 'static,
    R: MwV2Result + Send + 'static,
    M: MwArgMapper + 'static,
{
    type Fut = Fu;
    type Result = R;
    type NewCtx = R::Ctx; // TODO: Make this work with context switching
    type Arg<T: Type + DeserializeOwned + 'static> = M::Input<T>;

    fn run_me(&self, ctx: TLCtx, mw: AlphaMiddlewareContext) -> Self::Fut {
        (self.0)(mw, ctx)
    }
}
