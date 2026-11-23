import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { useMe } from '../api/hooks';
import { m } from '../i18n/messages';

/**
 * True when the visitor prefers reduced motion. The hero
 * loop below autoplays only when this is false; with
 * reduced motion it is replaced by the poster still.
 */
function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState<boolean>(
    () =>
      typeof window !== 'undefined' &&
      typeof window.matchMedia === 'function' &&
      window.matchMedia('(prefers-reduced-motion: reduce)').matches,
  );
  useEffect(() => {
    if (typeof window.matchMedia !== 'function') return;
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    const onChange = (event: MediaQueryListEvent) => setReduced(event.matches);
    mq.addEventListener?.('change', onChange);
    return () => mq.removeEventListener?.('change', onChange);
  }, []);
  return reduced;
}

const REPO = 'https://github.com/aotlover9-base-eth/forgehub';
const GUIDE = `${REPO}/blob/main/docs/user-guide.md`;

/**
 * The public home page: a single full-viewport "Observe"
 * scene (restored 2026-11-23 at the owner's direction, after
 * the Peak Material You redesign was rejected) - dark glass
 * over a drifting abstract backdrop, an Instrument Serif
 * headline, and the canonical command in a glass pill. It
 * renders OUTSIDE the app shell (its own route in App.tsx):
 * the scene carries its own glass nav, so Home can never be
 * mistaken for another page. Signed-in users get the way to
 * their library; visitors get the way in. The serif lives
 * here and nowhere else in the product.
 */

function LogoMark() {
  // A ring resolving from dots into a solid arc - the noise
  // becoming a command - with a run chevron at its centre.
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" width="24" height="24">
      <path
        d="M4.21 16.5 A9 9 0 0 1 16.5 4.21"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
      />
      <path
        d="M16.5 4.21 A9 9 0 0 1 19.79 16.5"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeDasharray="2.4 2.3"
      />
      <path
        d="M19.79 16.5 A9 9 0 0 1 4.21 16.5"
        stroke="currentColor"
        strokeWidth="2.6"
        strokeLinecap="round"
        strokeDasharray="0.01 4.7"
      />
      <polyline
        points="10.2 9.2 13.4 12 10.2 14.8"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function TerminalIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      width="20"
      height="20"
    >
      <polyline points="4 17 10 11 4 5" />
      <line x1="12" y1="19" x2="20" y2="19" />
    </svg>
  );
}

function CompassIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      width="20"
      height="20"
    >
      <circle cx="12" cy="12" r="10" />
      <polygon points="16.24 7.76 14.12 14.12 7.76 16.24 9.88 9.88 16.24 7.76" />
    </svg>
  );
}

function BookIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      width="20"
      height="20"
    >
      <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
      <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
    </svg>
  );
}

