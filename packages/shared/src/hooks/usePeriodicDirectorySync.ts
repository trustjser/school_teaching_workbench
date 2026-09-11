import { useEffect } from 'react';
import { DIRECTORY_SYNC_INTERVAL_MS } from '@shared/constants/app';
import { directorySync } from '@shared/lib/db';
import { useAppStore } from '@shared/store/useAppStore';

/**
 * 班级端周期目录同步：应用就绪后挂载时立即静默同步一次，之后每 60s 一次。
 *
 * 换届后教室端不打开设置页也能自动切绑（final review F2）。所有错误静默吞掉，
 * 不发 toast、不弹横幅——横幅与提示由 SettingsPanel 既有路径负责；自动切换
 * 成功后 Rust 会 emit CLASS_CHANGED，既有监听负责刷新数据。
 */
export function usePeriodicDirectorySync(enabled: boolean): void {
  const phase = useAppStore((s) => s.phase);
  const configured = useAppStore((s) => s.settings.firstRunDone);

  useEffect(() => {
    if (!enabled || phase !== 'ready' || !configured) return;
    const run = (): void => {
      void directorySync().catch(() => undefined);
    };
    run();
    const timer = window.setInterval(run, DIRECTORY_SYNC_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [enabled, phase, configured]);
}
