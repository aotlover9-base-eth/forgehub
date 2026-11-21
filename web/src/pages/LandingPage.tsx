import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { useMe } from '../api/hooks';

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
 * scene (user-directed theme, 2026-11-20) - dark glass over
 * a drifting abstract backdrop, an Instrument Serif headline,
 * and the canonical command in a glass pill. It renders
 * OUTSIDE the app shell (its own route in App.tsx): the scene
 * carries its own glass nav, so Home can never be mistaken
 * for another page. Signed-in users get the way to their
 * library; visitors get the way in.
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
  const signedIn = Boolean(me.data);
  const reducedMotion = usePrefersReducedMotion();
  return (
    <div className="observe-root">
      <div className="observe-bg" aria-hidden="true" />
      <div className="observe-nav-wrap">
        <nav className="liquid-glass observe-nav" aria-label="Landing">
          <div className="observe-brand-group">
            <Link to="/" className="observe-brand" aria-label="ForgeHub home">
              <LogoMark />
              <span className="observe-wordmark">ForgeHub</span>
            </Link>
            <div className="observe-nav-links">
              <Link className="observe-link" to="/explore">
                Explore
              </Link>
              <a className="observe-link" href={REPO} target="_blank" rel="noreferrer">
                GitHub
              </a>
              <a className="observe-link" href={GUIDE} target="_blank" rel="noreferrer">
                Guide
              </a>
            </div>
          </div>
          <div className="observe-actions">
            {signedIn ? (
              <>
                <span className="observe-user">{me.data?.username}</span>
                <Link className="observe-cta-solid" to="/dashboard">
                  Open your library
                </Link>
              </>
            ) : (
              <>
                <Link className="observe-cta-text" to="/login">
                  Log in
                </Link>
                <Link className="observe-cta-solid" to="/signup">
                  Create account
                </Link>
              </>
            )}
          </div>
        </nav>
      </div>
      <main className="observe-hero">
        <h1 className="observe-display">
          Save a command once, run it <em>anywhere</em>.
        </h1>
        <p className="observe-sub">
          The shell commands you keep re-typing - stored once, shown in full, and run only with
          your approval.
        </p>
        <div className="observe-pill-wrap">
          <div className="liquid-glass observe-pill">
            <span className="observe-cmd">
              <span className="observe-cmd-prompt">$ </span>fg ubuntu-setup
            </span>
            {signedIn ? (
              <Link className="observe-join" to="/explore">
                Explore snippets
              </Link>
            ) : (
              <Link className="observe-join" to="/signup">
                Get started
              </Link>
            )}
          </div>
        </div>
      </main>
      <div className="observe-social">
        <a
          className="liquid-glass-circle observe-circle"
          href={REPO}
          target="_blank"
          rel="noreferrer"
          aria-label="ForgeHub source on GitHub"
        >
          <TerminalIcon />
        </a>
        <Link className="liquid-glass-circle observe-circle" to="/explore" aria-label="Explore public snippets">
          <CompassIcon />
        </Link>
        <a
          className="liquid-glass-circle observe-circle"
          href={GUIDE}
          target="_blank"
          rel="noreferrer"
          aria-label="Read the user guide"
        >
          <BookIcon />
        </a>
      </div>
      <section className="observe-intro" aria-labelledby="intro-title">
        <h2 className="observe-intro-title" id="intro-title">
          See it run before it runs
        </h2>
        <p className="observe-intro-sub">
          Sixty seconds, silent and captioned: a command is saved once, then run anywhere - and
          ForgeHub shows you exactly what will run before anything executes.
        </p>
        <a className="observe-skip" href="#after-intro">
          Skip video
        </a>
        <div className="liquid-glass observe-video-card">
          <video
            className="observe-video"
            controls
            preload="metadata"
            playsInline
            poster="/video/poster.png"
            aria-label="ForgeHub introduction video, 60 seconds, silent with captions"
          >
            <source src="/video/intro-1080p.mp4" type="video/mp4" />
            <source src="/video/intro-1080p.webm" type="video/webm" />
            <track kind="captions" src="/video/captions.vtt" srcLang="en" label="English" default />
          </video>
        </div>
        <figure className="observe-loop-card">
          {reducedMotion ? (
            <img
              className="observe-loop-media"
              src="/video/poster.png"
              alt="The ForgeHub confirmation card for fg ubuntu-setup: the exact command, version 1, its sha256 hash and the uses-sudo warning, waiting for approval"
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
              aria-label="Eight-second loop: the ForgeHub confirmation card for fg ubuntu-setup"
            >
              <source src="/video/hero-loop.mp4" type="video/mp4" />
              <source src="/video/hero-loop.webm" type="video/webm" />
            </video>
          )}
          <figcaption className="observe-loop-caption">
            The confirmation card, in an eight-second loop.
          </figcaption>
        </figure>
        <details className="observe-transcript">
          <summary>Transcript</summary>
          <div className="observe-transcript-body">
            <p>
              Every new machine starts the same way. You type the same long command, again. It
              lives in a note, a chat, a gist, or your shell history.
            </p>
            <p>
              ForgeHub is a home for your shell commands. Paste yours on the web, name it, save
              it. Text only, versioned, and private by default.
            </p>
            <p>
              Log in once from the terminal. Then just type fg and the name. ForgeHub shows
              exactly what it will run, and asks you first.
            </p>
            <p>
              Safety is the product. Other people's snippets need a typed yes. If a snippet
              changed, you see the diff first. With no terminal, it refuses to run.
            </p>
            <p>
              Keep snippets private, share a link, or publish them. Search, star and fork.
              Record in your terminal and push like git.
            </p>
            <p>ForgeHub. Save a command once. Run it anywhere with fg.</p>
          </div>
        </details>
      </section>
      <div id="after-intro" tabIndex={-1} />
    </div>
  );
}
