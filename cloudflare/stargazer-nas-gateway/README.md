# Private gateway Worker

This Worker gives the Stargazer frontend read-only access to selected archive
API routes through Cloudflare Workers VPC.

Copy `wrangler.example.jsonc` to the ignored `wrangler.jsonc` file. Replace the
all-zero `tunnel_id` with the Cloudflare Tunnel ID for the target environment.
Do not commit the environment-specific file.

Keep credentials and API tokens outside both files. Use Cloudflare secrets for
credentials and `.dev.vars` for local-only values.
