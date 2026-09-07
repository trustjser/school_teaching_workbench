export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {
      overrideBrowserslist: [
        'last 2 Chrome versions',
        'last 2 Edge versions',
        'Safari >= 13',
        'Firefox ESR',
      ],
    },
  },
};
