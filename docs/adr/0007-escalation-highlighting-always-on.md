# Escalation highlighting is always on, with no toggle

Status: accepted

PRD §5.4 specifies escalation highlighting as "a togglable visual emphasis," and PD-13 was written to that spec. The finalized design contradicts it: the prototype exposes `highlightEscalations` as a component prop defaulting to true, with no control anywhere in its toolbar, and the token sheet justifies the amber wash (`#fdf2e4`) as "the only tinted row background on the screen" — the property that makes escalations findable by eye at any zoom with no filter applied. We are following the design: escalations always carry the wash, the `↑` glyph and the word tag, and always roll up as `↑n esc` on card and project headers.

A toggle would make the product's single most important signal switchable off, which is the opposite of what an escalation audit wants, and it would consume space in a 31px filter bar the design never budgeted for it. Consequence: PRD §5.4's "togglable" wording is superseded and PD-13 no longer describes work we intend to do. If per-row emphasis ever needs to be suppressed, the route is a state-chip *filter* over escalations, not a switch that removes their styling.
