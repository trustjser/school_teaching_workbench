/** @type {import('tailwindcss').Config} */
export default {
  content: ['./apps/*/index.html', './apps/*/src/**/*.{js,ts,jsx,tsx}', './packages/shared/src/**/*.{js,ts,jsx,tsx}'],
  darkMode: 'class',
  theme: {
    extend: {
      fontSize: {
        // 大屏字号档位：以 16px 为 1rem 基准，正文最小 18px（1.125rem）
        '2xs': ['0.75rem', { lineHeight: '1rem' }],
        xs: ['0.875rem', { lineHeight: '1.25rem' }],
        sm: ['1rem', { lineHeight: '1.5rem' }],
        base: ['1.125rem', { lineHeight: '1.75rem' }],
        lg: ['1.25rem', { lineHeight: '1.75rem' }],
        xl: ['1.375rem', { lineHeight: '2rem' }],
        '2xl': ['1.5rem', { lineHeight: '2rem' }],
        '3xl': ['1.875rem', { lineHeight: '2.25rem' }],
        '4xl': ['2.25rem', { lineHeight: '2.5rem' }],
        '5xl': ['3rem', { lineHeight: '3.25rem' }],
        '6xl': ['3.75rem', { lineHeight: '4rem' }],
        '7xl': ['4.5rem', { lineHeight: '4.75rem' }],
      },
      colors: {
        // 表面层（主题感知：全部映射到 src/theme.css 的 CSS 变量）
        surface: {
          DEFAULT: 'rgb(var(--surface) / <alpha-value>)',
          sunken: 'rgb(var(--surface-sunken) / <alpha-value>)',
          muted: 'rgb(var(--surface-muted) / <alpha-value>)',
          raised: 'rgb(var(--surface-raised) / <alpha-value>)',
          border: 'rgb(var(--surface-border) / <alpha-value>)',
        },
        // 文字（主题感知）
        ink: {
          DEFAULT: 'rgb(var(--ink) / <alpha-value>)',
          soft: 'rgb(var(--ink-soft) / <alpha-value>)',
          muted: 'rgb(var(--ink-muted) / <alpha-value>)',
          inverse: 'rgb(var(--ink-inverse) / <alpha-value>)',
        },
        // 品牌色（跨主题通用的高饱和强调色）
        brand: {
          50: '#eff8ff',
          100: '#dbeefe',
          200: '#bfe2fe',
          300: '#93d0fd',
          400: '#60b6fa',
          500: '#3b97f6',
          600: '#2579eb',
          700: '#1d63d8',
          800: '#1e51af',
          900: '#1e468a',
        },
        // 状态色（与 docs/03-tasks.md §4.7 锁定值一致）
        state: {
          present: '#16a34a',
          leave: '#ca8a04',
          absent: '#dc2626',
          late: '#ea580c',
          transferred: '#64748b',
        },
        // 状态节点色 token（与 DDL color_token CHECK 一致）
        node: {
          slate: '#475569',
          blue: '#2563eb',
          amber: '#ca8a04',
          emerald: '#16a34a',
          rose: '#e11d48',
          violet: '#7c3aed',
          cyan: '#0891b2',
          orange: '#ea580c',
        },
      },
      spacing: {
        // 触控目标 ≥ 44px
        touch: '2.75rem',
        'touch-lg': '3.25rem',
      },
      minWidth: {
        touch: '2.75rem',
      },
      minHeight: {
        touch: '2.75rem',
      },
      borderRadius: {
        card: '0.75rem',
        panel: '1rem',
        xl: '1.25rem',
      },
      boxShadow: {
        // 现代分层柔影：浅色更柔、深色更聚（由 --shadow-strength 间接控制见下方 rgba）
        card: '0 1px 2px rgba(15, 23, 42, 0.06), 0 2px 8px -2px rgba(15, 23, 42, 0.08)',
        soft: '0 1px 3px rgba(15, 23, 42, 0.05), 0 8px 24px -8px rgba(15, 23, 42, 0.12)',
        pop: '0 12px 32px -8px rgba(15, 23, 42, 0.28)',
        glow: '0 0 0 1px rgb(var(--brand) / 0.35), 0 8px 28px -6px rgb(var(--brand) / 0.45)',
      },
      backgroundImage: {
        'brand-gradient':
          'linear-gradient(135deg, rgb(var(--brand)) 0%, rgb(var(--brand-strong)) 100%)',
        'brand-sheen':
          'linear-gradient(120deg, transparent 0%, rgb(255 255 255 / 0.18) 50%, transparent 100%)',
      },
      screens: {
        // 大屏断点：教室一体机常见分辨率
        board: '1600px',
        wall: '1920px',
      },
      keyframes: {
        'toast-in': {
          '0%': { opacity: '0', transform: 'translateY(-8px) scale(0.98)' },
          '100%': { opacity: '1', transform: 'translateY(0) scale(1)' },
        },
        'pop-in': {
          '0%': { opacity: '0', transform: 'scale(0.96)' },
          '100%': { opacity: '1', transform: 'scale(1)' },
        },
        'slide-in-right': {
          '0%': { transform: 'translateX(100%)' },
          '100%': { transform: 'translateX(0)' },
        },
        'pulse-ring': {
          '0%': { boxShadow: '0 0 0 0 rgb(var(--ring-brand) / 0.5)' },
          '70%': { boxShadow: '0 0 0 12px rgb(var(--ring-brand) / 0)' },
          '100%': { boxShadow: '0 0 0 0 rgb(var(--ring-brand) / 0)' },
        },
        // —— 现代入场与微交互 ——
        'rise-in': {
          '0%': { opacity: '0', transform: 'translateY(14px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        'fade-in': {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' },
        },
        'scale-in': {
          '0%': { opacity: '0', transform: 'scale(0.94)' },
          '100%': { opacity: '1', transform: 'scale(1)' },
        },
        'slide-in-up': {
          '0%': { opacity: '0', transform: 'translateY(100%)' },
          '100%': { opacity: '1', transform: 'translateY(0)' },
        },
        'shimmer': {
          '0%': { backgroundPosition: '-200% 0' },
          '100%': { backgroundPosition: '200% 0' },
        },
        'float': {
          '0%, 100%': { transform: 'translateY(0)' },
          '50%': { transform: 'translateY(-6px)' },
        },
        'gradient-pan': {
          '0%': { backgroundPosition: '0% 50%' },
          '50%': { backgroundPosition: '100% 50%' },
          '100%': { backgroundPosition: '0% 50%' },
        },
        'ring-spin': {
          to: { transform: 'rotate(360deg)' },
        },
        'icon-pop': {
          '0%': { transform: 'scale(0.6)' },
          '60%': { transform: 'scale(1.15)' },
          '100%': { transform: 'scale(1)' },
        },
      },
      animation: {
        'toast-in': 'toast-in 160ms ease-out',
        'pop-in': 'pop-in 160ms ease-out',
        'slide-in-right': 'slide-in-right 180ms ease-out',
        'pulse-ring': 'pulse-ring 1.4s ease-out infinite',
        'rise-in': 'rise-in 0.5s cubic-bezier(0.21, 1.02, 0.73, 1) both',
        'fade-in': 'fade-in 0.4s ease both',
        'scale-in': 'scale-in 0.22s cubic-bezier(0.21, 1.02, 0.73, 1) both',
        'slide-in-up': 'slide-in-up 0.28s cubic-bezier(0.21, 1.02, 0.73, 1) both',
        'shimmer': 'shimmer 1.4s linear infinite',
        'float': 'float 6s ease-in-out infinite',
        'gradient-pan': 'gradient-pan 8s ease infinite',
        'icon-pop': 'icon-pop 0.3s cubic-bezier(0.21, 1.02, 0.73, 1) both',
      },
      zIndex: {
        drawer: '60',
        modal: '70',
        toast: '90',
      },
    },
  },
  plugins: [],
};
