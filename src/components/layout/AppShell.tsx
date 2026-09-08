import { useState, type ReactNode } from 'react';
import { useLocation } from 'react-router-dom';
import { Menu } from 'lucide-react';
import { SideNav } from './SideNav';
import { TopBar } from './TopBar';
import { StatusBar } from './StatusBar';
import { useAppStore } from '@/store/useAppStore';
import { useBigScreen } from '@/hooks/useBigScreen';
import { useAutoSync } from '@/hooks/useAutoSync';
import { PageTransition } from '@/components/motion/PageTransition';

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
  const location = useLocation();

  // 同步驱动：仅在进入主界面后启用
  useAutoSync(true);

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden bg-surface-sunken surface-grid">
      <TopBar showSearch={showSearch} search={search} onSearchChange={onSearchChange} />

      {/* 中栏作为定位容器：抽屉 absolute 锚定在顶栏与状态栏之间，避免魔法数字错位 */}
      <div className="relative flex min-h-0 flex-1">
        {/* 大屏常驻侧栏（玻璃拟态 + 悬浮描边）；小屏可折叠 */}
        {isBigScreen ? (
          <aside className="glass w-[15.5rem] shrink-0 border-r border-surface-border">
            <SideNav mode={mode} />
          </aside>
        ) : (
          <>
            <button
              type="button"
              aria-label="展开导航"
              aria-expanded={navOpen}
              onClick={() => setNavOpen((v) => !v)}
              className="group absolute bottom-2 left-4 z-40 flex h-12 w-12 items-center justify-center rounded-full bg-surface-raised/90 text-ink-soft shadow-soft backdrop-blur-sm transition-[transform,box-shadow] hover:scale-105 hover:shadow-md active:scale-95 border border-surface-border"
            >
              <Menu className="h-5 w-5 transition-transform group-hover:rotate-3" aria-hidden />
            </button>
            {navOpen && (
              <>
                <div
                  className="absolute inset-0 z-30 bg-black/55 backdrop-blur-sm"
                  role="presentation"
                  onClick={() => setNavOpen(false)}
                />
                <aside className="glass absolute left-0 top-0 bottom-0 z-40 w-[15.5rem] border-r border-surface-border shadow-pop">
                  <SideNav mode={mode} onNavigate={() => setNavOpen(false)} />
                </aside>
              </>
            )}
          </>
        )}

        <main className="min-w-0 flex-1 overflow-y-auto">
          <PageTransition routeKey={location.pathname} className="mx-auto w-full max-w-[1800px] px-6 py-6">
            {children}
          </PageTransition>
        </main>
      </div>

      <StatusBar />
    </div>
  );
}
