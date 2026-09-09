import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppStore } from '@/store/useAppStore';
import { classList, classroomAssign, classroomList, classroomUpsert, settingsCompleteSetup, settingsGetAll, toRuntimeSettings } from '@/lib/db';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import { Textarea } from '@/components/ui/Textarea';
import type { AppMode } from '@/types/enums';
import type { Class, Classroom } from '@/types/models';

/** 共享密钥必须是 base64 编码的 32 字节 */
function isBase64Secret(s: string): boolean {
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(s)) return false;
  try {
    return atob(s).length === 32;
  } catch {
    return false;
  }
}

/** 生成 base64 32 字节随机密钥 */
function randomSecret(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  let bin = '';
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

/**
 * 首次运行配置向导。
 * 收集运行模式、学校/年级/班级、设备名与共享密钥，提交到
 * `settingsCompleteSetup`；成功后重载运行期配置并进入主框架。
 *
 * 注意：Rust 侧把「已完成」写入 `completed_setup`，而前端 `firstRunDone`
 * 由该键推导，因此完成配置后重载不会再回到本向导。
 */
export function SetupWizard(): JSX.Element {
  const navigate = useNavigate();
  const applySettings = useAppStore((s) => s.applySettings);
  const setPhase = useAppStore((s) => s.setPhase);
  const pushToast = useAppStore((s) => s.pushToast);
  const toastError = useAppStore((s) => s.toastError);

  const [mode, setMode] = useState<AppMode>('client');
  const [schoolName, setSchoolName] = useState('');
  const [deviceName, setDeviceName] = useState('');
  const [secret, setSecret] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 班级端：从教务端目录选择绑定班级（目录为空时回退手动填写）
  const [dirClasses, setDirClasses] = useState<Class[]>([]);
  const [selectedClassId, setSelectedClassId] = useState<string | null>(null);
  const [dirRooms, setDirRooms] = useState<Classroom[]>([]);
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);

  useEffect(() => {
    if (mode === 'client') {
      classList()
        .then(setDirClasses)
        .catch(() => setDirClasses([]));
      classroomList()
        .then(setDirRooms)
        .catch(() => setDirRooms([]));
    }
  }, [mode]);

  const boundClass = dirClasses.find((c) => c.id === selectedClassId) ?? null;
  const secretBad = secret.trim() !== '' && !isBase64Secret(secret.trim());
  const secretMissing = secret.trim() === '';
  const secretRequired = mode === 'master';
  const canSubmit =
    deviceName.trim().length > 0 && (!secretRequired || !secretMissing) && !secretBad && !submitting;

  const submit = async (): Promise<void> => {
    setSubmitting(true);
    setError(null);
    try {
      await settingsCompleteSetup({
        mode,
        deviceName: deviceName.trim(),
        grade: boundClass ? boundClass.gradeName : null,
        className: boundClass ? boundClass.className : null,
        classId: boundClass ? boundClass.id : null,
        schoolYearId: boundClass ? boundClass.schoolYearId : null,
        boundClassId: boundClass ? boundClass.id : null,
        schoolName: mode === 'master' ? schoolName.trim() || null : null,
        secret: secret.trim() || null,
      });
      // 重载运行期配置（写入 completed_setup / app_mode / school_name 等），再进入主框架
      const raw = await settingsGetAll();
      if (mode === 'client' && selectedRoomId) {
        const runtime = toRuntimeSettings(raw);
        const room = dirRooms.find((item) => item.id === selectedRoomId);
        if (room) {
          await classroomUpsert({ id: room.id, roomName: room.roomName, deviceId: runtime.deviceId, remark: room.remark });
          if (boundClass?.schoolYearId) await classroomAssign(room.id, boundClass.schoolYearId, boundClass.id);
        }
      }
      applySettings(raw);
      setPhase('ready');
      pushToast({
        kind: 'success',
        title: '首次配置已完成',
        description: mode === 'master' ? '已切换为教务处端' : '已切换为班级端',
      });
      navigate(mode === 'master' ? '/master' : '/client');
    } catch (err) {
      setError((err as Error)?.message ?? '配置失败');
      toastError(err, '首次配置失败');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="flex min-h-screen w-full items-start justify-center overflow-y-auto bg-surface-sunken p-6">
      <Card className="max-w-xl w-full min-w-0">
        <h1 className="text-2xl font-bold text-ink">首次运行配置</h1>
        <p className="mt-2 text-ink-soft">
          配置一次后即可长期使用。两端需使用相同的共享密钥才能互相解密同步。
        </p>

        {/* 运行模式 */}
        <div className="mt-5">
          <p className="mb-1.5 text-base font-semibold text-ink">运行模式</p>
          <div className="flex flex-col gap-3 sm:flex-row">
            <button
              type="button"
              onClick={() => setMode('client')}
              className={[
                'min-w-0 flex-1 rounded-lg border px-4 py-3 text-left transition-colors',
                mode === 'client'
                  ? 'border-brand-500 bg-brand-50 ring-2 ring-brand-400'
                  : 'border-surface-border hover:border-surface-border',
              ].join(' ')}
            >
              <div className="font-semibold text-ink">班级端</div>
              <div className="text-sm text-ink-muted">签到、任务、接收教务指令</div>
            </button>
            <button
              type="button"
              onClick={() => setMode('master')}
              className={[
                'min-w-0 flex-1 rounded-lg border px-4 py-3 text-left transition-colors',
                mode === 'master'
                  ? 'border-brand-500 bg-brand-50 ring-2 ring-brand-400'
                  : 'border-surface-border hover:border-surface-border',
              ].join(' ')}
            >
              <div className="font-semibold text-ink">教务处端</div>
              <div className="text-sm text-ink-muted">名册、考勤大屏、下发广播</div>
            </button>
          </div>
        </div>

        {/* 基本信息 */}
        <div className="mt-5 grid min-w-0 grid-cols-1 gap-4 sm:grid-cols-2">
          <Input
            label="设备名"
            value={deviceName}
            onChange={(e) => setDeviceName(e.target.value)}
            placeholder="如：三年二班-前台机"
            error={deviceName.trim() === '' && submitting ? '必填' : undefined}
          />
          {mode === 'master' && (
            <Input
              label="学校名（可选）"
              value={schoolName}
              onChange={(e) => setSchoolName(e.target.value)}
              placeholder="如：阳光小学"
            />
          )}
        </div>

        {/* 班级端：绑定年级 / 班级 */}
        {mode === 'client' && (
          <div className="mt-4 space-y-3 rounded-lg bg-surface-muted p-4">
            <div className="flex items-center justify-between">
              <p className="text-base font-semibold text-ink">班级与教室（由教务处维护）</p>
              {dirClasses.length === 0 && <span className="text-sm text-ink-muted">等待教务处目录同步</span>}
            </div>

            {dirClasses.length > 0 ? (
              <Select
                label="选择本机所属班级"
                options={dirClasses.map((c) => ({
                  value: c.id,
                  label: c.gradeName ? `${c.gradeName} / ${c.className}` : c.className,
                }))}
                value={selectedClassId ?? ''}
                onChange={(e) => setSelectedClassId(e.target.value || null)}
                placeholder="— 请选择班级 —"
              />
            ) : null}

            {dirClasses.length > 0 && (
              <Select
                label="选择本机所在教室"
                options={dirRooms.map((room) => ({ value: room.id, label: room.roomName }))}
                value={selectedRoomId ?? ''}
                onChange={(e) => setSelectedRoomId(e.target.value || null)}
                placeholder="— 请选择教室 —"
              />
            )}

            {dirClasses.length === 0 && (
              <p className="text-sm text-ink-muted">
                教务端尚未同步年级 / 班级 / 教室目录。可以先完成设备配置，目录同步后再绑定班级和教室；班级端不需要手动维护这些信息。
              </p>
            )}
          </div>
        )}

        {/* 共享密钥 */}
        <div className="mt-5">
          <div className="mb-1.5 flex items-center justify-between">
            <p className="text-base font-semibold text-ink">
              共享密钥{mode === 'master' ? '（必填）' : '（可稍后在设置中录入）'}
            </p>
            <Button
              variant="secondary"
              size="md"
              onClick={() => setSecret(randomSecret())}
            >
              随机生成
            </Button>
          </div>
          <Textarea
            label=""
            rows={2}
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            placeholder="请输入或随机生成一串 base64 密钥；所有设备需使用同一密钥"
          />
          {mode === 'client' && secretMissing && (
            <p className="mt-1 text-sm text-ink-muted">班级端可先完成设备初始化；进入应用后打开“设置”录入教务处提供的密钥，再刷新目录完成绑定。</p>
          )}
          {secretRequired && secretMissing && submitting && <p className="mt-1 text-sm text-red-600">共享密钥不能为空</p>}
          {secretBad && (
            <p className="mt-1 text-sm text-red-600">密钥需为 base64 编码的 32 字节字符串</p>
          )}
        </div>

        {error && <p className="mt-4 text-sm text-red-600">{error}</p>}

        <div className="mt-6 flex justify-end">
          <Button onClick={() => void submit()} disabled={!canSubmit}>
            {submitting ? '配置中…' : '完成配置并进入'}
          </Button>
        </div>
      </Card>
    </div>
  );
}
