import Foundation
import FoundationModels

struct AvailabilityResponse: Codable {
  let available: Bool
  let reason: String?
  let details: String?
}

struct BoundsPercent: Codable {
  let xPercent: Double
  let yPercent: Double
  let widthPercent: Double
  let heightPercent: Double
}

struct AxInfo: Codable {
  let role: String
  let subrole: String?
  let roleDescription: String?
  let identifier: String?
  let label: String
  let elementBounds: BoundsPercent?
  let containerRole: String?
  let containerSubrole: String?
  let containerIdentifier: String?
  let windowRole: String?
  let windowSubrole: String?
  let topLevelRole: String?
  let topLevelSubrole: String?
  let parentDialogRole: String?
  let parentDialogSubrole: String?
  let isChecked: Bool?
  let isCancelButton: Bool
  let isDefaultButton: Bool
}

struct StepInput: Codable {
  let id: String
  let action: String
  let app: String
  let windowTitle: String
  let clickXPercent: Double
  let clickYPercent: Double
  let screenshotPath: String?
  let note: String?
  let ax: AxInfo?
}

struct GenerateRequest: Codable {
  let steps: [StepInput]
  let maxChars: Int?
}

struct GenerateResultItem: Codable {
  let id: String
  let text: String
  let debug: GenerateResultDebug?
}

struct GenerateResultDebug: Codable {
  let kind: String
  let location: String?
  let groundingLabel: String
  let groundingOcr: String?
  let baseline: String
  let candidate: String?
  let qualityGateReason: String
}

struct GenerateFailureItem: Codable {
  let id: String
  let error: String
}

struct GenerateResponse: Codable {
  let results: [GenerateResultItem]
  let failures: [GenerateFailureItem]
}

func encodeJSON<T: Encodable>(_ value: T) -> Data {
  let encoder = JSONEncoder()
  encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
  return (try? encoder.encode(value)) ?? Data("{}".utf8)
}

func readStdin() -> Data {
  FileHandle.standardInput.readDataToEndOfFile()
}

func writeStdout(_ data: Data) {
  FileHandle.standardOutput.write(data)
  FileHandle.standardOutput.write(Data("\n".utf8))
}

func availabilityReasonCode(_ reason: SystemLanguageModel.Availability.UnavailableReason) -> String {
  switch reason {
  case .deviceNotEligible:
    return "deviceNotEligible"
  case .appleIntelligenceNotEnabled:
    return "appleIntelligenceNotEnabled"
  case .modelNotReady:
    return "modelNotReady"
  @unknown default:
    return "unavailable"
  }
}

func availabilityReasonDetails(_ reason: SystemLanguageModel.Availability.UnavailableReason) -> String {
  switch reason {
  case .deviceNotEligible:
    return "This device is not eligible for Apple Intelligence."
  case .appleIntelligenceNotEnabled:
    return "Apple Intelligence is disabled. Enable it in System Settings."
  case .modelNotReady:
    return "The model is not ready yet. Try again in a moment."
  @unknown default:
    return "Apple Intelligence is unavailable."
  }
}

func checkAvailability() -> AvailabilityResponse {
  switch SystemLanguageModel.default.availability {
  case .available:
    return AvailabilityResponse(available: true, reason: nil, details: nil)
  case .unavailable(let reason):
    return AvailabilityResponse(
      available: false,
      reason: availabilityReasonCode(reason),
      details: availabilityReasonDetails(reason),
    )
  }
}

