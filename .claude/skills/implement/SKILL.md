---
name: implement
description: "Implement a piece of work based on a spec or set of tickets."
---

Implement the work described by the user in the spec or tickets.

Use `taskr start <id>` when beginning work on a ticket, and `taskr close <id> --summary "..."` when done.

When done, if the ticket relates to another ticket, check if the other ticket is ready to be closed (no open links), and if so, close it with a summary of the work done.

Use /tdd where possible, at pre-agreed seams.

Run typechecking regularly, single test files regularly, and the full test suite once at the end.

Once done, use /code-review to review the work.

Commit your work to the current branch. Do not add Claude as a co-author for your commits.