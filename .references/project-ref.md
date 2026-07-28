**repo-dashboard** — an internal access-auditing tool for repository permissions.

**What it does.** A job authenticates against your repo host and fetches, for a list of repositories, every user's permission level. Each record is `username · repo_project · repo · permission (read | write | admin)`. Every execution is stored as a versioned snapshot with a timestamp, so the tool accumulates a history rather than just a current picture.

**What the dashboard is for.** Two questions, on one screen:
- Who has access to each repository right now, and at what level?
- What changed between any two runs — grants added, revoked, or moved to a different level?

That second question was the original driver; the roster view got promoted to equal footing later, which is what pushed the design toward repository-centric cards rather than a change feed.

**Where it stands.** You have working HTML prototypes, built as visual references rather than production code — standalone files, mock dataset of 7 runs across 11 repos and ~30 principals, diff computed at runtime. Four information architectures were built and compared (unified log, side-by-side panes, matrix grid, grouped cards); you kept **grouped by repository**: project sections → repo cards, where a collapsed card summarises access distribution and change counts, and expanding gives the full member roster with the diff marked in place.

**Not built yet** — deliberately out of prototype scope: credential handling and auth storage, the fetch/run trigger, and the execution-history page.
