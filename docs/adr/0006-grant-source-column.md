# Roster rows carry an explicit grant-source column

Status: accepted

The reference design's roster grid is `12px 26px 46px 148px 132px 1fr` — `± | lvl | access | user | since baseline | notes` — keyed on `(repo, username)` with exactly one permission per row. That grid predates ADR-0001 and ADR-0002: since groups are Principals and the diff is computed per grant source, the same user legitimately occupies two or three rows in one repo card (a Direct grant plus a Member grant per group), and groups occupy rows of their own. We are inserting a dedicated grant-source column (`direct`, `grp:<name>`, `—` for a group's own grant) rather than folding that information into the trailing notes column.

Grant source is row identity here, not an annotation. Two rows differing only by it would otherwise render as visually identical duplicates, which reads as a rendering bug rather than as the two distinct grants it is. Consequence: this is a deliberate, permanent departure from the prototype's column grid and its density budget — the roster is wider than the reference by one column, and anyone diffing our screen against `1a Default access.html` will find a column the prototype does not have.
