import { useState } from 'react';
import { useAppStore } from '@shared/store/useAppStore';
import { settingsCompleteSetup, settingsGetAll } from '@shared/lib/db';
import { isBase64Secret, randomSecret } from '@shared/lib/secret';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';
import { Input } from '@shared/components/ui/Input';
import { Textarea } from '@shared/components/ui/Textarea';
import { ModeBadge } from '@shared/components/layout/ModeBadge';
import { APP_TARGET } from '../app-target';

/**
 * 教务端首次运行配置向导。
 *
 * 只收集学校信息与共享密钥（可一键随机生成）。运行角色由 app target 固定，
 * 向导**不提供**运行模式选择。密钥需分发给所有班级端，否则无法解密同步。
 */
export function SetupWizard(): JSX.Element {
  const applySettings = useAppStore((s) => s.applySettings);
  const setPhase = useAppStore((s) => s.setPhase);
  const pushToast = useAppStore((s) => s.pushToast);
  const toastError = useAppStore((s) => s.toastError);

  const [schoolName, setSchoolName] = useState('');
  const [deviceName, setDeviceName] = useState('');
  const [secret, setSecret] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const secretBad = secret.trim() !== '' && !isBase64Secret(secret.trim());
  const secretMissing = secret.trim() === '';
  const canSubmit = !secretMissing && !secretBad && !submitting;

  const submit = async (): Promise<void> => {
    setSubmitting(true);
    setError(null);
    try {
      await settingsCompleteSetup({
        deviceName: deviceName.trim() || '教务处端',
        grade: null,
        className: null,
        schoolName: schoolName.trim() || null,
        secret: secret.trim() || null,
      });
      // 重载运行期配置（写入 completed_setup / school_name 等），再进入主框架
      const raw = await settingsGetAll();
      applySettings(raw);
      setPhase('ready');
      pushToast({ kind: 'success', title: '首次配置已完成', description: '教务端已就绪' });
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
          <h1 className="text-2xl font-bold text-ink">教务端首次配置</h1>
          <ModeBadge appTarget={APP_TARGET} size="sm" />
        </div>
        <p className="mt-2 text-ink-soft">
          配置一次后即可长期使用。生成的共享密钥需要分发给所有班级端，两端密钥不一致将无法同步。
        </p>

        {/* 基本信息 */}
        <div className="mt-5 space-y-4">
          <Input
            label="学校名（可选）"
            value={schoolName}
            onChange={(e) => setSchoolName(e.target.value)}
            placeholder="如：阳光小学"
          />
          <Input
            label="本机名称（可选）"
            value={deviceName}
            onChange={(e) => setDeviceName(e.target.value)}
            placeholder="如：教务处主机"
          />
        </div>

        {/* 共享密钥 */}
        <div className="mt-5">
          <div className="mb-1.5 flex items-center justify-between">
            <p className="text-base font-semibold text-ink">共享密钥（必填）</p>
            <Button variant="secondary" size="md" onClick={() => setSecret(randomSecret())}>
              随机生成
            </Button>
          </div>
          <Textarea
            label=""
            rows={2}
            value={secret}
            onChange={(e) => setSecret(e.target.value)}
            placeholder="点击“随机生成”，或粘贴既有的 base64 密钥"
          />
          {secretMissing && submitting && <p className="mt-1 text-sm text-red-600">共享密钥不能为空</p>}
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
