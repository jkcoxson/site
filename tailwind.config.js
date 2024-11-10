/** @type {import('tailwindcss').Config} */
module.exports = {
  content: {
    files: ["*.html", "./src/**/*.rs"],
  },
  theme: {
    extend: {
      animation: {
        draw: 'draw 4s infinite',
      },
    },
  },
  plugins: [],
}
