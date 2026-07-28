# Groups are Principals, independent of member resolution

Status: accepted

`main.py` only ever recorded a group's grant as a prefix on its flattened member rows, and treated "no members returned" the same as "empty group" — but those are different things: sometimes the credential simply lacks the workspace-level scope to list a group's members, even though the group's own repo permission is fetchable. Losing that fact (a group holds admin/write/read on a repo) at exactly the moment we can't see its members would hide the information that matters most.

Decision: a group is a Principal in its own right. Its own grant is recorded and diffed the same way a user's Direct grant is, regardless of whether membership is resolvable. When membership *is* resolvable, each member additionally gets their own Member grant record layered on top — so a user can appear both as themselves (via a Member grant) and implicitly via the group's own row.

This reverses the project's original framing of group membership as pure non-diffable metadata: the group's own grant level is now a first-class diffable Principal, same as a user. What remains out of scope is diffing *membership itself* (who belongs to the group) as its own dimension — that's still not tracked; only the group's grant and (when known) its resolved members are.
