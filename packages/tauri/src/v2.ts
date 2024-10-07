import type { Link, RspcRequest, RspcResponse } from '@spacedrive/rspc-client'

import { RSPCError } from '@spacedrive/rspc-client'
import { listen } from '@tauri-apps/api/event'
import { Window } from '@tauri-apps/api/window'

/**
 * Link for the rspc Tauri plugin
 */
export function tauriLink(): Link {
  const activeMap = new Map<
    string | number,
    {
      resolve: (result: unknown) => void
      reject: (error: Error | RSPCError) => void
    }
  >()
  const listener = listen<RspcResponse>('plugin:rspc:transport:resp', event => {
    const { id, result } = event.payload
    if (activeMap.has(id)) {
      if (result.type === 'event') {
        activeMap.get(id)?.resolve(result.data)
      } else if (result.type === 'response') {
        activeMap.get(id)?.resolve(result.data)
        activeMap.delete(id)
      } else if (result.type === 'error') {
        const { message, code } = result.data
        activeMap.get(id)?.reject(new RSPCError(code, message))
        activeMap.delete(id)
      } else {
        console.error(`rspc: received event of unknown type '${result.type}'`)
      }
    } else {
      console.error(`rspc: received event for unknown id '${id}'`)
    }
  })

  const batch: RspcRequest[] = []
  let batchQueued = false
  const queueBatch = () => {
    if (batchQueued) return
    batchQueued = true

    setTimeout(() => {
      const currentBatch = [...batch]
      // Reset the batch
      batch.length = 0
      batchQueued = false
      listener
        .then(() => Window.getCurrent().emit('plugin:rspc:transport', currentBatch))
        .catch(err => {
          console.error('Failed to emit to plugin:rspc:transport', err)
        })
    })
  }

  return ({ op }) => {
    let finished = false
    return {
      exec: async (resolve, reject) => {
        activeMap.set(op.id, {
          resolve,
          reject,
        })

        if (op.type === 'subscriptionStop') {
          if (op.input != null && typeof op.input !== 'string' && typeof op.input !== 'number') {
            throw new Error(
              `Expected 'input' to be of type 'string' or 'number' for 'subscriptionStop', but got ${typeof op.input}`
            )
          }
          batch.push({
            id: op.id,
            method: op.type,
            params: {
              input: op.input ?? null,
            },
          })
        } else {
          batch.push({
            id: op.id,
            method: op.type,
            params: {
              path: op.path,
              input: op.input,
            },
          })
        }
        queueBatch()
      },
      abort() {
        if (finished) return
        finished = true

        const subscribeEventIdx = batch.findIndex(b => b.id === op.id)
        if (subscribeEventIdx === -1) {
          if (op.type === 'subscription') {
            batch.push({
              id: op.id,
              method: 'subscriptionStop',
              params: null,
            })
            queueBatch()
          }
        } else {
          batch.splice(subscribeEventIdx, 1)
        }

        activeMap.delete(op.id)
      },
    }
  }
}
