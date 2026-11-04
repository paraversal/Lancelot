#!/usr/bin/env node
// Drop the glyph outlines a .fig file stores for its text, so OpenPencil shapes every text layer
// again from the installed fonts when it opens the file. This is "apply the font again" for all
// layers at once.
//
// Why: OpenPencil draws a text layer from the outlines saved with it. Outlines saved while it was
// using Google Fonts subsets can belong to the wrong glyphs ("Budget" drawn as "Bt dgeß"), and
// they stay wrong on every machine. Run scripts/openpencil-purge-font.py first, so the next save
// writes good outlines.
//
//   node scripts/openpencil-reshape-text.mjs UI.fig
//
// Close the file in OpenPencil first. The original is kept as <name>.<timestamp>.bak.fig. After
// opening the file, save it once (⌘S) to store fresh outlines.
//
// Uses the codec of the globally installed @open-pencil/mcp package (npm i -g @open-pencil/mcp).

import { execSync } from 'node:child_process'
import { copyFileSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

const file = process.argv[2]
if (!file) {
  console.error('Usage: node scripts/openpencil-reshape-text.mjs <file.fig>')
  process.exit(2)
}

const root = join(execSync('npm root -g').toString().trim(), '@open-pencil/mcp/node_modules')
const load = (path) => import(pathToFileURL(join(root, path)).href)
const { parseFigKiwiContainer } = await load('@open-pencil/kiwi/dist/fig/parse.js')
const { buildFigKiwi } = await load('@open-pencil/kiwi/dist/fig/container.js')
const { compileSchema, decodeBinarySchema, ByteBuffer } = await load(
  '@open-pencil/kiwi/dist/schema-runtime.js'
)
const { inflateSync, unzipSync, zipSync } = await load('fflate/esm/index.mjs')

const archive = unzipSync(new Uint8Array(readFileSync(file)))
const canvas = archive['canvas.fig']
if (!canvas) throw new Error(`${file} has no canvas.fig; is it an OpenPencil or Figma file?`)

const payload = parseFigKiwiContainer(canvas)
if (!payload) throw new Error('canvas.fig is not a fig-kiwi container')
const schema = compileSchema(decodeBinarySchema(new ByteBuffer(inflateSync(payload.schemaDeflated))))
const message = schema.decodeMessage(payload.dataRaw)

let texts = 0
let cleared = 0
for (const node of message.nodeChanges) {
  if (node.type !== 'TEXT') continue
  texts++
  if (node.derivedTextData?.glyphs?.length) {
    node.derivedTextData.glyphs = []
    cleared++
  }
}

const encoded = schema.encodeMessage(message)
// Check the result reads back before touching the original.
const check = schema.decodeMessage(encoded)
if (check.nodeChanges.length !== message.nodeChanges.length) {
  throw new Error('The rewritten canvas does not read back the same; nothing was changed.')
}

archive['canvas.fig'] = buildFigKiwi(payload.schemaDeflated, encoded, payload.version)
const backup = file.replace(/\.fig$/, '') + `.${new Date().toISOString().replace(/[:.]/g, '-')}.bak.fig`
copyFileSync(file, backup)
// Stored, not deflated, like OpenPencil writes it.
writeFileSync(file, zipSync(archive, { level: 0 }))

console.log(`${cleared} of ${texts} text layers will be shaped again from the installed fonts.`)
console.log(`Backup: ${backup}`)
console.log('Open the file in OpenPencil and save it once to store the new outlines.')
