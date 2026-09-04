import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// Tauri serves this build; the dev server port is fixed so the shell can find it.
export default defineConfig({
  plugins: [react()],
  server: { port: 1420, strictPort: true },
  build: { outDir: 'dist' },
  test: {
    environment: 'jsdom',
    include: ['../tests/frontend/**/*.test.{ts,tsx}', 'src/**/*.test.{ts,tsx}'],
    // Story test tasks add the first suites; the scaffold must not fail yet.
    passWithNoTests: true,
  },
});
