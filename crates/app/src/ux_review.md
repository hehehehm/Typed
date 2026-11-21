## Expert review

Read from the headless PNG shots of every screen (light/dark, Win/Mac preview, min width, empty and many-item edge cases). This is a qualitative pass on placement, size and usage logic, separate from the automated heuristics above. The UI was not changed in this pass except the Exceptions chip row.

### Must fix

- **Muted counts on dark home cards fail WCAG AA.** The «3» / «9» badges on the dusk tiles sit on the card fill at about 4:1 (`/tmp/typed-ui-shots/01-home-dark.png`, highlight `/tmp/typed-ui-ux-issues/home-dark-issues.png`). Use ink, or a frost plate like the captions already have.

- **First-letter app icons fail contrast.** «C», «T», «N» on the beige marks are around 3.4:1 (`/tmp/typed-ui-shots/02-apps-long-path.png`, `/tmp/typed-ui-ux-issues/apps-edge-issues.png`). The mark is the only colour cue besides the name; it needs ink-level contrast.

- **Delete on a program row is next to EN/RU, with no confirm.** The trash icon sits in the same cluster as the layout segment (`/tmp/typed-ui-shots/02-apps-long-path.png`, highlight `/tmp/typed-ui-ux-issues/apps-light-issues.png`). One slip removes the assignment. Separate delete (a «…» menu, a confirm, or at least a gap/divider) so the destructive target is not beside the primary action.

### Should fix

- **Header still says «Программы» on the Exceptions pane.** After switching to Исключения the sea title does not follow, so the header and the segment disagree (`/tmp/typed-ui-shots/02-exceptions.png`, `/tmp/typed-ui-shots/02-exceptions-dark.png`). Change the header to «Исключения», or use a single product title and let the segment be the only pane label.

- **Header nav dots are 24 px wide.** The five marks are the only jump-to-section control besides Back, but they are below the 32 px primary target (`/tmp/typed-ui-shots/07-settings.png`). Use the same 32 px hit as the back button.

- **Changing the undo key is a deep, hidden path.** From Home: Settings → the Backspace keycap → «Назначить…» in a popup → a key press (3 clicks + a key). The popup is easy to miss (`/tmp/typed-ui-shots/07-settings.png`). A direct «press a new key» affordance on the row would shorten this.

- **Empty + does nothing visible.** Submitting a blank program or exception field leaves the screen unchanged (`/tmp/typed-ui-shots/10-exceptions-empty.png`, `/tmp/typed-ui-ux-issues/apps-empty.png`). A short inline error («Введите имя или путь») would close the loop.

- **Back button hover at min width is unreliable.** At 432 px the harness does not always see hover on the ‹ control, which suggests the title plate is tight (`/tmp/typed-ui-ux-issues/apps-min-issues.png`). Keep a clear 32 px back target that the title frost does not cover.

### Nice to have

- **Welcome «Не показывать» is already a switch, but mirrored.** The track sits to the left of the label; Settings rows put the label on the left and the switch on the right (`/tmp/typed-ui-shots/11-welcome.png` vs `/tmp/typed-ui-shots/07-settings.png`). Flip it to match.

- **«Программы» is repeated** in the header and in the Programs | Exceptions segment. A quieter header (just the mark, or the product name) would let the segment speak.

- **Empty Exceptions is a blank field and nothing else.** A one-line hint under the add row («Typed здесь полностью выключен») would explain the pane when there are no chips (`/tmp/typed-ui-shots/10-exceptions-empty.png`).

- **Dictionary group counts are borderline AA** (the «9» next to «Свои» was measured at 4.45: `/tmp/typed-ui-ux-issues/words-issues.png`). A slightly darker muted on the page background would clear the bar.

- **Chip × is a small delete.** It matches the dictionary chips and is the right pattern for a wrap row (`/tmp/typed-ui-shots/02-exceptions.png`), but the hit is closer to 24 px than 32. Growing the hit (keeping the glyph small) would help touch use.

- **Many exception chips rely on scroll** at the default 496 px height (`/tmp/typed-ui-shots/10-exceptions-many.png`). The section already scrolls; growing the window slightly, or a fade at the fold, would make the continuation obvious.

- **The undo-key popup could list the current key more obviously** and offer «Назначить» as a full-width row rather than a text link, so the assign path is the same kind of control as the presets.

The new Exceptions chip row itself reads well: add field stays on top, names wrap in lines, long basenames sit on their own row, light and dark both stay clear, and min-width wraps `windowsterminal` onto the next line (`/tmp/typed-ui-shots/10-exceptions-min-width.png`).

