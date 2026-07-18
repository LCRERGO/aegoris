# LinkedIn scraping is best-effort and opt-in

A LinkedIn profile URL can be passed as `--profile`. The CLI fetches the public
page over HTTP and `aegoris-core` extracts the `schema.org/Person` JSON-LD block
and OpenGraph meta tags. It is one of several profile inputs alongside canonical
JSON, plain text, and the official data-export `.zip`.

Scraping is deliberately limited: it reads server-rendered structured data and
does not execute JavaScript or authenticate. LinkedIn rate-limits and blocks
automated requests (HTTP 999) and frequently redirects to a login wall, so the
adapter fails with a clear error rather than pretending to succeed.

The alternative was to refuse scraping entirely for ToS and reliability
reasons. We kept it because a public URL is by far the lowest-friction way to
try the tool, while making the official export the documented reliable path and
keeping the parser pure and offline-testable. The trade-off is that this input
can break without warning as LinkedIn changes its markup.
