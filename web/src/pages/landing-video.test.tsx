import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import LandingPage from './LandingPage';

// The landing page (Peak A5): the hero shows the real
// confirmation exchange as text, and the Intro section
// carries the 60-second video. The contract under test:
// poster + controls + default captions on the video, no
// autoplay anywhere on the page (the Observe-era ambient
// loop is gone), a skip link, and the full transcript.

function renderLanding() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>
        <LandingPage />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

describe('LandingPage', () => {
  it('shows the confirmation exchange in the hero', () => {
    renderLanding();
    expect(screen.getByRole('heading', { name: 'Save a command once, run it anywhere.' })).toBeTruthy();
    expect(document.body.textContent).toContain('About to run: ubuntu-setup v3 by ada');
    expect(document.body.textContent).toContain('Run it? [y/N]');
    expect(document.body.textContent).toContain('uses sudo');
  });

  it('offers the intro video with poster, controls and default captions, without autoplay', () => {
    const { container } = renderLanding();
    const video = container.querySelector<HTMLVideoElement>('video.fh-landing__video');
    expect(video).not.toBeNull();
    expect(video!.getAttribute('poster')).toBe('/video/poster.png');
    expect(video!.hasAttribute('controls')).toBe(true);
    expect(video!.getAttribute('preload')).toBe('metadata');
    expect(video!.playsInline).toBe(true);
    expect(video!.autoplay).toBe(false);
    expect(video!.hasAttribute('autoplay')).toBe(false);

    const sources = Array.from(video!.querySelectorAll('source')).map((s) => s.getAttribute('src'));
    expect(sources).toEqual(['/video/intro-1080p.mp4', '/video/intro-1080p.webm']);

    const track = video!.querySelector('track');
    expect(track).not.toBeNull();
    expect(track!.getAttribute('kind')).toBe('captions');
    expect(track!.getAttribute('src')).toBe('/video/captions.vtt');
    expect(track!.hasAttribute('default')).toBe(true);
  });

  it('has no autoplaying media anywhere (the ambient loop was removed)', () => {
    const { container } = renderLanding();
    const autoplaying = container.querySelectorAll('[autoplay]');
    expect(autoplaying).toHaveLength(0);
  });

  it('has a skip link and the full transcript', () => {
    renderLanding();
    const skip = screen.getByRole('link', { name: 'Skip video' });
    expect(skip.getAttribute('href')).toBe('#after-intro');
    expect(screen.getByText('Transcript')).toBeTruthy();
    expect(document.body.textContent).toContain('Save a command once. Run it anywhere with fg.');
    expect(document.body.textContent).toContain("Other people's snippets need a typed yes.");
  });
});
