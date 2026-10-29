<script setup lang="ts">
import { useData } from 'vitepress'
import { onMounted, ref, useId, watch } from 'vue'

const props = defineProps<{ graph: string }>()

const { isDark } = useData()
const svg = ref('')
const renderId = `mermaid-${useId()}`

onMounted(render)
watch(isDark, render)

async function render() {
  const { default: mermaid } = await import('mermaid')
  await document.fonts.ready
  mermaid.initialize({
    startOnLoad: false,
    theme: 'base',
    fontFamily: FONT_FAMILY,
    themeVariables: isDark.value ? DARK : LIGHT,
    flowchart: { curve: 'basis', padding: 12, wrappingWidth: 420 },
    sequence: {
      mirrorActors: false,
      width: 120,
      actorMargin: 40,
      actorFontSize: 14,
      messageFontSize: 14,
      noteFontSize: 13,
    },
  })
  const rendered = await mermaid.render(renderId, decodeURIComponent(props.graph))
  svg.value = rendered.svg
}

const FONT_FAMILY = 'Inter, ui-sans-serif, system-ui, sans-serif'

const LIGHT = {
  fontFamily: FONT_FAMILY,
  fontSize: '14px',
  background: '#f6f6f7',
  primaryColor: '#fff3ea',
  primaryBorderColor: '#f07020',
  primaryTextColor: '#3c3c43',
  secondaryColor: '#ffffff',
  secondaryBorderColor: '#c2c2c4',
  tertiaryColor: '#ffffff',
  lineColor: '#8e8e93',
  textColor: '#3c3c43',
  clusterBkg: '#ffffff',
  clusterBorder: '#e2e2e3',
  edgeLabelBackground: '#f6f6f7',
  actorBkg: '#fff3ea',
  actorBorder: '#f07020',
  actorTextColor: '#3c3c43',
  signalColor: '#3c3c43',
  signalTextColor: '#3c3c43',
  labelBoxBkgColor: '#ffffff',
  noteBkgColor: '#ffffff',
  noteBorderColor: '#e2e2e3',
  noteTextColor: '#3c3c43',
  activationBkgColor: '#ffe2cc',
  activationBorderColor: '#f07020',
}

const DARK = {
  ...LIGHT,
  background: '#202127',
  primaryColor: '#3a2414',
  primaryTextColor: '#dfdfd6',
  secondaryColor: '#1b1b1f',
  secondaryBorderColor: '#3c3f44',
  tertiaryColor: '#1b1b1f',
  lineColor: '#98989f',
  textColor: '#dfdfd6',
  clusterBkg: '#1b1b1f',
  clusterBorder: '#3c3f44',
  edgeLabelBackground: '#202127',
  actorBkg: '#3a2414',
  actorTextColor: '#dfdfd6',
  signalColor: '#dfdfd6',
  signalTextColor: '#dfdfd6',
  labelBoxBkgColor: '#1b1b1f',
  noteBkgColor: '#1b1b1f',
  noteBorderColor: '#3c3f44',
  noteTextColor: '#dfdfd6',
  activationBkgColor: '#5a3317',
}
</script>

<template>
  <div class="mermaid-diagram" v-html="svg" />
</template>

<style scoped>
.mermaid-diagram {
  display: flex;
  justify-content: center;
  margin: 24px 0;
  padding: 20px 16px;
  min-height: 80px;
  overflow-x: auto;
  border: 1px solid var(--vp-c-divider);
  border-radius: 12px;
  background: var(--vp-c-bg-soft);
}

.mermaid-diagram :deep(svg) {
  max-width: 100%;
  height: auto;
}

.mermaid-diagram :deep(foreignObject p) {
  margin: 0;
  line-height: 1.5;
}
</style>
