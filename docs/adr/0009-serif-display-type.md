# ADR 0009 - Drop the serif display face; Roboto Flex carries display type

- **Status:** Accepted (Peak WP A0, 2026-11-22)
- **Context:** Since 2026-11-20 the landing page headline has used Instrument Serif (self-hosted), a user-directed exception to the Material You system, while the signed-in app used Roboto Flex after the owner clarified the serif belongs to the landing only. The Peak plan (Phase A) rebuilds the visual system as faithful Material You 3 and asks for a decision on the serif, recommending it be dropped from the app and at most retained as one display style on the landing hero - in Roboto Flex.

## Decision

The serif display face is dropped everywhere, including the landing hero.

- All display, headline and title styles use Roboto Flex (variable weight carries the expression the serif provided).
- Roboto Mono remains the code face. No third typeface is introduced.
- The Instrument Serif font files (`web/public/fonts/instrument-serif*.woff2`) and the landing's serif styles are removed in A5 when the landing is rebuilt; A1 removes the theme scope that referenced them. Until A5 lands, the files are inert assets of the old landing.

## Why

- **One system, honestly applied.** Material 3 type is a single-family scale. A serif hero over an otherwise strict M3 product reads as a second brand, and the owner has said the current look is not satisfying; the Peak brief is to rethink, not to preserve exceptions.
- **The exception already failed once.** The serif leaked from the landing into the app in 2026-11-20's work and had to be reverted the same day (commits `c0204b9`, `6ce8d8a`). A landing-only exception is a recurring accident, not a stable rule.
- **Cost.** Two extra font files (43 KB) preload on the landing for one headline style; Roboto Flex is already loaded for the whole product and its weight axis covers the display range.
- **Reversibility.** This is a style decision, not data or API. If the owner later wants a serif hero back, it is one token and one font file away; nothing in A1-A8 will hard-code against it.

## Alternatives rejected

- **Keep Instrument Serif on the landing hero only (the plan's fallback).** Rejected for the leak risk and second-brand problem above; it also forces A7's contrast and A8's visual suites to carry a one-route type exception forever.
- **Replace the serif with a different display face (Fraunces, Playfair, Space Grotesk were trialled in 2026-11-20 and dropped).** Same problems, plus a new self-hosting and licensing review for no functional gain.
