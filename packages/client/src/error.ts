/**
 * Custom error class for RSPC errors.
 *
 * @class RSPCError
 * @implements {Error}
 */
export class RSPCError implements Error {
  readonly name: string = 'RSPCError'
  readonly code: number
  readonly message: string
  readonly stack?: string

  /**
   * Creates an instance of RSPCError.
   *
   * @param {number} code - The error code.
   * @param {string} message - The error message.
   */
  constructor(code: number, message: string) {
    this.code = code
    this.message = message
    this.stack = new Error().stack
  }

  /**
   * Returns a string representation of the error.
   *
   * @returns {string} The string representation of the error.
   */
  toString(): string {
    return `${this.name} (code: ${this.code}): ${this.message}`
  }
}
