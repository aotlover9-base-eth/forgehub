# ForgeHub intro video - storyboard

60 seconds, 6 scenes x 10 s, 1920x1080, 30 fps, no
voice-over - a synthesized sound-effects track instead
(cues in script.md; the voice-over script lives there
too, and the same text runs as captions). One line:
**Save a command once. Run it anywhere with fg.**

Visual language: the product's current Observe theme -
dark monochrome glass over the smoke backdrop, Instrument
Serif for the wordmark, Roboto Flex for captions,
Roboto Mono for terminals. Every terminal text and every
app screenshot is a real capture (see generation-log.md);
nothing is AI-generated.

Demo user: **alice**. A second account, bob, appears only
as the owner of someone else's snippet in scenes 3-4 -
the trust prompts need a real second owner.

| # | Time | Scene | What you see | Lower-third |
|---|------|-------|--------------|-------------|
| 1 | 0-10 s | HOOK | A terminal types the long apt setup command and runs it; labels fade in around it: notes.txt, chat, ~/.bash_history, old gist | "You've typed this before." then "Scattered across notes, chats and shell history." |
| 2 | 10-20 s | ANSWER | ForgeHub wordmark + tagline; the real web editor with ubuntu-setup filled in (Private selected, tags, Checks panel: Valid, Uses sudo, size) ringed; Save; the saved snippet page with its v1 chip and the "ubuntu-setup saved" toast | "Text only. Versioned. Private by default." |
| 3 | 20-30 s | RUN ANYWHERE | `forgehub login` prints the device code; the real /device page card with Approve ringed; "Logged in as alice"; `fg ubuntu-setup`; the real confirmation card held readable for 3 s (name, owner, version, sha256, private, mode, sudo warning, the numbered command, [y/N]); y typed; real apt output scrolls; the real summary line "exit code 0 · 1 executed, 0 skipped · 10.2s" | "fg ubuntu-setup" then "You see exactly what will run. Then you decide." |
| 4 | 30-40 s | SAFETY | Three terminal panels: (1) bob's snippet - the prompt takes the full typed word `yes`; (2) the same snippet after bob changed it - "CHANGED since you last ran it", the real unified diff with -/+ rows, answered `no`, "not run (declined)"; (3) the snippet piped through `tee` - the real no-terminal refusal (what / why / fix) and an "exit code 2" chip | "Other people's snippets need a typed yes." / "Changed since last time? You see the diff." / "No terminal? It won't run." |
| 5 | 40-50 s | SHARE / DISCOVER | The real Explore page searched for "docker" (cards, tags, stars); the real Versions compare view with the v1/v2 diff; terminal: `forgehub record deploy` captures two commands, `forgehub push deploy` prints "alice/deploy v1 sha256:bd02407b" | "Private, unlisted or public." then "Record. Push. Pull. Just like git." |
| 6 | 50-60 s | CLOSE | Wordmark + tagline centre; filled button "Create your account", text button "Install the CLI"; a mono card with the README's real install path for this private repo: `gh release download v0.1.6` and `sha256sum -c SHA256SUMS`, blinking cursor; "Linux-only - on other systems, use the web app in a browser."; fade out over the last 12 frames | (captions carry the close) |

## Hero loop (8 s, muted, seamless)

The scene-3 terminal on its own: `fg ubuntu-setup` types
in, the confirmation card fades in and holds with the
cursor blinking at [y/N] (1 s blink period - it divides
8 s exactly), then the card fades out and the loop
restarts. The landing page autoplays it only when the
visitor has not asked for reduced motion; otherwise the
poster is shown.

## Poster

poster.png is the scene-3 frame at t = 6.8 s: the full
confirmation card on screen under the login block - the
product's whole promise in one frame.
