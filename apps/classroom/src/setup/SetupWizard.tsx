import { useState } from 'react';
import { useAppStore } from '@shared/store/useAppStore';
import {
  classList,
  classroomAssignments,
  classroomClaim,
  classroomList,
  directorySync,
  schoolYearList,
  settingsCompleteSetup,
  settingsGetAll,
  settingsSetSharedSecret,
} from '@shared/lib/db';
import { isBase64Secret } from '@shared/lib/secret';
import { keyFingerprint } from '@shared/lib/crypto';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';
import { Textarea } from '@shared/components/ui/Textarea';
import { SearchableSelect } from '@shared/components/ui/SearchableSelect';
import { ModeBadge } from '@shared/components/layout/ModeBadge';
import { APP_TARGET } from '../app-target';
import type { Class, Classroom, ClassroomAssignment, SchoolYear } from '@shared/types/models';

/**
 * 班级端首次运行配置向导。
 *
 * 录入教务端共享密钥 → 连接并加载目录 → 认领本机所在教室 → 完成初始化。
 * 运行角色由 app target 固定，向导**不提供**运行模式选择。
 */
export function SetupWizard(): JSX.Element {
  const applySettings = useAppStore((s) => s.applySettings);
  const setPhase = useAppStore((s) => s.setPhase);
  const pushToast = useAppStore((s) => s.pushToast);
  const toastError = useAppStore((s) => s.toastError);

  const [secret, setSecret] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [loadingDirectory, setLoadingDirectory] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 向导打开时不读取本地空目录，避免与“连接并加载目录”的异步请求竞态
  // 而把刚拉到的数据覆盖为空。
  const [dirClasses, setDirClasses] = useState<Class[]>([]);
  const [dirRooms, setDirRooms] = useState<Classroom[]>([]);
  const [dirAssignments, setDirAssignments] = useState<ClassroomAssignment[]>([]);
  const [dirYears, setDirYears] = useState<SchoolYear[]>([]);
  const [selectedRoomId, setSelectedRoomId] = useState<string | null>(null);

  const selectedAssignment = dirAssignments.find((item) => item.classroomId === selectedRoomId) ?? null;
  const selectedYear = dirYears.find((year) => year.id === selectedAssignment?.schoolYearId) ?? dirYears[0] ?? null;
  const boundClass = dirClasses.find((c) => c.id === selectedAssignment?.classId) ?? null;
  const secretBad = secret.trim() !== '' && !isBase64Secret(secret.trim());
  const secretMissing = secret.trim() === '';
  const canSubmit =
    !secretMissing &&
    !secretBad &&
    Boolean(selectedRoomId && boundClass?.schoolYearId) &&
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
      const [nextClasses, nextRooms, nextAssignments, nextYears] = await Promise.all([
        classList(),
        classroomList(),
        classroomAssignments(),
        schoolYearList(),
      ]);
      setDirClasses(nextClasses);
      setDirRooms(nextRooms);
      setDirAssignments(nextAssignments);
      setDirYears(nextYears);
      setSelectedRoomId((current) => (nextRooms.some((item) => item.id === current) ? current : null));
      if (report.classes === 0) {
        setError('已连接教务端，但教务端还没有可用班级');
      } else {
        pushToast({
          kind: 'success',
          title: '已连接教务端',
          description: `已加载 ${report.classes} 个班级、${report.classrooms} 间教室`,
        });
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
      if (selectedRoomId && boundClass?.schoolYearId) {
        await classroomClaim(selectedRoomId, boundClass.schoolYearId);
      }
      await settingsCompleteSetup({
        deviceName: boundClass ? `${boundClass.gradeName ?? ''}${boundClass.className}` : '班级端',
        grade: boundClass ? boundClass.gradeName : null,
        className: boundClass ? boundClass.className : null,
        classId: boundClass ? boundClass.id : null,
        schoolYearId: boundClass ? boundClass.schoolYearId : null,
        boundClassId: boundClass ? boundClass.id : null,
        schoolName: null,
        secret: secret.trim() || null,
      });
      const raw = await settingsGetAll();
      applySettings(raw);
      setPhase('ready');
      pushToast({ kind: 'success', title: '首次配置已完成', description: '班级端已就绪' });
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
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold text-ink">班级端首次配置</h1>
          <ModeBadge appTarget={APP_TARGET} size="sm" />
        </div>
        <p className="mt-2 text-ink-soft">
          配置一次后即可长期使用。共享密钥必须与教务端一致，否则无法互相解密同步。
        </p>

        {/* 认领教务端已维护的教室 */}
        <div className="mt-5 space-y-3 rounded-lg bg-surface-muted p-4">
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
                  return assignment && klass
                    ? { value: room.id, label: `${room.roomName} · ${klass.gradeName ?? ''}${klass.className}` }
                    : null;
                })
                .filter((item): item is { value: string; label: string } => item !== null)}
              value={selectedRoomId ?? ''}
              onChange={(value) => setSelectedRoomId(value || null)}
              placeholder="— 请选择教室 —"
            />
          )}

          {selectedRoomId && boundClass && (
            <p className="text-sm text-ink-muted">
              将绑定：{selectedYear?.schoolYearName ?? ''} · {boundClass.gradeName} · {boundClass.className}
            </p>
          )}
          {dirRooms.length === 0 && (
            <p className="text-sm text-ink-muted">
              输入共享密钥，再点击“连接并加载目录”。只有教务端已绑定当前学年班级的教室才会出现在这里。
            </p>
          )}
        </div>

        {/* 共享密钥 */}
        <div className="mt-5">
          <p className="mb-1.5 text-base font-semibold text-ink">共享密钥（必填）</p>
          <Textarea
            label=""
            rows={2}
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            placeholder="请粘贴教务端提供的 base64 密钥"
          />
          {secretMissing && submitting && <p className="mt-1 text-sm text-red-600">共享密钥不能为空</p>}
          {secretBad && (
            <p className="mt-1 text-sm text-red-600">密钥需为 base64 编码的 32 字节字符串</p>
          )}
          <Button
            className="mt-3"
            variant="secondary"
            onClick={() => void loadRemoteDirectory()}
            loading={loadingDirectory}
            disabled={secretMissing || secretBad || loadingDirectory}
          >
            连接教务端并加载目录
          </Button>
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
