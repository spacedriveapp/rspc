/**
 * A function type that represents an abort operation.
 *
 * @internal
 */
export type AbortFn = () => void

/**
 * Represents a promise along with a function to cancel the operation.
 *
 * @template TValue The type of the value that the promise resolves to.
 * @internal
 */
export type PromiseAndCancel<TValue> = {
  promise: Promise<TValue>
  abort: AbortFn
}

/**
 * Represents a fake observable with an execution method that returns a promise and a cancel function.
 *
 * @internal
 */
export type FakeObservable = { exec: () => PromiseAndCancel<unknown> }
