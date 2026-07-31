# Permission edits are staged locally and applied as a reviewed global batch

Clicking a level option or "Remove access" in a row's menu does not call Bitbucket immediately — it stages a Pending edit in local state. Pending edits accumulate globally across every repo/project in the roster tree (not scoped per repo card, matching the reference design's flat pending-state shape), and only reach Bitbucket when the user clicks a single global Apply action. Apply shows a confirm dialog listing every staged change — including cascade counts for Group removals, since removing a Group grant also drops every Member grant derived from it — before firing anything. This is the safety checkpoint for a tool that now mutates live, production Bitbucket permissions from a batch of possibly-unrelated edits.

## Consequences

- Editing is only enabled while the Comparison is the latest Snapshot — editing against an older historical view would mean mutating the present based on a look at the past.
- If a new Run completes while edits are staged, the Pending edits are discarded rather than carried forward, since they were staged against a Snapshot that is no longer latest.
- Applying a batch does not auto-trigger a new Run; the user is prompted to run again to see changes reflected, since Snapshots are immutable and never self-update (extends the immutability rule in the `Snapshot` glossary entry).
- Within a batch, each Pending edit is applied independently: one failing (insufficient scope, a 404 from a grant already changed elsewhere, rate limiting) does not block the rest. Failed items stay staged for retry; results are reported per item after the batch finishes.
