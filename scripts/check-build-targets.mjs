/**
 * 构建目标静态检查。
 *
 * 教务端与班级端各自需要 dev / build / tauri:dev / tauri:build 四个脚本，
 * 且两个 Tauri 配置必须存在并使用不同的 identifier。该脚本防止某端配置在
 * 重构中被遗漏（例如只加了 dev 忘了 build）。
 */
import { readFileSync } from 'node:fs';

const REQUIRED_SCRIPTS = [
  'dev:affairs',
  'dev:classroom',
  'build:affairs',
  'build:classroom',
  'tauri:dev:affairs',
  'tauri:dev:classroom',
  'tauri:build:affairs',
  'tauri:build:classroom',
];

const REQUIRED_CONFIGS = [
  'src-tauri/tauri.affairs.conf.json',
  'src-tauri/tauri.classroom.conf.json',
  'src-tauri/tauri.affairs.dev.conf.json',
  'src-tauri/tauri.classroom.dev.conf.json',
];

let failed = false;

const packageJson = JSON.parse(readFileSync('package.json', 'utf8'));
for (const name of REQUIRED_SCRIPTS) {
  if (!packageJson.scripts[name]) {
    console.error(`✗ package.json 缺少脚本：${name}`);
    failed = true;
  }
}

const identifiers = new Map();
for (const file of REQUIRED_CONFIGS) {
  let config;
  try {
    config = JSON.parse(readFileSync(file, 'utf8'));
  } catch {
    console.error(`✗ 缺少或无效的 Tauri 配置：${file}`);
    failed = true;
    continue;
  }
  if (!config.identifier) {
    console.error(`✗ ${file} 缺少 identifier`);
    failed = true;
    continue;
  }
  if (identifiers.has(config.identifier)) {
    console.error(
      `✗ identifier 重复：${config.identifier}（${file} 与 ${identifiers.get(config.identifier)}）`,
    );
    failed = true;
  }
  identifiers.set(config.identifier, file);
}

if (failed) process.exit(1);

console.log('✓ 构建目标检查通过：八个端脚本与四份 Tauri 配置齐全且 identifier 唯一');