func generateDescriptions(_ req: GenerateRequest) async -> GenerateResponse {
  let maxChars = max(20, min(req.maxChars ?? 110, 140))
  let availability = checkAvailability()
  if !availability.available {
    let failures = req.steps.map {
      GenerateFailureItem(id: $0.id, error: availability.details ?? "Apple Intelligence unavailable.")
    }
    return GenerateResponse(results: [], failures: failures)
  }

  let instructions =
    "You generate concise UI tutorial step descriptions. " +
    "Keep output short and specific. Never invent UI labels; use only provided context."

  var results: [GenerateResultItem] = []
  let failures: [GenerateFailureItem] = []
  results.reserveCapacity(req.steps.count)

  for step in req.steps {
    do {
      let kind = classifyKind(step)
      let location = locationHint(step, kind: kind)
      let grounding = chooseGroundingLabel(step, kind: kind)
      let baseline = baselineDescription(
        step,
        kind: kind,
        label: grounding.label,
        location: location,
        maxChars: maxChars
      )

      // Deterministic kinds are best handled without model calls.
      if kind == "close button" || kind == "minimize button" || kind == "zoom button"
        || kind == "menu item" || kind == "menu bar item" || kind == "checkbox"
      {
        results.append(GenerateResultItem(
          id: step.id,
          text: baseline,
          debug: GenerateResultDebug(
            kind: kind,
            location: location,
            groundingLabel: grounding.label,
            groundingOcr: grounding.ocr,
            baseline: baseline,
            candidate: nil,
            qualityGateReason: "deterministic_baseline"
          )
        ))
        continue
      }

      let prompt = promptForStep(
        step,
        kind: kind,
        baseline: baseline,
        label: grounding.label,
        ocr: grounding.ocr,
        location: location,
        maxChars: maxChars
      )
      let session = LanguageModelSession(instructions: instructions)
      let options = GenerationOptions(sampling: .greedy)
      let response = try await session.respond(to: prompt, options: options)
      let candidate = sanitizeDescription(response.content, maxChars: maxChars)
      let decision = applyQualityGate(
        step: step,
        kind: kind,
        baseline: baseline,
        candidate: candidate,
        label: grounding.label
      )
      let finalText = decision.text.isEmpty ? baseline : decision.text
      results.append(GenerateResultItem(
        id: step.id,
        text: finalText,
        debug: GenerateResultDebug(
          kind: kind,
          location: location,
          groundingLabel: grounding.label,
          groundingOcr: grounding.ocr,
          baseline: baseline,
          candidate: candidate,
          qualityGateReason: decision.reason
        )
      ))
    } catch {
      // Keep UI stable even when model refuses/errs (safety, transient availability).
      let kind = classifyKind(step)
      let location = locationHint(step, kind: kind)
      let grounding = chooseGroundingLabel(step, kind: kind)
      let baseline = baselineDescription(
        step,
        kind: kind,
        label: grounding.label,
        location: location,
        maxChars: maxChars
      )
      results.append(GenerateResultItem(
        id: step.id,
        text: baseline,
        debug: GenerateResultDebug(
          kind: kind,
          location: location,
          groundingLabel: grounding.label,
          groundingOcr: grounding.ocr,
          baseline: baseline,
          candidate: nil,
          qualityGateReason: "model_error_fallback"
        )
      ))
    }
  }

  return GenerateResponse(results: results, failures: failures)
}

@main
struct StepCastAIHelper {
  static func main() async {
    let args = Array(CommandLine.arguments.dropFirst())
    guard let cmd = args.first else {
      writeStdout(Data("{}".utf8))
      exit(2)
    }

    switch cmd {
    case "availability":
      writeStdout(encodeJSON(checkAvailability()))
    case "generate":
      let input = readStdin()
      let decoder = JSONDecoder()
      decoder.keyDecodingStrategy = .convertFromSnakeCase
      guard let req = try? decoder.decode(GenerateRequest.self, from: input) else {
        let resp = GenerateResponse(results: [], failures: [GenerateFailureItem(id: "*", error: "Invalid JSON input.")])
        writeStdout(encodeJSON(resp))
        exit(2)
      }
      let resp = await generateDescriptions(req)
      writeStdout(encodeJSON(resp))
    default:
      writeStdout(Data("{}".utf8))
      exit(2)
    }
  }
}
