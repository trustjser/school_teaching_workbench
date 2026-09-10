import { twMerge } from 'tailwind-merge';

/**
 * 合并 Tailwind 类名，并消解冲突的同类工具类。
 *
 * Tailwind 的同类属性（如 py-2 / py-0、px-1 / px-0）优先级由**编译后 CSS 的生成顺序**
 * 决定，而非 className 字符串里的书写顺序——后写入 CSS 的胜出。因此把调用方的
 * className 直接拼在组件内部类之后，常常会被内部类覆盖（典型如 `py-2` 排在 `py-0` 前，
 * 导致传入的 `py-0` 失效）。
 *
 * `twMerge` 理解 Tailwind 语义，能识别「同属性、后者胜出」，让调用方的覆盖类稳稳生效。
 * 用法：`cn('px-1 py-2', props.className)`。
 */
export function cn(...classes: Array<string | false | null | undefined>): string {
  return twMerge(classes.filter(Boolean).join(' '));
}
