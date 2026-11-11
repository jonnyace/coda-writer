import type { Db, Param } from './db'

// The part of Cloudflare's D1 binding this server uses. Declared here so the
// Node build does not need Cloudflare's type package.
interface D1Statement {
  bind(...params: Param[]): D1Statement
  all<T>(): Promise<{ results: T[] }>
  run(): Promise<unknown>
}

export interface D1Like {
  prepare(sql: string): D1Statement
  batch(statements: D1Statement[]): Promise<unknown>
}

export function d1(database: D1Like): Db {
  return {
    async exec(statements) {
      await database.batch(statements.map((sql) => database.prepare(sql)))
    },
    async all<T>(sql: string, params: Param[] = []) {
      const { results } = await database.prepare(sql).bind(...params).all<T>()
      return results
    },
    async run(sql, params = []) {
      await database.prepare(sql).bind(...params).run()
    },
  }
}
