import { useEffect, useState } from 'react';

/** Material 3 window size classes (design-spec §4). */
export type WidthClass = 'compact' | 'medium' | 'expanded';

export function widthClassFor(width: number): WidthClass {
  if (width < 600) return 'compact';
  if (width < 840) return 'medium';
  return 'expanded';
}

/** Track the current width class, updating on resize. */
export function useWidthClass(): WidthClass {
  const [widthClass, setWidthClass] = useState<WidthClass>(() =>
    widthClassFor(window.innerWidth),
  );
  useEffect(() => {
    const onResize = () => setWidthClass(widthClassFor(window.innerWidth));
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);
  return widthClass;
}
