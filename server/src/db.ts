// The only storage the server needs. Cloudflare D1 and a local SQLite file
// both speak SQLite's SQL, so one small interface covers every deployment.

export type Param = string | number | null

export interface Db {
  /** Run statements with no parameters and no results (schema setup). */
  exec(statements: string[]): Promise<void>
  all<T>(sql: string, params?: Param[]): Promise<T[]>
  run(sql: string, params?: Param[]): Promise<void>
}

/**
 * `site_id` is on every row from the start so one deployment can host many
 * writers later without a migration. `access` is `public` or `subscribers`.
 */
export const SCHEMA = [
  `CREATE TABLE IF NOT EXISTS posts (
    site_id TEXT NOT NULL DEFAULT 'main',
    id TEXT NOT NULL,
    slug TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    date TEXT NOT NULL,
    updated TEXT NOT NULL,
    access TEXT NOT NULL DEFAULT 'public',
    PRIMARY KEY (site_id, id)
  )`,
  `CREATE UNIQUE INDEX IF NOT EXISTS posts_slug ON posts (site_id, slug)`,
]
