import { Plus, Trash2 } from 'lucide-react';
import { IconButton, resolveIcon } from '@/components/ui/IconButton';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { Button } from '@/components/ui/Button';
import { COLOR_TOKENS, COLOR_TOKEN_HEX, COLOR_TOKEN_LABEL, NODE_ICON_OPTIONS } from '@/constants/status';
import { TASK_NODE_MIN, TASK_NODE_MAX } from '@/constants/app';
import type { ColorToken } from '@/types/enums';
import { uuidV4 } from '@/lib/crypto';

/** 可编辑状态节点（新建任务 / 编辑节点共用，班级端与教务处端一致） */
export interface EditableNode {
  id?: string;
  nodeKey: string;
  label: string;
  colorToken: ColorToken;
  iconName: string | null;
  nodeOrder: number;
  isFinal: boolean;
  isDefault: boolean;
}

/** 节点配色色板：直接在选项中展示颜色 */
function ColorSwatchPicker({
  value,
  onChange,
}: {
  value: ColorToken;
  onChange: (color: ColorToken) => void;
}): JSX.Element {
  return (
    <div className="flex flex-wrap items-center gap-2">
      {COLOR_TOKENS.map((token) => {
        const hex = COLOR_TOKEN_HEX[token] ?? '#64748b';
        const selected = value === token;
        return (
          <button
            key={token}
            type="button"
            title={COLOR_TOKEN_LABEL[token] ?? token}
            aria-label={`配色：${COLOR_TOKEN_LABEL[token] ?? token}`}
            onClick={() => onChange(token as ColorToken)}
            className={[
              'h-7 w-7 min-h-7 shrink-0 aspect-square box-border rounded-full border-2 transition-transform',
              selected ? 'scale-110 border-ink shadow-sm' : 'border-transparent hover:scale-105',
            ].join(' ')}
            style={{ backgroundColor: hex }}
          />
        );
      })}
    </div>
  );
}

/** 节点图标选择器：直接在选项中展示图标 */
function IconPicker({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (icon: string) => void;
}): JSX.Element {
  return (
    <div className="flex flex-wrap items-center gap-2">
      {NODE_ICON_OPTIONS.map((opt) => {
        const Icon = resolveIcon(opt.value);
        const selected = value === opt.value || (value == null && opt.value === 'circle');
        return (
          <button
            key={opt.value}
            type="button"
            title={opt.label}
            aria-label={`图标：${opt.label}`}
            onClick={() => onChange(opt.value)}
            className={[
              'inline-flex h-9 w-9 min-h-9 items-center justify-center rounded-lg border transition-colors',
              selected
                ? 'border-brand-600 bg-brand-50 text-brand-700'
                : 'border-surface-border bg-surface-raised text-ink-soft hover:bg-surface-muted hover:text-ink',
            ].join(' ')}
          >
            <Icon className="h-4 w-4" />
          </button>
        );
      })}
    </div>
  );
}

/** 单条状态节点编辑卡：预览 + 名称 + 颜色/图标选项 + 终态 */
function NodeEditorCard({
  node,
  canRemove,
  onChange,
  onRemove,
}: {
  node: EditableNode;
  canRemove: boolean;
  onChange: (patch: Partial<EditableNode>) => void;
  onRemove: () => void;
}): JSX.Element {
  const Icon = resolveIcon(node.iconName);
  const hex = COLOR_TOKEN_HEX[node.colorToken] ?? '#64748b';
  return (
    <div className="rounded-xl border border-surface-border bg-surface-raised p-4">
      <div className="flex items-start gap-4">
        <div
          className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full shadow-sm"
          style={{ backgroundColor: hex }}
          aria-label="节点预览"
        >
          <Icon className="h-6 w-6 text-white" />
        </div>
        <div className="min-w-0 flex-1 space-y-4">
          <div className="flex flex-wrap items-end gap-3">
            <div className="min-w-0 flex-1">
              <Input label="节点名称" value={node.label} onChange={(e) => onChange({ label: e.target.value })} />
            </div>
            <div>
              <Toggle
                label="终态"
                checked={node.isFinal}
                onChange={(v) => onChange({ isFinal: v })}
                className="py-0 px-0"
              />
            </div>
          </div>
          <div className="space-y-1">
            <span className="text-sm font-semibold text-ink-muted">配色</span>
            <ColorSwatchPicker value={node.colorToken} onChange={(c) => onChange({ colorToken: c })} />
          </div>
          <div className="space-y-1">
            <span className="text-sm font-semibold text-ink-muted">图标</span>
            <IconPicker value={node.iconName} onChange={(i) => onChange({ iconName: i })} />
          </div>
        </div>
        <IconButton
          icon={<Trash2 className="h-5 w-5" />}
          label="删除节点"
          variant="ghost"
          disabled={!canRemove}
          onClick={onRemove}
          className="text-ink-muted hover:text-red-600"
        />
      </div>
    </div>
  );
}

export interface StatusNodeEditorProps {
  nodes: EditableNode[];
  onChange: (nodes: EditableNode[]) => void;
  /** 数量下限（默认 TASK_NODE_MIN） */
  min?: number;
  /** 数量上限（默认 TASK_NODE_MAX） */
  max?: number;
  /**
   * 删除已落库节点（带 id）时的回调，用于立即从数据库移除。
   * 不传则仅做本地移除（适用于尚未保存的新建任务）。
   */
  onRemoveNode?: (node: EditableNode) => void;
}

/** 状态节点编辑器：受控列表，含添加 / 删除与数量约束（2~4） */
export function StatusNodeEditor({
  nodes,
  onChange,
  min = TASK_NODE_MIN,
  max = TASK_NODE_MAX,
  onRemoveNode,
}: StatusNodeEditorProps): JSX.Element {
  const addNode = (): void => {
    if (nodes.length >= max) return;
    onChange([
      ...nodes,
      {
        id: undefined,
        nodeKey: `node-${uuidV4().slice(0, 4)}`,
        label: '新节点',
        colorToken: (COLOR_TOKENS[nodes.length % COLOR_TOKENS.length] as ColorToken) ?? 'blue',
        iconName: 'circle',
        nodeOrder: nodes.length,
        isFinal: false,
        isDefault: false,
      },
    ]);
  };

  const removeNode = (idx: number): void => {
    if (nodes.length <= min) return;
    const target = nodes[idx];
    if (target?.id && onRemoveNode) onRemoveNode(target);
    onChange(nodes.filter((_, i) => i !== idx).map((n, i) => ({ ...n, nodeOrder: i })));
  };

  const patchNode = (idx: number, patch: Partial<EditableNode>): void => {
    onChange(nodes.map((n, i) => (i === idx ? { ...n, ...patch } : n)));
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <p className="text-base font-semibold text-ink">
          状态节点（{nodes.length} 个，{min}~{max}）
        </p>
        <Button variant="secondary" size="md" onClick={addNode} disabled={nodes.length >= max}>
          添加节点
        </Button>
      </div>
      {nodes.map((n, idx) => (
        <NodeEditorCard
          key={n.id ?? n.nodeKey}
          node={n}
          canRemove={nodes.length > min}
          onChange={(patch) => patchNode(idx, patch)}
          onRemove={() => removeNode(idx)}
        />
      ))}
    </div>
  );
}
