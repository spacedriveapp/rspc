import {
  _inferInfiniteQueryProcedureHandlerInput,
  _inferProcedureHandlerInput,
  Client,
  inferInfiniteQueries,
  inferInfiniteQueryResult,
  inferMutationInput,
  inferMutationResult,
  inferProcedures,
  inferQueryInput,
  inferQueryResult,
  inferSubscriptionResult,
  ProcedureDef,
  ProceduresDef,
  ProceduresLike,
  RSPCError,
} from '@oscartbeaumont-sd/rspc-client'
import {
  useInfiniteQuery as __useInfiniteQuery,
  useMutation as __useMutation,
  useQuery as __useQuery,
  hashQueryKey,
  QueryClient,
  QueryClientProvider,
  UseInfiniteQueryOptions,
  UseInfiniteQueryResult,
  UseMutationOptions,
  UseMutationResult,
  UseQueryOptions,
  UseQueryResult,
} from '@tanstack/react-query'
import React, { useContext as _useContext, createContext, ReactElement, useEffect } from 'react'

export interface BaseOptions<TProcedures extends ProceduresDef> {
  rspc?: {
    client?: Client<TProcedures>
  }
}

export interface SubscriptionOptions<TOutput> {
  enabled?: boolean
  onStarted?: () => void
  onData: (data: TOutput) => void
  onError?: (err: RSPCError) => void
}

export interface Context<TProcedures extends ProceduresDef> {
  client: Client<TProcedures>
  queryClient: QueryClient
}

