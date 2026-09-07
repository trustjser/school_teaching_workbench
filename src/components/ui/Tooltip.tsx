import { useState, type ReactNode } from 'react';

export interface TooltipProps {
  content: ReactNode;
  children: ReactNode;
  /** 位置 */
  placement?: 'top' | 'bottom';
  className?: string;
}

/** 悬浮提示：hover + focus 均触发，鼠标与键盘均可访问 */
export function Tooltip({
  content,
  children,
  placement = 'top',
  className = '',
}: TooltipProps): JSX.Element {
  const [visible, setVisible] = useState(false);
  return (
    <span
      className={['relative inline-flex', className].join(' ')}
      onMouseEnter={() => setVisible(true)}
      onMouseLeave={() => setVisible(false)}
      onFocus={() => setVisible(true)}
      onBlur={() => setVisible(false)}
    >
      {children}
      {visible && (
        <span
          role="tooltip"
          className={[
            'pointer-events-none absolute left-1/2 z-toast -translate-x-1/2 whitespace-nowrap',
            'rounded-md bg-slate-900 px-3 py-1.5 text-sm font-medium text-white shadow-pop',
            placement === 'top' ? 'bottom-full mb-2' : 'top-full mt-2',
          ].join(' ')}
        >
          {content}
        </span>
      )}
    </span>
  );
}
