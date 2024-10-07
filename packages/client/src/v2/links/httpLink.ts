import type { Link, Operation } from './link'

import { RSPCError } from '../../error'

type HTTPHeaders = Record<string, string | string[] | undefined>

type BaseHttpLinkOpts = {
  url: string
  /**
   * Add ponyfill for fetch
   */
  fetch?: typeof fetch
  /**
   * Add ponyfill for AbortController
   */
  AbortController?: typeof AbortController | null
}

type HttpLinkOpts = BaseHttpLinkOpts & {
  /**
   * Headers to be set on outgoing requests or a callback that of said headers
   */
  headers?: HTTPHeaders | ((opts: { op: Operation }) => HTTPHeaders | Promise<HTTPHeaders>)
}

/**
 * HTTP Fetch link for rspc
 */
export function httpLink(opts: HttpLinkOpts): Link {
  const fetchFn = opts.fetch || globalThis.fetch.bind(globalThis)
  const abortController = opts.AbortController || globalThis.AbortController.bind(globalThis)

  return ({ op }) => {
    const abort = new abortController()
    return {
      exec: async (resolve, reject) => {
        if (op.type === 'subscription' || op.type === 'subscriptionStop') {
          reject(
            // TODO: Move to `RSPCError` type??
            new Error(
              `Subscribing to '${op.path}' failed as the HTTP transport does not support subscriptions! Maybe try using the websocket transport?`
            )
          )
          return
        }

        let method = 'GET'
        let body: BodyInit | null = null
        let headers = new Headers()

        const defaultHeaders =
          typeof opts.headers === 'function' ? await opts.headers({ op }) : opts.headers
        if (defaultHeaders) {
          for (const [key, value] of Object.entries(defaultHeaders)) {
            if (Array.isArray(value)) {
              for (const v of value) {
                headers.append(key, v)
              }
            } else {
              headers.set(key, value || '')
            }
          }
        }

        const params = new URLSearchParams()
        if (op.type === 'query') {
          if (op.input !== undefined) {
            params.append('input', JSON.stringify(op.input))
          }
        } else if (op.type === 'mutation') {
          method = 'POST'
          body = JSON.stringify(op.input || {})
          headers.set('Content-Type', 'application/json')
        }

        const paramsStr = params.toString()
        const resp = await fetchFn(
          `${opts.url}/${op.path}${paramsStr.length > 0 ? `?${paramsStr}` : ''}`,
          {
            method,
            body,
            headers,
            signal: abort.signal,
          }
        )
        // TODO: validate response
        const respBody = await resp.json()
        const { type, data } = respBody.result
        if (type === 'error') {
          const { code, message } = data
          reject(new RSPCError(code, message))
        } else {
          resolve(data)
        }
      },
      execBatch: async () => {},
      abort() {
        abort.abort()
      },
    }
  }
}

type HttpBatchLinkOpts = BaseHttpLinkOpts & {
  /**
   * Headers to be set on outgoing requests or a callback that of said headers
   */
  headers?: HTTPHeaders | ((opts: { ops: Operation[] }) => HTTPHeaders | Promise<HTTPHeaders>)
}

type BatchedItem = {
  op: Operation
  resolve: (result: unknown) => void
  reject: (error: Error | RSPCError) => void
  abort: AbortController
}

/**
 * Wrapper around httpLink that applies request batching. This is great for performance but may be problematic if your using HTTP caching.
 */
// TODO: Ability to use context to skip batching on certain operations
export function httpBatchLink(opts: HttpBatchLinkOpts): Link {
  const fetchFn = opts.fetch || globalThis.fetch.bind(globalThis)
  const abortController = opts.AbortController || globalThis.AbortController.bind(globalThis)

  const pushBatch = async (batch: BatchedItem[]) => {
    let headers = new Headers()
    const defaultHeaders =
      typeof opts.headers === 'function'
        ? await opts.headers({ ops: batch.map(b => b.op) })
        : opts.headers
    if (defaultHeaders) {
      for (const [key, value] of Object.entries(defaultHeaders)) {
        if (Array.isArray(value)) {
          for (const v of value) {
            headers.append(key, v)
          }
        } else {
          headers.set(key, value || '')
        }
      }
    }

    const resp = await fetchFn(`${opts.url}/_batch`, {
      method: 'POST',
      headers,
      body: JSON.stringify(
        batch.map(({ op }) => ({
          id: op.id,
          method: op.type,
          params: {
            path: op.path,
            input: op.input,
          },
        }))
      ),
    })

    // TODO: Validate response
    const body: unknown = await resp.json()
    if (!Array.isArray(body)) {
      console.error('rspc: batch response not an array!')
      return
    }

    if (body.length !== batch.length) {
      console.error('rspc: batch response length mismatch!')
      return
    }

    for (const [i, item] of body.entries()) {
      const batchItem = batch[i]
      if (!batchItem || batchItem.abort.signal?.aborted) {
        continue
      }

      if (item.result.type === 'response') {
        batch[i]?.resolve(item.result.data)
      } else if (item.result.type === 'error') {
        batch[i]?.reject(new RSPCError(item.result.data.code, item.result.data.message))
      } else {
        console.error('rspc: batch response type mismatch!')
      }
    }
  }

  const batch: BatchedItem[] = []
  let batchQueued = false
  const queueBatch = () => {
    if (!batchQueued) {
      batchQueued = true
      setTimeout(() => {
        pushBatch([...batch])
        batch.splice(0, batch.length)
        batchQueued = false
      })
    }
  }

  return ({ op }) => {
    const abort = new abortController()
    return {
      exec: async (resolve, reject) => {
        if (op.type === 'subscription' || op.type === 'subscriptionStop') {
          reject(
            // TODO: Move to `RSPCError` type??
            new Error(
              `Subscribing to '${op.path}' failed as the HTTP transport does not support subscriptions! Maybe try using the websocket transport?`
            )
          )
          return
        }

        batch.push({
          op,
          resolve,
          reject,
          abort,
        })
        queueBatch()
      },
      abort() {
        abort.abort()
      },
    }
  }
}
