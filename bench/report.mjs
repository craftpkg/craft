import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { cpus, totalmem } from 'node:os'
import { join } from 'node:path'

const SCENARIOS = {
  cold: 'Cold: no cache, no lockfile',
  lockfile: 'Lockfile, empty cache',
  cache: 'Warm cache, no lockfile',
  'cache+lockfile': 'Warm cache and lockfile',
  'up-to-date': 'Already installed',
}

const out = process.argv[2]
const fixturesDir = new URL('./fixtures/', import.meta.url)
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
  const scenarios = Object.keys(SCENARIOS).filter((scenario) => existsSync(join(out, fixture, `${scenario}.json`)))
  const runs = scenarios.map((scenario) => JSON.parse(readFileSync(join(out, fixture, `${scenario}.json`), 'utf8')).results.map(wallClock))
  const pms = runs[0]?.map((result) => result.name) ?? []
  const manifest = JSON.parse(readFileSync(new URL(`${fixture}/package.json`, fixturesDir), 'utf8'))

  const section = [`## ${fixture}`, '', `${manifest.description}${packageCount(fixture)}.`, '']
  section.push(`| | ${pms.join(' | ')} |`, `|---|${pms.map(() => '---:').join('|')}|`)
  scenarios.forEach((scenario, index) => {
    section.push(`| ${SCENARIOS[scenario]} | ${timings(runs[index]).join(' | ')} |`)
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

// hyperfine 2 nests the statistics under summary; hyperfine 1 keeps them at the top.
function wallClock(result) {
  const { mean, stddev } = result.summary?.time_wall_clock ?? result
  return { name: result.name ?? result.command, mean, stddev }
}

function formatSeconds(seconds) {
  if (seconds < 0.01) {
    return `${(seconds * 1000).toFixed(1)} ms`
  }
  if (seconds < 1) {
    return `${Math.round(seconds * 1000)} ms`
  }
  return `${seconds.toFixed(2)} s`
}

function packageCount(fixture) {
  const lockfile = join(out, fixture, 'craft.lock')
  if (!existsSync(lockfile)) {
    return ''
  }
  const { packages } = JSON.parse(readFileSync(lockfile, 'utf8'))
  return `, ${Object.keys(packages).length} packages`
}

function byFixtureSize(left, right) {
  const order = ['small', 'medium', 'large']
  return order.indexOf(left) - order.indexOf(right)
}
