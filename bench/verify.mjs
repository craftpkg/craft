import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

const project = process.argv[2]
const manifest = JSON.parse(readFileSync(join(project, 'package.json'), 'utf8'))
const direct = Object.keys({ ...manifest.dependencies, ...manifest.devDependencies })
const missing = direct.filter((name) => !existsSync(join(project, 'node_modules', name, 'package.json')))

if (missing.length > 0) {
  console.error(`error: ${project} is missing ${missing.join(', ')}`)
  process.exit(1)
}
