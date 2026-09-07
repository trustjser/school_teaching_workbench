import { useState, type ReactNode } from 'react';
import { SideNav } from './SideNav';
import { TopBar } from './TopBar';
import { StatusBar } from './StatusBar';
import { useAppStore } from '@/store/useAppStore';
import { useBigScreen } from '@/hooks/useBigScreen';
import { useAutoSync } from '@/hooks/useAutoSync';

export interface AppShellProps {
  children: ReactNode;
  /** 是否显示搜索框（由页面决定） */
  showSearch?: boolean;
  search?: string;
  onSearchChange?: (value: string) => void;
}

/** 外壳布局：侧边导航 + 顶栏 + 内容区 + 状态栏 */
export function AppShell({
  children,
  showSearch = false,
  search = '',
  onSearchChange,
}: AppShellProps): JSX.Element {
  const mode = useAppStore((s) => s.settings.appMode);
  const [navOpen, setNavOpen] = useState(false);
  const { isBigScreen } = useBigScreen();

  // 同步驱动：仅在进入主界面后启用
  useAutoSync(true);

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-surface-sunken">
      <TopBar showSearch={showSearch} search={search} onSearchChange={onSearchChange} />

      <div className="flex min-h-0 flex-1">
        {/* 大屏常驻侧栏；小屏可折叠 */}
        {isBigScreen ? (
          <aside className="w-[15.5rem] shrink-0 border-r border-slate-200 bg-white">
            <SideNav mode={mode} />
          </aside>
        ) : (
          <>
            <button
              type="button"
              aria-label="展开导航"
              aria-expanded={navOpen}
              onClick={() => setNavOpen((v) => !v)}
              className="absolute left-2 top-24 z-40 rounded-lg border border-surface-border bg-white px-3 py-2 text-sm font-semibold text-ink-soft shadow-card"
            >
              {navOpen ? '收起' : '菜单'}
            </button>
            {navOpen && (
              <>
                <div
                  className="absolute inset-0 z-30 bg-slate-900/30"
                  role="presentation"
                  onClick={() => setNavOpen(false)}
                />
                <aside className="absolute left-0 top-0 z-40 h-full w-[15.5rem] border-r border-slate-200 bg-white shadow-pop">
                  <SideNav mode={mode} onNavigate={() => setNavOpen(false)} />
                </aside>
              </>
            )}
          </>
        )}

        <main className="min-w-0 flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-[1800px] px-6 py-6">{children}</div>
        </main>
      </div>

      <StatusBar />
    </div>
  );
}
