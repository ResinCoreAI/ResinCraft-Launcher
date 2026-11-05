// Stripe.js for the launcher's own payment form. It has to come from js.stripe.com (Stripe doesn't allow bundling it),
// and its card fields are Stripe's own frames: card numbers never reach the launcher or the relay.

/* eslint-disable @typescript-eslint/no-explicit-any */
export type StripeJs = any

declare global {
  interface Window {
    Stripe?: (key: string) => StripeJs
  }
}

let loading: Promise<(key: string) => StripeJs> | undefined

export function loadStripe(): Promise<(key: string) => StripeJs> {
  if (window.Stripe) return Promise.resolve(window.Stripe)
  loading ??= new Promise((resolve, reject) => {
    const s = document.createElement('script')
    s.src = 'https://js.stripe.com/v3/'
    s.async = true
    s.onload = () => (window.Stripe ? resolve(window.Stripe) : reject(new Error('Stripe.js did not start')))
    s.onerror = () => {
      loading = undefined
      s.remove()
      reject(new Error('js.stripe.com could not be reached'))
    }
    document.head.append(s)
  })
  return loading
}

/** The payment form in the launcher's colours (custom themes included): read from its CSS variables. */
export function stripeAppearance() {
  const css = getComputedStyle(document.documentElement)
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback
  return {
    theme: 'night',
    variables: {
      colorPrimary: v('--accent', '#b26bff'),
      colorBackground: v('--surface', '#13111d'),
      colorText: v('--text', '#eeecf6'),
      colorTextSecondary: v('--text-dim', '#a8a3bd'),
      colorTextPlaceholder: v('--text-faint', '#7a7492'),
      colorDanger: v('--danger', '#ff5c6a'),
      colorSuccess: v('--success', '#39ff88'),
      fontFamily: v('--font', 'system-ui, sans-serif'),
      borderRadius: v('--radius', '6px'),
      spacingUnit: '4px'
    },
    rules: {
      '.Input': { border: `1px solid ${v('--border-strong', '#372f4f')}`, boxShadow: 'none' },
      '.Input:focus': { border: `1px solid ${v('--accent', '#b26bff')}`, boxShadow: `0 0 0 3px ${v('--accent-soft', 'rgba(178,107,255,.13)')}` },
      '.Tab': { border: `1px solid ${v('--border', '#262238')}` },
      '.Label': { color: v('--text-dim', '#a8a3bd') }
    }
  }
}
