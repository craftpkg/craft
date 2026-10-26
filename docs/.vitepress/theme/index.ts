import DefaultTheme from 'vitepress/theme'
import type { Theme } from 'vitepress'

import BenchmarkChart from './BenchmarkChart.vue'

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    app.component('BenchmarkChart', BenchmarkChart)
  },
} satisfies Theme
