FROM debian:bookworm-slim

RUN apt-get update \
	&& apt-get install -y --no-install-recommends awscli bash ca-certificates coreutils sqlite3 zstd \
	&& rm -rf /var/lib/apt/lists/* \
	&& mkdir -p /opt/firewatch/bin /tmp/firewatch-backups

COPY deploy/backend/backup-sqlite-to-r2.sh /opt/firewatch/bin/backup-sqlite-to-r2.sh

ENTRYPOINT ["/opt/firewatch/bin/backup-sqlite-to-r2.sh"]
