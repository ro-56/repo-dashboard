# Responsive floor at 900px, shed by dropping chrome before roster columns

Status: accepted

The reference design states a fixed density budget "at 1440×900" and does not reflow. We are supporting a window down to roughly 900px wide anyway, and the order in which the layout gives way is fixed: the repo card's grants text, then its 62px distribution bar, then project meta, then the summary bar wrapping to two rows, and only last the transition column compressing from `read → write` to `→ write`. The roster row's six columns are never dropped.

The roster row is the product; the chrome exists to frame it, so the chrome pays first. The transition column can compress safely because the from-level is redundantly encoded in the row's meter and type weight (ADR-0005), and it is the *last* thing to go for that reason rather than the first. Consequence, and the reason this is written down: the grant-source column must survive to the floor even though it looks like the most droppable thing on the row — under ADR-0006 it is row identity, and without it two Member grants for the same user render as indistinguishable duplicates. A future reader trying to reclaim horizontal space should take it from the card header, not the roster.
