# Keyboard walkthrough

Every flow in the web app is completable with the keyboard alone.
This document records the tab order and keys for the five flows
that matter most, verified against the Peak A7 build. Conventions:
all interactive elements show the focus ring (or ring their
container, like the search bar); every control is reachable by Tab
and operable with Enter/Space unless noted; dialogs trap focus and
close with Esc.

## Global

- `n` - new snippet (from anywhere, when not typing in a field).
- `/` - focus the search field on the page (Library / Explore).
- `?` - open the keyboard shortcuts help dialog.
- The theme can be changed from the account menu (Tab to the
  account button, Enter, arrow keys in the menu, Enter on
  "Theme: ...") or in Settings - Appearance.

## 1. Signup

1. Tab to "Create account" on the landing page, Enter.
2. Fields in order: Username, Email, Password (with a Show
   password toggle button after the field), Create account button.
3. Inline errors are announced with the field; a failed submit
   shows one generic message (no account oracle) above the form.
4. Success lands on Home (`/home`).

## 2. Create a snippet

1. Press `n` anywhere, or Tab to "New snippet" (the FAB on
   Library/Profile, or the header button) and Enter.
2. Tab order: Name, Description, Visibility (segmented control -
   Tab to it, arrow keys move between Private / Unlisted /
   Public, the help text for the selected option is read below
   it), Add a tag field + Add button, tag chips with remove
   buttons, Script / Steps tabs, Soft wrap and Line numbers
   switches, the body editor, Version message (when editing),
   Save, Cancel.
3. In the body editor, Tab inserts spaces; press Esc then Tab to
   move focus out of the editor (documented in the Checks panel).
4. `Ctrl+S` / `Cmd+S` saves from anywhere in the form. On success
   the Save button shows a check ("Saved") briefly, then the app
   navigates to the snippet.
5. Leaving with unsaved changes opens the discard dialog (Tab
   between Keep editing and Discard; Esc stays).
6. If the snippet changed elsewhere, the conflict dialog offers
   Reload theirs / Keep editing / Save mine as new version.

## 3. Run instructions (snippet page)

1. Tab order: header actions (Edit / Star / Fork / Report / Raw),
   the Run card's copy button, then the Code / Versions / Forks
   tabs (arrow keys move between tabs).
2. The Run card shows `fg <name>` (or `fg owner/name`) as text;
   its copy button copies it and morphs to a check; a toast
   confirms ("Copied: ..."), announced via the status live region.
3. Versions tab: each version row has a checkbox (Space toggles)
   for the two-version compare; "Compare selected" activates when
   exactly two are checked and renders the diff below.

## 4. Settings - create an API token

1. Account menu (top bar) - Settings, Enter. On expanded screens
   the sections are a vertical rail (Tab to the section button,
   Enter); on medium they are tabs (arrow keys); on compact all
   sections are stacked on one page.
2. Tokens section - "New token" button opens the dialog: Name
   field, Read / Write checkboxes (Space toggles), Expiry select
   (arrow keys), Create token.
3. The one-time reveal shows the token in a code block with a
   copy button; "I've copied it" must be checked (Space) before
   Done activates. After Done, the full token is never shown
   again - only its first characters.

## 5. Device approval

1. Open `/device` (the CLI prints the address and a user code).
2. Enter the code in the field and submit (Enter). The approval
   card names the client and states exactly what approval
   grants.
3. Tab order on the card: Deny first, then Approve (Deny is
   deliberately first in tab order - approval is the considered
   action). Nothing is approved by default or by timeout.
4. The end state (approved / denied) replaces the card content
   and is announced as text.

## Library extras

- `j` / `k` move the row cursor, Enter opens the row (on expanded
  screens it opens the preview sheet instead; "Open full page" in
  the sheet is Tab-reachable).
- Bulk select: Tab to "Select", Enter; row checkboxes appear;
  the bulk bar (visibility menu, Add tag, Delete, Close) is in
  the tab order above the list.
