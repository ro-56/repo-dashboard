# Dashboard counters count grants, not people

Status: accepted

Every numeric readout on the dashboard — summary-bar totals, per-project rollups, per-repo header counts, and the collapsed card's admin/write/read distribution bar — counts `PermissionRecord` rows, and is labelled accordingly ("7 grants · 2 admin"), never "7 with access" or "7 people" as the reference prototype phrases it. The prototype could use people-language safely because its model held one permission per `(repo, username)`; ours does not.

The alternative — counting distinct Principals and banding each by their highest grant — reads more naturally but requires deciding a user's *effective* permission across several records, which is exactly the cross-record concept ADR-0001 deferred out of v1. Adopting it for display only would put a second, informal permission model on screen next to the diff engine's. Consequence, accepted: a user holding a Direct `read` and a Member `admin` on one repo contributes two units to that card's distribution bar, so the bar measures grant shape rather than headcount. The labels must always say "grants" so the number is never read as a headcount.
