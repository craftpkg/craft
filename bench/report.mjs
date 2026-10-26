import { readdirSync, readFileSync } from 'node:fs'
import { cpus, totalmem } from 'node:os'
import { join } from 'node:path'

import {
  FIXTURES,
  SCENARIOS,
  fixtureManifest,
  formatBytes,
  formatSeconds,
  packageCount,
  readScenario,
} from './results.mjs'

const out = process.argv[2]
const versions = readFileSync(join(out, 'versions.txt'), 'utf8').trim().split('\n')
const fixtures = readdirSync(out, { withFileTypes: true })
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
  .sort(byFixtureSize)

const lines = [
  '# craft benchmark',
  '',
  `${new Date().toISOString().slice(0, 10)} · ${process.platform} ${process.arch} · ${cpus()[0].model} · ${cpus().length} cores · ${Math.round(totalmem() / 2 ** 30)} GB`,
  '',
  versions.map((version) => `\`${version}\``).join(' · '),
  '',
]

for (const fixture of fixtures) {
  lines.push(...fixtureSection(fixture))
}

console.log(lines.join('\n'))

function fixtureSection(fixture) {
  const scenarios = Object.keys(SCENARIOS).filter((scenario) => readScenario(out, fixture, scenario))
  const runs = scenarios.map((scenario) => readScenario(out, fixture, scenario))
  const pms = runs[0]?.map((result) => result.name) ?? []
  const packages = packageCount(out, fixture)
  const count = packages === undefined ? '' : `, ${packages} packages`

  const section = [`## ${fixture}`, '', `${fixtureManifest(fixture).description}${count}.`, '']
  section.push(`| | ${pms.join(' | ')} |`, `|---|${pms.map(() => '---:').join('|')}|`)
  scenarios.forEach((scenario, index) => {
    section.push(`| ${SCENARIOS[scenario]} | ${timings(runs[index]).join(' | ')} |`)
  })
  section.push('')

  const hasMemory = runs.flat().some((result) => result.memory !== undefined)
  if (!hasMemory) {
    return section
  }
  section.push('Peak memory:', '', `| | ${pms.join(' | ')} |`, `|---|${pms.map(() => '---:').join('|')}|`)
  scenarios.forEach((scenario, index) => {
    section.push(`| ${SCENARIOS[scenario]} | ${memories(runs[index]).join(' | ')} |`)
  })
  section.push('')
  return section
}

function timings(results) {
  const fastest = Math.min(...results.map((result) => result.mean))
  return results.map((result) => {
    const spread = result.stddev ? ` ± ${formatSeconds(result.stddev)}` : ''
    const time = `${formatSeconds(result.mean)}${spread}`
    if (result.mean === fastest) {
      return `**${time}**`
    }
    return `${time} (${(result.mean / fastest).toFixed(1)}×)`
  })
}

function memories(results) {
  const leanest = Math.min(...results.map((result) => result.memory ?? Infinity))
  return results.map((result) => {
    if (result.memory === undefined) {
      return '–'
    }
    const memory = formatBytes(result.memory)
    return result.memory === leanest ? `**${memory}**` : memory
  })
}

function byFixtureSize(left, right) {
  return FIXTURES.indexOf(left) - FIXTURES.indexOf(right)
}
