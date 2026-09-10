import { createViteConfig } from '../../vite.shared';

// 教务端：dev 1420 / HMR 1421，产物 dist/affairs
export default createViteConfig({
  appTarget: 'affairs',
  root: './apps/affairs',
  port: 1420,
  hmrPort: 1421,
  outDir: './dist/affairs',
});
