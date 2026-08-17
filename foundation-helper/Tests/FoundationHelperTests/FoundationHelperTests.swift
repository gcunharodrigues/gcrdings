import Testing
import Foundation
@testable import FoundationHelper

private let request = #"{"schema_version":1,"request_id":"r1","record_type":"meeting","principal_transcript_revision":1,"participants":[],"transcript":[{"id":"b1","text":"Principal text","participant_id":"p1","start_ms":0,"end_ms":1000}],"prompt_version":"meeting-v1"}"#

@Test func oneRequestProducesOneTypedResponse() async throws {
    let response = await HelperCore.handle(request)
    #expect(response.requestId == "r1")
    #expect(["completed", "unavailable"].contains(response.status))
}

@Test func malformedInputIsTypedAndSanitized() async {
    let response = await HelperCore.handle("private transcript")
    #expect(response.code == "invalid_request")
    #expect(response.requestId == "invalid")
}

@Test func requestDecoderRejectsUnknownFieldsAndInvalidTimes() {
    #expect(throws: (any Error).self) { try HelperCore.decode(request.dropLast() + #", "path":"private"}"#) }
    let invalidTime = request.replacingOccurrences(of: #""end_ms":1000"#, with: #""end_ms":-1"#)
    #expect(throws: (any Error).self) { try HelperCore.decode(invalidTime) }
}

@Test func fixedRecordTypesEncodeOnlyTheirPurposeSpecificFields() throws {
    let finding = HelperFinding(label: "Grounded", detail: "Detail", evidence: [HelperEvidence(blockId: "b1", timestampMs: 500)])
    let results: [(HelperResult, Set<String>)] = [
        (.meeting(summary: "Summary", keyPoints: [finding]), ["record_type", "summary", "decisions", "action_items", "key_points"]),
        (.interview(summary: "Summary", themes: [finding]), ["record_type", "summary", "answers", "themes", "follow_ups"]),
        (.content(summary: "Summary", claims: [finding]), ["record_type", "summary", "claims", "outline", "source_notes"]),
    ]
    for (result, expectedKeys) in results {
        let object = try #require(try JSONSerialization.jsonObject(with: JSONEncoder().encode(result)) as? [String: Any])
        #expect(Set(object.keys) == expectedKeys)
    }
}

@Test func transcriptPromptKeepsBlockIdSeparateFromPlayableTimes() throws {
    let decoded = try HelperCore.decode(request)
    let prompt = HelperCore.prompt(for: decoded)
    #expect(prompt.contains("BLOCK_ID: b1\n"))
    #expect(prompt.contains("START_MS: 0\nEND_MS: 1000\n"))
    #expect(!prompt.contains("[b1 0-1000ms"))
}
