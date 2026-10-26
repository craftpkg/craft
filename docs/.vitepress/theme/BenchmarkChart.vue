<script setup lang="ts">
import { computed, ref } from 'vue'
import benchmarks from './benchmarks.json'

type Metric = 'time' | 'memory'

const props = withDefaults(
  defineProps<{
    fixture?: string
    scenarios?: string[]
    controls?: boolean
  }>(),
  { fixture: 'large', scenarios: undefined, controls: false },
)

const selectedFixture = ref(props.fixture)
const metric = ref<Metric>('time')

const fixture = computed(
  () => benchmarks.fixtures.find((candidate) => candidate.name === selectedFixture.value) ?? benchmarks.fixtures[0],
)

const panels = computed(() => {
  const wanted = props.scenarios ?? benchmarks.scenarios.map((scenario) => scenario.id)
  return wanted
    .map((id) => fixture.value.scenarios.find((scenario) => scenario.id === id))
    .filter((scenario) => scenario !== undefined)
    .map((scenario) => {
      const values = scenario.results.map((result) => valueOf(result))
      const best = Math.min(...values)
      const worst = Math.max(...values)
      return {
        id: scenario.id,
        label: benchmarks.scenarios.find((known) => known.id === scenario.id)?.label ?? scenario.id,
        rows: scenario.results.map((result) => {
          const value = valueOf(result)
          return {
            pm: result.pm,
            value: metric.value === 'time' ? formatSeconds(value) : formatBytes(value),
            width: `${Math.max(1, (value / worst) * 100)}%`,
            ratio: value === best ? 'best' : formatRatio(value / best),
            isBest: value === best,
          }
        }),
      }
    })
})

function valueOf(result: { mean: number; memory?: number | null }) {
  return metric.value === 'time' ? result.mean : (result.memory ?? 0)
}

function formatSeconds(seconds: number) {
  if (seconds < 0.01) {
    return `${(seconds * 1000).toFixed(1)} ms`
  }
  if (seconds < 1) {
    return `${Math.round(seconds * 1000)} ms`
  }
  return `${seconds.toFixed(2)} s`
}

function formatRatio(ratio: number) {
  const digits = ratio < 1.1 ? 2 : ratio < 10 ? 1 : 0
  return `${ratio.toFixed(digits)}×`
}

function formatBytes(bytes: number) {
  const megabytes = bytes / 2 ** 20
  return megabytes < 1024 ? `${Math.round(megabytes)} MB` : `${(megabytes / 1024).toFixed(1)} GB`
}
</script>

<template>
  <div class="benchmarks">
    <div v-if="controls" class="controls">
      <div class="segmented" role="group" aria-label="Project">
        <button
          v-for="candidate in benchmarks.fixtures"
          :key="candidate.name"
          :class="{ active: candidate.name === fixture.name }"
          @click="selectedFixture = candidate.name"
        >
          {{ candidate.name }}
        </button>
      </div>
      <div class="segmented" role="group" aria-label="Measure">
        <button :class="{ active: metric === 'time' }" @click="metric = 'time'">Time</button>
        <button :class="{ active: metric === 'memory' }" @click="metric = 'memory'">Peak memory</button>
      </div>
    </div>
    <p class="caption">
      {{ fixture.description }}, {{ fixture.packages.toLocaleString('en-US') }} packages ·
      {{ metric === 'time' ? 'mean wall time' : 'mean peak memory' }}, lower is better
    </p>

    <div class="grid">
      <section v-for="panel in panels" :key="panel.id" class="panel">
        <h4>{{ panel.label }}</h4>
        <div v-for="row in panel.rows" :key="row.pm" class="row" :class="{ craft: row.pm === 'craft' }">
          <span class="pm">{{ row.pm }}</span>
          <span class="track">
            <span class="bar" :style="{ width: row.width }" />
          </span>
          <span class="value">
            {{ row.value }}
            <span class="ratio" :class="{ best: row.isBest }">{{ row.ratio }}</span>
          </span>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.benchmarks {
  --craft: #f07020;
  margin: 24px 0;
}

.controls {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-bottom: 12px;
}

.segmented {
  display: inline-flex;
  border: 1px solid var(--vp-c-divider);
  border-radius: 8px;
  overflow: hidden;
}

.segmented button {
  padding: 4px 14px;
  font-size: 14px;
  color: var(--vp-c-text-2);
  background: var(--vp-c-bg-soft);
  transition: color 0.2s, background-color 0.2s;
}

.segmented button + button {
  border-left: 1px solid var(--vp-c-divider);
}

.segmented button.active {
  color: var(--vp-c-white);
  background: var(--craft);
}

.caption {
  margin: 0 0 16px;
  font-size: 14px;
  color: var(--vp-c-text-2);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(max(300px, calc(50% - 8px)), 1fr));
  gap: 16px;
}

.panel {
  padding: 16px 18px;
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  background: var(--vp-c-bg-soft);
}

.panel h4 {
  margin: 0 0 12px;
  font-size: 15px;
  font-weight: 600;
}

.row {
  display: grid;
  grid-template-columns: 44px 1fr 118px;
  align-items: center;
  gap: 10px;
  margin: 6px 0;
  font-size: 13px;
}

.pm {
  color: var(--vp-c-text-2);
}

.track {
  height: 14px;
  border-radius: 4px;
  background: var(--vp-c-default-soft);
  overflow: hidden;
}

.bar {
  display: block;
  height: 100%;
  border-radius: 4px;
  background: var(--vp-c-text-3);
  transition: width 0.4s ease;
}

.value {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.ratio {
  margin-left: 4px;
  color: var(--vp-c-text-3);
}

.ratio.best {
  color: var(--vp-c-green-1);
}

.row.craft .pm,
.row.craft .value {
  font-weight: 700;
  color: var(--vp-c-text-1);
}

.row.craft .bar {
  background: var(--craft);
}
</style>
