import { useEffect } from 'react';
import { Link } from 'react-router-dom';
import { useMe } from '../api/hooks';
import { Card, CodeBlock, Icon } from '../design';
import { m } from '../i18n/messages';

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
    title: m.landing.feature1Title,
    body: m.landing.feature1Body,
  },
  {
    icon: 'terminal',
    title: m.landing.feature2Title,
    body: m.landing.feature2Body,
  },
  {
    icon: 'shield',
    title: m.landing.feature3Title,
    body: m.landing.feature3Body,
  },
];

export default function LandingPage() {
  const me = useMe();
  useEffect(() => {
    document.title = m.landing.docTitle;
  }, []);
  const signedIn = Boolean(me.data);
  return (
    <div className="fh-landing">
      <header className="fh-landing__bar">
        <Link to="/" className="fh-landing__brand" aria-label={m.landing.homeAria}>
          <Icon name="terminal" />
          <span>ForgeHub</span>
        </Link>
        <nav className="fh-landing__links" aria-label={m.landing.navAria}>
          <Link to="/explore">{m.landing.explore}</Link>
          <a href={REPO} target="_blank" rel="noreferrer">
            {m.landing.github}
          </a>
          <a href={GUIDE} target="_blank" rel="noreferrer">
            {m.landing.guide}
          </a>
        </nav>
        <div className="fh-landing__actions">
          {signedIn ? (
            <>
              <span className="fh-landing__user">{me.data?.username}</span>
              <Link className="fh-landing__cta" to="/dashboard">
                {m.landing.openLibrary}
              </Link>
            </>
          ) : (
            <>
              <Link className="fh-landing__cta fh-landing__cta--text" to="/login">
                {m.landing.login}
              </Link>
              <Link className="fh-landing__cta" to="/signup">
                {m.landing.createAccount}
              </Link>
            </>
          )}
        </div>
      </header>

      <main className="fh-landing__main">
        <section className="fh-landing__hero">
          <h1 className="fh-landing__display">{m.landing.heroTitle}</h1>
          <p className="fh-landing__sub">
            {m.landing.heroSub}
          </p>
          <div className="fh-landing__demo">
            <CodeBlock code={DEMO} label={m.landing.demoLabel} />
          </div>
          <div className="fh-landing__hero-actions">
            {signedIn ? (
              <Link className="fh-landing__cta" to="/dashboard">
                {m.landing.openLibrary}
              </Link>
            ) : (
              <Link className="fh-landing__cta" to="/signup">
                {m.landing.createAccount}
              </Link>
            )}
            <Link className="fh-landing__cta fh-landing__cta--outlined" to="/explore">
              {m.landing.exploreSnippets}
            </Link>
          </div>
        </section>

        <section className="fh-landing__features" aria-label={m.landing.featuresAria}>
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
            {m.landing.introTitle}
          </h2>
          <p className="fh-landing__sub">
            {m.landing.introSub}
          </p>
          <a className="fh-landing__skip" href="#after-intro">
            {m.landing.skipVideo}
          </a>
          <video
            className="fh-landing__video"
            controls
            preload="metadata"
            playsInline
            poster="/video/poster.png"
            aria-label={m.landing.videoAria}
          >
            <source src="/video/intro-1080p.mp4" type="video/mp4" />
            <source src="/video/intro-1080p.webm" type="video/webm" />
            <track kind="captions" src="/video/captions.vtt" srcLang="en" label={m.landing.captionsLabel} default />
          </video>
          <details className="fh-landing__transcript">
            <summary>{m.landing.transcript}</summary>
            <div>
              <p>
                {m.landing.transcript1}
              </p>
              <p>
                {m.landing.transcript2}
              </p>
              <p>
                {m.landing.transcript3}
              </p>
              <p>
                {m.landing.transcript4}
              </p>
              <p>
                {m.landing.transcript5}
              </p>
              <p>{m.landing.transcript6}</p>
            </div>
          </details>
        </section>
        <div id="after-intro" tabIndex={-1} />
      </main>

      <footer className="fh-landing__footer">
        <a href={REPO} target="_blank" rel="noreferrer">
          {m.landing.sourceOnGithub}
        </a>
        <span>{m.landing.noTracking}</span>
        <span>{m.landing.footerTagline}</span>
      </footer>
    </div>
  );
}
