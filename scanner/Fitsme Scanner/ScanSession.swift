import ARKit

struct ScanFrame {
  public let time: TimeInterval
  public let image: CGImage
  public let depthWidth: Int
  public let depthHeight: Int
  public let depths: [Float]
  public let depthConfidences: [UInt8]

  public static func decode(data: inout Data) -> ScanFrame {
    var offset = 0
    let time = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: TimeInterval.self)
    }
    offset += MemoryLayout<TimeInterval>.size

    let imageWidth = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: Int.self)
    }
    offset += MemoryLayout<Int>.size

    let imageHeight = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: Int.self)
    }
    offset += MemoryLayout<Int>.size

    var imageContext: CGContext?
    data.withUnsafeMutableBytes { ptr in
      imageContext = CGContext(
        data: ptr.baseAddress! + offset, width: imageWidth, height: imageHeight,
        bitsPerComponent: 8,
        bytesPerRow: imageWidth * 4,
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
    }
    offset += imageWidth * 4 * imageHeight
    let image = imageContext!.makeImage()!

    let depthWidth = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: Int.self)
    }
    offset += MemoryLayout<Int>.size

    let depthHeight = data.withUnsafeBytes { ptr in
      (ptr.baseAddress! + offset).load(as: Int.self)
    }
    offset += MemoryLayout<Int>.size

    let depthCount = depthHeight * depthWidth

    var depths: [Float] = []
    depths.reserveCapacity(depthCount)
    data.withUnsafeBytes { ptr in
      for i in 0..<depthCount {
        depths.append(
          (ptr.baseAddress! + offset + i * MemoryLayout<Float>.stride).load(
            as: Float.self))
      }
    }
    offset += MemoryLayout<Float>.stride * depthCount

    let depthConfidences = [UInt8](
      data.subdata(in: offset..<offset + depthCount))
    assert(data.count == offset + depthCount)

    return ScanFrame(
      time: time,
      image: image,
      depthWidth: depthWidth,
      depthHeight: depthHeight,
      depths: depths,
      depthConfidences: depthConfidences
    )
  }

  public func encode() -> Data {
    var data = Data()

    var time = self.time
    data.append(
      Data(
        bytes: &time,
        count: MemoryLayout<TimeInterval>.size))

    var imageWidth = image.width
    data.append(
      Data(
        bytes: &imageWidth,
        count: MemoryLayout<Int>.size))
    var imageHeight = image.height
    data.append(
      Data(
        bytes: &imageHeight,
        count: MemoryLayout<Int>.size))
    let imageData = image.dataProvider!.data! as Data
    data.append(imageData)

    var depthWidth = self.depthWidth
    data.append(
      Data(
        bytes: &depthWidth,
        count: MemoryLayout<Int>.size))
    var depthHeight = self.depthHeight
    data.append(
      Data(
        bytes: &depthHeight,
        count: MemoryLayout<Int>.size))
    var depths = self.depths
    data.append(
      Data(
        bytes: &depths,
        count: depthHeight * depthWidth * MemoryLayout<Float>.stride))
    data.append(Data(depthConfidences))

    return data
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
      run(videoFormat: 0)
    }
    useCount += 1
  }

  public func activate(videoFormat: Int) {
    assert(useCount >= 0)
    run(videoFormat: videoFormat)
    useCount += 1
  }

  func run(videoFormat: Int) {
    let formats = ARWorldTrackingConfiguration.supportedVideoFormats
    let config = ARWorldTrackingConfiguration()
    config.videoFormat = formats[videoFormat]
    config.frameSemantics = .sceneDepth
    arSession.run(config)
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

    for i in 0..<depthWidth * depthHeight {
      depths.append(depthBuf[i])
      depthConfidences.append(confidenceBuf[i] + 1)
    }

    CVPixelBufferUnlockBaseAddress(depthMap, lockFlags)
    CVPixelBufferUnlockBaseAddress(confidenceMap, lockFlags)

    onFrame?(
      ScanFrame(
        time: didUpdate.timestamp,
        image: cgImage,
        depthWidth: depthWidth,
        depthHeight: depthHeight,
        depths: depths,
        depthConfidences: depthConfidences
      ))
  }
}
