import CoreML
import Darwin
import FluidAudio
import Foundation

private struct LocalSegment {
    let speakerId: String
    let startTime: Float
    let endTime: Float
    let qualityScore: Float
}
@available(macOS 14.0, *)
private final class LocalDiarizationBridge {
    private var manager: OfflineDiarizerManager?

    func initialize(threshold: Double, modelDirectory: String) throws {
        let semaphore = DispatchSemaphore(value: 0)
        var initializationError: Error?

        Task {
            do {
                let directory = URL(fileURLWithPath: modelDirectory, isDirectory: true)
                let startedAt = Date()
                let allUnits = MLModelConfiguration()
                allUnits.computeUnits = .all
                let cpuOnly = MLModelConfiguration()
                cpuOnly.computeUnits = .cpuOnly

                let segmentation = try MLModel(
                    contentsOf: directory.appendingPathComponent("Segmentation.mlmodelc", isDirectory: true),
                    configuration: allUnits
                )
                let embedding = try MLModel(
                    contentsOf: directory.appendingPathComponent("Embedding.mlmodelc", isDirectory: true),
                    configuration: allUnits
                )
                let pldaRho = try MLModel(
                    contentsOf: directory.appendingPathComponent("PldaRho.mlmodelc", isDirectory: true),
                    configuration: allUnits
                )
                let fbank = try MLModel(
                    contentsOf: directory.appendingPathComponent("FBank.mlmodelc", isDirectory: true),
                    configuration: cpuOnly
                )
                let models = OfflineDiarizerModels(
                    segmentationModel: segmentation,
                    fbankModel: fbank,
                    embeddingModel: embedding,
                    pldaRhoModel: pldaRho,
                    pldaPsi: try Self.loadPldaPsi(from: directory),
                    compilationDuration: Date().timeIntervalSince(startedAt)
                )
                var configuration = OfflineDiarizerConfig()
                configuration.clustering.threshold = threshold
                configuration.postProcessing.exclusiveSegments = false
                let manager = OfflineDiarizerManager(config: configuration)
                manager.initialize(models: models)
                self.manager = manager
            } catch {
                initializationError = error
            }
            semaphore.signal()
        }

        semaphore.wait()
        if let initializationError {
            throw initializationError
        }
    }

    func diarize(path: String) throws -> [LocalSegment] {
        guard let manager else {
            throw CocoaError(.featureUnsupported)
        }
        let semaphore = DispatchSemaphore(value: 0)
        var output: DiarizationResult?
        var processingError: Error?
        Task {
            do {
                output = try await manager.process(URL(fileURLWithPath: path))
            } catch {
                processingError = error
            }
            semaphore.signal()
        }
        semaphore.wait()
        if let processingError {
            throw processingError
        }
        guard let output else {
            throw CocoaError(.fileReadUnknown)
        }
        return output.segments.map {
            LocalSegment(
                speakerId: $0.speakerId,
                startTime: $0.startTimeSeconds,
                endTime: $0.endTimeSeconds,
                qualityScore: $0.qualityScore
            )
        }
    }

    private static func loadPldaPsi(from directory: URL) throws -> [Double] {
        let data = try Data(contentsOf: directory.appendingPathComponent("plda-parameters.json"))
        guard
            let root = try JSONSerialization.jsonObject(with: data) as? [String: Any],
            let tensors = root["tensors"] as? [String: Any],
            let psi = tensors["psi"] as? [String: Any],
            let encoded = psi["data_base64"] as? String,
            let decoded = Data(base64Encoded: encoded, options: [.ignoreUnknownCharacters]),
            !decoded.isEmpty,
            decoded.count % MemoryLayout<Float>.size == 0
        else {
            throw CocoaError(.fileReadCorruptFile)
        }
        var values = [Float](repeating: 0, count: decoded.count / MemoryLayout<Float>.size)
        _ = values.withUnsafeMutableBytes { decoded.copyBytes(to: $0) }
        return values.map(Double.init)
    }
}

