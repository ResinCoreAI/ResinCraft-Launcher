// Catches a page that crashes while drawing, so one broken page shows what went wrong (with a way back) instead of
// leaving the whole window black. Around each page (App.tsx, `resetKey` = the page) and around everything (main.tsx).
import { Component, type ErrorInfo, type ReactNode } from 'react'
import { t } from '../i18n'

interface Props {
  children: ReactNode
  /** When it changes (another page was opened), the crash is forgotten and the page draws again. */
  resetKey?: string
  /** Where "Back to Home" goes; without it the button reloads the window. */
  onHome?: () => void
}
interface State {
  error: Error | null
  where: string
}

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, where: '' }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('A page crashed', error, info.componentStack)
    this.setState({ where: info.componentStack ?? '' })
  }

  componentDidUpdate(prev: Props) {
    if (prev.resetKey !== this.props.resetKey && this.state.error) this.setState({ error: null, where: '' })
  }

  render() {
    const { error, where } = this.state
    if (!error) return this.props.children
    const details = [`${error.name}: ${error.message}`, error.stack ?? '', where.trim(), `UA: ${navigator.userAgent}`].filter(Boolean).join('\n\n')
    // Plain text if the translations themselves are what broke.
    const say = (key: Parameters<typeof t>[0], fallback: string) => {
      try {
        return t(key)
      } catch {
        return fallback
      }
    }
    return (
      <div className="crash-card card" role="alert">
        <b>{say('pageCrash.title', 'This page ran into a problem')}</b>
        <p className="hint">{say('pageCrash.body', 'The rest of ResinCraft still works. Copy the details below into Report an issue so it can be fixed.')}</p>
        <pre className="mono crash-details">{`${error.name}: ${error.message}`}</pre>
        <div className="row">
          <button className="btn sm" onClick={() => void navigator.clipboard?.writeText(details)}>
            {say('pageCrash.copy', 'Copy details')}
          </button>
          <button className="btn sm primary" onClick={() => (this.props.onHome ? (this.setState({ error: null, where: '' }), this.props.onHome()) : location.reload())}>
            {this.props.onHome ? say('pageCrash.home', 'Back to Home') : say('pageCrash.reload', 'Reload')}
          </button>
        </div>
      </div>
    )
  }
}
