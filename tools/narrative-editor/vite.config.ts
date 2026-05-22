import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

// Ticket 448/449 — the sprite editor needs to read the workspace's
// `assets/sprites/bindings.toml` and load PNGs by manifest path. We
// expose the repo's `assets/` directory at `/assets/*` via the
// `publicDir`-equivalent server.fs.allow + a tiny middleware that
// serves files from that directory. Dev-only; not part of the build.
const REPO_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../..'
)
const ASSETS_DIR = path.join(REPO_ROOT, 'assets')

export default defineConfig({
  plugins: [
    svelte(),
    tailwindcss(),
    {
      name: 'clowder-assets-passthrough',
      configureServer(server) {
        // Read-only static serve for any file under assets/.
        server.middlewares.use('/assets', async (req, res, next) => {
          if (!req.url) return next()
          if (req.method && req.method !== 'GET' && req.method !== 'HEAD') {
            return next()
          }
          const rel = decodeURIComponent(req.url.split('?')[0])
          const abs = path.join(ASSETS_DIR, rel)
          if (!abs.startsWith(ASSETS_DIR)) {
            res.statusCode = 403
            return res.end('forbidden')
          }
          try {
            const fs = await import('node:fs/promises')
            const data = await fs.readFile(abs)
            const ext = path.extname(abs).toLowerCase()
            const type =
              ext === '.png' ? 'image/png'
              : ext === '.toml' ? 'text/plain; charset=utf-8'
              : ext === '.json' ? 'application/json'
              : 'application/octet-stream'
            res.setHeader('Content-Type', type)
            res.setHeader('Cache-Control', 'no-store')
            res.end(data)
          } catch {
            next()
          }
        })

        // Ticket 449 Phase 4 — write-back. Accepts a TOML body and atomically
        // replaces assets/sprites/bindings.toml. Bevy's hot-reload watcher
        // (sprite_bindings::watch_sprite_bindings) picks up the change
        // within ~0.5s. Dev-only; no auth, no rate limit, no production build.
        const BINDINGS_PATH = path.join(ASSETS_DIR, 'sprites', 'bindings.toml')
        server.middlewares.use('/api/sprite-bindings', async (req, res, next) => {
          if (req.method !== 'POST') return next()
          try {
            const chunks: Buffer[] = []
            for await (const chunk of req) {
              chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk))
            }
            const body = Buffer.concat(chunks).toString('utf-8')
            // Sanity: must look like TOML and contain the four required tables.
            const required = ['[items.', '[buildings.', '[herbs.', '[flavor_plants.']
            for (const marker of required) {
              if (!body.includes(marker)) {
                res.statusCode = 400
                res.setHeader('Content-Type', 'application/json')
                return res.end(JSON.stringify({
                  error: `payload missing required table marker: ${marker}`,
                }))
              }
            }
            const fs = await import('node:fs/promises')
            // Atomic-ish write: tmp file + rename so the Bevy watcher never
            // observes a half-written file.
            const tmp = BINDINGS_PATH + '.tmp'
            await fs.writeFile(tmp, body, 'utf-8')
            await fs.rename(tmp, BINDINGS_PATH)
            res.statusCode = 200
            res.setHeader('Content-Type', 'application/json')
            res.end(JSON.stringify({ ok: true, bytes: body.length }))
          } catch (e) {
            res.statusCode = 500
            res.setHeader('Content-Type', 'application/json')
            res.end(JSON.stringify({ error: String(e) }))
          }
        })

        // GET /api/sprite-assets/png — enumerate every PNG under assets/
        // recursively, returning asset-relative paths. Powers the
        // building-texture-path picker (the editor needs to know what
        // textures exist so the user can swap from a list rather than
        // free-form typing). Cached per dev-server run.
        let pngCache: string[] | null = null
        server.middlewares.use('/api/sprite-assets/png', async (req, res, next) => {
          if (req.method !== 'GET') return next()
          if (!pngCache) {
            const fs = await import('node:fs/promises')
            const collected: string[] = []
            const walk = async (dir: string) => {
              const entries = await fs.readdir(dir, { withFileTypes: true })
              for (const e of entries) {
                const full = path.join(dir, e.name)
                if (e.isDirectory()) await walk(full)
                else if (e.isFile() && e.name.toLowerCase().endsWith('.png')) {
                  collected.push(path.relative(ASSETS_DIR, full))
                }
              }
            }
            try {
              await walk(ASSETS_DIR)
              collected.sort()
              pngCache = collected
            } catch (e) {
              res.statusCode = 500
              return res.end(JSON.stringify({ error: String(e) }))
            }
          }
          res.statusCode = 200
          res.setHeader('Content-Type', 'application/json')
          res.setHeader('Cache-Control', 'no-store')
          res.end(JSON.stringify({ paths: pngCache }))
        })
      },
    },
  ],
  server: {
    fs: {
      // Allow Vite to read from the parent assets/ dir for any direct
      // file imports (the middleware above is the primary serving path).
      allow: [REPO_ROOT],
    },
  },
  base: process.env.BASE_PATH ?? './',
})
