/**
 * 端边界静态检查。
 *
 * 教务端与班级端是两个独立 app，各自的入口与路由不得引用另一端的页面或路径。
 * 该脚本在 CI 与本地 `npm run check:boundaries` 中运行，防止拆分后被重新耦合。
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const checks = [
  [
    'apps/affairs/src/router.tsx',
    ['/client', 'ClientHome', 'CheckinPage', 'TaskManage', 'TaskMatrix', 'InboxPage'],
  ],
  [
    'apps/classroom/src/router.tsx',
    [
      '/master',
      'MasterHome',
      'DeviceMonitor',
      'AttendanceBoard',
      'BroadcastCenter',
      'GradeClassManage',
      'Analytics',
      'TaskDashboard',
      'TaskDetail',
    ],
  ],
];

/** shared 层不得反向依赖任一 app：端专属实现必须留在各自的入口内。 */
const SHARED_FORBIDDEN = ['@affairs/', '@classroom/', 'apps/affairs', 'apps/classroom'];
const SHARED_DIR = 'packages/shared/src';

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else if (/\.(ts|tsx)$/.test(full)) out.push(full);
  }
  return out;
}

let failed = false;

for (const [file, forbidden] of checks) {
  let source;
  try {
    source = readFileSync(file, 'utf8');
  } catch {
    console.error(`✗ ${file} 不存在`);
    failed = true;
    continue;
  }
  for (const value of forbidden) {
    if (source.includes(value)) {
      console.error(`✗ ${file} 包含另一端引用：${value}`);
      failed = true;
    }
  }
}

for (const file of walk(SHARED_DIR)) {
  const source = readFileSync(file, 'utf8');
  for (const value of SHARED_FORBIDDEN) {
    if (source.includes(value)) {
      console.error(`✗ ${file} 反向依赖了端专属代码：${value}`);
      failed = true;
    }
  }
}

if (failed) {
  process.exit(1);
}

console.log('✓ 端边界检查通过：两个入口未互相引用，shared 未反向依赖 app');
