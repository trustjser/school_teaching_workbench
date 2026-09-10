import { useCallback, useEffect, useState } from 'react';
import { useAppStore } from '@shared/store/useAppStore';
import {
  classroomList,
  classroomRelease,
  directorySync,
  settingsGetAll,
  settingsKeyInfo,
  settingsResetClient,
  settingsSetSharedSecret,
} from '@shared/lib/db';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';
import { Select } from '@shared/components/ui/Select';
import { ConfirmDialog } from '@shared/components/ui/ConfirmDialog';
import { ThemeSwitcher } from '@shared/components/motion/ThemeSwitcher';
import { UI_SCALE_OPTIONS } from '@shared/constants/ui';
import { formatFingerprint, keyFingerprint } from '@shared/lib/crypto';
import { Input } from '@shared/components/ui/Input';
import type { AppTarget } from '@shared/app-target';
import type { KeyInfo } from '@shared/types/api';
import type { Classroom } from '@shared/types/models';

export interface SettingsPanelProps {
  /** 固定 app target：决定展示哪些端专属操作，面板本身不提供切换能力 */
  appTarget: AppTarget;
}

/**
 * 共享设置面板：外观、共享密钥、端口与端专属维护操作。
 *
 * 面板只消费固定 target，**不渲染运行模式选择器**——教务端与班级端是两个
 * 独立安装包，角色由构建期 identifier 决定，运行期不可切换。
 */
