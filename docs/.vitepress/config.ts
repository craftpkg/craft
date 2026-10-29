import { defineConfig } from 'vitepress'

const commands = [
  'install',
  'add',
  'remove',
  'update',
  'outdated',
  'list',
  'why',
  'run',
  'exec',
  'dlx',
  'trust',
  'init',
  'ci',
  'cache',
]

export default defineConfig({
  title: 'craft',
  description: 'A fast, strict package manager for Node.js, written in Rust.',
  base: '/craft/',
  cleanUrls: true,
  head: [['link', { rel: 'icon', type: 'image/png', href: '/craft/logo.png' }]],

  markdown: {
    config(md) {
      const fence = md.renderer.rules.fence!
      md.renderer.rules.fence = (tokens, index, options, env, renderer) => {
        const token = tokens[index]
        if (token.info.trim() === 'mermaid') {
          return `<MermaidDiagram graph="${encodeURIComponent(token.content)}" />`
        }
        return fence(tokens, index, options, env, renderer)
      }
    },
  },

  themeConfig: {
    logo: '/logo.png',

    nav: [
      { text: 'Guide', link: '/guide/introduction', activeMatch: '/guide/' },
      { text: 'CLI', link: '/reference/cli', activeMatch: '/reference/' },
      { text: 'Benchmarks', link: '/guide/benchmarks' },
      { text: 'Developers', link: '/developers/anatomy', activeMatch: '/developers/' },
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Getting started',
          items: [
            { text: 'Introduction', link: '/guide/introduction' },
            { text: 'Installation', link: '/guide/installation' },
            { text: 'Quick start', link: '/guide/getting-started' },
            { text: 'Benchmarks', link: '/guide/benchmarks' },
          ],
        },
        {
          text: 'Concepts',
          items: [
            { text: 'How it works', link: '/guide/how-it-works' },
            { text: 'Lockfile', link: '/guide/lockfile' },
            { text: 'Dependency sources', link: '/guide/dependency-sources' },
            { text: 'Install scripts', link: '/guide/install-scripts' },
            { text: 'Platform packages', link: '/guide/platforms' },
          ],
        },
        {
          text: 'Using craft',
          items: [
            { text: 'Configuration', link: '/guide/configuration' },
            { text: 'Continuous integration', link: '/guide/ci' },
            { text: 'Windows', link: '/guide/windows' },
            { text: 'Migrating from npm or pnpm', link: '/guide/migrating' },
            { text: 'Troubleshooting', link: '/guide/troubleshooting' },
          ],
        },
      ],
      '/reference/': [
        {
          text: 'CLI',
          items: [
            { text: 'Overview and global flags', link: '/reference/cli' },
            ...commands.map((command) => ({
              text: `craft ${command}`,
              link: `/reference/${command}`,
            })),
          ],
        },
      ],
      '/developers/': [
        {
          text: 'How craft works',
          items: [
            { text: 'Anatomy', link: '/developers/anatomy' },
            { text: 'Architecture', link: '/developers/architecture' },
          ],
        },
        {
          text: 'Contributing',
          items: [
            { text: 'Getting set up', link: '/developers/' },
            { text: 'Testing', link: '/developers/testing' },
          ],
        },
      ],
    },

    socialLinks: [{ icon: 'github', link: 'https://github.com/craftpkg/craft' }],
    search: { provider: 'local' },
    editLink: {
      pattern: 'https://github.com/craftpkg/craft/edit/main/docs/:path',
      text: 'Edit this page on GitHub',
    },
    outline: [2, 3],
    footer: {
      message: 'Released under the MIT License.',
    },
  },
})
