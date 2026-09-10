import { useCallback, useEffect, useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { classroomList, classroomRelease, directorySync, settingsGetAll, settingsKeyInfo, settingsSet, settingsSetSharedSecret, settingsResetClient } from '@/lib/db';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { ThemeSwitcher } from '@/components/motion/ThemeSwitcher';
import { UI_SCALE_OPTIONS } from '@/constants/ui';
import { formatFingerprint } from '@/lib/crypto';
import { keyFingerprint } from '@/lib/crypto';
import { Input } from '@/components/ui/Input';
import type { KeyInfo } from '@/types/api';
import type { Classroom } from '@/types/models';

/** 设置页：UI 缩放、主题、共享密钥与班级端重置 */
export function Settings(): JSX.Element {
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

  useEffect(() => {
    settingsKeyInfo()
      .then(setKeyInfo)
      .catch(() => setKeyInfo(null));
  }, []);

  const loadDirectory = useCallback(async (showResult = false): Promise<void> => {
    if (settings.appMode !== 'client') return;
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
  }, [settings.appMode, pushToast]);

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
      pushToast({ kind: 'success', title: '共享密钥已保存', description: '已尝试连接教务端并刷新目录' });
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
          <Input label="录入教务处提供的密钥" type="password" value={secret} onChange={(e) => setSecret(e.target.value)} placeholder="base64，32 字节" error={secretError ?? undefined} />
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
          <p className="text-amber-700">尚未配置共享密钥。录入教务处提供的密钥后，班级端才能发现并同步教务处目录。</p>
        )}
      </Card>

      {settings.appMode === 'client' && (
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
      <ConfirmDialog open={resetOpen} title="重置班级端配置" message="确定要重置本机班级端配置吗？" detail="当前教室认领会先释放，完成后需要重新输入共享密钥并选择教室。" confirmText="确认重置" danger loading={resetting} onConfirm={() => void resetClient()} onCancel={() => setResetOpen(false)} />
    </div>
  );
}
