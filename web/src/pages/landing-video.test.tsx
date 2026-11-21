import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';
import LandingPage from './LandingPage';

// The landing page's Intro section: the 60-second intro
// video and the 8-second hero loop. The contract under
// test: poster + controls + default captions on the main
// video, never autoplay-with-sound anywhere, and the loop
// degrades to the poster still under reduced motion.

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

function stubMatchMedia(matches: boolean) {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
}

afterEach(() => {
  // jsdom has no matchMedia; drop any stub between tests.
  delete (window as { matchMedia?: unknown }).matchMedia;
});

describe('LandingPage intro video', () => {
  it('offers the intro video with poster, controls and default captions, without autoplay', () => {
    const { container } = renderLanding();
    const video = container.querySelector<HTMLVideoElement>('video.observe-video');
    expect(video).not.toBeNull();
    expect(video!.getAttribute('poster')).toBe('/video/poster.png');
    expect(video!.hasAttribute('controls')).toBe(true);
    expect(video!.getAttribute('preload')).toBe('metadata');
    expect(video!.playsInline).toBe(true);
    // No autoplay at all on the main video, so never with sound.
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

  it('autoplays the hero loop muted when reduced motion is not requested', () => {
    stubMatchMedia(false);
    const { container } = renderLanding();
    const loop = container.querySelector<HTMLVideoElement>('video.observe-loop-media');
    expect(loop).not.toBeNull();
    expect(loop!.autoplay).toBe(true);
    expect(loop!.muted).toBe(true);
    expect(loop!.loop).toBe(true);
    expect(loop!.playsInline).toBe(true);
    expect(loop!.getAttribute('poster')).toBe('/video/poster.png');
  });

  it('shows the poster still instead of the loop under reduced motion', () => {
    stubMatchMedia(true);
    const { container } = renderLanding();
    expect(container.querySelector('video.observe-loop-media')).toBeNull();
    const still = container.querySelector<HTMLImageElement>('img.observe-loop-media');
    expect(still).not.toBeNull();
    expect(still!.getAttribute('src')).toBe('/video/poster.png');
    expect(still!.getAttribute('alt')).toContain('confirmation card');
  });

  it('has a skip link and the full transcript', () => {
    renderLanding();
    const skip = screen.getByRole('link', { name: 'Skip video' });
    expect(skip.getAttribute('href')).toBe('#after-intro');
    const transcript = screen.getByText('Transcript');
    expect(transcript).toBeTruthy();
    // The whole voice-over script is in the transcript,
    // including the closing line.
    expect(document.body.textContent).toContain(
      'Save a command once. Run it anywhere with fg.',
    );
    expect(document.body.textContent).toContain(
      "Other people's snippets need a typed yes.",
    );
  });
});
