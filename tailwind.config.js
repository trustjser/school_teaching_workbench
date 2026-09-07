/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
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
        // 高对比品牌色
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
        ink: {
          DEFAULT: '#0f172a',
          soft: '#334155',
          muted: '#64748b',
          inverse: '#f8fafc',
        },
        surface: {
          DEFAULT: '#ffffff',
          sunken: '#f1f5f9',
          raised: '#ffffff',
          border: '#cbd5e1',
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
      },
      boxShadow: {
        card: '0 1px 2px 0 rgba(15, 23, 42, 0.08), 0 1px 3px 0 rgba(15, 23, 42, 0.06)',
        pop: '0 12px 32px -8px rgba(15, 23, 42, 0.28)',
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
          '0%': { boxShadow: '0 0 0 0 rgba(37, 121, 235, 0.5)' },
          '70%': { boxShadow: '0 0 0 12px rgba(37, 121, 235, 0)' },
          '100%': { boxShadow: '0 0 0 0 rgba(37, 121, 235, 0)' },
        },
      },
      animation: {
        'toast-in': 'toast-in 160ms ease-out',
        'pop-in': 'pop-in 120ms ease-out',
        'slide-in-right': 'slide-in-right 180ms ease-out',
        'pulse-ring': 'pulse-ring 1.4s ease-out infinite',
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
