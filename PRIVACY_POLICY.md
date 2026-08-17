# gcrdings Privacy Policy

*Last updated: [Current Date]*

## Our Privacy-First Commitment

gcrdings is built on the principle that your recordings and the verifiable records derived from them
should remain private and under your control. This privacy policy explains how gcrdings handles data.

## Data Processing Philosophy

### Local-First Processing
- **Transcription**: Processed entirely on your device using local Whisper or Parakeet models
- **Audio recordings**: Never transmitted to external servers
- **Verifiable records**: Remain on your infrastructure
- **AI summaries**: Generated locally or through your chosen LLM provider

### Your Data Ownership
- You own all recordings, transcripts, and derived verifiable records
- Data is stored locally on your device
- No vendor lock-in - export your data anytime
- Complete control over data retention and deletion

## Usage Analytics

### What We Collect
Usage analytics is optional and off by default, and gcrdings ships with **no telemetry credential
configured** — the analytics client cannot send a single event until a build or deployment explicitly
supplies its own analytics API key. If you or your organization configure one, gcrdings collects
minimal, anonymized usage data:

**Application Usage:**
- Feature usage patterns (which tools you use most)
- Session duration and frequency
- Performance metrics (transcription success rates, error frequencies)
- UI interaction patterns (button clicks, navigation flows)

**Technical Metrics:**
- Application version and platform information
- Error logs and crash reports (anonymized)
- Performance benchmarks (processing times, resource usage)

### What We DON'T Collect
We never collect:
- ❌ Recording content, transcripts, or verifiable records
- ❌ Personal information or identifiable data
- ❌ File names, session titles, or metadata
- ❌ Audio data or voice patterns
- ❌ Participant names or contact information
- ❌ LLM conversations or AI-generated content

### Why We Collect This Data
When configured and enabled, analytics helps with:
- **Product Quality**: Identifying and fixing bugs that impact user experience
- **Performance Optimization**: Understanding resource usage and system bottlenecks
- **Security**: Detecting potential security issues and vulnerabilities
- **Feature Development**: Making data-driven decisions about new features

### Analytics Implementation
- **Provider**: PostHog (privacy-focused analytics platform), only if a build supplies its own API key
- **Default**: No telemetry credential ships with gcrdings; analytics is a no-op until one is configured
  and you enable it in settings
- **Anonymization**: All data linked to generated user IDs only - no personal identification
- **Data retention**: Governed by whichever PostHog project the configuring deployment controls
- **Encryption**: All data encrypted in transit using industry-standard protocols

## Third-Party Services

### LLM Providers (Optional)
If you choose to use external LLM providers:
- **Anthropic Claude**: Subject to Anthropic's privacy policy
- **Groq**: Subject to Groq's privacy policy
- **Local Ollama**: Processed entirely on your device

### Analytics Service (Optional)
- **PostHog**: Only reachable if a build configures its own API key; ships disabled with no key by default
- **Data**: Only anonymized usage patterns, no recording content
- **Control**: Completely optional, off by default, and user-controlled

## Your Privacy Rights

### Data Control
- **Access**: View all data stored locally on your device
- **Export**: Export your data in standard formats
- **Delete**: Remove all data from your device

### Analytics Transparency
- **Open source**: Full analytics implementation available for review in the source code
  (`frontend/src-tauri/src/analytics/`)
- **Opt-in**: New and existing installs have analytics disabled until you turn it on, and it stays a
  no-op until an API key is configured
- **Questions**: Contact the maintainers for any analytics-related concerns

## Data Security

### Local Security
- Local files rely on macOS file permissions and disk protection; enable FileVault for encryption at rest
- Recording audio is not sent to external summary providers
- Standard file system permissions protect your data

### Open Source Transparency
- Full source code available for security review
- No hidden data collection or tracking

## Changes to This Policy

We will notify users of any material changes to this privacy policy through:
- Updates to this document in the project repository
- Release notes for application updates
- In-app notifications for significant privacy changes

## Open Source Commitment

As an open-source project under the MIT license (see [`LICENSE.md`](LICENSE.md) and
[`PROVENANCE.md`](PROVENANCE.md)), you can:
- Review the complete privacy implementation
- Modify data handling to meet your requirements
- Deploy entirely on your own infrastructure
- Contribute to privacy improvements

---

*This privacy policy applies to gcrdings. For enterprise deployments, additional privacy controls may
be available.*