export function SettingsPanel({ appTarget }: SettingsPanelProps): JSX.Element {
  const settings = useAppStore((s) => s.settings);
  const setUiScale = useAppStore((s) => s.setUiScale);
  const [keyInfo, setKeyInfo] = useState<KeyInfo | null>(null);
  const [secret, setSecret] = useState('');
  const [savingSecret, setSavingSecret] = useState(false);
  const [secretError, setSecretError] = useState<string | null>(null);
  const [rooms, setRooms] = useState<Classroom[]>([]);
  const [bindingError, setBindingError] = useState<string | null>(null);
  const [resetOpen, setResetOpen] = useState(false);
  const [resetting, setResetting] = useState(false);
  const pushToast = useAppStore((s) => s.pushToast);

  const isClassroom = appTarget === 'classroom';

  useEffect(() => {
    settingsKeyInfo()
      .then(setKeyInfo)
      .catch(() => setKeyInfo(null));
  }, []);

  const loadDirectory = useCallback(async (showResult = false): Promise<void> => {
    if (!isClassroom) return;
    setBindingError(null);
    try {
      const report = await directorySync();
      if (showResult) {
        pushToast({ kind: 'success', title: '目录已刷新', description: `已同步 ${report.classes} 个班级、${report.classrooms} 间教室` });
      }
    } catch (err) {
      if (showResult) setBindingError((err as Error).message || '未能连接教务端');
    }
    await classroomList().then((nextRooms) => {
      setRooms(nextRooms);
    });
  }, [isClassroom, pushToast]);

  useEffect(() => {
    void loadDirectory().catch(() => undefined);
  }, [loadDirectory]);

  const saveSecret = async (): Promise<void> => {
    const value = secret.trim();
    try {
      const bytes = Uint8Array.from(atob(value), (char) => char.charCodeAt(0));
      if (bytes.length !== 32) throw new Error('共享密钥必须是 base64 编码的 32 字节');
      setSavingSecret(true); setSecretError(null);
      const fingerprint = await keyFingerprint(value);
      await settingsSetSharedSecret(value, fingerprint.slice(0, 8));
      setKeyInfo(await settingsKeyInfo());
      await loadDirectory(true);
      setSecret('');
      pushToast({ kind: 'success', title: '共享密钥已保存', description: isClassroom ? '已尝试连接教务端并刷新目录' : '班级端需使用同一密钥才能同步' });
    } catch (err) {
      setSecretError((err as Error).message || '共享密钥格式无效');
    } finally { setSavingSecret(false); }
  };

  const resetClient = async (): Promise<void> => {
    setResetting(true); setBindingError(null);
    try {
      const boundRoom = rooms.find((room) => room.deviceId === settings.deviceId);
      if (boundRoom) await classroomRelease(boundRoom.id);
      await settingsResetClient();
      useAppStore.getState().applySettings(await settingsGetAll());
      useAppStore.getState().setPhase('need-setup');
      setResetOpen(false);
      pushToast({ kind: 'success', title: '已重置班级端配置', description: '请重新录入共享密钥并认领教室' });
    } catch (err) {
      setBindingError((err as Error).message || '重置失败');
    } finally { setResetting(false); }
  };

  return (
    <div className="max-w-2xl space-y-4">
      <h1 className="text-3xl font-bold text-ink">设置</h1>

      <Card title="外观">
        <Select
          label="UI 缩放"
          options={UI_SCALE_OPTIONS.map((o) => ({ value: String(o.value), label: o.label }))}
          value={String(settings.uiScale)}
          onChange={(e) => void setUiScale(Number(e.target.value))}
        />
        <div className="mt-5 flex flex-wrap items-center justify-between gap-4">
          <div>
            <p className="text-base font-semibold text-ink">主题</p>
            <p className="mt-0.5 text-sm text-ink-muted">浅色 / 深色 / 高对比，即时切换并自动记忆。</p>
          </div>
          <ThemeSwitcher />
        </div>
      </Card>

      <Card title="共享密钥">
        <div className="mb-4 space-y-2 rounded-lg bg-surface-muted p-3">
          <Input
            label={isClassroom ? '录入教务处提供的密钥' : '录入共享密钥'}
            type="password"
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            placeholder="base64，32 字节"
            error={secretError ?? undefined}
          />
          <Button onClick={() => void saveSecret()} loading={savingSecret} disabled={!secret.trim()}>保存并连接</Button>
        </div>
        {keyInfo?.configured ? (
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
          <p className="text-amber-700">
            {isClassroom
              ? '尚未配置共享密钥。录入教务处提供的密钥后，班级端才能发现并同步教务处目录。'
              : '尚未配置共享密钥。请录入或轮换出一枚密钥，并提供给所有班级端。'}
          </p>
        )}
      </Card>

      <Card title="运行信息">
        <p className="text-ink-soft">
          本机设备 ID：<span className="font-mono text-ink">{settings.deviceId || '未初始化'}</span>
        </p>
        <p className="mt-1 text-ink-soft">
          API 端口：<span className="font-mono text-ink">{settings.apiPort}</span>
        </p>
        <p className="mt-1 text-sm text-ink-muted">
          端口由后端从 5178 起自动探测；同机同时运行两端时会自动错开，并通过 mDNS 发布实际端口。
        </p>
      </Card>

      {isClassroom && (
        <Card title="班级端配置">
          <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
            <p className="text-sm text-ink-muted">班级和教室由教务端维护，班级端完成初始化后不能直接换绑。</p>
            <Button variant="secondary" size="md" onClick={() => void loadDirectory(true).catch(() => undefined)}>刷新目录</Button>
          </div>
          <p className="text-ink">当前班级：{settings.grade && settings.className ? `${settings.grade} · ${settings.className}` : '未配置'}</p>
          <p className="mt-1 text-ink">当前教室：{rooms.find((room) => room.deviceId === settings.deviceId)?.roomName ?? '未认领'}</p>
          <Button className="mt-4" variant="danger" onClick={() => setResetOpen(true)}>重置班级端配置</Button>
          {bindingError && <p className="mt-2 text-sm text-red-600">{bindingError}</p>}
        </Card>
      )}
      {isClassroom && (
        <ConfirmDialog open={resetOpen} title="重置班级端配置" message="确定要重置本机班级端配置吗？" detail="当前教室认领会先释放，完成后需要重新输入共享密钥并选择教室。" confirmText="确认重置" danger loading={resetting} onConfirm={() => void resetClient()} onCancel={() => setResetOpen(false)} />
      )}
    </div>
  );
}
