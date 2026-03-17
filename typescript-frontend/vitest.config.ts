import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    environment: 'jsdom',
    setupFiles: './test/setup.ts',
    testTimeout: parseInt(process.env.TEST_TIMEOUT || '15000'),
    globals: true,
    exclude: [
      '**/node_modules/**',
      '**/dist/**',
      '**/.{idea,git,cache,output,temp}/**'
    ]
  },
  resolve: {
    alias: {
      '@': './src'
    }
  }
})