import { useEffect } from 'react';
import { useBroadcastStore } from '@/store/useBroadcastStore';
import { Card } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Badge } from '@/components/ui/Badge';
import { EmptyState } from '@/components/ui/EmptyState';
import { PRIORITY_OPTIONS } from '@/constants/status';
import { parseBroadcastPayload } from '@/types/broadcast';
import { formatDateTime } from '@/lib/format';

/** 教务指令收件箱（班级端）：查看并「接受」生成班级待办 */
export function InboxPage(): JSX.Element {
  const inbox = useBroadcastStore((s) => s.inbox);
  const loadInbox = useBroadcastStore((s) => s.loadInbox);
  const accept = useBroadcastStore((s) => s.accept);
  const markAllRead = useBroadcastStore((s) => s.markAllRead);

  useEffect(() => {
    void loadInbox();
  }, [loadInbox]);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-ink">教务指令</h1>
        <Button variant="secondary" size="md" onClick={markAllRead}>
          全部标为已读
        </Button>
      </div>

      {inbox.length === 0 ? (
        <EmptyState title="收件箱为空" description="教务处下发的任务与通知会显示在这里。" />
      ) : (
        <div className="space-y-3">
          {inbox.map((task) => {
            const tpl = parseBroadcastPayload(task.payload);
            const prio = PRIORITY_OPTIONS.find((o) => o.value === task.priority)?.label ?? task.priority;
            return (
              <Card
                key={task.id}
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
                <div className="mt-3">
                  <Button onClick={() => void accept(task.id)}>接受并生成班级待办</Button>
                </div>
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}
