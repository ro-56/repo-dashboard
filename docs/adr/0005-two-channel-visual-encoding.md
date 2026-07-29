# Two-channel visual encoding: hue means diff state, never permission level

Status: accepted

The finalized design in `.references/standalone/1c Token sheet.html` sets out one rule the whole dashboard is built on: permission level is carried by three redundant non-hue channels — a three-cell meter (`■■■`/`■■□`/`■□□`), type weight (600/500/400), and position in the roster's admin→write→read ordering — while hue is reserved exclusively for diff state (added `#10653c`, revoked `#9e2b20`, changed `#4b46a8`, with escalation as an amber wash modifier on changed). We are adopting that rule wholesale rather than treating it as prototype styling.

This is worth recording because the obvious thing to do is the opposite, and it is what the pre-existing `src/routes/dashboard/+page.svelte` already did: colour-code `read`/`write`/`admin` directly. Under the two-channel rule that is a bug, not a style preference — a red `admin` badge competes with the red that means "revoked" and destroys the one signal an escalation audit exists to surface. Consequence: any future state that is neither a permission level nor a diff outcome (Fetch failure, Unresolvable membership) must express itself through tags, gutters or the notice strip, and may not claim a hue of its own.
