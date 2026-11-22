import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import RtlTestPage from './RtlTestPage';
import { m } from '../i18n/messages';

/**
 * RTL layout test page (Peak C9): the sample panel carries
 * dir="rtl", renders the Arabic samples and the design
 * components inside the mirrored subtree, while the page
 * chrome stays left-to-right.
 */
describe('RTL test page', () => {
  it('scopes dir=rtl to the sample panel only', () => {
    const { container } = render(<RtlTestPage />);
    const rtlRoots = container.querySelectorAll('[dir="rtl"]');
    expect(rtlRoots).toHaveLength(1);
    const panel = rtlRoots[0];
    expect(panel.getAttribute('aria-label')).toBe(m.rtlTest.panelLabel);
    // The Arabic samples and controls live inside the panel.
    expect(panel.textContent).toContain(m.rtlTest.sampleHeading);
    expect(panel.textContent).toContain(m.rtlTest.sampleBody);
    expect(panel.textContent).toContain(m.rtlTest.sampleItemOne);
    expect(panel.querySelector('md-list')).not.toBeNull();
    // The page title stays outside (LTR chrome).
    expect(screen.getByRole('heading', { name: m.rtlTest.title })).toBeTruthy();
    expect(panel.contains(screen.getByRole('heading', { name: m.rtlTest.title }))).toBe(false);
  });
});
