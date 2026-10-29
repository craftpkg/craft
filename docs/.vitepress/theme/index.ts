import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'

import BenchmarkChart from './BenchmarkChart.vue'
import MermaidDiagram from './MermaidDiagram.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('BenchmarkChart', BenchmarkChart)
    app.component('MermaidDiagram', MermaidDiagram)
  },
} satisfies Theme
