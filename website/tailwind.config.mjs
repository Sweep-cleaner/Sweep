/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{astro,html,js,jsx,ts,tsx,md,mdx}'],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        bg: {
          base: '#1a1b26',
          surface: '#282828',
          elevated: '#2f3142'
        },
        rust: {
          DEFAULT: '#DEA584',
          deep: '#E43717',
          glow: '#ff7a45'
        },
        cyan: {
          DEFAULT: '#7FDBCA',
          glow: '#9ece6a',
          deep: '#0db9d7'
        },
        text: {
          primary: '#c0caf5',
          muted: '#9aa5ce',
          dim: '#565f89'
        },
        border: {
          DEFAULT: '#414868',
          glow: '#7FDBCA'
        }
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', 'sans-serif'],
        mono: ['"JetBrains Mono"', 'ui-monospace', 'monospace']
      },
      boxShadow: {
        'glow-cyan': '0 0 30px rgba(127,219,202,0.3)',
        'glow-rust': '0 0 30px rgba(222,165,132,0.3)'
      },
      animation: {
        'pulse-glow': 'pulse-glow 3s ease-in-out infinite',
        'fade-up': 'fade-up 0.6s ease-out'
      },
      keyframes: {
        'pulse-glow': {
          '0%, 100%': { opacity: '1', filter: 'drop-shadow(0 0 20px rgba(127,219,202,0.5))' },
          '50%': { opacity: '0.9', filter: 'drop-shadow(0 0 40px rgba(127,219,202,0.8))' }
        },
        'fade-up': {
          '0%': { opacity: '0', transform: 'translateY(20px)' },
          '100%': { opacity: '1', transform: 'translateY(0)' }
        }
      }
    }
  },
  plugins: []
};
