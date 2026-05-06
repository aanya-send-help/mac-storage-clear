/** @type {import('tailwindcss').Config} */
export default {
  content: ["./src/**/*.{astro,html,js,jsx,ts,tsx,md,mdx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          "SF Pro Text",
          "system-ui",
          "sans-serif",
        ],
        mono: ["SF Mono", "Menlo", "Monaco", "Consolas", "monospace"],
      },
      colors: {
        ink: "#0f0f10",
        paper: "#fafaf9",
        rose: {
          50: "#fff3f7",
          100: "#ffe6ee",
          400: "#f472b6",
          500: "#ec4899",
          600: "#db2777",
        },
      },
    },
  },
  plugins: [],
};
