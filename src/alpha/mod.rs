//! Alpha API. This is going to be the new API in the `v1.0.0` release.
//!
//! WARNING: Anything in this module does not follow semantic versioning until it's released however the API is fairly stable at this poinR.
//!

mod layer;
mod middleware;
mod procedure;
mod procedure_like;
mod router;
mod router_builder_like;
mod rspc;

pub use self::rspc::*;
pub use layer::*;
pub use middleware::*;
pub use procedure::*;
pub use procedure_like::*;
pub use router::*;
pub use router_builder_like::*;

pub use crate::alpha_stable::*;

#[cfg(feature = "unstable")]
pub mod unstable;

#[cfg(test)]
#[cfg(feature = "unstable")]
mod tests {
    use std::{path::PathBuf, time::Duration};

    use async_stream::stream;
    use serde::{de::DeserializeOwned, Serialize};
    use specta::Type;
    use tokio::time::sleep;

    use crate::alpha::{unstable::MwArgMapper, MwV2, ProcedureLike};

    use super::{unstable::MwArgMapperMiddleware, Rspc};

    #[allow(non_upper_case_globals)]
    const R: Rspc<()> = Rspc::new();

    #[test]
    fn test_alpha_api() {
        let r = R
            .router()
            .procedure(
                "todo",
                R.with(|mw: super::AlphaMiddlewareContext, ctx| async move { Ok(mw.next(ctx)) })
                    .query(|ctx, _: ()| {
                        println!("TODO: {:?}", ctx);
                        Ok(())
                    }),
            )
            .procedure(
                "todo2",
                R.with(|mw, ctx| async move {
                    let msg = format!(
                        "[LOG] req='{:?}' ctx='{:?}' input='{:?}'",
                        mw.req, ctx, mw.input
                    );
                    Ok(mw.next(ctx).resp(|result| async move {
                        println!("{msg} result='{result:?}'");
                        result
                    }))
                })
                .with(|mw, ctx| async move {
                    let msg = format!(
                        "[LOG2] req='{:?}' ctx='{:?}' input='{:?}'",
                        mw.req, ctx, mw.input
                    );
                    Ok(mw.next(ctx).resp(|result| async move {
                        println!("{msg} result='{result:?}'");
                        result
                    }))
                })
                .query(|ctx, _: ()| {
                    println!("TODO: {:?}", ctx);
                    Ok(())
                }),
            )
            .procedure(
                "todo3",
                R.query(|ctx, _: ()| {
                    println!("TODO: {:?}", ctx);
                    Ok(())
                }),
            )
            .procedure(
                "demoSubscriptions",
                R.subscription(|_ctx, _: ()| {
                    stream! {
                        println!("Client subscribed to 'pings'");
                        for i in 0..5 {
                            println!("Sending ping {}", i);
                            yield "ping".to_string();
                            sleep(Duration::from_secs(1)).await;
                        }
                    }
                }),
            )
            .compat();

        r.export_ts(PathBuf::from("./demo.bindings.ts"))
            .expect("Failed to export TypeScript bindings");
    }

    #[test]
    fn test_context_switching() {
        const R: Rspc = Rspc::new();

        let _ = R
            .with(|mw, ctx| async move { Ok(mw.next((ctx, 42))) })
            .with(|mw, ctx| async move { Ok(mw.next((ctx, 42))) })
            .with(|mw, ctx| async move { Ok(mw.next(ctx)) })
            .query(|ctx, _: ()| {
                let ((_, _), _) = ctx; // Assert correct type

                Ok(())
            });
    }

    #[test]
    fn test_init_from_function() {
        const R: Rspc = Rspc::new();

        fn demo() -> impl ProcedureLike<LayerCtx = ((), i32)> {
            R.with(|mw, ctx| async move {
                Ok(mw.next((ctx, 42))) // Context switch
            })
        }

        let _ = demo().query(|ctx, _: ()| {
            let (_, _) = ctx; // Assert correct type
            Ok(())
        });
    }

    #[test]
    fn with_middleware_from_func() {
        pub fn library<TLCtx>() -> impl MwV2<TLCtx, NewCtx = (TLCtx, i32)>
        where
            TLCtx: Send + Sync + 'static,
        {
            |mw, ctx| async move { Ok(mw.next((ctx, 42))) }
        }

        let _ = R.with(library()).with(library()).query(|ctx, _: ()| {
            let ((_, _), _) = ctx; // Assert correct type
            Ok(())
        });
    }

    #[test]
    fn middleware_args() {
        pub struct LibraryArgsMap;

        impl MwArgMapper for LibraryArgsMap {
            type Input<T> = (T, i32)
            where
                T: DeserializeOwned + Type + 'static;
            type State = i32;

            fn map<T: Serialize + DeserializeOwned + Type + 'static>(
                arg: Self::Input<T>,
            ) -> (T, Self::State) {
                (arg.0, arg.1)
            }
        }

        let _p = R
            .with2(MwArgMapperMiddleware::<LibraryArgsMap>::new().mount(
                |mw, ctx, state| async move {
                    let _state: i32 = state; // Assert correct type
                    let _ctx: () = (); // Assert correct type

                    Ok(mw.next((ctx, 42)))
                },
            ))
            .query(|ctx, _: ()| {
                println!("TODO: {:?}", ctx);
                ctx.0; // Test Rust inference is working
                Ok(())
            });
    }

    #[test]
    fn multiple_middleware_args() {
        pub struct DoubleTupleMapper;

        impl MwArgMapper for DoubleTupleMapper {
            type Input<T> = (T, Self::State)
            where
                T: DeserializeOwned + Type + 'static;
            type State = ((), ());

            fn map<T: Serialize + DeserializeOwned + Type + 'static>(
                arg: Self::Input<T>,
            ) -> (T, Self::State) {
                (arg.0, ((), ()))
            }
        }

        pub struct TripleTupleMapper;

        impl MwArgMapper for TripleTupleMapper {
            type Input<T> = (T, Self::State)
            where
                T: DeserializeOwned + Type + 'static;
            type State = ((), (), ());

            fn map<T: Serialize + DeserializeOwned + Type + 'static>(
                arg: Self::Input<T>,
            ) -> (T, Self::State) {
                (arg.0, ((), (), ()))
            }
        }

        let p = R
            .with2(MwArgMapperMiddleware::<DoubleTupleMapper>::new().mount(
                |mw, ctx, state| async move {
                    let (_, _) = state; // Assert type is correct
                    Ok(mw.next(ctx))
                },
            ))
            .with2(MwArgMapperMiddleware::<TripleTupleMapper>::new().mount(
                |mw, ctx, state| async move {
                    let (_, _, _) = state; // Assert type is correct

                    Ok(mw.next(ctx))
                },
            ))
            .query(|_, _: i32| Ok(()));

        R.router()
            .procedure("demo", p)
            .compat()
            .export_ts(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./demo2.bindings.ts"))
            .expect("Failed to export TypeScript bindings");
    }
}
