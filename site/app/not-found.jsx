export const metadata = { title: 'Page not found' }

export default function NotFound() {
  return (
    <section className="not-found">
      <span className="code">404</span>
      <h1>This block isn't here</h1>
      <p className="muted">The page may have moved. Try the home page instead.</p>
      <div className="cta-row center">
        <a className="btn primary" href="/">
          Back to ResinCraft
        </a>
      </div>
    </section>
  )
}
