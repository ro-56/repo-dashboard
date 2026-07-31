# Permission editing is a live write-back capability, scoped to edit/remove of existing grants only

repo-dashboard adds the ability to change or remove a Direct or Group grant's permission level directly against Bitbucket (`PUT`/`DELETE` on `permissions-config/users` and `permissions-config/groups`, at Repo or Project scope) — moving the tool from pure read-only audit to also mutating live production access. Scope is deliberately narrow:

- Only existing Direct and Group grants can be edited or removed. Member grants get no edit menu at all — a member's access is derived from their group's own grant, and Bitbucket has no endpoint to set an individual member's permission independent of that group.
- Adding a brand-new grant for a principal with currently zero access is out of scope (a future ticket, not this feature).
- The level picker stays limited to Read/Write/Admin, matching the existing `PermissionRecord` model, even though Bitbucket's project-level API also accepts `none`/`create-repo` — modeling those would require a parallel non-diffable enum this tool has never needed.

## Consequences

A future "add a new grant" feature and a future "model create-repo" feature are natural follow-ups; this decision doesn't preclude either, it just keeps this feature's surface area to what the reference design (`.references/new-design/`) actually shows.
