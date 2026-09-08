import type { ReactNode } from 'react';

export interface TableColumn<T> {
  key: string;
  header: ReactNode;
  /** 自定义渲染 */
  render?: (row: T, index: number) => ReactNode;
  /** 单元格取值（未提供 render 时使用） */
  accessor?: (row: T) => string | number | null | undefined;
  /** 宽度类，如 'w-32' */
  widthClass?: string;
  align?: 'left' | 'center' | 'right';
  /** 是否粘性列（用于矩阵表格首列） */
  sticky?: boolean;
}

export interface TableProps<T> {
  columns: TableColumn<T>[];
  data: T[];
  /** 行 key */
  rowKey: (row: T, index: number) => string;
  /** 行点击 */
  onRowClick?: (row: T, index: number) => void;
  empty?: ReactNode;
  /** 斑马纹（默认开启） */
  striped?: boolean;
  className?: string;
  /** 最大高度（超出滚动，表头粘性） */
  maxHeightClass?: string;
}

/** 表格：粘性表头、斑马纹、大行高（≥44px） */
export function Table<T>({
  columns,
  data,
  rowKey,
  onRowClick,
  empty,
  striped = true,
  className = '',
  maxHeightClass = 'max-h-[70vh]',
}: TableProps<T>): JSX.Element {
  const alignClass = (align?: 'left' | 'center' | 'right'): string => {
    if (align === 'center') return 'text-center';
    if (align === 'right') return 'text-right';
    return 'text-left';
  };

  return (
    <div className={['overflow-auto rounded-lg border border-surface-border', className].join(' ')}>
      <div className={maxHeightClass ? `overflow-y-auto ${maxHeightClass}` : ''}>
        <table className="w-full border-collapse text-base">
          <thead className="sticky top-0 z-10 bg-surface-muted">
            <tr>
              {columns.map((col) => (
                <th
                  key={col.key}
                  scope="col"
                  className={[
                    'whitespace-nowrap border-b border-surface-border px-4 py-3',
                    'text-base font-bold text-ink',
                    alignClass(col.align),
                    col.widthClass ?? '',
                    col.sticky ? 'sticky left-0 z-20 bg-surface-muted' : '',
                  ].join(' ')}
                >
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.length === 0 && (
              <tr>
                <td colSpan={columns.length} className="px-4 py-10 text-center">
                  {empty ?? <span className="text-ink-muted">暂无数据</span>}
                </td>
              </tr>
            )}
            {data.map((row, index) => (
              <tr
                key={rowKey(row, index)}
                onClick={onRowClick ? () => onRowClick(row, index) : undefined}
                className={[
                  'border-b border-surface-border transition-colors',
                  striped && index % 2 === 1 ? 'bg-surface-muted' : 'bg-surface-raised',
                  onRowClick ? 'cursor-pointer hover:bg-brand-50' : '',
                ].join(' ')}
              >
                {columns.map((col) => (
                  <td
                    key={col.key}
                    className={[
                      'h-14 px-4 py-2 align-middle text-ink',
                      alignClass(col.align),
                      col.widthClass ?? '',
                      col.sticky ? 'sticky left-0 z-10 bg-inherit' : '',
                    ].join(' ')}
                  >
                    {col.render
                      ? col.render(row, index)
                      : (col.accessor?.(row) ?? '')}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
