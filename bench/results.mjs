import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

export const SCENARIOS = {
  cold: 'Cold: no cache, no lockfile',
  lockfile: 'Lockfile, empty cache',
  cache: 'Warm cache, no lockfile',
  stale: 'Warm cache past its max-age, no lockfile',
  'cache+lockfile': 'Warm cache and lockfile',
  'up-to-date': 'Already installed',
}

export const FIXTURES = ['small', 'medium', 'large']

/** Each package manager's result for one scenario of a run, or undefined if it wasn't timed. */
export function readScenario(run, fixture, scenario) {
  const path = join(run, fixture, `${scenario}.json`)
  if (!existsSync(path)) {
    return undefined
  }
  return JSON.parse(readFileSync(path, 'utf8')).results.map(wallClock)
}

/** How many packages the fixture installs, from the lockfile craft wrote in that run. */
export function packageCount(run, fixture) {
  const lockfile = join(run, fixture, 'craft.lock')
  if (!existsSync(lockfile)) {
    return undefined
  }
  return Object.keys(JSON.parse(readFileSync(lockfile, 'utf8')).packages).length
}

export function fixtureManifest(fixture) {
  return JSON.parse(readFileSync(new URL(`./fixtures/${fixture}/package.json`, import.meta.url), 'utf8'))
}

export function formatSeconds(seconds) {
  if (seconds < 0.01) {
    return `${(seconds * 1000).toFixed(1)} ms`
  }
  if (seconds < 1) {
    return `${Math.round(seconds * 1000)} ms`
  }
  return `${seconds.toFixed(2)} s`
}

/** How many times slower than the fastest, precise enough that a near tie doesn't read as one. */
export function formatRatio(ratio) {
  const digits = ratio < 1.1 ? 2 : ratio < 10 ? 1 : 0
  return `${ratio.toFixed(digits)}×`
}

export function formatBytes(bytes) {
  const megabytes = bytes / 2 ** 20
  if (megabytes < 1024) {
    return `${Math.round(megabytes)} MB`
  }
  return `${(megabytes / 1024).toFixed(1)} GB`
}

// hyperfine 2 nests the statistics under summary; hyperfine 1 keeps them at the top, with
// memory per run.
function wallClock(result) {
  const { mean, stddev } = result.summary?.time_wall_clock ?? result
  const memory = result.summary?.memory_peak_resident?.mean ?? average(result.memory_usage_byte)
  return { name: result.name ?? result.command, mean, stddev, memory }
}

function average(values) {
  if (!values?.length) {
    return undefined
  }
  return values.reduce((sum, value) => sum + value, 0) / values.length
}