export default function LandingPage() {
  const me = useMe();
  useEffect(() => {
    document.title = m.landing.docTitle;
  }, []);
  const signedIn = Boolean(me.data);
  const reducedMotion = usePrefersReducedMotion();
  return (
    <div className="observe-root">
      <div className="observe-bg" aria-hidden="true" />
      <div className="observe-nav-wrap">
        <nav className="liquid-glass observe-nav" aria-label={m.landing.navAria}>
          <div className="observe-brand-group">
            <Link to="/" className="observe-brand" aria-label={m.landing.homeAria}>
              <LogoMark />
              <span className="observe-wordmark">ForgeHub</span>
            </Link>
            <div className="observe-nav-links">
              <Link className="observe-link" to="/explore">
                {m.landing.explore}
              </Link>
              <a className="observe-link" href={REPO} target="_blank" rel="noreferrer">
                {m.landing.github}
              </a>
              <a className="observe-link" href={GUIDE} target="_blank" rel="noreferrer">
                {m.landing.guide}
              </a>
            </div>
          </div>
          <div className="observe-actions">
            {signedIn ? (
              <>
                <span className="observe-user">{me.data?.username}</span>
                <Link className="observe-cta-solid" to="/dashboard">
                  {m.landing.openLibrary}
                </Link>
              </>
            ) : (
              <>
                <Link className="observe-cta-text" to="/login">
                  {m.landing.login}
                </Link>
                <Link className="observe-cta-solid" to="/signup">
                  {m.landing.createAccount}
                </Link>
              </>
            )}
          </div>
        </nav>
      </div>
      <main className="observe-hero">
        <h1 className="observe-display">
          {m.landing.heroTitleStart}
          <em>{m.landing.heroTitleWord}</em>.
        </h1>
        <p className="observe-sub">{m.landing.heroSub}</p>
        <div className="observe-pill-wrap">
          <div className="liquid-glass observe-pill">
            <span className="observe-cmd">
              <span className="observe-cmd-prompt">$ </span>fg ubuntu-setup
            </span>
            {signedIn ? (
              <Link className="observe-join" to="/explore">
                {m.landing.exploreSnippets}
              </Link>
            ) : (
              <Link className="observe-join" to="/signup">
                {m.landing.getStarted}
              </Link>
            )}
          </div>
        </div>
      </main>
      {/* A nav landmark (not a bare div) so the strip sits
          inside a landmark region for the axe sweep; the
          class drives all styling, so the look is unchanged
          from the original Observe markup. */}
      <nav className="observe-social" aria-label={m.landing.socialAria}>
        <a
          className="liquid-glass-circle observe-circle"
          href={REPO}
          target="_blank"
          rel="noreferrer"
          aria-label={m.landing.sourceAria}
        >
          <TerminalIcon />
        </a>
        <Link
          className="liquid-glass-circle observe-circle"
          to="/explore"
          aria-label={m.landing.explorePublicAria}
        >
          <CompassIcon />
        </Link>
        <a
          className="liquid-glass-circle observe-circle"
          href={GUIDE}
          target="_blank"
          rel="noreferrer"
          aria-label={m.landing.guideAria}
        >
          <BookIcon />
        </a>
      </nav>
      <section className="observe-intro" aria-labelledby="intro-title">
        <h2 className="observe-intro-title" id="intro-title">
          {m.landing.introTitle}
        </h2>
        <p className="observe-intro-sub">{m.landing.introSub}</p>
        <a className="observe-skip" href="#after-intro">
          {m.landing.skipVideo}
        </a>
        <div className="liquid-glass observe-video-card">
          <video
            className="observe-video"
            controls
            preload="metadata"
            playsInline
            poster="/video/poster.png"
            aria-label={m.landing.videoAria}
          >
            <source src="/video/intro-1080p.mp4" type="video/mp4" />
            <source src="/video/intro-1080p.webm" type="video/webm" />
            <track
              kind="captions"
              src="/video/captions.vtt"
              srcLang="en"
              label={m.landing.captionsLabel}
              default
            />
          </video>
        </div>
        <figure className="observe-loop-card">
          {reducedMotion ? (
            <img
              className="observe-loop-media"
              src="/video/poster.png"
              alt={m.landing.loopAlt}
            />
          ) : (
            <video
              className="observe-loop-media"
              autoPlay
              muted
              loop
              playsInline
              preload="metadata"
              poster="/video/poster.png"
              aria-label={m.landing.loopAria}
            >
              <source src="/video/hero-loop.mp4" type="video/mp4" />
              <source src="/video/hero-loop.webm" type="video/webm" />
            </video>
          )}
          <figcaption className="observe-loop-caption">{m.landing.loopCaption}</figcaption>
        </figure>
        <details className="observe-transcript">
          <summary>{m.landing.transcript}</summary>
          <div className="observe-transcript-body">
            <p>{m.landing.transcript1}</p>
            <p>{m.landing.transcript2}</p>
            <p>{m.landing.transcript3}</p>
            <p>{m.landing.transcript4}</p>
            <p>{m.landing.transcript5}</p>
            <p>{m.landing.transcript6}</p>
          </div>
        </details>
      </section>
      <div id="after-intro" tabIndex={-1} />
    </div>
  );
}
