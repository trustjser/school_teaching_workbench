import { createViteConfig } from '../../vite.shared';

// 班级端：dev 1430 / HMR 1431，产物 dist/classroom
export default createViteConfig({
  appTarget: 'classroom',
  root: './apps/classroom',
  port: 1430,
  hmrPort: 1431,
  outDir: './dist/classroom',
});
