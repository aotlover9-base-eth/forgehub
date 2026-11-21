import { useEffect } from 'react';
import { Link } from 'react-router-dom';
import { useMe } from '../api/hooks';
import { Card, CodeBlock, Icon } from '../design';

const REPO = 'https://github.com/aotlover9-base-eth/forgehub';
const GUIDE = `${REPO}/blob/main/docs/user-guide.md`;

/**
 * The public home page (Peak A5): a plain Material You 3
 * page built from the same tokens and components as the
 * signed-in app - no separate theme, no serif, no glass.
 * It renders OUTSIDE the app shell (its own route in
 * App.tsx) with a simple top bar. The hero shows the real
 * product exchange: the command, the full confirmation
 * card, and only then execution.
 */

const DEMO = [
  '$ fg ubuntu-setup',
  '',
  'About to run: ubuntu-setup v3 by ada',
  '  sudo apt-get update',
  '  sudo apt-get install -y curl git',
  'sha256: 9f2c41ab...  uses sudo',
  'Run it? [y/N] y',
  'Done in 41s.',
].join('\n');

const FEATURES: { icon: string; title: string; body: string }[] = [
  {
    icon: 'file_upload',
    title: 'Save it on the web',
    body: 'Paste a command, name it, add tags. Private by default, versioned every time you change it.',
  },
  {
    icon: 'terminal',
    title: 'Run it anywhere',
    body: 'One short command - fg and the name - on any machine where you are logged in. No paths, no hunting through history.',
  },
  {
    icon: 'shield',
    title: 'Safety first',
    body: "You always see the full commands, the owner, the version and the hash before anything runs. Other people's snippets need a typed yes.",
  },
];

export default function LandingPage() {
  const me = useMe();
  useEffect(() => {
    document.title = 'ForgeHub - save a command once, run it anywhere';
  }, []);
  const signedIn = Boolean(me.data);
  return (
    <div className="fh-landing">
      <header className="fh-landing__bar">
        <Link to="/" className="fh-landing__brand" aria-label="ForgeHub home">
          <Icon name="terminal" />
          <span>ForgeHub</span>
        </Link>
        <nav className="fh-landing__links" aria-label="Landing">
          <Link to="/explore">Explore</Link>
          <a href={REPO} target="_blank" rel="noreferrer">
            GitHub
          </a>
          <a href={GUIDE} target="_blank" rel="noreferrer">
            Guide
          </a>
        </nav>
        <div className="fh-landing__actions">
          {signedIn ? (
            <>
              <span className="fh-landing__user">{me.data?.username}</span>
              <Link className="fh-landing__cta" to="/dashboard">
                Open your library
              </Link>
            </>
          ) : (
            <>
              <Link className="fh-landing__cta fh-landing__cta--text" to="/login">
                Log in
              </Link>
              <Link className="fh-landing__cta" to="/signup">
                Create account
              </Link>
            </>
          )}
        </div>
      </header>

      <main className="fh-landing__main">
        <section className="fh-landing__hero">
          <h1 className="fh-landing__display">Save a command once, run it anywhere.</h1>
          <p className="fh-landing__sub">
            The shell commands you keep re-typing - stored once, shown in full, and run only with
            your approval.
          </p>
          <div className="fh-landing__demo">
            <CodeBlock code={DEMO} label="Terminal - fg ubuntu-setup" />
          </div>
          <div className="fh-landing__hero-actions">
            {signedIn ? (
              <Link className="fh-landing__cta" to="/dashboard">
                Open your library
              </Link>
            ) : (
              <Link className="fh-landing__cta" to="/signup">
                Create account
              </Link>
            )}
            <Link className="fh-landing__cta fh-landing__cta--outlined" to="/explore">
              Explore snippets
            </Link>
          </div>
        </section>

        <section className="fh-landing__features" aria-label="What ForgeHub does">
          {FEATURES.map((feature) => (
            <Card key={feature.title} variant="filled">
              <p className="fh-landing__feature-icon">
                <Icon name={feature.icon} />
              </p>
              <h2 className="fh-landing__feature-title">{feature.title}</h2>
              <p className="fh-landing__feature-body">{feature.body}</p>
            </Card>
          ))}
        </section>

        <section className="fh-landing__intro" aria-labelledby="intro-title">
          <h2 className="fh-landing__section-title" id="intro-title">
            See it run before it runs
          </h2>
          <p className="fh-landing__sub">
            Sixty seconds, captioned: a command is saved once, then run anywhere - and ForgeHub
            shows you exactly what will run before anything executes.
          </p>
          <a className="fh-landing__skip" href="#after-intro">
            Skip video
          </a>
          <video
            className="fh-landing__video"
            controls
            preload="metadata"
            playsInline
            poster="/video/poster.png"
            aria-label="ForgeHub introduction video, 60 seconds, with voice-over and captions"
          >
            <source src="/video/intro-1080p.mp4" type="video/mp4" />
            <source src="/video/intro-1080p.webm" type="video/webm" />
            <track kind="captions" src="/video/captions.vtt" srcLang="en" label="English" default />
          </video>
          <details className="fh-landing__transcript">
            <summary>Transcript</summary>
            <div>
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
      </main>

      <footer className="fh-landing__footer">
        <a href={REPO} target="_blank" rel="noreferrer">
          Source on GitHub
        </a>
        <span>No tracking. No ads.</span>
        <span>ForgeHub - save a command once, run it anywhere.</span>
      </footer>
    </div>
  );
}
