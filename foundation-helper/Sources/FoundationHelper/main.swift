import Foundation
import FoundationModels

private let schemaVersion = 1
private let maximumRequestBytes = 8_000_000

struct HelperParticipant: Codable, Sendable {
    let id: String
    let displayName: String
    let speakerClusterId: String?

    enum CodingKeys: String, CodingKey {
        case id
        case displayName = "display_name"
        case speakerClusterId = "speaker_cluster_id"
    }
}

struct HelperBlock: Codable, Sendable {
    let id: String
    let text: String
    let participantId: String
    let startMs: Int
    let endMs: Int

    enum CodingKeys: String, CodingKey {
        case id, text
        case participantId = "participant_id"
        case startMs = "start_ms"
        case endMs = "end_ms"
    }
}

struct HelperRequest: Codable, Sendable {
    let schemaVersion: Int
    let requestId: String
    let recordType: String
    let principalTranscriptRevision: Int
    let participants: [HelperParticipant]
    let transcript: [HelperBlock]
    let promptVersion: String

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case requestId = "request_id"
        case recordType = "record_type"
        case principalTranscriptRevision = "principal_transcript_revision"
        case participants, transcript
        case promptVersion = "prompt_version"
    }
}

struct HelperEvidence: Codable, Sendable {
    let blockId: String
    let timestampMs: Int

    enum CodingKeys: String, CodingKey {
        case blockId = "block_id"
        case timestampMs = "timestamp_ms"
    }
}

struct HelperFinding: Codable, Sendable {
    let label: String
    let detail: String
    let evidence: [HelperEvidence]
}

enum HelperResult: Encodable, Sendable {
    case meeting(summary: String, keyPoints: [HelperFinding])
    case interview(summary: String, themes: [HelperFinding])
    case content(summary: String, claims: [HelperFinding])

    private enum CodingKeys: String, CodingKey {
        case recordType = "record_type", summary, decisions
        case actionItems = "action_items"
        case keyPoints = "key_points"
        case answers, themes
        case followUps = "follow_ups"
        case claims, outline
        case sourceNotes = "source_notes"
    }

    func encode(to encoder: Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .meeting(summary, keyPoints):
            try values.encode("meeting", forKey: .recordType)
            try values.encode(summary, forKey: .summary)
            try values.encode([HelperFinding](), forKey: .decisions)
            try values.encode([HelperFinding](), forKey: .actionItems)
            try values.encode(keyPoints, forKey: .keyPoints)
        case let .interview(summary, themes):
            try values.encode("interview", forKey: .recordType)
            try values.encode(summary, forKey: .summary)
            try values.encode([HelperFinding](), forKey: .answers)
            try values.encode(themes, forKey: .themes)
            try values.encode([HelperFinding](), forKey: .followUps)
        case let .content(summary, claims):
            try values.encode("content", forKey: .recordType)
            try values.encode(summary, forKey: .summary)
            try values.encode(claims, forKey: .claims)
            try values.encode([HelperFinding](), forKey: .outline)
            try values.encode([HelperFinding](), forKey: .sourceNotes)
        }
    }
}

struct HelperResponse: Encodable, Sendable {
    let status: String
    let schemaVersion: Int
    let requestId: String
    let principalTranscriptRevision: Int?
    let result: HelperResult?
    let reason: String?
    let code: String?

    enum CodingKeys: String, CodingKey {
        case status
        case schemaVersion = "schema_version"
        case requestId = "request_id"
        case principalTranscriptRevision = "principal_transcript_revision"
        case result, reason, code
    }
}

@available(macOS 26.0, *)
@Generable
private struct ModelEvidence {
    @Guide(description: "Copy only the exact value after BLOCK_ID, without times or other text")
    var blockId: String
    @Guide(description: "Playable millisecond inside that block", .range(0...86_400_000))
    var timestampMs: Int
}

@available(macOS 26.0, *)
@Generable
private struct ModelFinding {
    @Guide(description: "Short type-specific label")
    var label: String
    @Guide(description: "Grounded finding, without speculation")
    var detail: String
    @Guide(description: "One or more exact transcript references", .minimumCount(1), .maximumCount(8))
    var evidence: [ModelEvidence]
}

@available(macOS 26.0, *)
@Generable
private struct ModelResult {
    @Guide(description: "Concise summary grounded only in the supplied transcript")
    var summary: String
    @Guide(description: "Type-specific grounded findings", .minimumCount(1), .maximumCount(24))
    var findings: [ModelFinding]
}

enum HelperCore {
    private static let allowedKeys: Set<String> = [
        "schema_version", "request_id", "record_type", "principal_transcript_revision",
        "participants", "transcript", "prompt_version",
    ]

