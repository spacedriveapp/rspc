import type {
  _inferInfiniteQueryProcedureHandlerInput,
  _inferProcedureHandlerInput,
  inferMutationResult,
  inferProcedureResult,
  inferQueryResult,
  ProceduresDef,
} from '../typescript'
import type { Link, LinkResult, Operation, OperationContext } from './links/link'

import { RSPCError } from '../error'

export interface SubscriptionOptions<TOutput> {
  onData: (data: TOutput) => void
  onError?: (err: RSPCError) => void
}

export type KeyAndInput = [string, ...unknown[]]

type OperationOpts = {
  signal?: AbortSignal
  context?: OperationContext
  // skipBatch?: boolean; // TODO: Make this work + add this to React
}

interface ClientArgs {
  links: Link[]
  onError?: (err: RSPCError) => void | Promise<void>
}

export function initRspc<P extends ProceduresDef>(args: ClientArgs) {
  return new AlphaClient<P>(args)
}

const generateRandomId = () => Math.random().toString(36).slice(2)

export class AlphaClient<P extends ProceduresDef> {
  private links: Link[]
  private onError?: (err: RSPCError) => void | Promise<void>
  mapQueryKey?: (keyAndInput: KeyAndInput) => KeyAndInput // TODO: Do something so a single React.context can handle multiple of these

  constructor(args: ClientArgs) {
    if (args.links.length === 0) {
      throw new Error('Must provide at least one link')
    }

    this.links = args.links
    this.onError = args.onError
  }

  async query<K extends P['queries']['key'] & string>(
    keyAndInput: [key: K, ...input: _inferProcedureHandlerInput<P, 'queries', K>],
    opts?: OperationOpts
  ): Promise<inferQueryResult<P, K>> {
    try {
      const keyAndInput2 = this.mapQueryKey?.(keyAndInput) ?? keyAndInput

      const result = exec(
        {
          id: generateRandomId(),
          type: 'query',
          input: keyAndInput2[1],
          path: keyAndInput2[0],
          context: opts?.context ?? {},
        },
        this.links
      )
      opts?.signal?.addEventListener('abort', result.abort)

      return await new Promise(result.exec)
    } catch (err) {
      if (this.onError && err instanceof RSPCError) await this.onError(err)
      throw err
    }
  }

  async mutation<K extends P['mutations']['key'] & string>(
    keyAndInput: [key: K, ...input: _inferProcedureHandlerInput<P, 'mutations', K>],
    opts?: OperationOpts
  ): Promise<inferMutationResult<P, K>> {
    try {
      const keyAndInput2 = this.mapQueryKey?.(keyAndInput) ?? keyAndInput

      const result = exec(
        {
          id: generateRandomId(),
          type: 'mutation',
          input: keyAndInput2[1],
          path: keyAndInput2[0],
          context: opts?.context ?? {},
        },
        this.links
      )
      opts?.signal?.addEventListener('abort', result.abort)

      return await new Promise(result.exec)
    } catch (err) {
      if (this.onError && err instanceof RSPCError) await this.onError(err)
      throw err
    }
  }

  async addSubscription<
    K extends P['subscriptions']['key'] & string,
    TData = inferProcedureResult<P, 'subscriptions', K>,
  >(
    keyAndInput: [K, ..._inferProcedureHandlerInput<P, 'subscriptions', K>],
    opts: SubscriptionOptions<TData> & { context?: OperationContext }
  ): Promise<() => void> {
    try {
      const keyAndInput2 = this.mapQueryKey?.(keyAndInput) ?? keyAndInput

      const result = exec(
        {
          id: generateRandomId(),
          type: 'subscription',
          input: keyAndInput2[1],
          path: keyAndInput2[0],
          context: opts.context ?? {},
        },
        this.links
      )

      result.exec(
        data => opts.onData(data as TData),
        err => {
          if (err instanceof RSPCError) opts.onError?.(err)
          console.error(err)
        }
      )
      return result.abort
    } catch (err) {
      if (err instanceof RSPCError) {
        if (this.onError) await this.onError(err)
        return () => {}
      }

      throw err
    }
  }

  dangerouslyHookIntoInternals<P2 extends ProceduresDef = P>(opts?: {
    mapQueryKey?: (keyAndInput: KeyAndInput) => KeyAndInput
  }): AlphaClient<P2> {
    this.mapQueryKey = opts?.mapQueryKey
    return this as unknown as AlphaClient<P2>
  }
}

function exec(op: Operation, links: Link[]): LinkResult {
  if (!links[0]) throw new Error('No links provided')

  let prevLinkResult: LinkResult = {
    exec: () => {
      throw new Error(
        "rspc: no terminating link was attached! Did you forget to add a 'httpLink' or 'wsLink' link?"
      )
    },
    abort: () => {},
  }

  for (let linkIndex = links.length - 1; linkIndex >= 0; linkIndex--) {
    const link = links[linkIndex]
    if (!link) continue
    const result = link({
      op,
      next: () => prevLinkResult,
    })
    prevLinkResult = result
  }

  return prevLinkResult
}