export function createReactQueryHooks<TProceduresLike extends ProceduresLike>() {
  type TProcedures = inferProcedures<TProceduresLike>
  type TBaseOptions = BaseOptions<TProcedures>

  const Context = createContext<Context<TProcedures>>(undefined!)

  function useContext() {
    const ctx = _useContext(Context)
    if (ctx?.queryClient === undefined)
      throw new Error(
        'The rspc context has not been set. Ensure you have the <rspc.Provider> component higher up in your component tree.'
      )
    return ctx
  }

  type CustomQueryHookReturn<TConstrainedProcedures extends ProcedureDef> = <
    K extends TConstrainedProcedures['key'] & string,
    TQueryFnData = Extract<TConstrainedProcedures, { key: K }>['result'],
    TData = Extract<TConstrainedProcedures, { key: K }>['result'],
  >(
    keyAndInput: [
      key: K,
      ...input: Extract<TConstrainedProcedures, { key: K }>['input'] extends never | null
        ? []
        : [Extract<TConstrainedProcedures, { key: K }>['input']],
    ],
    opts?: Omit<
      UseQueryOptions<
        TQueryFnData,
        RSPCError,
        TData,
        [K, Extract<TConstrainedProcedures, { key: K }>['input']]
      >,
      'queryKey' | 'queryFn'
    > &
      TBaseOptions
  ) => UseQueryResult<TData, RSPCError>

  /*
  [UNDOCUMENTED]: This function IS NOT and will probably never be completely type safe. It is for people doing crazy stuff on top of rspc.
  By using it you accept the risk involved with a lack of type safety. If you can make this more typesafe a PR would be welcome!
  */
  function customQuery<TConstrainedProcedures extends ProcedureDef>(
    map: (
      key: [key: TConstrainedProcedures['key'], ...input: TConstrainedProcedures['input']]
    ) => [
      inferProcedures<TProceduresLike>['queries']['key'] & string,
      inferProcedures<TProceduresLike>['queries']['input'],
    ]
  ): CustomQueryHookReturn<TConstrainedProcedures> {
    return (keyAndInput, opts) => {
      const { rspc, ...rawOpts } = opts ?? {}
      let client = rspc?.client
      if (!client) {
        client = useContext().client
      }

      return __useQuery({
        queryKey: map(keyAndInput as any),
        queryFn: async () => {
          return await client!.query(map(keyAndInput as any) as any)
        },
        ...(rawOpts as any),
      })
    }
  }

  function useQuery<
    K extends inferProcedures<TProceduresLike>['queries']['key'] & string,
    TQueryFnData = inferQueryResult<TProcedures, K>,
    TData = inferQueryResult<TProcedures, K>,
  >(
    keyAndInput: [key: K, ...input: _inferProcedureHandlerInput<TProcedures, 'queries', K>],
    opts?: Omit<
      UseQueryOptions<TQueryFnData, RSPCError, TData, [K, inferQueryInput<TProcedures, K>]>,
      'queryKey' | 'queryFn'
    > &
      TBaseOptions
  ): UseQueryResult<TData, RSPCError> {
    const { rspc, ...rawOpts } = opts ?? {}
    let client = rspc?.client
    if (!client) {
      client = useContext().client
    }

    return __useQuery(
      keyAndInput,
      async () => {
        return await client!.query(keyAndInput)
      },
      rawOpts as any
    )
  }

  function useInfiniteQuery<K extends inferInfiniteQueries<TProcedures>['key'] & string>(
    keyAndInput: [key: K, ...input: _inferInfiniteQueryProcedureHandlerInput<TProcedures, K>],
    opts?: Omit<
      UseInfiniteQueryOptions<
        inferInfiniteQueryResult<TProcedures, K>,
        RSPCError,
        inferInfiniteQueryResult<TProcedures, K>,
        inferInfiniteQueryResult<TProcedures, K>,
        [K, inferQueryInput<TProcedures, K>]
      >,
      'queryKey' | 'queryFn'
    > &
      TBaseOptions
  ): UseInfiniteQueryResult<inferInfiniteQueryResult<TProcedures, K>, RSPCError> {
    const { rspc, ...rawOpts } = opts ?? {}
    let client = rspc?.client
    if (!client) {
      client = useContext().client
    }

    return __useInfiniteQuery({
      queryKey: keyAndInput,
      queryFn: async () => {
        throw new Error('TODO') // TODO: Finish this
      },
      ...(rawOpts as any),
    })
  }

  type CustomMutationHookReturn<TConstrainedProcedures extends ProcedureDef> = <
    K extends TConstrainedProcedures['key'] & string,
    TContext = unknown,
  >(
    key: K | [K],
    opts?: UseMutationOptions<
      Extract<TConstrainedProcedures, { key: K }>['result'],
      RSPCError,
      Extract<TConstrainedProcedures, { key: K }>['result'] extends never
        ? undefined
        : Extract<TConstrainedProcedures, { key: K }>['result'],
      TContext
    > &
      TBaseOptions
  ) => UseMutationResult<
    Extract<TConstrainedProcedures, { key: K }>['result'],
    RSPCError,
    Extract<TConstrainedProcedures, { key: K }>['input'] extends never
      ? undefined
      : Extract<TConstrainedProcedures, { key: K }>['input'],
    TContext
  >

  /*
  [UNDOCUMENTED]: This function IS NOT and will probably never be completely type safe. It is for people doing crazy stuff on top of rspc.
  By using it you accept the risk involved with a lack of type safety. If you can make this more typesafe a PR would be welcome!
  */
  function customMutation<TConstrainedProcedures extends ProcedureDef>(
    map: (
      key: [key: TConstrainedProcedures['key'], ...input: [TConstrainedProcedures['input']]]
    ) => [
      inferProcedures<TProceduresLike>['mutations']['key'] & string,
      inferProcedures<TProceduresLike>['mutations']['input'],
    ]
  ): CustomMutationHookReturn<TConstrainedProcedures> {
    return (key, opts) => {
      const { rspc, ...rawOpts } = opts ?? {}
      let client = rspc?.client
      if (!client) {
        client = useContext().client
      }

      return __useMutation({
        mutationFn: async input => {
          const actualKey = Array.isArray(key) ? key[0] : key
          return client!.mutation(map([actualKey, input]) as any)
        },
        ...(rawOpts as any),
      })
    }
  }

  function useMutation<K extends TProcedures['mutations']['key'] & string, TContext = unknown>(
    key: K | [K],
    opts?: UseMutationOptions<
      inferMutationResult<TProcedures, K>,
      RSPCError,
      inferMutationInput<TProcedures, K> extends never
        ? undefined
        : inferMutationInput<TProcedures, K>,
      TContext
    > &
      TBaseOptions
  ): UseMutationResult<
    inferMutationResult<TProcedures, K>,
    RSPCError,
    inferMutationInput<TProcedures, K> extends never
      ? undefined
      : inferMutationInput<TProcedures, K>,
    TContext
  > {
    const { rspc, ...rawOpts } = opts ?? {}
    let client = rspc?.client
    if (!client) {
      client = useContext().client
    }

    return __useMutation(async (input: any) => {
      const actualKey = Array.isArray(key) ? key[0] : key
      return client!.mutation([actualKey, input] as any)
    }, rawOpts as any)
  }

  function useSubscription<
    K extends TProcedures['subscriptions']['key'] & string,
    TData = inferSubscriptionResult<TProcedures, K>,
  >(
    keyAndInput: [key: K, ...input: _inferProcedureHandlerInput<TProcedures, 'subscriptions', K>],
    opts: SubscriptionOptions<TData> & TBaseOptions
  ) {
    let client = opts?.rspc?.client
    if (!client) {
      client = useContext().client
    }
    const queryKey = hashQueryKey(keyAndInput)

    const enabled = opts?.enabled ?? true

    return useEffect(() => {
      if (!enabled) {
        return
      }
      let isStopped = false
      // @ts-ignore // TODO: Fix this
      const unsubscribe = client!.addSubscription<K, TData>(keyAndInput as any, {
        onStarted: () => {
          if (!isStopped) {
            opts.onStarted?.()
          }
        },
        // @ts-ignore// TODO: Fix this
        onData: data => {
          if (!isStopped) {
            opts.onData(data)
          }
        },
        // @ts-ignore // TODO: Fix this
        onError: err => {
          if (!isStopped) {
            opts.onError?.(err)
          }
        },
      })
      return () => {
        isStopped = true
        unsubscribe()
      }
    }, [queryKey, enabled])
  }

  return {
    _rspc_def: undefined! as TProcedures, // This allows inferring the operations type from TS helpers
    Provider: ({
      children,
      client,
      queryClient,
    }: {
      children?: ReactElement
      client: Client<TProcedures>
      queryClient: QueryClient
    }) => (
      <Context.Provider
        value={{
          client,
          queryClient,
        }}
      >
        <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
      </Context.Provider>
    ),
    useContext,
    customQuery,
    useQuery,
    // useInfiniteQuery,
    customMutation,
    useMutation,
    useSubscription,
  }
}