    static func decode(_ line: String) throws -> HelperRequest {
        guard let data = line.data(using: .utf8), data.count <= maximumRequestBytes,
              let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              Set(object.keys).isSubset(of: allowedKeys) else {
            throw HelperFailure.invalidRequest
        }
        let request = try JSONDecoder().decode(HelperRequest.self, from: data)
        guard request.schemaVersion == schemaVersion,
              !request.requestId.isEmpty,
              ["meeting", "interview", "content"].contains(request.recordType),
              request.principalTranscriptRevision >= 0,
              !request.transcript.isEmpty,
              request.transcript.allSatisfy({ !$0.id.isEmpty && $0.startMs >= 0 && $0.endMs >= $0.startMs }) else {
            throw HelperFailure.invalidRequest
        }
        return request
    }

    static func prompt(for request: HelperRequest) -> String {
        let participantNames = Dictionary(uniqueKeysWithValues: request.participants.map { ($0.id, $0.displayName) })
        return request.transcript.map { block in
            """
            BLOCK_ID: \(block.id)
            START_MS: \(block.startMs)
            END_MS: \(block.endMs)
            PARTICIPANT: \(participantNames[block.participantId] ?? "Unknown participant")
            TEXT: \(block.text)
            """
        }.joined(separator: "\n---\n")
    }

    static func handle(_ line: String) async -> HelperResponse {
        let request: HelperRequest
        do { request = try decode(line) }
        catch { return failed(requestId: "invalid", code: "invalid_request") }

        guard #available(macOS 26.0, *) else {
            return unavailable(requestId: request.requestId, reason: "unsupported_os")
        }
        let model = SystemLanguageModel.default
        switch model.availability {
        case .unavailable(.deviceNotEligible):
            return unavailable(requestId: request.requestId, reason: "ineligible_device")
        case .unavailable(.appleIntelligenceNotEnabled):
            return unavailable(requestId: request.requestId, reason: "apple_intelligence_disabled")
        case .unavailable(.modelNotReady):
            return unavailable(requestId: request.requestId, reason: "model_not_ready")
        case .available:
            break
        @unknown default:
            return unavailable(requestId: request.requestId, reason: "model_not_ready")
        }
        guard model.supportsLocale() else {
            return unavailable(requestId: request.requestId, reason: "unsupported_locale")
        }

        let instructions = "Generate \(request.recordType) findings. Use only the supplied principal transcript. For evidence.blockId, copy only the exact BLOCK_ID value. Never include timestamps or other text in blockId. Set timestampMs inside that block's START_MS and END_MS range."
        do {
            let session = LanguageModelSession(model: model, instructions: instructions)
            let response = try await session.respond(to: prompt(for: request), generating: ModelResult.self)
            let findings = response.content.findings.map { finding in
                    HelperFinding(label: finding.label, detail: finding.detail, evidence: finding.evidence.map { HelperEvidence(blockId: $0.blockId, timestampMs: $0.timestampMs) })
                }
            let result: HelperResult = switch request.recordType {
            case "meeting": .meeting(summary: response.content.summary, keyPoints: findings)
            case "interview": .interview(summary: response.content.summary, themes: findings)
            default: .content(summary: response.content.summary, claims: findings)
            }
            return HelperResponse(status: "completed", schemaVersion: schemaVersion, requestId: request.requestId, principalTranscriptRevision: request.principalTranscriptRevision, result: result, reason: nil, code: nil)
        } catch LanguageModelSession.GenerationError.exceededContextWindowSize {
            return failed(requestId: request.requestId, code: "context_size")
        } catch is CancellationError {
            return failed(requestId: request.requestId, code: "cancelled")
        } catch {
            return failed(requestId: request.requestId, code: "generation_failed")
        }
    }

    private static func unavailable(requestId: String, reason: String) -> HelperResponse {
        HelperResponse(status: "unavailable", schemaVersion: schemaVersion, requestId: requestId, principalTranscriptRevision: nil, result: nil, reason: reason, code: nil)
    }

    private static func failed(requestId: String, code: String) -> HelperResponse {
        HelperResponse(status: "failed", schemaVersion: schemaVersion, requestId: requestId, principalTranscriptRevision: nil, result: nil, reason: nil, code: code)
    }
}

private enum HelperFailure: Error { case invalidRequest }

@main enum FoundationHelperMain {
    static func main() async {
        guard let line = readLine() else { return }
        let response = await HelperCore.handle(line)
        guard let data = try? JSONEncoder().encode(response), let output = String(data: data, encoding: .utf8) else { return }
        print(output)
    }
}
