// Publishes benchmark results to the website and the README: the data behind the docs'
// benchmark charts, and the chart image the README shows. Takes one or more result
// directories; a scenario measured in a later one replaces the same scenario from an earlier.
//
//   node bench/publish.mjs bench/results/<run> [bench/results/<newer run> ...]
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { cpus, totalmem } from 'node:os'
import { join } from 'node:path'

import {
  FIXTURES,
  SCENARIOS,
  fixtureManifest,
  formatRatio,
  formatSeconds,
  packageCount,
  readScenario,
} from './results.mjs'

const runs = process.argv.slice(2)
if (runs.length === 0) {
  console.error('usage: node bench/publish.mjs <results dir>...')
  process.exit(2)
}

const root = new URL('../', import.meta.url).pathname
const data = {
  date: new Date().toISOString().slice(0, 10),
  machine: `${process.platform === 'darwin' ? 'macOS' : process.platform}, ${cpus()[0].model}, ${cpus().length} cores, ${Math.round(totalmem() / 2 ** 30)} GB`,
  versions: readVersions(runs.at(-1)),
  scenarios: Object.entries(SCENARIOS).map(([id, label]) => ({ id, label })),
  fixtures: FIXTURES.map(fixtureData).filter((fixture) => fixture.scenarios.length > 0),
}

const dataPath = join(root, 'docs/.vitepress/theme/benchmarks.json')
mkdirSync(join(root, 'docs/.vitepress/theme'), { recursive: true })
writeFileSync(dataPath, `${JSON.stringify(data, null, 2)}\n`)
console.log(`wrote ${dataPath}`)

const large = data.fixtures.find((fixture) => fixture.name === 'large')
if (large) {
  for (const [theme, colors] of Object.entries(themes())) {
    const path = join(root, `docs/public/benchmarks-${theme}.svg`)
    writeFileSync(path, chart(large, colors))
    console.log(`wrote ${path}`)
  }
}

function fixtureData(name) {
  const scenarios = []
  let packages
  for (const id of Object.keys(SCENARIOS)) {
    for (const run of runs) {
      const results = readScenario(run, name, id)
      if (!results) {
        continue
      }
      packages = packageCount(run, name) ?? packages
      const measured = results.map(({ name: pm, mean, stddev, memory }) => ({ pm, mean, stddev, memory }))
      const existing = scenarios.findIndex((scenario) => scenario.id === id)
      const scenario = { id, results: measured }
      if (existing === -1) {
        scenarios.push(scenario)
      } else {
        scenarios[existing] = scenario
      }
    }
  }
  return { name, description: fixtureManifest(name).description, packages, scenarios }
}

function readVersions(run) {
  return Object.fromEntries(
    readFileSync(join(run, 'versions.txt'), 'utf8')
      .trim()
      .split('\n')
      .map((line) => line.split(' ')),
  )
}

// The README chart: the large fixture's everyday scenarios, one panel each, bars scaled per panel.
function chart(fixture, colors) {
  const panels = ['cache+lockfile', 'cache', 'cold', 'up-to-date']
    .map((id) => fixture.scenarios.find((scenario) => scenario.id === id))
    .filter(Boolean)
  const width = 760
  const labelWidth = 64
  const valueWidth = 150
  const barHeight = 18
  const rowGap = 8
  const panelGap = 30
  const titleHeight = 26
  const headerHeight = 54
  const barArea = width - labelWidth - valueWidth - 24

  let y = headerHeight
  const parts = [
    `<text x="0" y="22" class="heading">Installing a ${fixture.packages.toLocaleString('en-US')}-package Next.js app</text>`,
    `<text x="0" y="42" class="note">Mean wall time, lower is better · ${escape(data.machine)} · ${data.date}</text>`,
  ]
  for (const panel of panels) {
    const label = SCENARIOS[panel.id]
    parts.push(`<text x="0" y="${y + 16}" class="title">${escape(label)}</text>`)
    y += titleHeight
    const fastest = Math.min(...panel.results.map((result) => result.mean))
    const slowest = Math.max(...panel.results.map((result) => result.mean))
    for (const result of panel.results) {
      const barWidth = Math.max(2, (result.mean / slowest) * barArea)
      const isCraft = result.pm === 'craft'
      const isFastest = result.mean === fastest
      const ratio = isFastest ? 'fastest' : formatRatio(result.mean / fastest)
      const fill = isCraft ? colors.craft : colors.other
      parts.push(
        `<text x="0" y="${y + 13}" class="pm${isCraft ? ' craft' : ''}">${result.pm}</text>`,
        `<rect x="${labelWidth}" y="${y}" width="${barWidth.toFixed(1)}" height="${barHeight}" rx="4" fill="${fill}"/>`,
        `<text x="${labelWidth + barWidth + 8}" y="${y + 13}" class="value${isCraft ? ' craft' : ''}">${formatSeconds(result.mean)} <tspan class="ratio">${ratio}</tspan></text>`,
      )
      y += barHeight + rowGap
    }
    y += panelGap - rowGap
  }

  const height = y - panelGap + rowGap + 8
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" role="img" aria-label="craft, npm, pnpm and Yarn install times">
<style>
  text { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif; fill: ${colors.text}; }
  .heading { font-size: 18px; font-weight: 600; }
  .note { font-size: 12px; fill: ${colors.muted}; }
  .title { font-size: 14px; font-weight: 600; }
  .pm { font-size: 13px; }
  .value { font-size: 13px; font-variant-numeric: tabular-nums; }
  .craft { font-weight: 700; fill: ${colors.craftText}; }
  .ratio { fill: ${colors.muted}; font-weight: 400; }
</style>
${parts.join('\n')}
</svg>
`
}

function themes() {
  return {
    light: { text: '#1f2328', muted: '#656d76', craft: '#f07020', craftText: '#c2410c', other: '#d0d7de' },
    dark: { text: '#e6edf3', muted: '#8d96a0', craft: '#fb923c', craftText: '#fdba74', other: '#3d444d' },
  }
}

function escape(text) {
  return text.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')
}