@_cdecl("fluidaudio_local_create")
public func fluidaudioLocalCreate() -> UnsafeMutableRawPointer? {
    guard #available(macOS 14.0, *) else { return nil }
    return Unmanaged.passRetained(LocalDiarizationBridge()).toOpaque()
}

@_cdecl("fluidaudio_local_destroy")
public func fluidaudioLocalDestroy(_ pointer: UnsafeMutableRawPointer?) {
    guard let pointer else { return }
    Unmanaged<AnyObject>.fromOpaque(pointer).release()
}

@_cdecl("fluidaudio_local_initialize_diarization")
public func fluidaudioLocalInitialize(
    _ pointer: UnsafeMutableRawPointer?,
    _ threshold: Double,
    _ modelDirectory: UnsafePointer<CChar>?
) -> Int32 {
    guard #available(macOS 14.0, *), let pointer, let modelDirectory else { return -1 }
    let bridge = Unmanaged<LocalDiarizationBridge>.fromOpaque(pointer).takeUnretainedValue()
    do {
        try bridge.initialize(
            threshold: threshold,
            modelDirectory: String(cString: modelDirectory)
        )
        return 0
    } catch {
        return -1
    }
}

@_cdecl("fluidaudio_local_diarize_file")
public func fluidaudioLocalDiarize(
    _ pointer: UnsafeMutableRawPointer?,
    _ path: UnsafePointer<CChar>?,
    _ outSpeakerIds: UnsafeMutablePointer<UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?>?,
    _ outStartTimes: UnsafeMutablePointer<UnsafeMutablePointer<Float>?>?,
    _ outEndTimes: UnsafeMutablePointer<UnsafeMutablePointer<Float>?>?,
    _ outQualityScores: UnsafeMutablePointer<UnsafeMutablePointer<Float>?>?,
    _ outCount: UnsafeMutablePointer<UInt32>?
) -> Int32 {
    guard #available(macOS 14.0, *), let pointer, let path else { return -1 }
    let bridge = Unmanaged<LocalDiarizationBridge>.fromOpaque(pointer).takeUnretainedValue()
    do {
        let segments = try bridge.diarize(path: String(cString: path))
        outCount?.pointee = UInt32(segments.count)
        guard !segments.isEmpty else {
            outSpeakerIds?.pointee = nil
            outStartTimes?.pointee = nil
            outEndTimes?.pointee = nil
            outQualityScores?.pointee = nil
            return 0
        }

        let ids = UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>.allocate(capacity: segments.count)
        let starts = UnsafeMutablePointer<Float>.allocate(capacity: segments.count)
        let ends = UnsafeMutablePointer<Float>.allocate(capacity: segments.count)
        let scores = UnsafeMutablePointer<Float>.allocate(capacity: segments.count)
        for (index, segment) in segments.enumerated() {
            ids[index] = strdup(segment.speakerId)
            starts[index] = segment.startTime
            ends[index] = segment.endTime
            scores[index] = segment.qualityScore
        }
        outSpeakerIds?.pointee = ids
        outStartTimes?.pointee = starts
        outEndTimes?.pointee = ends
        outQualityScores?.pointee = scores
        return 0
    } catch {
        return -1
    }
}

@_cdecl("fluidaudio_local_free_result")
public func fluidaudioLocalFreeResult(
    _ speakerIds: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?,
    _ startTimes: UnsafeMutablePointer<Float>?,
    _ endTimes: UnsafeMutablePointer<Float>?,
    _ qualityScores: UnsafeMutablePointer<Float>?,
    _ count: UInt32
) {
    if let speakerIds {
        for index in 0..<Int(count) {
            free(speakerIds[index])
        }
        speakerIds.deallocate()
    }
    startTimes?.deallocate()
    endTimes?.deallocate()
    qualityScores?.deallocate()
}
