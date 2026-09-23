# Security

Please report security issues through a private GitHub security advisory when available. Do not put
passwords, access tokens, full logs containing credentials, or private server addresses in a public
issue.

The Windows installer is not code-signed yet. Verify release downloads using the published
`SHA256SUMS.txt` before running them.

Server secondary-login passwords are stored locally by the embedded WebView and are not encrypted.
Use a dedicated, non-valuable password.
