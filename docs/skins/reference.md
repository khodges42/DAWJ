# DAWJ skin surface v1 reference

Package keys are validated in `skin-format.mjs`; product minimums are enforced by
`skin-safety.css` and the rendering audit. Selectors listed here are stable for
surface version 1. Breaking changes require a surface-version change.

## Semantic variables

Colors use opaque `#RRGGBB`. Package JSON uses the left column; CSS uses the right.

| JSON variable | CSS variable | Purpose |
| --- | --- | --- |
| background | --color-background | Window background |
| surface | --color-surface | Deck/card/inspector surfaces |
| raised | --color-raised | Control backgrounds |
| primary | --color-primary | Active controls and links |
| secondary | --color-secondary | Secondary emphasis |
| tertiary | --color-tertiary | Additional palette emphasis |
| text | --color-text | Critical readable text |
| muted | --color-muted | Supporting captions |
| deckA | --color-deck-a | Deck A and timeline accent |
| deckB | --color-deck-b | Deck B and timeline accent |
| onPrimary | --color-on-primary | Text on timeline accent blocks |
| border | --color-border | Control/panel borders |
| focus | --color-focus | Protected keyboard focus outline |
| meterActive | --color-meter-active | Diagnostic meter normal range |
| meterWarn | --color-meter-warn | Diagnostic meter warning range |
| meterClip | --color-meter-clip | Diagnostic meter upper range/errors |
| grid | --color-grid | Canvas beat/bar grid |

| JSON variable | CSS variable | Default / supported range |
| --- | --- | --- |
| fontFamily | --font-family | system-ui, sans-serif; local families below |
| fontSize | --font-size | 14; 12–18 px |
| spacing | --spacing | 8; 4–12 px |
| controlHeight | --control-height | 32; 32–44 px |
| controlRadius | --control-radius | 4; 0–12 px |

Allowed local families: system-ui, sans-serif, serif, monospace, ui-sans-serif,
ui-monospace, Segoe UI, Arial, Verdana, Tahoma, Trebuchet MS, Georgia, Times New
Roman, Courier New, Inter, Roboto, Noto Sans and DejaVu Sans. Include at least one
generic/system fallback. Font files are not imported. The installed fonts on
the receiving machine determine which family is available.

Application-owned text scale is 90–125%. Critical buttons remain at least
28×32 px; range controls at least 28×24 px. Critical control text is at least
12 px and lifecycle text at least 11 px. Contrast validation, enhanced contrast
and visible focus apply independently of decoration. Reduced motion disables
animation/transitions. Meter colors represent diagnostics, not physical cue or
successful command execution.

## Stable selectors

| Selector | Component |
| --- | --- |
| #decks, #deck-0, #deck-1, .deck | Two-deck surface; A/B |
| .slots, article | Eight fixed slots per deck and each slot card |
| .identity, .state, .actions, .context | Part name/select, lifecycle, action buttons, contextual knob |
| [data-active=true], [data-selected=true] | Authoritative playback and selected-part presentation |
| [aria-pressed=true] | Active/toggled control presentation |
| #transport, #clock, #modes, #mode-status | Clock, quantization and control modes |
| #phrase-panel, #timeline, #phrase-position | Phrase overview and Canvas |
| #inspector, #selected-name, #inspection, #rule-status | Selected part and rules |
| footer, .meters, #cue-status | Persistent mixer, meters and monitoring notice |
| #notices, #notice, #midi-badge, #error | Application feedback |
| #settings, #help, .bar | Settings/help dialogs and control rows |
| #mapping-list, .mapping-row, #profile-status, #midi-status | MIDI configuration |
| #led-status, #led-list, #led-values, .led-row | LED configuration/status |
| .truncate | Supporting text with full information in existing titles/help |

Allowed tags: body, header, footer, main, section, article, button, input, select,
option, label, h1, h2, small, p, div, span, output, meter, canvas, dialog, fieldset,
legend, details and summary. Descendant and `>`, `+`, `~` combinations and comma
lists are supported. Supported pseudo selectors: hover, focus-visible, focus,
disabled and active. Attribute selectors are limited to data-active, data-selected
and aria-pressed with true/false values. State-specific rules cannot change layout.
Internal skin-management controls, native menus, nonces and preview UI are not
authoring selectors. The preview confirmation strip uses a closed shadow root.

## Permitted component declarations

Paint: color, background-color/image/repeat/size/position, border-color/width/style/
radius and individual edge colors, outline-color. Color values must be six-digit
hex or documented semantic color variables. Background images must use packaged
`asset:path.png` references. Essential controls and state labels retain image-free
backgrounds; images decorate surrounding panels. No box-shadow or effects outside component bounds.

Typography: font-size, font-weight, line-height, letter-spacing, text-align.
Protected critical text sizes/colors override unsafe component choices. Fonts
are selected through the fontFamily variable, not CSS imports or font-face.

Layout: padding/margin and individual edges, gap/row-gap/column-gap, display,
grid-template-columns/rows, align-items, justify-content, width/height and
min/max-width/height. Display values are block, inline-block, flex, inline-flex
and grid, limited to .deck, .slots, .actions, .bar, .mapping-row, .led-row,
#decks, #inspector, #phrase-panel and article. Protected deck columns/card rows
take precedence. Global body/state selectors permit paint changes only.

Any undeclared property, selector, at-rule, `!important`, escape or external
resource is rejected. The browser still interprets ordinary CSS value semantics;
use valid values for the declared property. Test narrow/tall windows and text
scaling before keeping a layout. Below 1180 logical pixels, the default puts
slots in two columns per deck; both decks remain side by side and vertical
scrolling is available. Larger text can also increase vertical scrolling.
