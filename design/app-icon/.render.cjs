const sharp = require('/Users/liuyan/.workbuddy/binaries/node/workspace/node_modules/sharp');
const path = require('path');
const dir = '/Users/liuyan/Documents/github/school_teaching_workbench/design/app-icon';

async function render(svgFile, outPng, size) {
  await sharp(path.join(dir, svgFile)).resize(size, size).png().toFile(path.join(dir, outPng));
  console.log(`rendered ${outPng} @${size}`);
}

(async () => {
  await render('icon-a2-timetable-stack-clean.svg', 'preview-a2.png', 512);
  await render('icon-b-timetable-sync.svg', 'preview-b.png', 512);
  // 小尺寸压力测试：32
  await render('icon-a2-timetable-stack-clean.svg', 'preview-a2-32.png', 32);
  await render('icon-b-timetable-sync.svg', 'preview-b-32.png', 32);
})();
