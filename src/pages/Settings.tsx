import { useCallback, useEffect, useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { classList, classroomAssign, classroomList, classroomUpsert, settingsGetAll, settingsKeyInfo, settingsSet, settingsSetSharedSecret } from '@/lib/db';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { SearchableSelect } from '@/components/ui/SearchableSelect';
import { ModeBadge } from '@/components/layout/ModeBadge';
import { ThemeSwitcher } from '@/components/motion/ThemeSwitcher';
import { UI_SCALE_OPTIONS } from '@/constants/ui';
import { formatFingerprint } from '@/lib/crypto';
import { keyFingerprint } from '@/lib/crypto';
import { Input } from '@/components/ui/Input';
import type { KeyInfo } from '@/types/api';
import type { Class, Classroom } from '@/types/models';

/** 设置页：运行模式切换、UI 缩放、主题、共享密钥与班级教室绑定 */
export function Settings(): JSX.Element {
  const settings = useAppStore((s) => s.settings);
  const setUiScale = useAppStore((s) => s.setUiScale);
  const switchMode = useAppStore((s) => s.switchMode);
  const [keyInfo, setKeyInfo] = useState<KeyInfo | null>(null);
  const [secret, setSecret] = useState('');
  const [savingSecret, setSavingSecret] = useState(false);
  const [secretError, setSecretError] = useState<string | null>(null);
  const [classes, setClasses] = useState<Class[]>([]);
  const [rooms, setRooms] = useState<Classroom[]>([]);
  const [classId, setClassId] = useState('');
  const [roomId, setRoomId] = useState('');
  const [savingBinding, setSavingBinding] = useState(false);
  const [bindingError, setBindingError] = useState<string | null>(null);
  const pushToast = useAppStore((s) => s.pushToast);

  useEffect(() => {
    settingsKeyInfo()
      .then(setKeyInfo)
      .catch(() => setKeyInfo(null));
  }, []);

  const loadDirectory = useCallback(async (): Promise<void> => {
    if (settings.appMode !== 'client') return;
    await Promise.all([classList(), classroomList()]).then(([nextClasses, nextRooms]) => {
      setClasses(nextClasses);
      setRooms(nextRooms);
      setClassId(settings.classId ?? '');
    });
  }, [settings.appMode, settings.classId]);

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
      setSecret('');
      pushToast({ kind: 'success', title: '共享密钥已保存', description: '请在其他设备使用相同密钥后再同步目录' });
    } catch (err) {
      setSecretError((err as Error).message || '共享密钥格式无效');
    } finally { setSavingSecret(false); }
  };

  const saveBinding = async (): Promise<void> => {
    const selectedClass = classes.find((item) => item.id === classId);
    const selectedRoom = rooms.find((item) => item.id === roomId);
    if (!selectedClass || !selectedRoom || !selectedClass.schoolYearId) {
      setBindingError('所选班级缺少学年信息，无法绑定');
      return;
    }
    setSavingBinding(true);
    setBindingError(null);
    try {
      await classroomUpsert({ id: selectedRoom.id, roomName: selectedRoom.roomName, deviceId: settings.deviceId, remark: selectedRoom.remark });
      await classroomAssign(selectedRoom.id, selectedClass.schoolYearId, selectedClass.id);
      await Promise.all([
        settingsSet('class_id', selectedClass.id),
        settingsSet('grade', selectedClass.gradeName ?? ''),
        settingsSet('class_name', selectedClass.className),
        settingsSet('school_year_id', selectedClass.schoolYearId),
        settingsSet('bound_class_id', selectedClass.id),
      ]);
      useAppStore.getState().applySettings(await settingsGetAll());
      pushToast({ kind: 'success', title: '班级与教室已绑定' });
    } catch (err) {
      setBindingError((err as Error).message || '绑定失败');
    } finally { setSavingBinding(false); }
  };

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
        <Card title="工作身份绑定（班级 + 教室）">
          <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
            <p className="text-sm text-ink-muted">教室是教务端维护的物理位置；此处只选择本机服务的班级和教室，可随时更换。</p>
            <Button variant="secondary" size="md" onClick={() => void loadDirectory().catch(() => undefined)}>刷新目录</Button>
          </div>
          <div className="grid gap-3 sm:grid-cols-2">
            <SearchableSelect label="班级" options={classes.map((item) => ({ value: item.id, label: `${item.gradeName} · ${item.className}` }))} value={classId} onChange={setClassId} placeholder="— 请选择班级 —" />
            <SearchableSelect label="教室" options={rooms.map((item) => ({ value: item.id, label: item.roomName }))} value={roomId} onChange={setRoomId} placeholder="— 请选择教室 —" />
          </div>
          <Button className="mt-4" onClick={() => void saveBinding()} loading={savingBinding} disabled={!classId || !roomId || savingBinding}>保存绑定</Button>
          {bindingError && <p className="mt-2 text-sm text-red-600">{bindingError}</p>}
          {classes.length === 0 && <p className="mt-3 text-sm text-ink-muted">尚未同步到教务处目录，请先录入共享密钥并等待节点上线。</p>}
        </Card>
      )}
    </div>
  );
}
