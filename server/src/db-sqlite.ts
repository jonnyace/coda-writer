import { DatabaseSync } from 'node:sqlite'
import type { Db, Param } from './db'

/** A SQLite file on disk, or `:memory:` for tests. */
export function sqlite(path: string): Db {
  const database = new DatabaseSync(path)
  return {
    async exec(statements) {
      for (const sql of statements) database.exec(sql)
    },
    async all<T>(sql: string, params: Param[] = []) {
      return database.prepare(sql).all(...params) as T[]
    },
    async run(sql, params = []) {
      database.prepare(sql).run(...params)
    },
  }
}
