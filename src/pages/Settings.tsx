import { useEffect, useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { settingsKeyInfo } from '@/lib/db';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { ModeBadge } from '@/components/layout/ModeBadge';
import { UI_SCALE_OPTIONS } from '@/constants/ui';
import { formatFingerprint } from '@/lib/crypto';
import type { ThemeName } from '@/types/enums';
import type { KeyInfo } from '@/types/api';

/** 设置页：运行模式切换、UI 缩放、主题、共享密钥指纹 */
export function Settings(): JSX.Element {
  const settings = useAppStore((s) => s.settings);
  const setUiScale = useAppStore((s) => s.setUiScale);
  const setTheme = useAppStore((s) => s.setTheme);
  const switchMode = useAppStore((s) => s.switchMode);
  const [keyInfo, setKeyInfo] = useState<KeyInfo | null>(null);

  useEffect(() => {
    settingsKeyInfo()
      .then(setKeyInfo)
      .catch(() => setKeyInfo(null));
  }, []);

  return (
    <div className="max-w-2xl space-y-4">
      <h1 className="text-3xl font-bold text-ink">设置</h1>

      <Card title="运行模式">
        <div className="flex flex-wrap items-center gap-3">
          <ModeBadge mode={settings.appMode} />
          <Button
            variant="secondary"
            onClick={() => void switchMode(settings.appMode === 'master' ? 'client' : 'master')}
          >
            切换到{settings.appMode === 'master' ? '班级端' : '教务处端'}
          </Button>
        </div>
        <p className="mt-3 text-sm text-ink-muted">
          切换模式会刷新本地运行配置并重新发现局域网节点。
        </p>
      </Card>

      <Card title="外观">
        <Select
          label="UI 缩放"
          options={UI_SCALE_OPTIONS.map((o) => ({ value: String(o.value), label: o.label }))}
          value={String(settings.uiScale)}
          onChange={(e) => void setUiScale(Number(e.target.value))}
        />
        <Select
          className="mt-4"
          label="主题"
          options={[
            { value: 'light', label: '浅色' },
            { value: 'dark', label: '深色' },
            { value: 'high-contrast', label: '高对比' },
          ]}
          value={settings.theme}
          onChange={(e) => void setTheme(e.target.value as ThemeName)}
        />
      </Card>

      <Card title="共享密钥">
        {keyInfo ? (
          <div className="space-y-2">
            <p className="text-ink-soft">
              密钥 ID：<span className="font-mono text-ink">{keyInfo.kid}</span>
            </p>
            <p className="text-ink-soft">
              指纹：<span className="font-mono text-ink">{formatFingerprint(keyInfo.fingerprint)}</span>
            </p>
            <p className="text-sm text-ink-muted">
              请核对各端指纹是否一致，不一致将无法互相解密同步数据。
            </p>
          </div>
        ) : (
          <p className="text-ink-muted">无法读取密钥信息（需在 Tauri 环境中运行）。</p>
        )}
      </Card>
    </div>
  );
}
