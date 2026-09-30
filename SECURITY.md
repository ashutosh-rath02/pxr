# Security

This experimental release is not a secure network control service. Callers must
authenticate principals and authorize lease grants before entering the runtime.
No network listener is included. CRC and owner IDs do not provide authentication,
cryptographic integrity, durable anti-replay or attestation.

Supported version for fixes: the latest `0.1.x` release. Read SAFETY_MODEL.md for the
platform trust assumptions and failure behavior.

Report a suspected vulnerability using GitHub's private vulnerability reporting
on this repository when available. If it is unavailable, open an issue requesting
a private reporting channel without including exploit details or sensitive data.
