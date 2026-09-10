/**
 * shared 包统一出口。
 *
 * 只导出「跨端契约」级别的内容：固定 app target 与领域类型。
 * 其余模块（components / lib / store / hooks）继续按文件路径导入，
 * 避免为了 barrel 而制造巨大的循环依赖面。
 */

export * from './app-target';
export * from './types';
