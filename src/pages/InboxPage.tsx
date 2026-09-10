import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useBroadcastStore } from '@shared/store/useBroadcastStore';
import { Card } from '@shared/components/ui/Card';
import { Button } from '@shared/components/ui/Button';
import { Badge } from '@shared/components/ui/Badge';
import { EmptyState } from '@shared/components/ui/EmptyState';
import { SkeletonCard } from '@shared/components/ui/Skeleton';
import { PRIORITY_OPTIONS } from '@shared/constants/status';
import { parseBroadcastPayload } from '@shared/types/broadcast';
import { formatDateTime } from '@shared/lib/format';

/** 通知资料收件箱（班级端）：查看教务通知、资料并确认接收 */
export function InboxPage(): JSX.Element {
  const inbox = useBroadcastStore((s) => s.inbox);
  const loadInbox = useBroadcastStore((s) => s.loadInbox);
  const markAllRead = useBroadcastStore((s) => s.markAllRead);
  const [booting, setBooting] = useState(true);
  const navigate = useNavigate();

  useEffect(() => {
    void loadInbox();
    const t = setTimeout(() => setBooting(false), 450);
    return () => clearTimeout(t);
  }, [loadInbox]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-ink">通知资料</h1>
        <Button variant="secondary" size="md" onClick={markAllRead}>
          全部标为已读
        </Button>
      </div>

      {booting && inbox.length === 0 ? (
        <div className="space-y-3">
          <SkeletonCard />
          <SkeletonCard />
          <SkeletonCard />
        </div>
      ) : inbox.length === 0 ? (
        <EmptyState title="收件箱为空" description="教务处下发的任务与通知会显示在这里。" />
      ) : (
        <div className="space-y-3">
          {inbox.map((task, i) => {
            const tpl = parseBroadcastPayload(task.payload);
            const prio = PRIORITY_OPTIONS.find((o) => o.value === task.priority)?.label ?? task.priority;
            return (
              <div
                key={task.id}
                className="animate-rise-in"
                style={{ animationDelay: `${Math.min(i, 8) * 60}ms` }}
              >
                <Card
                  title={task.title}
                  description={tpl?.description ?? '（无描述）'}
                  actions={<Badge tone="warning">{prio}</Badge>}
                >
                  <p className="text-sm text-ink-muted">
                    下发时间：{task.sentAt ? formatDateTime(task.sentAt) : '—'}
                  </p>
                  {tpl && (
                    <div className="mt-2 flex flex-wrap gap-2">
                      {tpl.statusNodes.map((n) => (
                        <Badge key={n.nodeKey} tone="neutral">
                          {n.label}
                        </Badge>
                      ))}
                    </div>
                  )}
                  <div className="mt-3 flex items-center justify-between gap-3"><p className="text-sm text-emerald-700">已自动生成班级待办</p><Button size="md" onClick={() => navigate(`/client/matrix?broadcast=${encodeURIComponent(task.id)}`)}>进入任务看板</Button></div>
                </Card>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
