import ARKit

struct ScanFrame {
  public let time: TimeInterval
  public let image: CGImage
  public let depths: [Float]
  public let depthConfidences: [UInt8]

  //  public static func read(from: URL) -> ScanFrame {
  //
  //  }

  public func write(to: URL) {
    var data = Data()

    var time = self.time
    data.append(
      Data(
        bytes: &time,
        count: MemoryLayout<TimeInterval>.size))

    let imageData = image.dataProvider!.data! as Data
    var imageCount = imageData.count
    data.append(
      Data(
        bytes: &imageCount,
        count: MemoryLayout<Int>.size))
    data.append(imageData)

    var depths = self.depths
    var depthsCount = depths.count
    data.append(
      Data(
        bytes: &depthsCount,
        count: MemoryLayout<Int>.size))
    data.append(
      Data(
        bytes: &depths,
        count: MemoryLayout<Float>.stride * depthsCount))
    data.append(Data(depthConfidences))

    try! data.write(to: to)
  }
}

class ScanSession: NSObject, ARSessionDelegate {
  public let arSession = ARSession()
  public var onFrame: ((ScanFrame) -> Void)?

  var useCount = 0

  override init() {
    super.init()
    arSession.delegate = self
  }

  public func activate() {
    assert(useCount >= 0)
    if useCount == 0 {
      let config = ARWorldTrackingConfiguration()
      config.frameSemantics = .sceneDepth
      arSession.run(config)
    }
    useCount += 1
  }

  public func release() {
    assert(useCount >= 0)
    useCount -= 1
    if useCount == 0 {
      arSession.pause()
    }
  }

  func session(_ session: ARSession, didUpdate: ARFrame) {
    let context = CIContext(options: nil)
    let ciImage = CIImage(cvPixelBuffer: didUpdate.capturedImage)
    let cgImage = context.createCGImage(ciImage, from: ciImage.extent)!

    let depthMap = didUpdate.sceneDepth!.depthMap
    let confidenceMap = didUpdate.sceneDepth!.confidenceMap!
    let depthWidth = CVPixelBufferGetWidth(depthMap)
    let depthHeight = CVPixelBufferGetHeight(depthMap)

    var depths: [Float] = []
    var depthConfidences: [UInt8] = []
    depths.reserveCapacity(depthWidth * depthHeight)
    depthConfidences.reserveCapacity(depthWidth * depthHeight)

    let lockFlags = CVPixelBufferLockFlags(rawValue: 0)
    CVPixelBufferLockBaseAddress(depthMap, lockFlags)
    let depthBuf = unsafeBitCast(
      CVPixelBufferGetBaseAddress(depthMap),
      to: UnsafeMutablePointer<Float32>.self)

    CVPixelBufferLockBaseAddress(confidenceMap, lockFlags)
    let confidenceBuf = unsafeBitCast(
      CVPixelBufferGetBaseAddress(confidenceMap),
      to: UnsafeMutablePointer<UInt8>.self)

    for i in 0...depthWidth * depthHeight - 1 {
      depths.append(depthBuf[i])
      depthConfidences.append(confidenceBuf[i] + 1)
    }

    CVPixelBufferUnlockBaseAddress(depthMap, lockFlags)
    CVPixelBufferUnlockBaseAddress(confidenceMap, lockFlags)

    onFrame?(
      ScanFrame(
        time: didUpdate.timestamp,
        image: cgImage,
        depths: depths,
        depthConfidences: depthConfidences
      ))
  }
}
