// Package migrations embeds the SQL migration files into the binary so the
// service is self-contained (no files to ship alongside it).
package migrations

import "embed"

//go:embed *.sql
var FS embed.FS
