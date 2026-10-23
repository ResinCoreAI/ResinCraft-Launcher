import { DISCLAIMER, EMAIL, GITHUB } from '@/lib/site'

export default function Footer() {
  return (
    <footer className="footer">
      <div className="wrap foot-grid">
        <div className="foot-brand">
          <a className="brand" href="/">
            <img src="/assets/logo.svg" alt="" width="26" height="33" />
            <span>
              <b>ResinCraft</b> Launcher
            </span>
          </a>
          <p>A Minecraft launcher, made with care by Puirkle.</p>
          <p>
            Email: <a href={`mailto:${EMAIL}`}>{EMAIL}</a>
          </p>
        </div>
        <nav aria-label="Product">
          <h4>Product</h4>
          <a href="/#features">Features</a>
          <a href="/#screens">Screenshots</a>
          <a href="/#downloads">Downloads</a>
          <a href="/#faq">FAQ</a>
        </nav>
        <nav aria-label="Help">
          <h4>Help</h4>
          <a href={`${GITHUB}/issues`}>Report a bug</a>
          <a href={`${GITHUB}/releases`}>All releases</a>
          <a href="/privacy/">Privacy</a>
          <a href="/terms/">Terms</a>
        </nav>
        <nav aria-label="More from Puirkle">
          <h4>More from Puirkle</h4>
          <a href="https://qr.puirkle.net">PuirkleQR Studio</a>
          <a href={GITHUB}>ResinCraft on GitHub</a>
        </nav>
      </div>
      <div className="wrap foot-bottom">
        <p>© 2026 ResinCore · ResinCraft Launcher · GPLv3</p>
        <p className="legal">{DISCLAIMER}</p>
      </div>
    </footer>
  )
}
