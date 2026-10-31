# Cosmetic by construction: time, input and schedules

Moved from the README on 2026-10-31, unchanged, when the README was brought to the [org documentation guide](https://github.com/bevy-coven/.github/blob/main/DOCUMENTATION.md).

Everything runs on `Time<Real>`, so balloons still expire and the typewriter still types while the host has paused virtual time. The conversation runtime runs in `PreUpdate` after Bevy's input systems, in the public `DialogInputSet`, and consumes the key and click presses it uses (`clear_just_pressed`): order your own `PreUpdate` systems `.after(DialogInputSet)` and a Space that ended a line is no longer `just_pressed` for them. The cosmetic systems (typewriter, caret, balloon tracking and expiry) stay in `Update`. The typewriter reads `DialogSkin::chars_per_second` every tick, so a settings menu can change the speed of the line on screen.
