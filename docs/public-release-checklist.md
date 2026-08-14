# Public release checklist

The private development repository contains historical data that is not for
public release. Do not change its visibility. Create the public repository from
one reviewed snapshot with new Git history.

Before each public release:

1. Confirm that the working tree contains no private planning documents,
   customer records, personal contact details, credentials, private endpoints,
   environment-specific infrastructure IDs, databases, logs, or key material.
2. Run the full test and build commands in this repository.
3. Confirm that local `.env`, `.dev.vars`, SQLite, governance state, Wrangler
   configuration, build, cache, and dependency files are ignored.
4. Rotate or disable any dedicated service endpoint that appeared in the
   private repository history.
5. Export only the reviewed files. Never copy `.git`, ignored files, caches, or
   a full workspace archive into the public repository.
6. Create the public root commit with a role address or a GitHub `noreply`
   address. Do not copy private commit metadata.
7. Enable secret scanning and push protection on the public repository. Review
   workflow output before it becomes public.

Do not use a mirror push or a history-preserving import for this release.
