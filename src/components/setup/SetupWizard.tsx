import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAppStore } from '@/store/useAppStore';
import { classroomAssignments, classroomClaim, classroomList, classList, directorySync, settingsCompleteSetup, settingsGetAll, settingsSetSharedSecret, schoolYearList } from '@/lib/db';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { SearchableSelect } from '@/components/ui/SearchableSelect';
import { Textarea } from '@/components/ui/Textarea';
import type { AppMode } from '@/types/enums';
import type { Class, Classroom, ClassroomAssignment, SchoolYear } from '@/types/models';
import { keyFingerprint } from '@/lib/crypto';

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
 * 收集运行模式、学校信息、班级端教室认领与共享密钥，提交到
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
  const [secret, setSecret] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [loadingDirectory, setLoadingDirectory] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 班级端：从教务端目录选择绑定班级。向导打开时不读取本地空目录，
  // 避免与“连接并加载目录”的异步请求竞态而把刚拉到的数据覆盖为空。
  const [dirClasses, setDirClasses] = useState<Class[]>([]);
  const [dirRooms, setDirRooms] = useState<Classroom[]>([]);
  const [dirAssignments, setDirAssignments] = useState<ClassroomAssignment[]>([]);
  const [dirYears, setDirYears] = useState<SchoolYear[]>([]);
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);

  const activeYear = dirYears[0] ?? null;
  const selectedAssignment = dirAssignments.find((item) => item.classroomId === selectedRoomId) ?? null;
  const selectedYear = dirYears.find((year) => year.id === selectedAssignment?.schoolYearId) ?? activeYear;
  const boundClass = dirClasses.find((c) => c.id === selectedAssignment?.classId) ?? null;
  const secretBad = secret.trim() !== '' && !isBase64Secret(secret.trim());
  const secretMissing = secret.trim() === '';
  const secretRequired = true;
  const canSubmit =
    !secretMissing &&
    !secretBad &&
    (mode === 'master' || Boolean(selectedRoomId && boundClass?.schoolYearId)) &&
    !submitting;

  const loadRemoteDirectory = async (): Promise<void> => {
    const value = secret.trim();
    if (!value || !isBase64Secret(value)) {
      setError('请先输入教务端提供的 32 字节 Base64 共享密钥');
      return;
    }
    setLoadingDirectory(true);
    setError(null);
    try {
      const fingerprint = await keyFingerprint(value);
      await settingsSetSharedSecret(value, fingerprint.slice(0, 8));
      const report = await directorySync();
      const [nextClasses, nextRooms, nextAssignments, nextYears] = await Promise.all([classList(), classroomList(), classroomAssignments(), schoolYearList()]);
      setDirClasses(nextClasses);
      setDirRooms(nextRooms);
      setDirAssignments(nextAssignments);
      setDirYears(nextYears);
      setSelectedRoomId((current) => nextRooms.some((item) => item.id === current) ? current : null);
      if (report.classes === 0) {
        setError('已连接教务端，但教务端还没有可用班级');
      } else {
        pushToast({ kind: 'success', title: '已连接教务端', description: `已加载 ${report.classes} 个班级、${report.classrooms} 间教室` });
      }
    } catch (err) {
      setError((err as Error)?.message ?? '连接教务端失败，请检查两端密钥和在线状态');
    } finally {
      setLoadingDirectory(false);
    }
  };

  const submit = async (): Promise<void> => {
    setSubmitting(true);
    setError(null);
    try {
      if (mode === 'client' && selectedRoomId && boundClass?.schoolYearId) {
        await classroomClaim(selectedRoomId, boundClass.schoolYearId);
      }
      await settingsCompleteSetup({
        mode,
        deviceName: mode === 'master' ? '教务处端' : '班级端',
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
        {mode === 'master' && (
          <div className="mt-5">
            <Input
              label="学校名（可选）"
              value={schoolName}
              onChange={(e) => setSchoolName(e.target.value)}
              placeholder="如：阳光小学"
            />
          </div>
        )}

        {/* 班级端：认领教务端已维护的教室 */}
        {mode === 'client' && (
          <div className="mt-4 space-y-3 rounded-lg bg-surface-muted p-4">
            <div className="flex items-center justify-between">
              <p className="text-base font-semibold text-ink">认领教室（由教务处维护）</p>
              {dirRooms.length === 0 && <span className="text-sm text-ink-muted">请先在下方连接教务端</span>}
            </div>

            {dirRooms.length > 0 && (
              <SearchableSelect
                label="选择本机所在教室"
                options={dirRooms
                  .map((room) => {
                    const assignment = dirAssignments.find((item) => item.classroomId === room.id);
                    const klass = dirClasses.find((item) => item.id === assignment?.classId);
                    return assignment && klass ? { value: room.id, label: `${room.roomName} · ${klass.gradeName ?? ''}${klass.className}` } : null;
                  })
                  .filter((item): item is { value: string; label: string } => item !== null)}
                value={selectedRoomId ?? ''}
                onChange={(value) => setSelectedRoomId(value || null)}
                placeholder="— 请选择教室 —"
              />
            )}

            {selectedRoomId && boundClass && (
              <p className="text-sm text-ink-muted">将绑定：{selectedYear?.schoolYearName ?? ''} · {boundClass.gradeName} · {boundClass.className}</p>
            )}
            {dirRooms.length === 0 && (
              <p className="text-sm text-ink-muted">
                输入共享密钥，再点击“连接并加载目录”。只有教务端已绑定当前学年班级的教室才会出现在这里。
              </p>
            )}
          </div>
        )}

        {/* 共享密钥 */}
        <div className="mt-5">
          <div className="mb-1.5 flex items-center justify-between">
            <p className="text-base font-semibold text-ink">
              共享密钥（必填）
            </p>
            {mode === 'master' && (
              <Button
                variant="secondary"
                size="md"
                onClick={() => setSecret(randomSecret())}
              >
                随机生成
              </Button>
            )}
          </div>
          <Textarea
            label=""
            rows={2}
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            placeholder="请输入或随机生成一串 base64 密钥；所有设备需使用同一密钥"
          />
          {secretRequired && secretMissing && submitting && <p className="mt-1 text-sm text-red-600">共享密钥不能为空</p>}
          {secretBad && (
            <p className="mt-1 text-sm text-red-600">密钥需为 base64 编码的 32 字节字符串</p>
          )}
          {mode === 'client' && (
            <Button
              className="mt-3"
              variant="secondary"
              onClick={() => void loadRemoteDirectory()}
              loading={loadingDirectory}
              disabled={secretMissing || secretBad || loadingDirectory}
            >
              连接教务端并加载目录
            </Button>
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
