import type {
  _inferInfiniteQueryProcedureHandlerInput,
  _inferProcedureHandlerInput,
  inferMutationInput,
  inferMutationResult,
  inferProcedureResult,
  inferQueryInput,
  inferQueryResult,
  KeyAndInput,
  ProceduresDef,
} from '@spacedrive/rspc-client'
import type {
  UseMutationOptions,
  UseMutationResult,
  UseQueryOptions,
  UseQueryResult,
} from '@tanstack/react-query'
import type { ReactElement } from 'react'

import { AlphaClient, RSPCError } from '@spacedrive/rspc-client'
import {
  useInfiniteQuery as __useInfiniteQuery,
  useMutation as __useMutation,
  useQuery as __useQuery,
  hashKey,
  QueryClient,
  QueryClientProvider,
} from '@tanstack/react-query'
import React, { useContext as _useContext, createContext, useEffect, useMemo } from 'react'

export interface BaseOptions<TProcedures extends ProceduresDef> {
  rspc?: {
    client?: AlphaClient<TProcedures>
  }
}

export interface SubscriptionOptions<TOutput> {
  enabled?: boolean
  onStarted?: () => void
  onData: (data: TOutput) => void
  onError?: (err: RSPCError | Error) => void
}

export interface Context<TProcedures extends ProceduresDef> {
  client: AlphaClient<TProcedures>
  queryClient: QueryClient
}

export type HooksOpts<P extends ProceduresDef> = {
  context: React.Context<Context<P>>
}

/**
 * Creates React Query hooks for the given RSPC client.
 * @param client - The RSPC client.
 * @param opts - Optional hooks options.
 * @returns An object containing the hooks and provider component.
 */
export function createReactQueryHooks<P extends ProceduresDef>(
  client: AlphaClient<P>,
  opts?: HooksOpts<P>
) {
  type TBaseOptions = BaseOptions<P>

  const mapQueryKey: (keyAndInput: KeyAndInput) => KeyAndInput =
    client.mapQueryKey ?? ((x: KeyAndInput) => x)
  const Context =
    opts?.context ?? createContext<Context<P>>({ client, queryClient: new QueryClient() })

  function useContext() {
    const ctx = _useContext(Context)
    if (ctx?.queryClient == null)
      throw new Error(
        'The rspc context has not been set. Ensure you have the <rspc.Provider> component higher up in your component tree.'
      )
    return ctx
  }

  function useQuery<
    K extends P['queries']['key'] & string,
    TQueryFnData = inferQueryResult<P, K>,
    TData = inferQueryResult<P, K>,
  >(
    keyAndInput: [key: K, ...input: _inferProcedureHandlerInput<P, 'queries', K>],
    opts?: Omit<
      UseQueryOptions<TQueryFnData, RSPCError, TData, [K, inferQueryInput<P, K>]>,
      'queryKey' | 'queryFn'
    > &
      TBaseOptions
  ): UseQueryResult<TData, RSPCError> {
    const { rspc, ...rawOpts } = opts ?? {}
    const client = rspc?.client ?? useContext().client

    return __useQuery({
      queryKey: mapQueryKey(keyAndInput) as [K, inferQueryInput<P, K>],
      queryFn: async () => {
        return (await client.query(keyAndInput)) as TQueryFnData
      },
      ...rawOpts,
    })
  }

  function useMutation<K extends P['mutations']['key'] & string, TContext = unknown>(
    key: K | [K],
    opts?: UseMutationOptions<
      inferMutationResult<P, K>,
      RSPCError,
      inferMutationInput<P, K> extends never ? undefined : inferMutationInput<P, K>,
      TContext
    > &
      TBaseOptions
  ): UseMutationResult<
    inferMutationResult<P, K>,
    RSPCError,
    inferMutationInput<P, K> extends never ? undefined : inferMutationInput<P, K>,
    TContext
  > {
    const { rspc, ...rawOpts } = opts ?? {}
    const client = rspc?.client ?? useContext().client

    return __useMutation({
      mutationFn: async (...input: inferMutationInput<P, K>[]) => {
        const actualKey = Array.isArray(key) ? key[0] : key
        return client.mutation([
          actualKey,
          ...(input as _inferProcedureHandlerInput<P, 'mutations', K>),
        ])
      },
      ...rawOpts,
    })
  }

  function useSubscription<
    K extends P['subscriptions']['key'] & string,
    TData = inferProcedureResult<P, 'subscriptions', K>,
  >(
    keyAndInput: [key: K, ...input: _inferProcedureHandlerInput<P, 'subscriptions', K>],
    opts: SubscriptionOptions<TData> & TBaseOptions
  ) {
    const client = opts?.rspc?.client ?? useContext().client
    const queryKey = hashKey(keyAndInput)
    const enabled = opts?.enabled ?? true

    useEffect(() => {
      if (!enabled) {
        return
      }
      return client.addSubscription<K, TData>(keyAndInput, {
        onData: opts.onData,
        onError: opts.onError,
      })
    }, [queryKey, enabled])
  }

  const Provider: React.FC<{
    children?: ReactElement
    client: AlphaClient<P>
    queryClient: QueryClient
  }> = ({ children, client, queryClient }) => {
    const contextValue = useMemo(() => ({ client, queryClient }), [client, queryClient])

    return (
      <Context.Provider value={contextValue}>
        <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
      </Context.Provider>
    )
  }

  return {
    _rspc_def: {} as P, // This allows inferring the operations type from TS helpers
    Provider,
    useContext,
    useQuery,
    useMutation,
    useSubscription,
  }
}
