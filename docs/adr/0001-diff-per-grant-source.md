# Diff per grant-source record, not per effective permission

Status: accepted

A user can hold multiple simultaneous permission records on the same repo — a Direct grant plus one or more Member grants via different groups — each potentially at a different level. We considered collapsing these into a single "effective permission" (the max across all of a Principal's records) for both storage and diffing. We decided against it: every record (Principal + repo + grant source) is stored and diffed independently.

Consequence, accepted deliberately for v1: if a user's Direct grant is untouched but they're newly added to a group that grants a higher level, this surfaces as a **Grant** of a new record, not an **Escalation** — even though the user's real-world access just went up. Catching that class of escalation is out of scope for v1; it can be added later as a derived "effective permission per Principal per repo" diff layered on top, without changing the underlying storage model.
